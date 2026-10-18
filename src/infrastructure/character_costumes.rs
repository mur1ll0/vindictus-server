//! Full Costume snapshot preservation and scoped appearance writes, not character initialization.
use super::{
    character_lobby::UserKey,
    user_export::{PrivateText, integer},
};
use crate::domain::{
    character_costume::{Costume, CostumeProfile, CostumeSet},
    game_user::LegacyNameKey,
};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use std::{
    collections::{BTreeMap, HashSet},
    io::Read,
    time::Duration,
};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum CostumeError {
    #[error(transparent)]
    Admission(#[from] super::fiona_character_state::StateError),
    #[error("costume database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("costume snapshot IO failed")]
    Io(#[from] std::io::Error),
    #[error("invalid bounded costume snapshot")]
    Export,
    #[error("costume preservation conflict or identity profile mismatch")]
    Conflict,
    #[error("costume write does not own the synthetic character revision")]
    Ownership,
    #[error(transparent)]
    Profile(#[from] CostumeProfile),
    #[error("costume identity has not been initialized")]
    Uninitialized,
    #[error("costume deadline exceeded; verify the original attempt before retry")]
    Timeout,
}
fn required<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    #[serde(deserialize_with = "required")]
    source_last_value: Option<PrivateText>,
    rows: Vec<ExportRow>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportRow {
    row_id: PrivateText,
    cid: PrivateText,
    is_base: bool,
    costume_type: u8,
    costume_key: u8,
    #[serde(deserialize_with = "required")]
    costume_sn: Option<i32>,
    #[serde(deserialize_with = "required")]
    c1: Option<i32>,
    #[serde(deserialize_with = "required")]
    c2: Option<i32>,
    #[serde(deserialize_with = "required")]
    c3: Option<i32>,
    #[serde(deserialize_with = "required")]
    gloss: Option<u8>,
    #[serde(deserialize_with = "required")]
    effect: Option<i32>,
}
#[derive(PartialEq, Eq)]
struct Record {
    row_id: i64,
    cid: i64,
    costume: Costume,
}
pub struct Snapshot {
    rows: Vec<Record>,
    last: Option<i64>,
}
const MAX_ROWS: usize = 20_000;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, CostumeError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(CostumeError::Export);
    }
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| CostumeError::Export)?;
    if e.format_version != 1 || e.source != "study_heroes.costume" || e.rows.len() > MAX_ROWS {
        return Err(CostumeError::Export);
    }
    let last = e
        .source_last_value
        .map(|v| integer(v).map_err(|_| CostumeError::Export))
        .transpose()?;
    if last.is_some_and(|v| !(0..i64::MAX).contains(&v)) {
        return Err(CostumeError::Export);
    }
    let mut rows = Vec::with_capacity(e.rows.len());
    let mut keys = HashSet::new();
    for r in e.rows {
        let row_id = integer(r.row_id).map_err(|_| CostumeError::Export)?;
        let cid = integer(r.cid).map_err(|_| CostumeError::Export)?;
        if row_id < 1 || !last.is_some_and(|v| row_id <= v) || !keys.insert(row_id) {
            return Err(CostumeError::Export);
        }
        rows.push(Record {
            row_id,
            cid,
            costume: Costume {
                is_base: r.is_base,
                kind: r.costume_type,
                key: r.costume_key,
                serial: r.costume_sn,
                colors: [r.c1, r.c2, r.c3],
                gloss: r.gloss,
                effect: r.effect,
            },
        });
    }
    rows.sort_by_key(|r| r.row_id);
    Ok(Snapshot { rows, last })
}
type Stored = (
    i64,
    i64,
    bool,
    i16,
    i16,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i16>,
    Option<i32>,
);
fn record(r: Stored) -> Result<Record, CostumeError> {
    Ok(Record {
        row_id: r.0,
        cid: r.1,
        costume: Costume {
            is_base: r.2,
            kind: r.3.try_into().map_err(|_| CostumeError::Conflict)?,
            key: r.4.try_into().map_err(|_| CostumeError::Conflict)?,
            serial: r.5,
            colors: [r.6, r.7, r.8],
            gloss: r
                .9
                .map(u8::try_from)
                .transpose()
                .map_err(|_| CostumeError::Conflict)?,
            effect: r.10,
        },
    })
}
async fn transaction(pool: &PgPool) -> Result<Transaction<'_, Postgres>, CostumeError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
#[derive(Serialize)]
pub struct Summary {
    pub source_rows: usize,
    pub inserted: usize,
    pub verified: usize,
    pub identity_initialized: bool,
    pub character_creation_available: bool,
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, CostumeError> {
    tokio::time::timeout(Duration::from_secs(30),async{
  let mut tx=transaction(pool).await?;
  let state:(bool,Option<i64>)=sqlx::query_as("SELECT initialized,source_last_value FROM game_costume_identity WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
  if state.0&&state.1!=s.last{return Err(CostumeError::Conflict)}
  sqlx::query("LOCK TABLE game_costumes IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let mut inserted=0;
  for r in &s.rows {
   let c=&r.costume;
   if !verify&&!state.0 {
    inserted+=sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT DO NOTHING")
     .bind(r.row_id).bind(r.cid).bind(c.is_base).bind(i16::from(c.kind)).bind(i16::from(c.key)).bind(c.serial).bind(c.colors[0]).bind(c.colors[1]).bind(c.colors[2]).bind(c.gloss.map(i16::from)).bind(c.effect).execute(&mut *tx).await?.rows_affected() as usize;
   }
   let stored:Option<Stored>=sqlx::query_as("SELECT row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect FROM game_costumes WHERE row_id=$1").bind(r.row_id).fetch_optional(&mut *tx).await?;
   if stored.map(record).transpose()?.as_ref()!=Some(r){return Err(CostumeError::Conflict)}
  }
  if !state.0&&!verify {
   let total:i64=sqlx::query_scalar("SELECT COUNT(*) FROM game_costumes").fetch_one(&mut *tx).await?;
   if total!=s.rows.len() as i64{return Err(CostumeError::Conflict)}
   let next=s.last.map_or(1,|v|v+1);
   // Validated integer, fixed identifier; transactional RESTART avoids setval rollback hazards.
   sqlx::query(&format!("ALTER SEQUENCE game_costume_row_id RESTART WITH {next}")).execute(&mut *tx).await?;
   sqlx::query("UPDATE game_costume_identity SET initialized=TRUE,source_last_value=$1 WHERE singleton").bind(s.last).execute(&mut *tx).await?;
  }
  tx.commit().await?;Ok(Summary{source_rows:s.rows.len(),inserted,verified:s.rows.len(),identity_initialized:state.0||!verify,character_creation_available:false})
 }).await.map_err(|_|CostumeError::Timeout)?
}
/// Persist appearance changes only. Caller must await this operation before compensating creation.
/// No removal or fabricated item-derived appearance is implemented by this boundary.
pub async fn save(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    changes: &[Costume],
) -> Result<usize, CostumeError> {
    save_inner(pool, key, cid, changes, false).await
}
/// Creation-only boundary; rechecks the first normal Fiona inside the write transaction.
pub(crate) async fn save_first_fiona(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    changes: &[Costume],
) -> Result<usize, CostumeError> {
    save_inner(pool, key, cid, changes, true).await
}
async fn save_inner(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    changes: &[Costume],
    first_fiona: bool,
) -> Result<usize, CostumeError> {
    if changes.len() > CostumeSet::MAX_ROWS {
        return Err(CostumeProfile.into());
    }
    let owner = LegacyNameKey::parse(key.name).map_err(|_| CostumeError::Ownership)?;
    tokio::time::timeout(Duration::from_secs(10),async{
  let mut tx=if first_fiona { super::fiona_character_state::owned_first_fiona(pool,key,cid).await? } else { transaction(pool).await? };
  let initialized:bool=sqlx::query_scalar("SELECT initialized FROM game_costume_identity WHERE singleton FOR SHARE").fetch_one(&mut *tx).await?;
  if !initialized{return Err(CostumeError::Uninitialized)}
  let actual:Option<(String,i64)>=sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE").bind(key.uid).fetch_optional(&mut *tx).await?;
  let origin:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)").bind(key.uid).bind(owner.as_str()).fetch_one(&mut *tx).await?;
  if actual!=Some((owner.as_str().to_owned(),key.revision))||!origin{return Err(CostumeError::Ownership)}
  // Cooperate with the existing creation/compensation table lock before taking a row lock.
  sqlx::query("LOCK TABLE game_character_info IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let owned:Option<i64>=sqlx::query_scalar("SELECT id FROM game_character_info WHERE id=$1 AND uid=$2 FOR UPDATE").bind(cid).bind(key.uid).fetch_optional(&mut *tx).await?;
  if owned.is_none(){return Err(CostumeError::Ownership)}
  let stored:Vec<Stored>=sqlx::query_as("SELECT row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect FROM game_costumes WHERE cid=$1 LIMIT 1025 FOR UPDATE").bind(cid).fetch_all(&mut *tx).await?;
  let records=stored.into_iter().map(record).collect::<Result<Vec<_>,_>>()?;
  let mut set=CostumeSet::load(records.iter().map(|r|r.costume))?;
  let old:BTreeMap<_,_>=records.iter().map(|r|(r.costume.identity(),r)).collect();
  for c in changes{set.set(*c)?}
  let mut changed=0;
  for c in set.rows(){
   if let Some(r)=old.get(&c.identity()){
    if r.costume!=*c{sqlx::query("UPDATE game_costumes SET costume_sn=$2,c1=$3,c2=$4,c3=$5,gloss=$6 WHERE row_id=$1")
     .bind(r.row_id).bind(c.serial).bind(c.colors[0]).bind(c.colors[1]).bind(c.colors[2]).bind(c.gloss.map(i16::from)).execute(&mut *tx).await?;changed+=1;}
   }else{
    sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect) VALUES(nextval('game_costume_row_id'),$1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
     .bind(cid).bind(c.is_base).bind(i16::from(c.kind)).bind(i16::from(c.key)).bind(c.serial).bind(c.colors[0]).bind(c.colors[1]).bind(c.colors[2]).bind(c.gloss.map(i16::from)).bind(c.effect).execute(&mut *tx).await?;changed+=1;
   }
  }
  tx.commit().await?;Ok(changed)
 }).await.map_err(|_|CostumeError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), CostumeError> {
    sqlx::query("SELECT row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect FROM game_costumes LIMIT 0").execute(pool).await?;
    sqlx::query(
        "SELECT singleton,initialized,source_last_value FROM game_costume_identity LIMIT 0",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Read constructor state only after the caller locks/admit the first-Fiona row.
/// The identity SHARE lock excludes preservation; all appearance writers cooperate
/// with the character row lock held by the caller. No identity allocation or Save.
pub(crate) async fn load_for_constructor(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<CostumeSet, CostumeError> {
    let initialized: bool = sqlx::query_scalar(
        "SELECT initialized FROM game_costume_identity WHERE singleton FOR SHARE",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !initialized {
        return Err(CostumeError::Uninitialized);
    }
    let rows:Vec<Stored>=sqlx::query_as("SELECT row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect FROM game_costumes WHERE cid=$1 ORDER BY row_id LIMIT 1025 FOR SHARE").bind(cid).fetch_all(&mut **tx).await?;
    let records = rows
        .into_iter()
        .map(record)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CostumeSet::load(records.into_iter().map(|r| r.costume))?)
}
