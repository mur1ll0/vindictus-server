//! Lossless shared-reference preservation and a scoped empty-owner loader.
use super::{
    character_lobby::UserKey,
    item_catalog,
    user_export::{PrivateText, integer, nullable},
};
use crate::domain::{game_user::LegacyNameKey, legacy_user_data::SqlDateTime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{collections::HashSet, io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum SharedError {
    #[error("shared inventory snapshot is invalid or outside bounds")]
    Export,
    #[error("shared inventory preservation conflicts with stored state")]
    Conflict,
    #[error("shared inventory is uninitialized, nonempty or not owned by the admitted revision")]
    Unavailable,
    #[error("shared inventory operation failed")]
    Database(#[from] sqlx::Error),
    #[error("shared inventory operation deadline exceeded")]
    Timeout,
}
fn id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    integer(PrivateText::deserialize(d)?)
        .map_err(|_| serde::de::Error::custom("invalid private identifier"))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    format_version: u32,
    source: String,
    #[serde(deserialize_with = "nullable")]
    last_value: Option<PrivateText>,
    items: Vec<Item>,
    storage: Vec<RawStorage>,
}
#[derive(Deserialize, sqlx::FromRow, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Item {
    #[serde(deserialize_with = "id")]
    id: i64,
    #[serde(deserialize_with = "nullable")]
    owner_uid: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    collection: Option<i16>,
    #[serde(deserialize_with = "nullable")]
    slot: Option<i32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStorage {
    #[serde(deserialize_with = "id")]
    id: i64,
    owner_uid: i32,
    storage_type: i16,
    storage: i16,
    #[serde(deserialize_with = "nullable")]
    expire_binary: Option<PrivateText>,
}
#[derive(sqlx::FromRow, PartialEq, Eq)]
struct Storage {
    id: i64,
    owner_uid: i32,
    storage_type: i16,
    storage: i16,
    expire_units300: Option<i64>,
}
pub struct Snapshot {
    fingerprint: Vec<u8>,
    last: Option<i64>,
    items: Vec<Item>,
    storage: Vec<Storage>,
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, SharedError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SharedError::Export)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(SharedError::Export);
    }
    let r: Raw = serde_json::from_slice(&bytes).map_err(|_| SharedError::Export)?;
    if r.format_version != 1
        || r.source != "study_heroes.shared-items"
        || r.items.len() > 100000
        || r.storage.len() > 100000
    {
        return Err(SharedError::Export);
    }
    let last = r
        .last_value
        .map(integer)
        .transpose()
        .map_err(|_| SharedError::Export)?;
    if last.is_some_and(|v| !(0..i64::MAX).contains(&v)) {
        return Err(SharedError::Export);
    }
    let mut seen = HashSet::new();
    for i in &r.items {
        if !seen.insert(i.id) || i.collection.is_some_and(|v| !(0..=255).contains(&v)) {
            return Err(SharedError::Export);
        }
    }
    let mut storage = Vec::new();
    let mut seen = HashSet::new();
    for s in r.storage {
        if !seen.insert(s.id)
            || !last.is_some_and(|v| s.id > 0 && s.id <= v)
            || !(0..=255).contains(&s.storage_type)
            || !(0..=255).contains(&s.storage)
        {
            return Err(SharedError::Export);
        }
        let expire = s
            .expire_binary
            .map(|v| {
                let b = item_catalog::opaque(Some(v))
                    .map_err(|_| SharedError::Export)?
                    .ok_or(SharedError::Export)?;
                let bin: [u8; 8] = b.as_bytes().try_into().map_err(|_| SharedError::Export)?;
                Ok::<_, SharedError>(
                    SqlDateTime::from_sql_binary(bin)
                        .map_err(|_| SharedError::Export)?
                        .units300(),
                )
            })
            .transpose()?;
        storage.push(Storage {
            id: s.id,
            owner_uid: s.owner_uid,
            storage_type: s.storage_type,
            storage: s.storage,
            expire_units300: expire,
        });
    }
    let mut items = r.items;
    items.sort_by_key(|r| r.id);
    storage.sort_by_key(|r| r.id);
    Ok(Snapshot {
        fingerprint: Sha256::digest(&bytes).to_vec(),
        last,
        items,
        storage,
    })
}
#[derive(Serialize)]
pub struct Summary {
    pub items: usize,
    pub storage: usize,
    pub identity_initialized: bool,
    pub character_creation_available: bool,
    pub empty_owner_verified: bool,
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, SharedError> {
    tokio::time::timeout(Duration::from_secs(20),async{
 let mut tx=pool.begin().await?;sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
 sqlx::query("LOCK TABLE game_shared_item_preservation,game_shared_items,game_shared_storage_status IN ACCESS EXCLUSIVE MODE").execute(&mut *tx).await?;
 let marker:Option<(Vec<u8>,Option<i64>)>=sqlx::query_as("SELECT source_sha256,source_last FROM game_shared_item_preservation WHERE singleton").fetch_optional(&mut *tx).await?;
 let old:Vec<Item>=sqlx::query_as("SELECT id,owner_uid,collection,slot FROM game_shared_items ORDER BY id LIMIT 100001").fetch_all(&mut *tx).await?;
 let old_storage:Vec<Storage>=sqlx::query_as("SELECT id,owner_uid,storage_type,storage,expire_units300 FROM game_shared_storage_status ORDER BY id LIMIT 100001").fetch_all(&mut *tx).await?;
 let (value,called):(i64,bool)=sqlx::query_as("SELECT last_value,is_called FROM game_shared_storage_id").fetch_one(&mut *tx).await?;
 if let Some((hash,last))=marker{if hash!=s.fingerprint||last!=s.last||old!=s.items||old_storage!=s.storage||last.is_some_and(|v|value<v||(!called&&value==v)){return Err(SharedError::Conflict)}}else{
 if verify||!old.is_empty()||!old_storage.is_empty()||called{return Err(SharedError::Conflict)}
 for i in &s.items{sqlx::query("INSERT INTO game_shared_items(id,owner_uid,collection,slot)VALUES($1,$2,$3,$4)").bind(i.id).bind(i.owner_uid).bind(i.collection).bind(i.slot).execute(&mut *tx).await?;}
 for r in &s.storage{sqlx::query("INSERT INTO game_shared_storage_status(id,owner_uid,storage_type,storage,expire_units300)VALUES($1,$2,$3,$4,$5)").bind(r.id).bind(r.owner_uid).bind(r.storage_type).bind(r.storage).bind(r.expire_units300).execute(&mut *tx).await?;}
 let next=s.last.map_or(1,|v|v+1);sqlx::query(&format!("ALTER SEQUENCE game_shared_storage_id RESTART WITH {next}")).execute(&mut *tx).await?;
 sqlx::query("INSERT INTO game_shared_item_preservation(source_sha256,source_last)VALUES($1,$2)").bind(&s.fingerprint).bind(s.last).execute(&mut *tx).await?;
 }
 tx.commit().await?;Ok(Summary{items:s.items.len(),storage:s.storage.len(),identity_initialized:true,character_creation_available:false,empty_owner_verified:false})
 }).await.map_err(|_|SharedError::Timeout)?
}
/// Empty shared load only. Existing tabs/items require the complete source loader and are refused.
pub async fn require_empty(pool: &PgPool, key: &UserKey<'_>) -> Result<(), SharedError> {
    tokio::time::timeout(Duration::from_secs(10),async{
 let owner=LegacyNameKey::parse(key.name).map_err(|_|SharedError::Unavailable)?;let mut tx=pool.begin().await?;sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
 let ready:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_shared_item_preservation WHERE singleton)").fetch_one(&mut *tx).await?;
 let actual:Option<(String,i64)>=sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE").bind(key.uid).fetch_optional(&mut *tx).await?;
 let admitted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)").bind(key.uid).bind(owner.as_str()).fetch_one(&mut *tx).await?;
 let nonempty:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_shared_items WHERE owner_uid=$1) OR EXISTS(SELECT 1 FROM game_shared_storage_status WHERE owner_uid=$1)").bind(key.uid).fetch_one(&mut *tx).await?;
 if !ready||!admitted||nonempty||actual!=Some((owner.as_str().to_owned(),key.revision)){return Err(SharedError::Unavailable)}tx.commit().await?;Ok(())
 }).await.map_err(|_|SharedError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), sqlx::Error> {
    for q in [
        "SELECT id,owner_uid,collection,slot FROM game_shared_items LIMIT 0",
        "SELECT id,owner_uid,storage_type,storage,expire_units300 FROM game_shared_storage_status LIMIT 0",
        "SELECT source_sha256,source_last FROM game_shared_item_preservation LIMIT 0",
        "SELECT last_value,is_called FROM game_shared_storage_id",
    ] {
        sqlx::query(q).execute(pool).await?;
    }
    Ok(())
}
