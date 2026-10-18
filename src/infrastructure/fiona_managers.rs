//! Ordered fresh first-Fiona constructor prefix: MissionBook, initial titles/base
//! stats, then SkillManager's SP slot. Stops before Costume/status/attendance/Join.
use super::{
    character_base_stats::{self, BaseStatError},
    character_costumes::{self, CostumeError},
    character_lobby::UserKey,
    character_skills::{self, SkillError},
    character_vocation::{self, VocationError},
    fiona_character_state::{self as state, StateError},
    fiona_initializer_profile::Profile,
    item_catalog::Package,
};
use crate::application::initial_costume::{self, Loaded};
use crate::{
    domain::{character_base_stats::BaseStats, legacy_user_data::SqlDateTime2},
    error::Error,
};
use sqlx::PgPool;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum ManagerError {
    #[error(transparent)]
    Vocation(#[from] VocationError),
    #[error(transparent)]
    Costume(#[from] CostumeError),
    #[error("initial manager database operation failed")]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Content(#[from] Error),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Stats(#[from] BaseStatError),
    #[error(transparent)]
    Skills(#[from] SkillError),
    #[error("initial manager prefix conflicts with an existing or partial attempt")]
    Conflict,
    #[error("initial manager prefix timed out; reconcile committed stages before retry")]
    Timeout,
}
#[must_use = "only the manager prefix completed; remaining constructors,Join/Login and creation must still run"]
pub struct Managers {
    cid: i64,
    base: BaseStats,
    state_source: [u8; 32],
}
/// Strictly fresh admission. The first mission insert elects a single caller even if
/// two preflights race. A partial attempt is never silently resumed. Commits remain
/// separate, preserving MissionBook before stats and base stats before the SP insert.
pub async fn initialize(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
    profile: &Profile,
    catalog: &Package,
) -> Result<Managers, ManagerError> {
    profile.bind(catalog)?;
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        {
            let mut tx=state::owned_first_fiona(pool,owner,cid).await?;
            let source:Option<Vec<u8>>=sqlx::query_scalar("SELECT source_sha256 FROM game_character_state_preservation WHERE singleton").fetch_one(&mut *tx).await?;
            if source.as_deref()!=Some(profile.source_state().as_slice()){return Err(ManagerError::Conflict)}
            let pristine:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_character_info WHERE id=$1 AND legacy_last_update IS NULL AND dirty AND str=0 AND dex=0 AND int=0 AND will=0 AND luck=0 AND hp=0 AND stamina=0 AND ap=0 AND total_used_ap=0 AND exp=0 AND play_time=0 AND selected_title=0) AND NOT EXISTS(SELECT 1 FROM game_character_mission_info WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_mission_assignments WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_titles WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_title_goal_progress WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_micro_play_status WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_skills WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_sp_skills WHERE cid=$1)").bind(cid).fetch_one(&mut *tx).await?;
            if !pristine{return Err(ManagerError::Conflict)}
            tx.commit().await?;
        }
        // Profile construction admits disabled RandomMission only. Its source initial
        // clear DateTime is zero, not a newly sampled wall clock.
        let clear=SqlDateTime2::from_ticks(0).map_err(|_|StateError::Profile)?;
        if state::initialize_mission(pool,owner,cid,clear).await?!=1{return Err(ManagerError::Conflict)}
        let base={
            let mut tx=state::owned_first_fiona(pool,owner,cid).await?;
            // No other CharacterInfo for this account (owned_first_fiona), no own
            // title/progress rows: both source title collections are empty at this point.
            let titles:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_titles WHERE cid=$1) OR EXISTS(SELECT 1 FROM game_title_goal_progress WHERE cid=$1)").bind(cid).fetch_one(&mut *tx).await?;
            if titles{return Err(ManagerError::Conflict)}
            // This lookup occurs even with zero contribution and can overflow. A
            // failure must leave the earlier mission commit, with no base/SP write.
            let _ratio=profile.account_title_stat_ratio()?;
            let base=profile.base_stats([0;7]);
            tx.commit().await?;
            base
        };
        if character_base_stats::initialize(pool,owner,cid,&base).await?!=1{return Err(ManagerError::Conflict)}
        if character_skills::initialize_sp_slot(pool,owner,cid).await?!=1{return Err(ManagerError::Conflict)}
        Ok(Managers{cid,base,state_source:*profile.source_state()})
    }).await.map_err(|_|ManagerError::Timeout)?
}

