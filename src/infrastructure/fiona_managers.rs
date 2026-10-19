//! Ordered fresh first-Fiona constructor prefix: MissionBook, initial titles/base
//! stats, SP slot, Costume/Vocation, detached Social and empty Status/TownCampfire.
//! Stops before Attendance and remaining constructors/Join/Login/Create.
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
    Attendance(#[from] crate::domain::initial_attendance::AdmissionError),
    #[error(transparent)]
    Status(#[from] super::character_status_effects::StatusError),
    #[error(transparent)]
    Clock(#[from] super::lab_clock::ClockError),
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

#[must_use = "initial Whisper/Guild context only;remaining constructors/Join/Login/Create must still run"]
pub struct SocialManagers {
    prior: VocatedManagers,
    social: crate::domain::initial_social::Loaded,
}
impl SocialManagers {
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
        self.prior.vocation()
    }
    pub fn social(&self) -> &crate::domain::initial_social::Loaded {
        &self.social
    }
}
/// WhisperFilter binds context; GuildInfoManager creates its initial projection and
/// skips UpdateBriefGuildInfo(null) only with the source-validated InGameGuild flag.
/// This detached context is not a published actor or a mutable service graph.
pub async fn load_social(
    pool: &PgPool,
    owner: &UserKey<'_>,
    prior: VocatedManagers,
    profile: &Profile,
    catalog: &Package,
) -> Result<SocialManagers, ManagerError> {
    profile.bind(catalog)?;
    if profile.source_state() != &prior.prior.prefix.state_source {
        return Err(ManagerError::Conflict);
    }
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = constructor_tx(pool, owner, &prior.prior.prefix).await?;
        let rows = character_costumes::load_for_constructor(&mut tx, prior.cid()).await?;
        if !prior
            .costume()
            .matches_source(rows)
            .map_err(CostumeError::from)?
        {
            return Err(ManagerError::Conflict);
        }
        // Reprove the retained absent-Vocation boundary under the same owner lock.
        let _vocation = character_vocation::load_first(&mut tx, prior.cid()).await?;
        let social =
            crate::domain::initial_social::Loaded::load(owner.uid, prior.cid(), profile.policy())?;
        tx.commit().await?;
        Ok(SocialManagers { prior, social })
    })
    .await
    .map_err(|_| ManagerError::Timeout)?
}

#[must_use = "initial StatusEffect/TownCampfire only;Attendance and remaining constructors/Join/Login/Create must still run"]
pub struct StatusManagers {
    prior: SocialManagers,
    loaded: crate::domain::initial_status::Loaded,
}
impl StatusManagers {
    pub fn prior(&self) -> &SocialManagers {
        &self.prior
    }
    pub fn loaded(&self) -> &crate::domain::initial_status::Loaded {
        &self.loaded
    }
}
/// Consume Social state before constructing empty StatusEffect then TownCampfire.
/// No runtime clock/Save is reached until all retained state and absence are
/// revalidated in the same transaction. A clock failure preserves earlier commits.
pub async fn load_status(
    pool: &PgPool,
    owner: &UserKey<'_>,
    prior: SocialManagers,
    clock: impl FnOnce() -> Result<SqlDateTime2, super::lab_clock::ClockError>,
    observations: &mut crate::domain::initial_status::Profile,
) -> Result<StatusManagers, ManagerError> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = constructor_tx(pool, owner, &prior.prior.prior.prefix).await?;
        let rows = character_costumes::load_for_constructor(&mut tx, prior.cid()).await?;
        if !prior
            .costume()
            .matches_source(rows)
            .map_err(CostumeError::from)?
        {
            return Err(ManagerError::Conflict);
        }
        let _vocation = character_vocation::load_first(&mut tx, prior.cid()).await?;
        super::character_status_effects::require_absent(&mut tx, prior.cid()).await?;
        let loaded =
            crate::domain::initial_status::Loaded::load(prior.social(), clock, observations)?;
        tx.commit().await?;
        Ok(StatusManagers { prior, loaded })
    })
    .await
    .map_err(|_| ManagerError::Timeout)?
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttendanceInfo {
    NotRead,
    Missing,
    Present(Option<i64>),
}
#[must_use = "initial Attendance only;Roulette/Marble/Captcha/Join/Login/Create remain"]
pub struct AttendanceManagers {
    user_info: AttendanceInfo,
    profile: [u8; 32],
    prior: StatusManagers,
    loaded: crate::domain::initial_attendance::Loaded,
}
impl AttendanceManagers {
    pub fn prior(&self) -> &StatusManagers {
        &self.prior
    }
    pub fn loaded(&self) -> &crate::domain::initial_attendance::Loaded {
        &self.loaded
    }
}
/// Source FirstCharacterCreated reads UserInfo, not CharacterInfo count. Keep its
/// Count/First order; lock the UserInfo row through window consumption. Fail closed
/// on database/admission errors rather than treating unavailable state as empty.
pub async fn load_attendance(
    pool: &PgPool,
    owner: &UserKey<'_>,
    prior: StatusManagers,
    profile: &Profile,
    catalog: &Package,
    mut clock: impl FnMut() -> Result<SqlDateTime2, super::lab_clock::ClockError>,
) -> Result<AttendanceManagers, ManagerError> {
    profile.bind(catalog)?;
    if profile.source_state() != &prior.prior.prior.prior.prefix.state_source {
        return Err(ManagerError::Conflict);
    }
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = constructor_tx(pool, owner, &prior.prior.prior.prior.prefix).await?;
        let rows = character_costumes::load_for_constructor(&mut tx, prior.prior.cid()).await?;
        if !prior
            .prior
            .costume()
            .matches_source(rows)
            .map_err(CostumeError::from)?
        {
            return Err(ManagerError::Conflict);
        }
        let _vocation = character_vocation::load_first(&mut tx, prior.prior.cid()).await?;
        super::character_status_effects::require_absent(&mut tx, prior.prior.cid()).await?;
        let mut loaded = crate::domain::initial_attendance::Loaded::all_user(prior.prior.social());
        let mut user_info = AttendanceInfo::NotRead;
        if crate::domain::initial_attendance::Loaded::enabled(profile.policy())? {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM game_user_info WHERE nexon_sn=$1")
                    .bind(owner.uid)
                    .fetch_one(&mut *tx)
                    .await?;
            let first = if count > 0 {
                let cid: Option<i64> = sqlx::query_scalar(
                    "SELECT last_login_cid FROM game_user_info WHERE nexon_sn=$1 FOR SHARE",
                )
                .bind(owner.uid)
                .fetch_one(&mut *tx)
                .await?;
                user_info = AttendanceInfo::Present(cid);
                cid.is_none()
            } else {
                user_info = AttendanceInfo::Missing;
                false
            };
            loaded.initialize_newbie(
                first,
                profile.policy(),
                |v| profile.attendance_period(v),
                &mut clock,
            )?;
        }
        tx.commit().await?;
        Ok(AttendanceManagers {
            prior,
            loaded,
            user_info,
            profile: *profile.fingerprint(),
        })
    })
    .await
    .map_err(|_| ManagerError::Timeout)?
}

