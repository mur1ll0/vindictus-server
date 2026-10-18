//! Full private Vocation/VocationSkill preservation and restricted first-Fiona read.
use crate::{
    domain::{
        initial_vocation::Loaded,
        legacy_user_data::{LegacyBytes, SqlDateTime2},
    },
    infrastructure::{
        item_catalog,
        user_export::{PrivateText, integer, nullable},
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::{collections::HashSet, io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum VocationError {
    #[error("invalid bounded vocation snapshot")]
    Export,
    #[error("vocation input could not be read")]
    Input(#[from] std::io::Error),
    #[error("vocation database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("vocation reference preservation is not initialized")]
    Uninitialized,
    #[error("vocation preservation conflict;no repair performed")]
    Conflict,
    #[error("first-Fiona constructor requires a proven absent vocation")]
    Profile,
    #[error("vocation deadline exceeded;reconcile committed stages before retry")]
    Timeout,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    #[serde(deserialize_with = "nullable")]
    source_last_value: Option<PrivateText>,
    vocations: Vec<RawVocation>,
    skills: Vec<RawSkill>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVocation {
    cid: PrivateText,
    vocation_class: i32,
    vocation_level: i32,
    vocation_exp: i32,
    #[serde(deserialize_with = "nullable")]
    last_transform_ticks: Option<PrivateText>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSkill {
    row_id: PrivateText,
    cid: PrivateText,
    skill_utf16: PrivateText,
    rank: i32,
}
#[derive(PartialEq, Eq)]
struct Vocation {
    cid: i64,
    class: i32,
    level: i32,
    exp: i32,
    last: Option<i64>,
}
struct Row {
    id: i64,
    cid: i64,
    name: LegacyBytes<50, true>,
    rank: i32,
}
pub struct Snapshot {
    last: Option<i64>,
    vocations: Vec<Vocation>,
    skills: Vec<Row>,
    hash: [u8; 32],
}
fn id(v: PrivateText) -> Result<i64, VocationError> {
    integer(v).map_err(|_| VocationError::Export)
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, VocationError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(VocationError::Export);
    }
    let hash = Sha256::digest(&*bytes).into();
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| VocationError::Export)?;
    if e.format_version != 1
        || e.source != "study_heroes.vocation-state"
        || e.vocations.len() > 100000
        || e.skills.len() > 100000
    {
        return Err(VocationError::Export);
    }
    let last = e.source_last_value.map(id).transpose()?;
    if last.is_some_and(|v| !(0..i64::MAX).contains(&v)) {
        return Err(VocationError::Export);
    }
    let mut cids = HashSet::new();
    let mut vocations = Vec::with_capacity(e.vocations.len());
    for r in e.vocations {
        let cid = id(r.cid)?;
        if !cids.insert(cid) {
            return Err(VocationError::Export);
        }
        let time = r.last_transform_ticks.map(id).transpose()?;
        if time.is_some_and(|v| SqlDateTime2::from_ticks(v).is_err()) {
            return Err(VocationError::Export);
        }
        vocations.push(Vocation {
            cid,
            class: r.vocation_class,
            level: r.vocation_level,
            exp: r.vocation_exp,
            last: time,
        });
    }
    let mut ids = HashSet::new();
    let mut skills = Vec::with_capacity(e.skills.len());
    for r in e.skills {
        let row_id = id(r.row_id)?;
        let cid = id(r.cid)?;
        if row_id < 1
            || !last.is_some_and(|v| row_id <= v)
            || !ids.insert(row_id)
            || !cids.contains(&cid)
        {
            return Err(VocationError::Export);
        }
        let name = item_catalog::opaque(Some(r.skill_utf16))
            .map_err(|_| VocationError::Export)?
            .ok_or(VocationError::Export)?;
        let name =
            LegacyBytes::from_bytes(name.as_bytes().to_vec()).map_err(|_| VocationError::Export)?;
        skills.push(Row {
            id: row_id,
            cid,
            name,
            rank: r.rank,
        });
    }
    Ok(Snapshot {
        last,
        vocations,
        skills,
        hash,
    })
}
#[derive(Serialize)]
pub struct Summary {
    pub vocations: usize,
    pub skills: usize,
    pub inserted: usize,
    pub verified: usize,
    pub identity_initialized: bool,
    pub character_creation_available: bool,
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, VocationError> {
    tokio::time::timeout(Duration::from_secs(30),async{
 let mut tx=pool.begin().await?;
 sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
 sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
 let state:(bool,Option<i64>,Option<Vec<u8>>)=sqlx::query_as("SELECT initialized,source_last_value,source_sha256 FROM game_vocation_preservation WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
 if state.0&&(state.1!=s.last||state.2.as_deref()!=Some(s.hash.as_slice())){return Err(VocationError::Conflict)}
 sqlx::query("LOCK TABLE game_vocations,game_vocation_skills IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
 let mut inserted=0;
 for r in &s.vocations{
  if !verify&&!state.0{inserted+=sqlx::query("INSERT INTO game_vocations(cid,vocation_class,vocation_level,vocation_exp,last_transform_ticks) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING").bind(r.cid).bind(r.class).bind(r.level).bind(r.exp).bind(r.last).execute(&mut *tx).await?.rows_affected() as usize;}
  let actual:Option<(i32,i32,i32,Option<i64>)>=sqlx::query_as("SELECT vocation_class,vocation_level,vocation_exp,last_transform_ticks FROM game_vocations WHERE cid=$1").bind(r.cid).fetch_optional(&mut *tx).await?;
  if actual!=Some((r.class,r.level,r.exp,r.last)){return Err(VocationError::Conflict)}
 }
 for r in &s.skills{
  if !verify&&!state.0{inserted+=sqlx::query("INSERT INTO game_vocation_skills(row_id,cid,skill_utf16,rank) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(r.id).bind(r.cid).bind(r.name.as_bytes()).bind(r.rank).execute(&mut *tx).await?.rows_affected() as usize;}
  let actual:Option<(i64,Vec<u8>,i32)>=sqlx::query_as("SELECT cid,skill_utf16,rank FROM game_vocation_skills WHERE row_id=$1").bind(r.id).fetch_optional(&mut *tx).await?;
  if !actual.is_some_and(|a|a.0==r.cid&&a.1==r.name.as_bytes()&&a.2==r.rank){return Err(VocationError::Conflict)}
 }
 if !state.0&&!verify{
  let actual:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM game_vocations),(SELECT COUNT(*) FROM game_vocation_skills)").fetch_one(&mut *tx).await?;
  if actual!=(s.vocations.len() as i64,s.skills.len() as i64){return Err(VocationError::Conflict)}
  let next=s.last.map_or(1,|v|v+1);
  sqlx::query(&format!("ALTER SEQUENCE game_vocation_skill_row_id RESTART WITH {next}")).execute(&mut *tx).await?;
  sqlx::query("UPDATE game_vocation_preservation SET initialized=true,source_last_value=$1,source_sha256=$2 WHERE singleton").bind(s.last).bind(s.hash.as_slice()).execute(&mut *tx).await?;
 }
 tx.commit().await?;
 Ok(Summary{vocations:s.vocations.len(),skills:s.skills.len(),inserted,verified:s.vocations.len()+s.skills.len(),identity_initialized:state.0||!verify,character_creation_available:false})
 }).await.map_err(|_|VocationError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), VocationError> {
    for q in [
        "SELECT cid,vocation_class,vocation_level,vocation_exp,last_transform_ticks FROM game_vocations LIMIT 0",
        "SELECT row_id,cid,skill_utf16,rank FROM game_vocation_skills LIMIT 0",
        "SELECT singleton,initialized,source_last_value,source_sha256 FROM game_vocation_preservation LIMIT 0",
    ] {
        sqlx::query(q).execute(pool).await?;
    }
    Ok(())
}
/// Caller holds the admitted first-Fiona character row. Preservation is complete and
/// frozen before absence is used as a constructor input;missing schema/state never means None.
pub(crate) async fn load_first(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Loaded, VocationError> {
    let initialized: bool = sqlx::query_scalar(
        "SELECT initialized FROM game_vocation_preservation WHERE singleton FOR SHARE",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !initialized {
        return Err(VocationError::Uninitialized);
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_vocations WHERE cid=$1)")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    if exists {
        return Err(VocationError::Profile);
    }
    Loaded::load(None, vec![]).map_err(|_| VocationError::Profile)
}
