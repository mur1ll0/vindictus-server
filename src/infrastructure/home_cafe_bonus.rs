//! Bounded HomeCafeBonus preservation and account-level lookup.
use crate::{
    domain::{
        game_user::LegacyNameKey,
        home_cafe_bonus::{HomeCafeBonus, HomeCafeValueError, MAX_ROWS, validate},
        legacy_user_data::{LegacyUserDataError, SqlDateTime},
    },
    infrastructure::user_export::{PrivateText, integer},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use std::{io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum HomeCafeError {
    #[error("home-cafe PostgreSQL operation failed")]
    Database(#[from] sqlx::Error),
    #[error("home-cafe export IO failed")]
    Io(#[from] std::io::Error),
    #[error("invalid bounded home-cafe export")]
    Export,
    #[error("home-cafe preservation conflict")]
    Conflict,
    #[error("home-cafe operation deadline exceeded")]
    Timeout,
    #[error("home-cafe owner or revision changed")]
    Ownership,
    #[error(transparent)]
    Value(#[from] HomeCafeValueError),
    #[error(transparent)]
    Date(#[from] LegacyUserDataError),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    home_cafe_bonus: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    uid: i32,
    cid: PrivateText,
    bonus_type: u8,
    expired_time_units300: PrivateText,
}
pub fn read_snapshot(input: impl Read) -> Result<Vec<HomeCafeBonus>, HomeCafeError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(HomeCafeError::Export);
    }
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| HomeCafeError::Export)?;
    if e.format_version != 1
        || e.source != "study_heroes.home-cafe-bonus"
        || e.home_cafe_bonus.len() > MAX_ROWS
    {
        return Err(HomeCafeError::Export);
    }
    let rows = e
        .home_cafe_bonus
        .into_iter()
        .map(|r| {
            Ok(HomeCafeBonus {
                uid: r.uid,
                cid: integer(r.cid).map_err(|_| HomeCafeError::Export)?,
                bonus_type: r.bonus_type,
                expired_time: SqlDateTime::from_units300(
                    integer(r.expired_time_units300).map_err(|_| HomeCafeError::Export)?,
                )?,
            })
        })
        .collect::<Result<Vec<_>, HomeCafeError>>()?;
    validate(&rows)?;
    Ok(rows)
}
#[derive(Serialize)]
pub struct ImportSummary {
    pub source_rows: usize,
    pub inserted: usize,
    pub existing: usize,
    pub verified: usize,
    pub game_authentication_available: bool,
}
async fn bounds(c: &mut PgConnection) -> Result<(), HomeCafeError> {
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *c)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(c)
        .await?;
    Ok(())
}
pub async fn preserve(
    pool: &PgPool,
    rows: &[HomeCafeBonus],
    verify: bool,
) -> Result<ImportSummary, HomeCafeError> {
    validate(rows)?;
    tokio::time::timeout(Duration::from_secs(10),async{
  let mut tx=pool.begin().await?;bounds(&mut tx).await?;let mut inserted=0usize;
  for row in rows {
   if !verify {inserted+=sqlx::query("INSERT INTO game_home_cafe_bonus(uid,cid,bonus_type,expired_time_units300) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(row.uid).bind(row.cid).bind(i16::from(row.bonus_type)).bind(row.expired_time.units300()).execute(&mut *tx).await?.rows_affected() as usize;}
   let value:Option<i64>=sqlx::query_scalar("SELECT expired_time_units300 FROM game_home_cafe_bonus WHERE uid=$1 AND cid=$2 AND bonus_type=$3 FOR SHARE").bind(row.uid).bind(row.cid).bind(i16::from(row.bonus_type)).fetch_optional(&mut *tx).await?;
   if value!=Some(row.expired_time.units300()){return Err(HomeCafeError::Conflict)}
  }
  tx.commit().await?;Ok(ImportSummary{source_rows:rows.len(),inserted,existing:rows.len()-inserted,verified:rows.len(),game_authentication_available:false})
 }).await.map_err(|_|HomeCafeError::Timeout)?
}
pub struct UserKey<'a> {
    pub uid: i32,
    pub name: &'a str,
    pub revision: i64,
}
/// Only the proven initial call with CID=NULL; other source procedure branches remain dependencies.
pub async fn query_account(
    pool: &PgPool,
    key: UserKey<'_>,
    initial: Option<u8>,
) -> Result<Option<u8>, HomeCafeError> {
    let name = LegacyNameKey::parse(key.name).map_err(|_| HomeCafeError::Ownership)?;
    tokio::time::timeout(Duration::from_secs(10),async{
  let mut tx=pool.begin().await?;bounds(&mut tx).await?;
  let owner:Option<(String,i64)>=sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE").bind(key.uid).fetch_optional(&mut *tx).await?;
  if !owner.is_some_and(|(n,r)|n==name.as_str()&&r==key.revision){return Err(HomeCafeError::Ownership)}
  let value:Option<i16>=sqlx::query_scalar("SELECT (CASE WHEN EXISTS(SELECT 1 FROM game_home_cafe_bonus WHERE uid=$1 AND cid=0 AND bonus_type=3) THEN 3 WHEN EXISTS(SELECT 1 FROM game_home_cafe_bonus WHERE uid=$1 AND cid=0 AND bonus_type=4) THEN 4 WHEN EXISTS(SELECT 1 FROM game_home_cafe_bonus WHERE uid=$1 AND cid=0 AND bonus_type=2) THEN 2 ELSE $2::smallint END)::smallint").bind(key.uid).bind(initial.map(i16::from)).fetch_one(&mut *tx).await?;
  tx.commit().await?;value.map(|v|u8::try_from(v).map_err(|_|HomeCafeError::Export)).transpose()
 }).await.map_err(|_|HomeCafeError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), HomeCafeError> {
    sqlx::query(
        "SELECT uid,cid,bonus_type,expired_time_units300 FROM game_home_cafe_bonus LIMIT 0",
    )
    .execute(pool)
    .await?;
    Ok(())
}
