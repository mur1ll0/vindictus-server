//! Exact source state needed before a normal-Fiona initializer; no gameplay success implied.
use crate::{
    domain::legacy_user_data::{SqlDateTime, SqlDateTime2},
    infrastructure::user_export::{self, PrivateText, nullable},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{collections::HashSet, io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;
const MAX_ROWS: usize = 100_000;
#[derive(Debug, Error)]
pub enum StateError {
    #[error("character initialization snapshot is outside the recovered profile")]
    Export,
    #[error("character initialization state conflicts with the source; nothing repaired")]
    Conflict,
    #[error("character initialization state database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("character initialization state input could not be read")]
    Input,
    #[error("character initialization state operation timed out")]
    Timeout,
    #[error("character initialization reference preservation is not initialized")]
    Uninitialized,
    #[error("character initialization is outside the recovered first-Fiona profile")]
    Profile,
    #[error("character initialization does not own the synthetic Fiona revision")]
    Ownership,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    titles: Vec<RawTitle>,
    goals: Vec<RawGoal>,
    presets: Vec<Preset>,
    micro: Vec<RawMicro>,
    missions: Vec<RawMission>,
    assignments: Vec<RawAssignment>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTitle {
    cid: PrivateText,
    title_id: i32,
    acquired: bool,
    #[serde(deserialize_with = "nullable")]
    acquired_time: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    acquired_quest_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    expire_ticks: Option<PrivateText>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGoal {
    cid: PrivateText,
    title_goal_id: i32,
    progress: i32,
}
#[derive(Deserialize, PartialEq, Eq, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
struct Preset {
    title_id: i32,
    class_restriction: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMicro {
    cid: PrivateText,
    micro_play_count: i32,
    micro_play_time: PrivateText,
    success_micro_play_count: i32,
    success_micro_play_time: PrivateText,
    daily_micro_play_count: i32,
    daily_free_play_count: i32,
    last_micro_play_started: PrivateText,
    daily_report_count: i32,
    last_reported_time: PrivateText,
    #[serde(deserialize_with = "nullable")]
    current_quest_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    report_received_count: Option<i32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMission {
    cid: PrivateText,
    completed_mission_count: i32,
    #[serde(deserialize_with = "nullable")]
    last_clear_ticks: Option<PrivateText>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAssignment {
    cid: PrivateText,
    id: PrivateText,
    mission_utf16: PrivateText,
    #[serde(deserialize_with = "nullable")]
    completion_progress: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    assignment_ticks: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    removed_ticks: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    progress0: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    progress1: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    progress2: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    progress3: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    progress4: Option<i32>,
}
#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Assignment {
    cid: i64,
    id: i64,
    mission_utf16: Vec<u8>,
    completion_progress: Option<i32>,
    assignment_ticks: Option<i64>,
    removed_ticks: Option<i64>,
    progress0: Option<i32>,
    progress1: Option<i32>,
    progress2: Option<i32>,
    progress3: Option<i32>,
    progress4: Option<i32>,
}
#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Title {
    cid: i64,
    title_id: i32,
    acquired: bool,
    acquired_time_units300: Option<i64>,
    acquired_quest_utf16: Option<Vec<u8>>,
    expire_ticks: Option<i64>,
}
#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Goal {
    cid: i64,
    title_goal_id: i32,
    progress: i32,
}
#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Micro {
    cid: i64,
    micro_play_count: i32,
    micro_play_time: i64,
    success_micro_play_count: i32,
    success_micro_play_time: i64,
    daily_micro_play_count: i32,
    daily_free_play_count: i32,
    last_micro_play_started_units300: i64,
    daily_report_count: i32,
    last_reported_time_units300: i64,
    current_quest_utf16: Option<Vec<u8>>,
    report_received_count: Option<i32>,
}
#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Mission {
    cid: i64,
    completed_mission_count: i32,
    last_clear_ticks: Option<i64>,
}
pub struct Snapshot {
    source_sha256: [u8; 32],
    titles: Vec<Title>,
    goals: Vec<Goal>,
    presets: Vec<Preset>,
    micro: Vec<Micro>,
    missions: Vec<Mission>,
    assignments: Vec<Assignment>,
}
fn id(v: PrivateText) -> Result<i64, StateError> {
    user_export::integer(v).map_err(|_| StateError::Export)
}
fn date(v: PrivateText) -> Result<i64, StateError> {
    let bytes: [u8; 8] = user_export::hex(v, 8)
        .map_err(|_| StateError::Export)?
        .try_into()
        .map_err(|_| StateError::Export)?;
    Ok(SqlDateTime::from_sql_binary(bytes)
        .map_err(|_| StateError::Export)?
        .units300())
}
fn ticks(v: PrivateText) -> Result<i64, StateError> {
    Ok(SqlDateTime2::from_ticks(id(v)?)
        .map_err(|_| StateError::Export)?
        .ticks())
}
fn text(v: PrivateText) -> Result<Vec<u8>, StateError> {
    let b = user_export::hex(v, 100).map_err(|_| StateError::Export)?;
    if !b.len().is_multiple_of(2) {
        return Err(StateError::Export);
    };
    Ok(b)
}
pub fn read_snapshot(reader: impl Read) -> Result<Snapshot, StateError> {
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| StateError::Input)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(StateError::Export);
    }
    let raw: Export = serde_json::from_slice(&bytes).map_err(|_| StateError::Export)?;
    if raw.format_version != 1
        || raw.source != "study_heroes.character-initialization"
        || [
            raw.titles.len(),
            raw.goals.len(),
            raw.presets.len(),
            raw.micro.len(),
            raw.missions.len(),
            raw.assignments.len(),
        ]
        .into_iter()
        .any(|n| n > MAX_ROWS)
    {
        return Err(StateError::Export);
    }
    let titles = raw
        .titles
        .into_iter()
        .map(|r| {
            Ok(Title {
                cid: id(r.cid)?,
                title_id: r.title_id,
                acquired: r.acquired,
                acquired_time_units300: r.acquired_time.map(date).transpose()?,
                acquired_quest_utf16: r.acquired_quest_utf16.map(text).transpose()?,
                expire_ticks: r.expire_ticks.map(ticks).transpose()?,
            })
        })
        .collect::<Result<Vec<_>, StateError>>()?;
    let goals = raw
        .goals
        .into_iter()
        .map(|r| {
            Ok(Goal {
                cid: id(r.cid)?,
                title_goal_id: r.title_goal_id,
                progress: r.progress,
            })
        })
        .collect::<Result<Vec<_>, StateError>>()?;
    let micro = raw
        .micro
        .into_iter()
        .map(|r| {
            Ok(Micro {
                cid: id(r.cid)?,
                micro_play_count: r.micro_play_count,
                micro_play_time: id(r.micro_play_time)?,
                success_micro_play_count: r.success_micro_play_count,
                success_micro_play_time: id(r.success_micro_play_time)?,
                daily_micro_play_count: r.daily_micro_play_count,
                daily_free_play_count: r.daily_free_play_count,
                last_micro_play_started_units300: date(r.last_micro_play_started)?,
                daily_report_count: r.daily_report_count,
                last_reported_time_units300: date(r.last_reported_time)?,
                current_quest_utf16: r.current_quest_utf16.map(text).transpose()?,
                report_received_count: r.report_received_count,
            })
        })
        .collect::<Result<Vec<_>, StateError>>()?;
    let missions = raw
        .missions
        .into_iter()
        .map(|r| {
            Ok(Mission {
                cid: id(r.cid)?,
                completed_mission_count: r.completed_mission_count,
                last_clear_ticks: r.last_clear_ticks.map(ticks).transpose()?,
            })
        })
        .collect::<Result<Vec<_>, StateError>>()?;
    let assignments = raw
        .assignments
        .into_iter()
        .map(|r| {
            let bytes = user_export::hex(r.mission_utf16, 256).map_err(|_| StateError::Export)?;
            if !bytes.len().is_multiple_of(2) {
                return Err(StateError::Export);
            }
            Ok(Assignment {
                cid: id(r.cid)?,
                id: id(r.id)?,
                mission_utf16: bytes,
                completion_progress: r.completion_progress,
                assignment_ticks: r.assignment_ticks.map(ticks).transpose()?,
                removed_ticks: r.removed_ticks.map(ticks).transpose()?,
                progress0: r.progress0,
                progress1: r.progress1,
                progress2: r.progress2,
                progress3: r.progress3,
                progress4: r.progress4,
            })
        })
        .collect::<Result<Vec<_>, StateError>>()?;
    let mut seen = HashSet::new();
    if assignments.iter().any(|r| !seen.insert((r.cid, r.id))) {
        return Err(StateError::Export);
    }
    let mut seen = HashSet::new();
    if titles.iter().any(|r| !seen.insert((r.cid, r.title_id))) {
        return Err(StateError::Export);
    }
    let mut seen = HashSet::new();
    if goals.iter().any(|r| !seen.insert((r.cid, r.title_goal_id))) {
        return Err(StateError::Export);
    }
    let mut seen = HashSet::new();
    if raw.presets.iter().any(|r| !seen.insert(r.title_id)) {
        return Err(StateError::Export);
    }
    let mut seen = HashSet::new();
    if micro.iter().any(|r| !seen.insert(r.cid)) {
        return Err(StateError::Export);
    }
    let mut seen = HashSet::new();
    if missions.iter().any(|r| !seen.insert(r.cid)) {
        return Err(StateError::Export);
    }
    Ok(Snapshot {
        source_sha256: Sha256::digest(bytes.as_slice()).into(),
        titles,
        goals,
        presets: raw.presets,
        micro,
        missions,
        assignments,
    })
}
#[derive(Serialize)]
pub struct Summary {
    pub source_rows: [usize; 6],
    pub inserted: [usize; 6],
    pub verified: [usize; 6],
    pub preservation_initialized: bool,
    pub character_creation_available: bool,
}
impl Snapshot {
    pub fn counts(&self) -> [usize; 6] {
        [
            self.titles.len(),
            self.goals.len(),
            self.presets.len(),
            self.micro.len(),
            self.missions.len(),
            self.assignments.len(),
        ]
    }
}
impl Keyed for Assignment {
    type Key = (i64, i64);
    fn key(&self) -> Self::Key {
        (self.cid, self.id)
    }
}
trait Keyed {
    type Key: Ord;
    fn key(&self) -> Self::Key;
}
impl Keyed for Title {
    type Key = (i64, i32);
    fn key(&self) -> Self::Key {
        (self.cid, self.title_id)
    }
}
impl Keyed for Goal {
    type Key = (i64, i32);
    fn key(&self) -> Self::Key {
        (self.cid, self.title_goal_id)
    }
}
impl Keyed for Preset {
    type Key = i32;
    fn key(&self) -> Self::Key {
        self.title_id
    }
}
impl Keyed for Micro {
    type Key = i64;
    fn key(&self) -> Self::Key {
        self.cid
    }
}
impl Keyed for Mission {
    type Key = i64;
    fn key(&self) -> Self::Key {
        self.cid
    }
}
fn compare<T: Keyed + PartialEq>(
    source: &[T],
    stored: Vec<T>,
    exact: bool,
) -> Result<(), StateError> {
    if stored.len() > MAX_ROWS {
        return Err(StateError::Conflict);
    }
    let map = stored
        .into_iter()
        .map(|r| (r.key(), r))
        .collect::<std::collections::BTreeMap<_, _>>();
    if exact && map.len() != source.len() {
        return Err(StateError::Conflict);
    }
    for r in source {
        if map.get(&r.key()) != Some(r) {
            return Err(StateError::Conflict);
        }
    }
    Ok(())
}
pub async fn preserve(
    pool: &PgPool,
    s: &Snapshot,
    verify_only: bool,
) -> Result<Summary, StateError> {
    tokio::time::timeout(Duration::from_secs(60),async{
  let mut tx=pool.begin().await?;
  sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
  sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
  let state:(bool,Option<Vec<i64>>,Option<Vec<u8>>)=sqlx::query_as("SELECT initialized,source_rows,source_sha256 FROM game_character_state_preservation WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
  let counts=s.counts();let source_counts=counts.map(|n|n as i64);
  if state.0&&(state.1.as_deref()!=Some(source_counts.as_slice())||state.2.as_deref()!=Some(s.source_sha256.as_slice())){return Err(StateError::Conflict)}
  if verify_only&&!state.0{return Err(StateError::Uninitialized)}
  sqlx::query("LOCK TABLE game_titles,game_title_goal_progress,game_title_presets,game_micro_play_status,game_character_mission_info,game_mission_assignments IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let mut inserted=[0;6];
  if !state.0&&!verify_only{
   for r in &s.titles {inserted[0]+=sqlx::query("INSERT INTO game_titles(cid,title_id,acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING").bind(r.cid).bind(r.title_id).bind(r.acquired).bind(r.acquired_time_units300).bind(r.acquired_quest_utf16.as_deref()).bind(r.expire_ticks).execute(&mut *tx).await?.rows_affected() as usize;}
   for r in &s.goals {inserted[1]+=sqlx::query("INSERT INTO game_title_goal_progress(cid,title_goal_id,progress) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(r.cid).bind(r.title_goal_id).bind(r.progress).execute(&mut *tx).await?.rows_affected() as usize;}
   for r in &s.presets {inserted[2]+=sqlx::query("INSERT INTO game_title_presets(title_id,class_restriction) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(r.title_id).bind(r.class_restriction).execute(&mut *tx).await?.rows_affected() as usize;}
   for r in &s.micro {inserted[3]+=sqlx::query("INSERT INTO game_micro_play_status(cid,micro_play_count,micro_play_time,success_micro_play_count,success_micro_play_time,daily_micro_play_count,daily_free_play_count,last_micro_play_started_units300,daily_report_count,last_reported_time_units300,current_quest_utf16,report_received_count) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT DO NOTHING").bind(r.cid).bind(r.micro_play_count).bind(r.micro_play_time).bind(r.success_micro_play_count).bind(r.success_micro_play_time).bind(r.daily_micro_play_count).bind(r.daily_free_play_count).bind(r.last_micro_play_started_units300).bind(r.daily_report_count).bind(r.last_reported_time_units300).bind(r.current_quest_utf16.as_deref()).bind(r.report_received_count).execute(&mut *tx).await?.rows_affected() as usize;}
   for r in &s.missions {inserted[4]+=sqlx::query("INSERT INTO game_character_mission_info(cid,completed_mission_count,last_clear_ticks) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(r.cid).bind(r.completed_mission_count).bind(r.last_clear_ticks).execute(&mut *tx).await?.rows_affected() as usize;}
  }
  if !state.0&&!verify_only{for r in &s.assignments{
   inserted[5]+=sqlx::query("INSERT INTO game_mission_assignments(cid,id,mission_utf16,completion_progress,assignment_ticks,removed_ticks,progress0,progress1,progress2,progress3,progress4) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT DO NOTHING").bind(r.cid).bind(r.id).bind(&r.mission_utf16).bind(r.completion_progress).bind(r.assignment_ticks).bind(r.removed_ticks).bind(r.progress0).bind(r.progress1).bind(r.progress2).bind(r.progress3).bind(r.progress4).execute(&mut *tx).await?.rows_affected() as usize;
  }}
  let exact=!state.0;
  compare(&s.titles,sqlx::query_as("SELECT cid,title_id,acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks FROM game_titles LIMIT 100001").fetch_all(&mut *tx).await?,exact)?;
  compare(&s.goals,sqlx::query_as("SELECT cid,title_goal_id,progress FROM game_title_goal_progress LIMIT 100001").fetch_all(&mut *tx).await?,exact)?;
  compare(&s.presets,sqlx::query_as("SELECT title_id,class_restriction FROM game_title_presets LIMIT 100001").fetch_all(&mut *tx).await?,exact)?;
  compare(&s.micro,sqlx::query_as("SELECT cid,micro_play_count,micro_play_time,success_micro_play_count,success_micro_play_time,daily_micro_play_count,daily_free_play_count,last_micro_play_started_units300,daily_report_count,last_reported_time_units300,current_quest_utf16,report_received_count FROM game_micro_play_status LIMIT 100001").fetch_all(&mut *tx).await?,exact)?;
  compare(&s.missions,sqlx::query_as("SELECT cid,completed_mission_count,last_clear_ticks FROM game_character_mission_info LIMIT 100001").fetch_all(&mut *tx).await?,exact)?;
  compare(&s.assignments,sqlx::query_as("SELECT cid,id,mission_utf16,completion_progress,assignment_ticks,removed_ticks,progress0,progress1,progress2,progress3,progress4 FROM game_mission_assignments LIMIT 100001").fetch_all(&mut *tx).await?,exact)?;
  if !state.0 {sqlx::query("UPDATE game_character_state_preservation SET initialized=true,source_rows=$1,source_sha256=$2 WHERE singleton").bind(source_counts.as_slice()).bind(s.source_sha256.as_slice()).execute(&mut *tx).await?;}
  tx.commit().await?;Ok(Summary{source_rows:counts,inserted,verified:counts,preservation_initialized:true,character_creation_available:false})
 }).await.map_err(|_|StateError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), StateError> {
    sqlx::query("SELECT cid,title_id,acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks FROM game_titles LIMIT 0").execute(pool).await?;
    sqlx::query("SELECT cid,title_goal_id,progress FROM game_title_goal_progress LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query("SELECT title_id,class_restriction FROM game_title_presets LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query("SELECT cid,micro_play_count,micro_play_time,success_micro_play_count,success_micro_play_time,daily_micro_play_count,daily_free_play_count,last_micro_play_started_units300,daily_report_count,last_reported_time_units300,current_quest_utf16,report_received_count FROM game_micro_play_status LIMIT 0").execute(pool).await?;
    sqlx::query("SELECT cid,completed_mission_count,last_clear_ticks FROM game_character_mission_info LIMIT 0").execute(pool).await?;
    sqlx::query("SELECT cid,id,mission_utf16,completion_progress,assignment_ticks,removed_ticks,progress0,progress1,progress2,progress3,progress4 FROM game_mission_assignments LIMIT 0").execute(pool).await?;
    sqlx::query(
        "SELECT singleton,initialized,source_rows,source_sha256 FROM game_character_state_preservation LIMIT 0",
    )
    .execute(pool)
    .await?;
    Ok(())
}
pub(crate) async fn owned_first_fiona<'p>(
    pool: &'p PgPool,
    key: &crate::infrastructure::character_lobby::UserKey<'_>,
    cid: i64,
) -> Result<sqlx::Transaction<'p, sqlx::Postgres>, StateError> {
    use crate::domain::game_user::LegacyNameKey;
    let owner = LegacyNameKey::parse(key.name).map_err(|_| StateError::Ownership)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let initialized: bool = sqlx::query_scalar(
        "SELECT initialized FROM game_character_state_preservation WHERE singleton FOR SHARE",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !initialized {
        return Err(StateError::Uninitialized);
    }
    let user: Option<(String, i64)> =
        sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE")
            .bind(key.uid)
            .fetch_optional(&mut *tx)
            .await?;
    let origin: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)",
    )
    .bind(key.uid)
    .bind(owner.as_str())
    .fetch_one(&mut *tx)
    .await?;
    if user != Some((owner.as_str().to_owned(), key.revision)) || !origin {
        return Err(StateError::Ownership);
    }
    sqlx::query("LOCK TABLE game_character_info IN ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let owned:Option<i64>=sqlx::query_scalar("SELECT id FROM game_character_info WHERE id=$1 AND uid=$2 AND class BETWEEN 0 AND 27 AND level>=1 AND NOT use_preset AND NOT is_premium AND status=0 FOR UPDATE").bind(cid).bind(key.uid).fetch_optional(&mut *tx).await?;
    if owned.is_none() {
        return Err(StateError::Ownership);
    }
    // Other characters of the account are allowed: every lab character carries its own
    // initialization marker, and account-wide state is checked where it is shared.
    Ok(tx)
}
/// Original MissionBook constructor precedes Stats. The clear time is the recovered
/// manager state; disabled RandomMission leaves its DateTime field at zero ticks.
pub async fn initialize_mission(
    pool: &PgPool,
    key: &crate::infrastructure::character_lobby::UserKey<'_>,
    cid: i64,
    clear: SqlDateTime2,
) -> Result<usize, StateError> {
    tokio::time::timeout(Duration::from_secs(15),async{
  let mut tx=owned_first_fiona(pool,key,cid).await?;
  sqlx::query("LOCK TABLE game_character_mission_info,game_mission_assignments IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let assignments:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_mission_assignments WHERE cid=$1)").bind(cid).fetch_one(&mut *tx).await?;
  if assignments{return Err(StateError::Profile)}
  let stored:Option<Mission>=sqlx::query_as("SELECT cid,completed_mission_count,last_clear_ticks FROM game_character_mission_info WHERE cid=$1 FOR UPDATE").bind(cid).fetch_optional(&mut *tx).await?;
  let changed=match stored{
   None=>{sqlx::query("INSERT INTO game_character_mission_info(cid,completed_mission_count,last_clear_ticks) VALUES($1,0,$2)").bind(cid).bind(clear.ticks()).execute(&mut *tx).await?;1},
   Some(r)if r.completed_mission_count==0&&r.last_clear_ticks==Some(clear.ticks())=>0,
   Some(_)=>return Err(StateError::Conflict),
  };
  tx.commit().await?;Ok(changed)
 }).await.map_err(|_|StateError::Timeout)?
}
/// Source InitializeMicroPlayStatus and InitializeChatReport sample UTC separately.
/// ReportReceivedCount is not assigned by either method and starts as SQL NULL.
#[derive(Clone, Copy)]
pub struct MicroSeed {
    pub started: SqlDateTime,
    pub reported: SqlDateTime,
}
pub async fn initialize_micro(
    pool: &PgPool,
    key: &crate::infrastructure::character_lobby::UserKey<'_>,
    cid: i64,
    seed: MicroSeed,
) -> Result<usize, StateError> {
    tokio::time::timeout(Duration::from_secs(15),async{
  let mut tx=owned_first_fiona(pool,key,cid).await?;
  sqlx::query("LOCK TABLE game_micro_play_status IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let expected=Micro{cid,micro_play_count:0,micro_play_time:0,success_micro_play_count:0,success_micro_play_time:0,daily_micro_play_count:0,daily_free_play_count:0,last_micro_play_started_units300:seed.started.units300(),daily_report_count:0,last_reported_time_units300:seed.reported.units300(),current_quest_utf16:None,report_received_count:None};
  let stored:Option<Micro>=sqlx::query_as("SELECT cid,micro_play_count,micro_play_time,success_micro_play_count,success_micro_play_time,daily_micro_play_count,daily_free_play_count,last_micro_play_started_units300,daily_report_count,last_reported_time_units300,current_quest_utf16,report_received_count FROM game_micro_play_status WHERE cid=$1 FOR UPDATE").bind(cid).fetch_optional(&mut *tx).await?;
  let changed=match stored{
   None=>{sqlx::query("INSERT INTO game_micro_play_status(cid,micro_play_count,micro_play_time,success_micro_play_count,success_micro_play_time,daily_micro_play_count,daily_free_play_count,last_micro_play_started_units300,daily_report_count,last_reported_time_units300,current_quest_utf16,report_received_count) VALUES($1,0,0,0,0,0,0,$2,0,$3,NULL,NULL)").bind(cid).bind(seed.started.units300()).bind(seed.reported.units300()).execute(&mut *tx).await?;1},
   Some(r)if r==expected=>0,
   Some(_)=>return Err(StateError::Conflict),
  };
  tx.commit().await?;Ok(changed)
 }).await.map_err(|_|StateError::Timeout)?
}
/// Prepared IDs must match the preserved, source-validated preset for the character's class bit.
/// Returned IDs are newly given titles whose original GiveTitle log is still required.
pub async fn seed_title_presets(
    pool: &PgPool,
    key: &crate::infrastructure::character_lobby::UserKey<'_>,
    cid: i64,
    prepared_ids: &[i32],
) -> Result<Vec<i32>, StateError> {
    if prepared_ids.len() > 4096 || prepared_ids.windows(2).any(|w| w[0] >= w[1]) {
        return Err(StateError::Profile);
    }
    tokio::time::timeout(Duration::from_secs(15),async{
  let mut tx=owned_first_fiona(pool,key,cid).await?;
  sqlx::query("LOCK TABLE game_titles,game_title_goal_progress,game_title_presets IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let ids:Vec<i32>=sqlx::query_scalar("SELECT title_id FROM game_title_presets WHERE (class_restriction & (1<<(SELECT class FROM game_character_info WHERE id=$1)))<>0 ORDER BY title_id LIMIT 4097 FOR SHARE").bind(cid).fetch_all(&mut *tx).await?;
  if ids!=prepared_ids{return Err(StateError::Profile)}
  let progress:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_title_goal_progress WHERE cid=$1)").bind(cid).fetch_one(&mut *tx).await?;
  if progress{return Err(StateError::Conflict)}
  let old:Vec<Title>=sqlx::query_as("SELECT cid,title_id,acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks FROM game_titles WHERE cid=$1 ORDER BY title_id LIMIT 4097 FOR UPDATE").bind(cid).fetch_all(&mut *tx).await?;
  if !old.is_empty(){
   if old.len()!=ids.len()||old.iter().zip(&ids).any(|(r,id)|r.title_id!=*id||r.acquired||r.acquired_time_units300.is_some()||r.acquired_quest_utf16.is_some()||r.expire_ticks.is_some()){return Err(StateError::Conflict)}
   tx.commit().await?;return Ok(Vec::new());
  }
  sqlx::query("INSERT INTO game_titles(cid,title_id,acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks) SELECT $1,unnest($2::int[]),false,NULL,NULL,NULL").bind(cid).bind(&ids).execute(&mut *tx).await?;
  tx.commit().await?;Ok(ids)
 }).await.map_err(|_|StateError::Timeout)?
}
/// Runtime content is bound to the exact protected import snapshot, not just row counts.
pub async fn verify_source_binding(pool: &PgPool, expected: &[u8; 32]) -> Result<(), StateError> {
    let row: (bool, Option<Vec<u8>>) = sqlx::query_as(
        "SELECT initialized,source_sha256 FROM game_character_state_preservation WHERE singleton",
    )
    .fetch_one(pool)
    .await?;
    if !row.0 {
        return Err(StateError::Uninitialized);
    }
    if row.1.as_deref() != Some(expected.as_slice()) {
        return Err(StateError::Conflict);
    }
    Ok(())
}

/// Reset following this fresh row's insert. Compare all twelve retained fields;
/// the source first Submit remains committed if this later Submit fails.
pub async fn reset_fresh_micro(
    pool: &PgPool,
    key: &crate::infrastructure::character_lobby::UserKey<'_>,
    cid: i64,
    seed: MicroSeed,
    reset: SqlDateTime,
) -> Result<(), StateError> {
    tokio::time::timeout(Duration::from_secs(15),async{
  let mut tx=owned_first_fiona(pool,key,cid).await?;
  let stored:Option<Micro>=sqlx::query_as("SELECT cid,micro_play_count,micro_play_time,success_micro_play_count,success_micro_play_time,daily_micro_play_count,daily_free_play_count,last_micro_play_started_units300,daily_report_count,last_reported_time_units300,current_quest_utf16,report_received_count FROM game_micro_play_status WHERE cid=$1 FOR UPDATE").bind(cid).fetch_optional(&mut *tx).await?;
  let expected=Micro{cid,micro_play_count:0,micro_play_time:0,success_micro_play_count:0,success_micro_play_time:0,daily_micro_play_count:0,daily_free_play_count:0,last_micro_play_started_units300:seed.started.units300(),daily_report_count:0,last_reported_time_units300:seed.reported.units300(),current_quest_utf16:None,report_received_count:None};
  if stored!=Some(expected){return Err(StateError::Conflict)}
  sqlx::query("UPDATE game_micro_play_status SET daily_micro_play_count=0,daily_free_play_count=0,last_micro_play_started_units300=$2 WHERE cid=$1").bind(cid).bind(reset.units300()).execute(&mut *tx).await?;
  tx.commit().await?;Ok(())
 }).await.map_err(|_|StateError::Timeout)?
}
