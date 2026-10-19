//! Fresh GiveTitlePreset:independent SQL insert,streamed cache/goals,logs and
//! detached TitleObservable. Not acquisition,network CacheSync or full TitleManager.
use super::{
    character_initialization_logs as logs,
    character_lobby::UserKey,
    fiona_character_state as state,
    fiona_initializer_profile::Profile,
    fiona_managers::MicroJoined,
    item_catalog::Package,
    user_export::{PrivateText, hex},
};
use crate::error::Error;
use serde::Deserialize;
use sqlx::PgPool;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    sync::Arc,
    time::Duration,
};
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    initializer_sha256: PrivateText,
    title_manager_sha256: String,
    contents_sha256: String,
    titles: Vec<TitleGoals>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TitleGoals {
    title_id: i32,
    goal_ids: Vec<i32>,
}
pub struct GoalProfile {
    initializer: [u8; 32],
    goals: BTreeMap<i32, Vec<i32>>,
}
impl GoalProfile {
    pub fn read(input: impl Read, profile: &Profile, catalog: &Package) -> Result<Self, Error> {
        profile.bind(catalog)?;
        let fail = || Error::FionaInitializerProfile;
        let mut bytes = Zeroizing::new(Vec::new());
        input.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(fail());
        }
        let r: Raw = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        let code: serde_json::Value =
            serde_json::from_str(include_str!("../domain/fiona_initializer_sources.json"))
                .map_err(|_| fail())?;
        let initializer: [u8; 32] = hex(r.initializer_sha256, 32)?
            .try_into()
            .map_err(|_| fail())?;
        if r.format_version != 1
            || r.source != "heroesContents.initial-title-goals"
            || r.database_sha256 != catalog.manifest.database_sha256
            || r.config_sha256 != catalog.manifest.config_sha256
            || initializer != *profile.fingerprint()
            || r.title_manager_sha256 != code["titles"]["sha256"]
            || r.contents_sha256 != code["contents"]["sha256"]
            || r.titles.len() > 4096
        {
            return Err(fail());
        }
        let mut goals = BTreeMap::new();
        let mut all = BTreeSet::new();
        for t in r.titles {
            if t.goal_ids.len() > 1024
                || t.goal_ids.iter().any(|g| !all.insert(*g))
                || all.len() > 16384
                || goals.insert(t.title_id, t.goal_ids).is_some()
            {
                return Err(fail());
            }
        }
        if goals.keys().copied().collect::<Vec<_>>() != profile.preset_ids() {
            return Err(fail());
        }
        Ok(Self { initializer, goals })
    }
    pub fn title_count(&self) -> usize {
        self.goals.len()
    }
    pub fn goal_count(&self) -> usize {
        self.goals.values().map(Vec::len).sum()
    }
}
pub struct Goal {
    cid: i64,
    id: i32,
    progress: i32,
}
impl Goal {
    pub fn cid(&self) -> i64 {
        self.cid
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn progress(&self) -> i32 {
        self.progress
    }
}
pub struct Title {
    cid: i64,
    id: i32,
    goals: Vec<Arc<Goal>>,
}
impl Title {
    pub fn cid(&self) -> i64 {
        self.cid
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn acquired(&self) -> bool {
        false
    }
    pub fn goals(&self) -> &[Arc<Goal>] {
        &self.goals
    }
}
#[derive(Default)]
pub struct Cache {
    titles: BTreeMap<i32, Title>,
    goals: BTreeMap<i32, Arc<Goal>>,
}
impl Cache {
    pub fn titles(&self) -> impl Iterator<Item = &Title> {
        self.titles.values()
    }
    pub fn goals(&self) -> impl Iterator<Item = &Arc<Goal>> {
        self.goals.values()
    }
    fn insert(&mut self, cid: i64, id: i32, ids: &[i32]) -> Result<(), TitleError> {
        if self.titles.contains_key(&id) {
            return Err(TitleError::Conflict);
        }
        // Source adds Title before its goals;retain that prefix on a later failure.
        self.titles.insert(
            id,
            Title {
                cid,
                id,
                goals: Vec::new(),
            },
        );
        for goal in ids {
            let entry = Arc::new(Goal {
                cid,
                id: *goal,
                progress: 0,
            });
            if let Some(t) = self.titles.get_mut(&id) {
                t.goals.push(Arc::clone(&entry));
            }
            if self.goals.contains_key(goal) {
                return Err(TitleError::Conflict);
            }
            self.goals.insert(*goal, entry);
        }
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum TitleError {
    #[error(transparent)]
    State(#[from] state::StateError),
    #[error(transparent)]
    Content(#[from] Error),
    #[error("fresh title phase database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("fresh title phase snapshot conflict;reconcile before retry")]
    Conflict,
}
#[must_use = "preset titles only;LoadPlayer/Login/customization/release/Create remain"]
pub struct TitlesJoined {
    prior: MicroJoined,
    cache: Cache,
    source_caught: bool,
    title_updated: bool,
    logs_written: usize,
    logs_failed: usize,
}
impl TitlesJoined {
    pub fn prior(&self) -> &MicroJoined {
        &self.prior
    }
    pub fn cache(&self) -> &Cache {
        &self.cache
    }
    pub fn source_caught(&self) -> bool {
        self.source_caught
    }
    pub fn title_updated(&self) -> bool {
        self.title_updated
    }
    pub fn logs_written(&self) -> usize {
        self.logs_written
    }
    pub fn logs_failed(&self) -> usize {
        self.logs_failed
    }
}
#[derive(sqlx::FromRow)]
struct Stored {
    title_id: i32,
    acquired: bool,
    acquired_time_units300: Option<i64>,
    acquired_quest_utf16: Option<Vec<u8>>,
    expire_ticks: Option<i64>,
}
/// Fresh profile is already synchronized:all applicable TitleInfo exist and their
/// features are enabled. Other title-content branches remain outside admission.
pub async fn give_presets(
    pool: &PgPool,
    owner: &UserKey<'_>,
    prior: MicroJoined,
    profile: &Profile,
    catalog: &Package,
    goals: &GoalProfile,
) -> Result<TitlesJoined, TitleError> {
    profile.bind(catalog)?;
    let root = prior.prior().root();
    let cid = root.cid();
    if goals.initializer != *profile.fingerprint()
        || root.profile_fingerprint() != profile.fingerprint()
        || root.uid() != owner.uid
        || !profile.policy().is_enabled(Some("AddTitleFromPreset"))?
    {
        return Err(TitleError::Conflict);
    }
    state::verify_source_binding(pool, profile.source_state()).await?;
    {
        let mut tx = state::owned_first_fiona(pool, owner, cid).await?;
        let pristine:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM game_titles WHERE cid=$1) AND NOT EXISTS(SELECT 1 FROM game_title_goal_progress WHERE cid=$1) AND EXISTS(SELECT 1 FROM game_micro_play_status WHERE cid=$1 AND last_micro_play_started_units300=$2 AND last_reported_time_units300=$3 AND daily_micro_play_count=0 AND daily_free_play_count=0) AND EXISTS(SELECT 1 FROM game_user_info WHERE nexon_sn=$4 AND last_login_cid=$1)").bind(cid).bind(prior.started().sql_parameter().map_err(|_|TitleError::Conflict)?.units300()).bind(prior.reported().sql_parameter().map_err(|_|TitleError::Conflict)?.units300()).bind(owner.uid).fetch_one(&mut *tx).await?;
        if !pristine {
            return Err(TitleError::Conflict);
        }
        tx.commit().await?;
    }
    let mut out = TitlesJoined {
        prior,
        cache: Cache::default(),
        source_caught: false,
        title_updated: false,
        logs_written: 0,
        logs_failed: 0,
    };
    let inserted = match state::seed_title_presets(pool, owner, cid, profile.preset_ids()).await {
        Ok(ids) => ids,
        Err(state::StateError::Database(_) | state::StateError::Timeout) => {
            out.source_caught = true;
            return Ok(out);
        }
        Err(e) => return Err(e.into()),
    };
    if inserted != profile.preset_ids() {
        return Err(TitleError::Conflict);
    }
    // The source INSERT has committed before result enumeration. A later provider
    // error retains both SQL rows and the cache/log prefix already processed.
    let mut rows=sqlx::query_as::<_,Stored>("SELECT title_id,acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks FROM game_titles WHERE cid=$1 ORDER BY title_id").bind(cid).fetch(pool);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let next = tokio::time::timeout_at(
            deadline,
            std::future::poll_fn(|cx| rows.as_mut().poll_next(cx)),
        )
        .await;
        let row = match next {
            Ok(Some(Ok(row))) => row,
            Ok(None) => break,
            _ => {
                out.source_caught = true;
                return Ok(out);
            }
        };
        let ids = goals.goals.get(&row.title_id).ok_or(TitleError::Conflict)?;
        if row.acquired
            || row.acquired_time_units300.is_some()
            || row.acquired_quest_utf16.is_some()
            || row.expire_ticks.is_some()
        {
            return Err(TitleError::Conflict);
        }
        out.cache.insert(cid, row.title_id, ids)?;
        if logs::give_title(pool, row.title_id, deadline).await {
            out.logs_written += 1
        } else {
            out.logs_failed += 1
        }
    }
    if out.cache.titles.len() != profile.preset_ids().len() {
        return Err(TitleError::Conflict);
    }
    // Exact Updated on a fresh collection:zero proxies and no CacheUpdated delegate.
    if out.prior.prior().observables().proxy_count() != 0 {
        return Err(TitleError::Conflict);
    }
    out.title_updated = true;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_title_goal_graph_matches_original_framework_constructors() {
        let f: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/framework-title-preset.json"
        ))
        .unwrap();
        assert_eq!(f["x86"], f["x64"]);
        assert_eq!(f["x86"].as_array().unwrap().len(), 40);
        // Operation/provider failures are covered by PG tests. Compare the original
        // title/goal constructor graph for every successfully enumerated source prefix.
        for c in f["x86"].as_array().unwrap() {
            let mut cache = Cache::default();
            for t in c["titles"].as_array().unwrap() {
                let id = i32::try_from(t["id"].as_i64().unwrap()).unwrap();
                let ids = t["goals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|g| i32::try_from(g["id"].as_i64().unwrap()).unwrap())
                    .collect::<Vec<_>>();
                cache.insert(101, id, &ids).unwrap();
                let title = &cache.titles[&id];
                assert_eq!(
                    title.cid(),
                    t["cid"].as_str().unwrap().parse::<i64>().unwrap()
                );
                assert_eq!(title.acquired(), t["acquired"].as_bool().unwrap());
                for (g, expected) in title.goals().iter().zip(t["goals"].as_array().unwrap()) {
                    assert_eq!(g.progress(), expected["progress"].as_i64().unwrap() as i32);
                    assert!(Arc::ptr_eq(g, &cache.goals[&g.id()]));
                    assert_eq!(expected["same"], true);
                }
            }
        }
    }
}