#[must_use = "Roulette/Marble instance constructors only;Captcha/Join/Login/Create remain"]
pub struct EventManagers {
    prior: AttendanceManagers,
    loaded: crate::domain::initial_events::Loaded,
}
impl EventManagers {
    pub fn prior(&self) -> &AttendanceManagers {
        &self.prior
    }
    pub fn loaded(&self) -> &crate::domain::initial_events::Loaded {
        &self.loaded
    }
}
/// Independent AddItemJournal commit is deliberately outside the owner transaction.
/// Refuse stale retained state before any sequence/log side effect; no automatic
/// retry. Cancellation or a later failure can leave a consumed identity/journal.
pub async fn load_events(
    pool: &PgPool,
    owner: &UserKey<'_>,
    prior: AttendanceManagers,
    profile: &Profile,
    catalog: &Package,
) -> Result<EventManagers, ManagerError> {
    profile.bind(catalog)?;
    if profile.fingerprint() != &prior.profile {
        return Err(ManagerError::Conflict);
    }
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let social = prior.prior.prior.social();
        let cid = prior.prior.prior.cid();
        let mut tx = constructor_tx(pool, owner, &prior.prior.prior.prior.prior.prefix).await?;
        let rows = character_costumes::load_for_constructor(&mut tx, cid).await?;
        if !prior
            .prior
            .prior
            .costume()
            .matches_source(rows)
            .map_err(CostumeError::from)?
        {
            return Err(ManagerError::Conflict);
        }
        let _vocation = character_vocation::load_first(&mut tx, cid).await?;
        super::character_status_effects::require_absent(&mut tx, cid).await?;
        if prior.user_info != AttendanceInfo::NotRead {
            let row: Option<(Option<i64>,)> = sqlx::query_as(
                "SELECT last_login_cid FROM game_user_info WHERE nexon_sn=$1 FOR SHARE",
            )
            .bind(owner.uid)
            .fetch_optional(&mut *tx)
            .await?;
            let current = row.map_or(AttendanceInfo::Missing, |(last,)| {
                AttendanceInfo::Present(last)
            });
            if current != prior.user_info {
                return Err(ManagerError::Conflict);
            }
        }
        let pending = crate::domain::initial_events::Pending::before_journal(social);
        let disabled = profile.policy().is_enabled(Some("DisableLogging"))?;
        if !disabled {
            let source: Option<(Option<i64>,)> = sqlx::query_as(
                "SELECT source_last FROM game_journal_identity WHERE kind=2 FOR SHARE",
            )
            .fetch_optional(&mut *tx)
            .await?;
            if source.is_none() {
                return Err(ManagerError::Conflict);
            }
        }
        let journal = super::item_journals::add_marble_journal(pool, disabled).await;
        let loaded = pending.finish(journal);
        tx.commit().await?;
        Ok(EventManagers { prior, loaded })
    })
    .await
    .map_err(|_| ManagerError::Timeout)?
}