impl Managers {
    pub fn cid(&self) -> i64 {
        self.cid
    }
    pub fn base(&self) -> BaseStats {
        self.base
    }
}
#[must_use = "costume loaded only;Vocation and remaining constructors/Join/Login/Create must still run"]
pub struct CostumedManagers {
    prefix: Managers,
    costume: Loaded,
}
impl CostumedManagers {
    pub fn cid(&self) -> i64 {
        self.prefix.cid
    }
    pub fn base(&self) -> BaseStats {
        self.prefix.base
    }
    pub fn costume(&self) -> &Loaded {
        &self.costume
    }
}
/// Consume a completed prefix. Recheck revision,source and the unchanged constructor
/// boundary under the same character lock as appearance writers. Read-only: failures
/// preserve all earlier commits and never rebuild or resume an actor implicitly.
pub async fn load_costume(
    pool: &PgPool,
    owner: &UserKey<'_>,
    prefix: Managers,
    context: initial_costume::Context,
) -> Result<CostumedManagers, ManagerError> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = constructor_tx(pool, owner, &prefix).await?;
        let rows = character_costumes::load_for_constructor(&mut tx, prefix.cid).await?;
        let costume = Loaded::load(rows, context).map_err(CostumeError::from)?;
        tx.commit().await?;
        Ok(CostumedManagers { prefix, costume })
    })
    .await
    .map_err(|_| ManagerError::Timeout)?
}

async fn constructor_tx<'p>(
    pool: &'p PgPool,
    owner: &UserKey<'_>,
    prefix: &Managers,
) -> Result<sqlx::Transaction<'p, sqlx::Postgres>, ManagerError> {
    let mut tx = state::owned_first_fiona(pool, owner, prefix.cid).await?;
    let source: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT source_sha256 FROM game_character_state_preservation WHERE singleton",
    )
    .fetch_one(&mut *tx)
    .await?;
    if source.as_deref() != Some(prefix.state_source.as_slice()) {
        return Err(ManagerError::Conflict);
    }
    let stored:(i32,i32,i32,i32,i32,i32,i32,bool,bool)=sqlx::query_as("SELECT str,dex,int,will,luck,hp,stamina,dirty,legacy_last_update IS NULL AND ap=0 AND total_used_ap=0 AND exp=0 AND play_time=0 AND selected_title=0 FROM game_character_info WHERE id=$1").bind(prefix.cid).fetch_one(&mut *tx).await?;
    if [
        stored.0, stored.1, stored.2, stored.3, stored.4, stored.5, stored.6,
    ] != *prefix.base.values()
        || stored.7
        || !stored.8
    {
        return Err(ManagerError::Conflict);
    }
    let boundary:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_character_mission_info WHERE cid=$1 AND completed_mission_count=0 AND last_clear_ticks=0) AND (SELECT COUNT(*) FROM game_sp_skills WHERE cid=$1)=1 AND EXISTS(SELECT 1 FROM game_sp_skills WHERE cid=$1 AND slot_id=0 AND skill_utf16 IS NULL) AND NOT EXISTS(SELECT 1 FROM game_titles WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_title_goal_progress WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_mission_assignments WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_micro_play_status WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_skills WHERE cid=$1)").bind(prefix.cid).fetch_one(&mut *tx).await?;
    if !boundary {
        return Err(ManagerError::Conflict);
    }

    Ok(tx)
}
#[must_use = "Vocation absent and initialized only;remaining constructors/Join/Login/Create must still run"]
pub struct VocatedManagers {
    prior: CostumedManagers,
    vocation: crate::domain::initial_vocation::Loaded,
}
impl VocatedManagers {
    pub fn cid(&self) -> i64 {
        self.prior.cid()
    }
    pub fn base(&self) -> BaseStats {
        self.prior.base()
    }
    pub fn costume(&self) -> &Loaded {
        self.prior.costume()
    }
    pub fn vocation(&self) -> &crate::domain::initial_vocation::Loaded {
        &self.vocation
    }
}
/// Source VocationManager follows Costume. Admit only a proven absent vocation;
/// no content provider,skill grant,transformation or Submit is invoked.
pub async fn load_vocation(
    pool: &PgPool,
    owner: &UserKey<'_>,
    prior: CostumedManagers,
) -> Result<VocatedManagers, ManagerError> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = constructor_tx(pool, owner, &prior.prefix).await?;
        let rows = character_costumes::load_for_constructor(&mut tx, prior.cid()).await?;
        if !prior
            .costume
            .matches_source(rows)
            .map_err(CostumeError::from)?
        {
            return Err(ManagerError::Conflict);
        }
        let vocation = character_vocation::load_first(&mut tx, prior.cid()).await?;
        tx.commit().await?;
        Ok(VocatedManagers { prior, vocation })
    })
    .await
    .map_err(|_| ManagerError::Timeout)?
}
