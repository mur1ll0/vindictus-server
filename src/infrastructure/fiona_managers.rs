//! Ordered fresh first-Fiona constructor prefix: MissionBook, initial titles/base
//! stats, then SkillManager's SP slot. Stops before Costume/status/attendance/Join.
use super::{
    character_base_stats::{self, BaseStatError},
    character_lobby::UserKey,
    character_skills::{self, SkillError},
    fiona_character_state::{self as state, StateError},
    fiona_initializer_profile::Profile,
    item_catalog::Package,
};
use crate::{
    domain::{character_base_stats::BaseStats, legacy_user_data::SqlDateTime2},
    error::Error,
};
use sqlx::PgPool;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum ManagerError {
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
    pub cid: i64,
    pub base: BaseStats,
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
        Ok(Managers{cid,base})
    }).await.map_err(|_|ManagerError::Timeout)?
}
