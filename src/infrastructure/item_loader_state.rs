//! Complete reference provider rows and scoped initial-loader absence guards.
//! Not ItemOwner construction,Save/Undo or a successful Create.
use super::{
    character_lobby::UserKey,
    fiona_character_state as state,
    user_export::{PrivateText, hex, integer},
};
use crate::domain::{initial_gear::Projection, legacy_user_data::SqlDateTime2};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::{collections::HashSet, io::Read, time::Duration};
use zeroize::Zeroizing;
#[derive(Debug, thiserror::Error)]
pub enum LoaderError {
    #[error("invalid bounded item-loader reference")]
    Input,
    #[error("item-loader reference conflicts;no repair performed")]
    Conflict,
    #[error("item-loader reference is not initialized")]
    Uninitialized,
    #[error("nonempty initial item-loader branch is unsupported")]
    Nonempty,
    #[error("item-loader database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("item-loader operation timed out;reconcile original attempt")]
    Timeout,
    #[error(transparent)]
    Owner(#[from] state::StateError),
    #[error("initial Gear content binding failed")]
    Content,
}
fn num<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    integer(PrivateText::deserialize(d)?)
        .map_err(|_| serde::de::Error::custom("invalid bounded integer"))
}
fn ticks<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let v = num(d)?;
    SqlDateTime2::from_ticks(v)
        .map(|v| v.ticks())
        .map_err(|_| serde::de::Error::custom("invalid datetime2 ticks"))
}
fn text<'de, D: Deserializer<'de>>(d: D, n: usize) -> Result<Vec<u8>, D::Error> {
    let v = hex(PrivateText::deserialize(d)?, n * 2)
        .map_err(|_| serde::de::Error::custom("invalid UTF16 bytes"))?;
    if v.len() % 2 != 0 {
        return Err(serde::de::Error::custom("invalid UTF16 length"));
    }
    Ok(v)
}
fn hex16<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    text(d, 16)
}
fn hex50<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    text(d, 50)
}
fn hex100<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    text(d, 100)
}
fn hex128<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    text(d, 128)
}
#[derive(Deserialize, PartialEq, Eq, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
struct Row0 {
    #[serde(deserialize_with = "num")]
    row_id: i64,
    #[serde(deserialize_with = "num")]
    owner_id: i64,
    #[serde(deserialize_with = "hex50")]
    item_class_utf16: Vec<u8>,
    slot: i32,
}
#[derive(Deserialize, PartialEq, Eq, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
struct Row1 {
    row_id: i32,
    #[serde(deserialize_with = "num")]
    cid: i64,
    slot: i32,
}
#[derive(Deserialize, PartialEq, Eq, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
struct Row2 {
    row_id: i32,
    #[serde(deserialize_with = "num")]
    cid: i64,
    #[serde(deserialize_with = "hex16")]
    manufacture_utf16: Vec<u8>,
    grade: i32,
    experience_point: i32,
}
#[derive(Deserialize, PartialEq, Eq, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
struct Row3 {
    row_id: i32,
    #[serde(deserialize_with = "num")]
    cid: i64,
    #[serde(deserialize_with = "hex128")]
    recipe_utf16: Vec<u8>,
    #[serde(deserialize_with = "ticks")]
    acquired_ticks: i64,
}
#[derive(Deserialize, PartialEq, Eq, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
struct Row4 {
    #[serde(deserialize_with = "num")]
    row_id: i64,
    #[serde(deserialize_with = "num")]
    cid: i64,
    #[serde(deserialize_with = "hex100")]
    recipe_utf16: Vec<u8>,
    #[serde(deserialize_with = "ticks")]
    new_until_ticks: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    format_version: u32,
    source: String,
    schema_sha256: String,
    source_last: [Option<PrivateText>; 5],
    quick_slots: Vec<Row0>,
    expanded_slots: Vec<Row1>,
    manufacture: Vec<Row2>,
    recipes: Vec<Row3>,
    new_recipes: Vec<Row4>,
}
pub struct Snapshot {
    hash: Vec<u8>,
    last: Vec<Option<i64>>,
    quick_slots: Vec<Row0>,
    expanded_slots: Vec<Row1>,
    manufacture: Vec<Row2>,
    recipes: Vec<Row3>,
    new_recipes: Vec<Row4>,
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, LoaderError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| LoaderError::Input)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(LoaderError::Input);
    }
    let mut r: Raw = serde_json::from_slice(&bytes).map_err(|_| LoaderError::Input)?;
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../domain/item_loader_sources.json"))
            .map_err(|_| LoaderError::Input)?;
    if r.format_version != 1
        || r.source != "study_heroes.item-loader-state"
        || r.schema_sha256 != source["schema_sha256"]
    {
        return Err(LoaderError::Input);
    }
    let last = r
        .source_last
        .into_iter()
        .map(|v| v.map(integer).transpose().map_err(|_| LoaderError::Input))
        .collect::<Result<Vec<_>, _>>()?;
    let mut total = 0;
    r.quick_slots.sort_by_key(|r| r.row_id);
    let mut seen = HashSet::new();
    total += r.quick_slots.len();
    if !r.quick_slots.iter().all(|r| seen.insert(r.row_id))
        || last[0].is_some_and(|v| !(1..i64::MAX).contains(&v))
        || r.quick_slots
            .iter()
            .any(|r| r.row_id > last[0].unwrap_or(0))
    {
        return Err(LoaderError::Input);
    }
    r.expanded_slots.sort_by_key(|r| r.row_id);
    let mut seen = HashSet::new();
    total += r.expanded_slots.len();
    if !r.expanded_slots.iter().all(|r| seen.insert(r.row_id))
        || last[1].is_some_and(|v| !(1..i64::from(i32::MAX)).contains(&v))
        || r.expanded_slots
            .iter()
            .any(|r| i64::from(r.row_id) > last[1].unwrap_or(0))
    {
        return Err(LoaderError::Input);
    }
    r.manufacture.sort_by_key(|r| r.row_id);
    let mut seen = HashSet::new();
    total += r.manufacture.len();
    if !r.manufacture.iter().all(|r| seen.insert(r.row_id))
        || last[2].is_some_and(|v| !(1..i64::from(i32::MAX)).contains(&v))
        || r.manufacture
            .iter()
            .any(|r| i64::from(r.row_id) > last[2].unwrap_or(0))
    {
        return Err(LoaderError::Input);
    }
    r.recipes.sort_by_key(|r| r.row_id);
    let mut seen = HashSet::new();
    total += r.recipes.len();
    if !r.recipes.iter().all(|r| seen.insert(r.row_id))
        || last[3].is_some_and(|v| !(1..i64::from(i32::MAX)).contains(&v))
        || r.recipes
            .iter()
            .any(|r| i64::from(r.row_id) > last[3].unwrap_or(0))
    {
        return Err(LoaderError::Input);
    }
    r.new_recipes.sort_by_key(|r| r.row_id);
    let mut seen = HashSet::new();
    total += r.new_recipes.len();
    if !r.new_recipes.iter().all(|r| seen.insert(r.row_id))
        || last[4].is_some_and(|v| !(1..i64::MAX).contains(&v))
        || r.new_recipes
            .iter()
            .any(|r| r.row_id > last[4].unwrap_or(0))
    {
        return Err(LoaderError::Input);
    }
    if total > 100000 {
        return Err(LoaderError::Input);
    }
    Ok(Snapshot {
        hash: Sha256::digest(&bytes).to_vec(),
        last,
        quick_slots: r.quick_slots,
        expanded_slots: r.expanded_slots,
        manufacture: r.manufacture,
        recipes: r.recipes,
        new_recipes: r.new_recipes,
    })
}
#[derive(Serialize)]
pub struct Summary {
    pub rows: [usize; 5],
    pub fields: [usize; 5],
    pub inserted: usize,
    pub verified: usize,
    pub identities_initialized: usize,
    pub character_creation_available: bool,
}
async fn bounds(tx: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, LoaderError> {
    tokio::time::timeout(Duration::from_secs(30),async{
 let mut tx=pool.begin().await?;bounds(&mut tx).await?;
sqlx::query("LOCK TABLE game_quick_slots,game_expanded_slots,game_manufacture,game_manufacture_recipes,game_new_recipes,game_item_loader_preservation IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
let marker:Option<(Vec<u8>,Vec<Option<i64>>)>=sqlx::query_as("SELECT source_sha256,source_last FROM game_item_loader_preservation WHERE singleton").fetch_optional(&mut *tx).await?;
let mut occupied=false;
let seq0:(i64,bool)=sqlx::query_as("SELECT last_value,is_called FROM game_quick_slot_id").fetch_one(&mut *tx).await?;occupied|=seq0.0!=1||seq0.1||sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM game_quick_slots)").fetch_one(&mut *tx).await?;
let seq1:(i64,bool)=sqlx::query_as("SELECT last_value,is_called FROM game_expanded_slot_id").fetch_one(&mut *tx).await?;occupied|=seq1.0!=1||seq1.1||sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM game_expanded_slots)").fetch_one(&mut *tx).await?;
let seq2:(i64,bool)=sqlx::query_as("SELECT last_value,is_called FROM game_manufacture_id").fetch_one(&mut *tx).await?;occupied|=seq2.0!=1||seq2.1||sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM game_manufacture)").fetch_one(&mut *tx).await?;
let seq3:(i64,bool)=sqlx::query_as("SELECT last_value,is_called FROM game_manufacture_recipe_id").fetch_one(&mut *tx).await?;occupied|=seq3.0!=1||seq3.1||sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM game_manufacture_recipes)").fetch_one(&mut *tx).await?;
let seq4:(i64,bool)=sqlx::query_as("SELECT last_value,is_called FROM game_new_recipe_id").fetch_one(&mut *tx).await?;occupied|=seq4.0!=1||seq4.1||sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM game_new_recipes)").fetch_one(&mut *tx).await?;
let mut inserted=0;
if let Some((hash,last))=marker{if hash!=s.hash||last!=s.last{return Err(LoaderError::Conflict)}
if s.last[0].is_some_and(|v|seq0.0<v||(!seq0.1&&seq0.0==v)){return Err(LoaderError::Conflict)}
if s.last[1].is_some_and(|v|seq1.0<v||(!seq1.1&&seq1.0==v)){return Err(LoaderError::Conflict)}
if s.last[2].is_some_and(|v|seq2.0<v||(!seq2.1&&seq2.0==v)){return Err(LoaderError::Conflict)}
if s.last[3].is_some_and(|v|seq3.0<v||(!seq3.1&&seq3.0==v)){return Err(LoaderError::Conflict)}
if s.last[4].is_some_and(|v|seq4.0<v||(!seq4.1&&seq4.0==v)){return Err(LoaderError::Conflict)}
}else{if verify||occupied{return Err(LoaderError::Conflict)}
for r in &s.quick_slots{inserted+=sqlx::query("INSERT INTO game_quick_slots(row_id,owner_id,item_class_utf16,slot)VALUES($1,$2,$3,$4)").bind(r.row_id).bind(r.owner_id).bind(&r.item_class_utf16).bind(r.slot).execute(&mut *tx).await?.rows_affected()as usize;}
let next=s.last[0].map_or(1,|v|v+1);sqlx::query(&format!("ALTER SEQUENCE game_quick_slot_id RESTART WITH {next}")).execute(&mut *tx).await?;
for r in &s.expanded_slots{inserted+=sqlx::query("INSERT INTO game_expanded_slots(row_id,cid,slot)VALUES($1,$2,$3)").bind(r.row_id).bind(r.cid).bind(r.slot).execute(&mut *tx).await?.rows_affected()as usize;}
let next=s.last[1].map_or(1,|v|v+1);sqlx::query(&format!("ALTER SEQUENCE game_expanded_slot_id RESTART WITH {next}")).execute(&mut *tx).await?;
for r in &s.manufacture{inserted+=sqlx::query("INSERT INTO game_manufacture(row_id,cid,manufacture_utf16,grade,experience_point)VALUES($1,$2,$3,$4,$5)").bind(r.row_id).bind(r.cid).bind(&r.manufacture_utf16).bind(r.grade).bind(r.experience_point).execute(&mut *tx).await?.rows_affected()as usize;}
let next=s.last[2].map_or(1,|v|v+1);sqlx::query(&format!("ALTER SEQUENCE game_manufacture_id RESTART WITH {next}")).execute(&mut *tx).await?;
for r in &s.recipes{inserted+=sqlx::query("INSERT INTO game_manufacture_recipes(row_id,cid,recipe_utf16,acquired_ticks)VALUES($1,$2,$3,$4)").bind(r.row_id).bind(r.cid).bind(&r.recipe_utf16).bind(r.acquired_ticks).execute(&mut *tx).await?.rows_affected()as usize;}
let next=s.last[3].map_or(1,|v|v+1);sqlx::query(&format!("ALTER SEQUENCE game_manufacture_recipe_id RESTART WITH {next}")).execute(&mut *tx).await?;
for r in &s.new_recipes{inserted+=sqlx::query("INSERT INTO game_new_recipes(row_id,cid,recipe_utf16,new_until_ticks)VALUES($1,$2,$3,$4)").bind(r.row_id).bind(r.cid).bind(&r.recipe_utf16).bind(r.new_until_ticks).execute(&mut *tx).await?.rows_affected()as usize;}
let next=s.last[4].map_or(1,|v|v+1);sqlx::query(&format!("ALTER SEQUENCE game_new_recipe_id RESTART WITH {next}")).execute(&mut *tx).await?;
sqlx::query("INSERT INTO game_item_loader_preservation(source_sha256,source_last)VALUES($1,$2)").bind(&s.hash).bind(&s.last).execute(&mut *tx).await?;}
let rows:Vec<Row0>=sqlx::query_as("SELECT row_id,owner_id,item_class_utf16,slot FROM game_quick_slots ORDER BY row_id LIMIT 100001").fetch_all(&mut *tx).await?;if rows!=s.quick_slots{return Err(LoaderError::Conflict)}
let rows:Vec<Row1>=sqlx::query_as("SELECT row_id,cid,slot FROM game_expanded_slots ORDER BY row_id LIMIT 100001").fetch_all(&mut *tx).await?;if rows!=s.expanded_slots{return Err(LoaderError::Conflict)}
let rows:Vec<Row2>=sqlx::query_as("SELECT row_id,cid,manufacture_utf16,grade,experience_point FROM game_manufacture ORDER BY row_id LIMIT 100001").fetch_all(&mut *tx).await?;if rows!=s.manufacture{return Err(LoaderError::Conflict)}
let rows:Vec<Row3>=sqlx::query_as("SELECT row_id,cid,recipe_utf16,acquired_ticks FROM game_manufacture_recipes ORDER BY row_id LIMIT 100001").fetch_all(&mut *tx).await?;if rows!=s.recipes{return Err(LoaderError::Conflict)}
let rows:Vec<Row4>=sqlx::query_as("SELECT row_id,cid,recipe_utf16,new_until_ticks FROM game_new_recipes ORDER BY row_id LIMIT 100001").fetch_all(&mut *tx).await?;if rows!=s.new_recipes{return Err(LoaderError::Conflict)}
let rows=[s.quick_slots.len(),s.expanded_slots.len(),s.manufacture.len(),s.recipes.len(),s.new_recipes.len()];tx.commit().await?;Ok(Summary{rows,fields:[4,3,5,4,4],inserted,verified:rows.iter().sum(),identities_initialized:5,character_creation_available:false})}).await.map_err(|_|LoaderError::Timeout)?
}
/// Caller validates ownership and retains this transaction through consumption.
/// These are absence proofs,not constructors or permission to omit later providers.
pub async fn require_slots_absent(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<(), LoaderError> {
    require_absent(tx, cid, false).await
}
/// Expanded slot rows only; quick slot rows are loaded by the selected-character path.
pub async fn require_expanded_absent(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<(), LoaderError> {
    lock_initialized(tx).await?;
    let present: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_expanded_slots WHERE cid=$1)")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    if present {
        return Err(LoaderError::Nonempty);
    }
    Ok(())
}
pub async fn require_manufacture_absent(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<(), LoaderError> {
    require_absent(tx, cid, true).await
}
async fn require_absent(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    manufacture: bool,
) -> Result<(), LoaderError> {
    lock_initialized(tx).await?;
    let queries = if manufacture {
        &[
            "SELECT EXISTS(SELECT 1 FROM game_manufacture WHERE cid=$1)",
            "SELECT EXISTS(SELECT 1 FROM game_manufacture_recipes WHERE cid=$1)",
            "SELECT EXISTS(SELECT 1 FROM game_new_recipes WHERE cid=$1)",
        ][..]
    } else {
        &[
            "SELECT EXISTS(SELECT 1 FROM game_quick_slots WHERE owner_id=$1)",
            "SELECT EXISTS(SELECT 1 FROM game_expanded_slots WHERE cid=$1)",
        ][..]
    };
    for q in queries {
        if sqlx::query_scalar::<_, bool>(q)
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?
        {
            return Err(LoaderError::Nonempty);
        }
    }
    Ok(())
}
/// Internal shared proof/lock prefix;does not query association rows out of order.
pub(crate) async fn lock_initialized(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), LoaderError> {
    bounds(tx).await?;
    sqlx::query("LOCK TABLE game_quick_slots,game_expanded_slots,game_manufacture,game_manufacture_recipes,game_new_recipes,game_item_loader_preservation IN SHARE MODE").execute(&mut **tx).await?;
    let ready: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_item_loader_preservation WHERE singleton)",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !ready {
        return Err(LoaderError::Uninitialized);
    }
    Ok(())
}
/// Reuse the source-validated empty Gear projection only after both actual provider
/// associations are initialized and empty. No full ItemOwner/Save/LoadPlayer implied.
pub async fn load_initial_gear(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    appearance: &super::fiona_appearance::Profile,
    catalog: &super::item_catalog::Package,
) -> Result<Projection, LoaderError> {
    let mut tx = state::owned_first_fiona(pool, key, cid).await?;
    require_slots_absent(&mut tx, cid).await?;
    sqlx::query("LOCK TABLE game_items,game_item_storage_status,game_item_identity IN SHARE MODE")
        .execute(&mut *tx)
        .await?;
    let ready: bool =
        sqlx::query_scalar("SELECT initialized FROM game_item_identity WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    if !ready {
        return Err(LoaderError::Uninitialized);
    }
    let nonempty:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_items WHERE owner_id=$1) OR EXISTS(SELECT 1 FROM game_item_storage_status WHERE owner_id=$1)").bind(cid).fetch_one(&mut *tx).await?;
    if nonempty {
        return Err(LoaderError::Nonempty);
    }
    let gear = appearance
        .initial_gear(catalog)
        .map_err(|_| LoaderError::Content)?;
    tx.commit().await?;
    Ok(gear)
}
pub async fn check_ready(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT row_id,owner_id,item_class_utf16,slot FROM game_quick_slots LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query("SELECT row_id,cid,slot FROM game_expanded_slots LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query(
        "SELECT row_id,cid,manufacture_utf16,grade,experience_point FROM game_manufacture LIMIT 0",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "SELECT row_id,cid,recipe_utf16,acquired_ticks FROM game_manufacture_recipes LIMIT 0",
    )
    .execute(pool)
    .await?;
    sqlx::query("SELECT row_id,cid,recipe_utf16,new_until_ticks FROM game_new_recipes LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query(
        "SELECT singleton,source_sha256,source_last FROM game_item_loader_preservation LIMIT 0",
    )
    .execute(pool)
    .await?;
    Ok(())
}
