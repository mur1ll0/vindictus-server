//! Full readonly AttendanceNewbieInfo reference and exact existing-UID absence.
//! Active registration/rewards and a present-row manager remain unsupported.
use crate::{
    domain::legacy_user_data::SqlDateTime2,
    infrastructure::user_export::{PrivateText, integer},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::{io::Read, time::Duration};
use zeroize::Zeroizing;
#[derive(Debug, thiserror::Error)]
pub enum AttendanceError {
    #[error("invalid bounded newbie attendance reference")]
    Input,
    #[error("newbie attendance reference database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("existing newbie attendance provider lookup failed")]
    Lookup(#[source] sqlx::Error),
    #[error("newbie attendance reference uninitialized or changed;no repair")]
    Reference,
    #[error("existing newbie row requires recovered manager lifecycle")]
    Present,
    #[error("newbie attendance reference deadline exceeded")]
    Timeout,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    rows: Vec<Raw>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    uid: i32,
    event_version: i32,
    start_ticks: PrivateText,
    end_ticks: PrivateText,
}
#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Row {
    uid: i32,
    event_version: i32,
    start_ticks: i64,
    end_ticks: i64,
}
pub struct Snapshot {
    rows: Vec<Row>,
    hash: [u8; 32],
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, AttendanceError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AttendanceError::Input)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(AttendanceError::Input);
    }
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| AttendanceError::Input)?;
    if e.format_version != 1
        || e.source != "study_heroes.newbie-attendance"
        || e.rows.len() > 100000
    {
        return Err(AttendanceError::Input);
    }
    let tick = |s| -> Result<i64, AttendanceError> {
        let n = integer(s).map_err(|_| AttendanceError::Input)?;
        SqlDateTime2::from_ticks(n).map_err(|_| AttendanceError::Input)?;
        Ok(n)
    };
    let mut rows = e
        .rows
        .into_iter()
        .map(|r| {
            Ok(Row {
                uid: r.uid,
                event_version: r.event_version,
                start_ticks: tick(r.start_ticks)?,
                end_ticks: tick(r.end_ticks)?,
            })
        })
        .collect::<Result<Vec<_>, AttendanceError>>()?;
    rows.sort_by_key(|r| r.uid);
    if rows.windows(2).any(|r| r[0].uid == r[1].uid) {
        return Err(AttendanceError::Input);
    }
    Ok(Snapshot {
        rows,
        hash: Sha256::digest(&*bytes).into(),
    })
}
#[derive(Serialize)]
pub struct Summary {
    pub rows: usize,
    pub fields: usize,
    pub inserted: usize,
    pub verified: usize,
    pub initialized: bool,
    pub character_creation_available: bool,
}
async fn bounds(tx: &mut Transaction<'_, Postgres>) -> Result<(), AttendanceError> {
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn preserve(
    pool: &PgPool,
    s: &Snapshot,
    verify: bool,
) -> Result<Summary, AttendanceError> {
    tokio::time::timeout(Duration::from_secs(20),async{
  let mut tx=pool.begin().await?;bounds(&mut tx).await?;
  sqlx::query("LOCK TABLE game_newbie_attendance_info,game_newbie_attendance_preservation IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let ids:Vec<i32>=s.rows.iter().map(|r|r.uid).collect();
  let marker:Option<(Vec<u8>,Vec<i32>)>=sqlx::query_as("SELECT source_sha256,source_uids FROM game_newbie_attendance_preservation WHERE singleton").fetch_optional(&mut *tx).await?;
  let mut inserted=0;
  if let Some((hash,stored_ids))=marker{
   if hash!=s.hash||stored_ids!=ids{return Err(AttendanceError::Reference)}
  }else{
   let occupied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_newbie_attendance_info)").fetch_one(&mut *tx).await?;
   if verify||occupied{return Err(AttendanceError::Reference)}
   for r in &s.rows{inserted+=sqlx::query("INSERT INTO game_newbie_attendance_info VALUES($1,$2,$3,$4)").bind(r.uid).bind(r.event_version).bind(r.start_ticks).bind(r.end_ticks).execute(&mut *tx).await?.rows_affected()as usize;}
   sqlx::query("INSERT INTO game_newbie_attendance_preservation VALUES(true,$1,$2)").bind(s.hash.as_slice()).bind(&ids).execute(&mut *tx).await?;
  }
  let actual:Vec<Row>=sqlx::query_as("SELECT uid,event_version,start_ticks,end_ticks FROM game_newbie_attendance_info ORDER BY uid LIMIT 100001").fetch_all(&mut *tx).await?;
  let total:i64=sqlx::query_scalar("SELECT count(*) FROM game_newbie_attendance_info").fetch_one(&mut *tx).await?;
  if actual!=s.rows||total!=actual.len()as i64{return Err(AttendanceError::Reference)}
  tx.commit().await?;Ok(Summary{rows:s.rows.len(),fields:4,inserted,verified:s.rows.len(),initialized:true,character_creation_available:false})
 }).await.map_err(|_|AttendanceError::Timeout)?
}
/// Caller holds current owner revision/UserInfo through consumption. The initialized
/// reference UID set forbids losing an expected row and interpreting it as absence.
/// SHARE prevents insertion after the negative lookup;stronger than source NOLOCK.
pub async fn require_absent(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
) -> Result<(), AttendanceError> {
    bounds(tx).await?;
    sqlx::query(
        "LOCK TABLE game_newbie_attendance_info,game_newbie_attendance_preservation IN SHARE MODE",
    )
    .execute(&mut **tx)
    .await?;
    let ids: Option<Vec<i32>> = sqlx::query_scalar(
        "SELECT source_uids FROM game_newbie_attendance_preservation WHERE singleton",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let ids = ids.ok_or(AttendanceError::Reference)?;
    if !ids.windows(2).all(|r| r[0] < r[1]) {
        return Err(AttendanceError::Reference);
    }
    let row:Option<Row>=sqlx::query_as("SELECT uid,event_version,start_ticks,end_ticks FROM game_newbie_attendance_info WHERE uid=$1").bind(uid).fetch_optional(&mut **tx).await.map_err(AttendanceError::Lookup)?;
    match row {
        Some(_) => Err(AttendanceError::Present),
        None if ids.binary_search(&uid).is_ok() => Err(AttendanceError::Reference),
        None => Ok(()),
    }
}
pub async fn check_ready(pool: &PgPool) -> Result<(), AttendanceError> {
    sqlx::query(
        "SELECT uid,event_version,start_ticks,end_ticks FROM game_newbie_attendance_info LIMIT 0",
    )
    .execute(pool)
    .await?;
    sqlx::query("SELECT singleton,source_sha256,source_uids FROM game_newbie_attendance_preservation LIMIT 0").execute(pool).await?;
    Ok(())
}
