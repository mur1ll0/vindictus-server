//! Complete private StatusEffect preservation; no constructor,expiry cleanup or Save.
use crate::{
    domain::legacy_user_data::{LegacyBytes, SqlDateTime2},
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
pub enum StatusError {
    #[error("invalid bounded status-effect snapshot")]
    Export,
    #[error("status-effect input could not be read")]
    Input(#[from] std::io::Error),
    #[error("status-effect database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("status-effect reference preservation is not initialized")]
    Uninitialized,
    #[error("status-effect preservation conflict;no repair performed")]
    Conflict,
    #[error("status-effect absence requires an existing character row")]
    MissingCharacter,
    #[error("fresh status-effect constructor requires proven absence")]
    Existing,
    #[error("status-effect deadline exceeded;reconcile committed state before retry")]
    Timeout,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    #[serde(deserialize_with = "nullable")]
    source_last_value: Option<PrivateText>,
    effects: Vec<Raw>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    row_id: PrivateText,
    cid: PrivateText,
    type_utf16: PrivateText,
    level: i32,
    #[serde(deserialize_with = "nullable")]
    remain_time: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    expire_ticks: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    remain_micro: Option<u8>,
}
struct Row {
    id: i64,
    cid: i64,
    kind: LegacyBytes<50, true>,
    level: i32,
    time: Option<i32>,
    expire: Option<i64>,
    micro: Option<u8>,
}
#[derive(sqlx::FromRow)]
struct Stored {
    cid: i64,
    type_utf16: Vec<u8>,
    level: i32,
    remain_time: Option<i32>,
    expire_ticks: Option<i64>,
    remain_micro: Option<i16>,
}
impl Stored {
    fn matches(&self, row: &Row) -> bool {
        self.cid == row.cid
            && self.type_utf16 == row.kind.as_bytes()
            && self.level == row.level
            && self.remain_time == row.time
            && self.expire_ticks == row.expire
            && self.remain_micro == row.micro.map(i16::from)
    }
}
pub struct Snapshot {
    last: Option<i64>,
    rows: Vec<Row>,
    hash: [u8; 32],
}
fn number(v: PrivateText) -> Result<i64, StatusError> {
    integer(v).map_err(|_| StatusError::Export)
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, StatusError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(StatusError::Export);
    }
    let hash = Sha256::digest(&*bytes).into();
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| StatusError::Export)?;
    if e.format_version != 1
        || e.source != "study_heroes.status-effects"
        || e.effects.len() > 100000
    {
        return Err(StatusError::Export);
    }
    let last = e.source_last_value.map(number).transpose()?;
    if last.is_some_and(|v| !(0..i64::MAX).contains(&v)) {
        return Err(StatusError::Export);
    }
    let mut ids = HashSet::new();
    let mut rows = Vec::with_capacity(e.effects.len());
    for r in e.effects {
        let id = number(r.row_id)?;
        let cid = number(r.cid)?;
        if id < 1 || !last.is_some_and(|v| id <= v) || !ids.insert(id) {
            return Err(StatusError::Export);
        }
        let expire = r.expire_ticks.map(number).transpose()?;
        if expire.is_some_and(|v| SqlDateTime2::from_ticks(v).is_err()) {
            return Err(StatusError::Export);
        }
        let kind = item_catalog::opaque(Some(r.type_utf16))
            .map_err(|_| StatusError::Export)?
            .ok_or(StatusError::Export)?;
        let kind =
            LegacyBytes::from_bytes(kind.as_bytes().to_vec()).map_err(|_| StatusError::Export)?;
        rows.push(Row {
            id,
            cid,
            kind,
            level: r.level,
            time: r.remain_time,
            expire,
            micro: r.remain_micro,
        });
    }
    Ok(Snapshot { last, rows, hash })
}
#[derive(Serialize)]
pub struct Summary {
    pub effects: usize,
    pub inserted: usize,
    pub verified: usize,
    pub identity_initialized: bool,
    pub character_creation_available: bool,
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, StatusError> {
    tokio::time::timeout(Duration::from_secs(30),async{
  let mut tx=pool.begin().await?;
  sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
  sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
  let state:(bool,Option<i64>,Option<Vec<u8>>)=sqlx::query_as("SELECT initialized,source_last_value,source_sha256 FROM game_status_effect_preservation WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
  if state.0&&(state.1!=s.last||state.2.as_deref()!=Some(s.hash.as_slice())){return Err(StatusError::Conflict)}
  sqlx::query("LOCK TABLE game_status_effects IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let mut inserted=0;
  for r in &s.rows {
   if !state.0&&!verify {inserted+=sqlx::query("INSERT INTO game_status_effects(row_id,cid,type_utf16,level,remain_time,expire_ticks,remain_micro)VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING").bind(r.id).bind(r.cid).bind(r.kind.as_bytes()).bind(r.level).bind(r.time).bind(r.expire).bind(r.micro.map(i16::from)).execute(&mut *tx).await?.rows_affected()as usize;}
   let actual:Option<Stored>=sqlx::query_as("SELECT cid,type_utf16,level,remain_time,expire_ticks,remain_micro FROM game_status_effects WHERE row_id=$1").bind(r.id).fetch_optional(&mut *tx).await?;
   if !actual.is_some_and(|a|a.matches(r)){return Err(StatusError::Conflict)}
  }
  if !state.0&&!verify {
   let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM game_status_effects").fetch_one(&mut *tx).await?;
   if count!=s.rows.len() as i64{return Err(StatusError::Conflict)}
   let next=s.last.map_or(1,|v|v+1);
   sqlx::query(&format!("ALTER SEQUENCE game_status_effect_row_id RESTART WITH {next}")).execute(&mut *tx).await?;
   sqlx::query("UPDATE game_status_effect_preservation SET initialized=true,source_last_value=$1,source_sha256=$2 WHERE singleton").bind(s.last).bind(s.hash.as_slice()).execute(&mut *tx).await?;
  }
  tx.commit().await?;
  Ok(Summary{effects:s.rows.len(),inserted,verified:s.rows.len(),identity_initialized:state.0||!verify,character_creation_available:false})
 }).await.map_err(|_|StatusError::Timeout)?
}
/// Reusable read guard, not a constructor or an actor capability. The guard locks the existing
/// character row; caller must prove ownership and keep this transaction until consuming the observation.
/// Existing effects are preserved, never silently expired or removed.
pub async fn require_absent(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<(), StatusError> {
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    let initialized: bool = sqlx::query_scalar(
        "SELECT initialized FROM game_status_effect_preservation WHERE singleton FOR SHARE",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !initialized {
        return Err(StatusError::Uninitialized);
    }
    let character: Option<i64> =
        sqlx::query_scalar("SELECT id FROM game_characters WHERE id=$1 FOR UPDATE")
            .bind(cid)
            .fetch_optional(&mut **tx)
            .await?;
    if character.is_none() {
        return Err(StatusError::MissingCharacter);
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_status_effects WHERE cid=$1)")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    if exists {
        return Err(StatusError::Existing);
    }
    Ok(())
}
pub async fn check_ready(pool: &PgPool) -> Result<(), StatusError> {
    for q in [
        "SELECT row_id,cid,type_utf16,level,remain_time,expire_ticks,remain_micro FROM game_status_effects LIMIT 0",
        "SELECT singleton,initialized,source_last_value,source_sha256 FROM game_status_effect_preservation LIMIT 0",
        "SELECT last_value,is_called FROM game_status_effect_row_id LIMIT 0",
    ] {
        sqlx::query(q).execute(pool).await?;
    }
    Ok(())
}
