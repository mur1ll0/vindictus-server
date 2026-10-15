//! Complete insert-only reference preservation, not Item.Initialize or playable delivery.
use super::{
    legacy_users::SourceBytes,
    user_export::{PrivateText, hex, integer, nullable},
};
use crate::domain::legacy_user_data::{LegacyBytes, SqlDateTime, SqlDateTime2};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::HashSet, io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum ItemStateError {
    #[error("item state database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("item state snapshot IO failed")]
    Io(#[from] std::io::Error),
    #[error("invalid bounded item state or unsupported attribute collation profile")]
    Export,
    #[error("item state preservation conflict")]
    Conflict,
    #[error("item state identity is not initialized")]
    Uninitialized,
    #[error("item state deadline exceeded; verify original attempt before retry")]
    Timeout,
}
fn number(v: PrivateText) -> Result<i64, ItemStateError> {
    integer(v).map_err(|_| ItemStateError::Export)
}
fn bytes<const N: usize, const U: bool>(
    v: PrivateText,
) -> Result<LegacyBytes<N, U>, ItemStateError> {
    LegacyBytes::from_bytes(hex(v, if U { N * 2 } else { N }).map_err(|_| ItemStateError::Export)?)
        .map_err(|_| ItemStateError::Export)
}
fn stored_bytes<const N: usize, const U: bool>(
    v: SourceBytes,
) -> Result<LegacyBytes<N, U>, ItemStateError> {
    LegacyBytes::from_bytes(v.into_bytes()).map_err(|_| ItemStateError::Conflict)
}
fn ascii(value: &[u8]) -> bool {
    value.iter().all(|v| (32..=126).contains(v))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    #[serde(deserialize_with = "nullable")]
    attribute_last_value: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    storage_last_value: Option<PrivateText>,
    items: Vec<ItemExport>,
    attributes: Vec<AttributeExport>,
    equipment: Vec<EquipmentExport>,
    storage: Vec<StorageExport>,
}
pub struct Snapshot {
    items: Vec<Item>,
    attributes: Vec<Attribute>,
    equipment: Vec<Equipment>,
    storage: Vec<Storage>,
    last: [Option<i64>; 2],
}

#[derive(PartialEq, Eq)]
struct Item {
    id: i64,
    item_class_utf16: LegacyBytes<50, true>,
    owner_id: Option<i64>,
    count: i32,
    collection: i16,
    slot: i32,
    expire_ticks: Option<i64>,
    tradable: bool,
    checksum: Option<i32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemExport {
    id: PrivateText,
    item_class_utf16: PrivateText,
    #[serde(deserialize_with = "nullable")]
    owner_id: Option<PrivateText>,
    count: i32,
    collection: u8,
    slot: i32,
    #[serde(deserialize_with = "nullable")]
    expire_ticks: Option<PrivateText>,
    tradable: bool,
    #[serde(deserialize_with = "nullable")]
    checksum: Option<i32>,
}
#[derive(sqlx::FromRow)]
struct ItemStored {
    id: i64,
    item_class_utf16: SourceBytes,
    owner_id: Option<i64>,
    count: i32,
    collection: i16,
    slot: i32,
    expire_ticks: Option<i64>,
    tradable: bool,
    checksum: Option<i32>,
}
impl ItemExport {
    fn record(self) -> Result<Item, ItemStateError> {
        Ok(Item {
            id: number(self.id)?,
            item_class_utf16: bytes(self.item_class_utf16)?,
            owner_id: self.owner_id.map(number).transpose()?,
            count: self.count,
            collection: i16::from(self.collection),
            slot: self.slot,
            expire_ticks: self.expire_ticks.map(number).transpose()?,
            tradable: self.tradable,
            checksum: self.checksum,
        })
    }
}
impl ItemStored {
    fn record(self) -> Result<Item, ItemStateError> {
        Ok(Item {
            id: self.id,
            item_class_utf16: stored_bytes(self.item_class_utf16)?,
            owner_id: self.owner_id,
            count: self.count,
            collection: self.collection,
            slot: self.slot,
            expire_ticks: self.expire_ticks,
            tradable: self.tradable,
            checksum: self.checksum,
        })
    }
}
const ITEMS_COLUMNS: &str =
    "id,item_class_utf16,owner_id,count,collection,slot,expire_ticks,tradable,checksum";
async fn preserve_items(
    tx: &mut sqlx::PgConnection,
    rows: &[Item],
    insert: bool,
) -> Result<usize, ItemStateError> {
    let mut inserted = 0;
    for r in rows {
        if insert {
            inserted+=sqlx::query("INSERT INTO game_items(id,item_class_utf16,owner_id,count,collection,slot,expire_ticks,tradable,checksum) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT DO NOTHING").bind(r.id).bind(r.item_class_utf16.as_bytes()).bind(r.owner_id).bind(r.count).bind(r.collection).bind(r.slot).bind(r.expire_ticks).bind(r.tradable).bind(r.checksum).execute(&mut *tx).await?.rows_affected() as usize;
        }
        let stored: Option<ItemStored> = sqlx::query_as(&format!(
            "SELECT {ITEMS_COLUMNS} FROM game_items WHERE id=$1"
        ))
        .bind(r.id)
        .fetch_optional(&mut *tx)
        .await?;
        if stored.map(ItemStored::record).transpose()?.as_ref() != Some(r) {
            return Err(ItemStateError::Conflict);
        }
    }
    Ok(inserted)
}

#[derive(PartialEq, Eq)]
struct Attribute {
    id: i64,
    item_id: i64,
    attribute_bytes: LegacyBytes<50, false>,
    value_bytes: LegacyBytes<50, false>,
    arg: i32,
    arg2: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttributeExport {
    id: PrivateText,
    item_id: PrivateText,
    attribute_bytes: PrivateText,
    value_bytes: PrivateText,
    arg: i32,
    arg2: i32,
}
#[derive(sqlx::FromRow)]
struct AttributeStored {
    id: i64,
    item_id: i64,
    attribute_bytes: SourceBytes,
    value_bytes: SourceBytes,
    arg: i32,
    arg2: i32,
}
impl AttributeExport {
    fn record(self) -> Result<Attribute, ItemStateError> {
        Ok(Attribute {
            id: number(self.id)?,
            item_id: number(self.item_id)?,
            attribute_bytes: bytes(self.attribute_bytes)?,
            value_bytes: bytes(self.value_bytes)?,
            arg: self.arg,
            arg2: self.arg2,
        })
    }
}
impl AttributeStored {
    fn record(self) -> Result<Attribute, ItemStateError> {
        Ok(Attribute {
            id: self.id,
            item_id: self.item_id,
            attribute_bytes: stored_bytes(self.attribute_bytes)?,
            value_bytes: stored_bytes(self.value_bytes)?,
            arg: self.arg,
            arg2: self.arg2,
        })
    }
}
const ATTRIBUTES_COLUMNS: &str = "id,item_id,attribute_bytes,value_bytes,arg,arg2";
async fn preserve_attributes(
    tx: &mut sqlx::PgConnection,
    rows: &[Attribute],
    insert: bool,
) -> Result<usize, ItemStateError> {
    let mut inserted = 0;
    for r in rows {
        if insert {
            inserted+=sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING").bind(r.id).bind(r.item_id).bind(r.attribute_bytes.as_bytes()).bind(r.value_bytes.as_bytes()).bind(r.arg).bind(r.arg2).execute(&mut *tx).await?.rows_affected() as usize;
        }
        let stored: Option<AttributeStored> = sqlx::query_as(&format!(
            "SELECT {ATTRIBUTES_COLUMNS} FROM game_item_attributes WHERE id=$1"
        ))
        .bind(r.id)
        .fetch_optional(&mut *tx)
        .await?;
        if stored.map(AttributeStored::record).transpose()?.as_ref() != Some(r) {
            return Err(ItemStateError::Conflict);
        }
    }
    Ok(inserted)
}

#[derive(PartialEq, Eq)]
struct Equipment {
    id: i64,
    part: Option<i16>,
    color1: i32,
    color2: i32,
    color3: i32,
    reduced_durability: i32,
    max_durability_bonus: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EquipmentExport {
    id: PrivateText,
    #[serde(deserialize_with = "nullable")]
    part: Option<u8>,
    color1: i32,
    color2: i32,
    color3: i32,
    reduced_durability: i32,
    max_durability_bonus: i32,
}
#[derive(sqlx::FromRow)]
struct EquipmentStored {
    id: i64,
    part: Option<i16>,
    color1: i32,
    color2: i32,
    color3: i32,
    reduced_durability: i32,
    max_durability_bonus: i32,
}
impl EquipmentExport {
    fn record(self) -> Result<Equipment, ItemStateError> {
        Ok(Equipment {
            id: number(self.id)?,
            part: self.part.map(i16::from),
            color1: self.color1,
            color2: self.color2,
            color3: self.color3,
            reduced_durability: self.reduced_durability,
            max_durability_bonus: self.max_durability_bonus,
        })
    }
}
impl EquipmentStored {
    fn record(self) -> Result<Equipment, ItemStateError> {
        Ok(Equipment {
            id: self.id,
            part: self.part,
            color1: self.color1,
            color2: self.color2,
            color3: self.color3,
            reduced_durability: self.reduced_durability,
            max_durability_bonus: self.max_durability_bonus,
        })
    }
}
const EQUIPMENT_COLUMNS: &str =
    "id,part,color1,color2,color3,reduced_durability,max_durability_bonus";
async fn preserve_equipment(
    tx: &mut sqlx::PgConnection,
    rows: &[Equipment],
    insert: bool,
) -> Result<usize, ItemStateError> {
    let mut inserted = 0;
    for r in rows {
        if insert {
            inserted+=sqlx::query("INSERT INTO game_equippables(id,part,color1,color2,color3,reduced_durability,max_durability_bonus) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING").bind(r.id).bind(r.part).bind(r.color1).bind(r.color2).bind(r.color3).bind(r.reduced_durability).bind(r.max_durability_bonus).execute(&mut *tx).await?.rows_affected() as usize;
        }
        let stored: Option<EquipmentStored> = sqlx::query_as(&format!(
            "SELECT {EQUIPMENT_COLUMNS} FROM game_equippables WHERE id=$1"
        ))
        .bind(r.id)
        .fetch_optional(&mut *tx)
        .await?;
        if stored.map(EquipmentStored::record).transpose()?.as_ref() != Some(r) {
            return Err(ItemStateError::Conflict);
        }
    }
    Ok(inserted)
}

#[derive(PartialEq, Eq)]
struct Storage {
    id: i64,
    owner_id: i64,
    storage_type: i16,
    storage: i16,
    expire_units300: Option<i64>,
    storage_name_utf16: LegacyBytes<50, true>,
    storage_tag: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageExport {
    id: PrivateText,
    owner_id: PrivateText,
    storage_type: u8,
    storage: u8,
    #[serde(deserialize_with = "nullable")]
    expire_units300: Option<PrivateText>,
    storage_name_utf16: PrivateText,
    storage_tag: i32,
}
#[derive(sqlx::FromRow)]
struct StorageStored {
    id: i64,
    owner_id: i64,
    storage_type: i16,
    storage: i16,
    expire_units300: Option<i64>,
    storage_name_utf16: SourceBytes,
    storage_tag: i32,
}
impl StorageExport {
    fn record(self) -> Result<Storage, ItemStateError> {
        Ok(Storage {
            id: number(self.id)?,
            owner_id: number(self.owner_id)?,
            storage_type: i16::from(self.storage_type),
            storage: i16::from(self.storage),
            expire_units300: self.expire_units300.map(number).transpose()?,
            storage_name_utf16: bytes(self.storage_name_utf16)?,
            storage_tag: self.storage_tag,
        })
    }
}
impl StorageStored {
    fn record(self) -> Result<Storage, ItemStateError> {
        Ok(Storage {
            id: self.id,
            owner_id: self.owner_id,
            storage_type: self.storage_type,
            storage: self.storage,
            expire_units300: self.expire_units300,
            storage_name_utf16: stored_bytes(self.storage_name_utf16)?,
            storage_tag: self.storage_tag,
        })
    }
}
const STORAGE_COLUMNS: &str =
    "id,owner_id,storage_type,storage,expire_units300,storage_name_utf16,storage_tag";
async fn preserve_storage(
    tx: &mut sqlx::PgConnection,
    rows: &[Storage],
    insert: bool,
) -> Result<usize, ItemStateError> {
    let mut inserted = 0;
    for r in rows {
        if insert {
            inserted+=sqlx::query("INSERT INTO game_item_storage_status(id,owner_id,storage_type,storage,expire_units300,storage_name_utf16,storage_tag) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING").bind(r.id).bind(r.owner_id).bind(r.storage_type).bind(r.storage).bind(r.expire_units300).bind(r.storage_name_utf16.as_bytes()).bind(r.storage_tag).execute(&mut *tx).await?.rows_affected() as usize;
        }
        let stored: Option<StorageStored> = sqlx::query_as(&format!(
            "SELECT {STORAGE_COLUMNS} FROM game_item_storage_status WHERE id=$1"
        ))
        .bind(r.id)
        .fetch_optional(&mut *tx)
        .await?;
        if stored.map(StorageStored::record).transpose()?.as_ref() != Some(r) {
            return Err(ItemStateError::Conflict);
        }
    }
    Ok(inserted)
}

const MAX_ROWS: usize = 50_000;
const MAX_BYTES: u64 = 16 * 1024 * 1024;
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, ItemStateError> {
    let mut b = Zeroizing::new(Vec::new());
    input.take(MAX_BYTES + 1).read_to_end(&mut b)?;
    if b.len() as u64 > MAX_BYTES {
        return Err(ItemStateError::Export);
    }
    let e: Export = serde_json::from_slice(&b).map_err(|_| ItemStateError::Export)?;
    if e.format_version != 1
        || e.source != "study_heroes.item-state"
        || e.items.len() + e.attributes.len() + e.equipment.len() + e.storage.len() > MAX_ROWS
    {
        return Err(ItemStateError::Export);
    }
    let mut s = Snapshot {
        items: e
            .items
            .into_iter()
            .map(ItemExport::record)
            .collect::<Result<_, _>>()?,
        attributes: e
            .attributes
            .into_iter()
            .map(AttributeExport::record)
            .collect::<Result<_, _>>()?,
        equipment: e
            .equipment
            .into_iter()
            .map(EquipmentExport::record)
            .collect::<Result<_, _>>()?,
        storage: e
            .storage
            .into_iter()
            .map(StorageExport::record)
            .collect::<Result<_, _>>()?,
        last: [
            e.attribute_last_value.map(number).transpose()?,
            e.storage_last_value.map(number).transpose()?,
        ],
    };
    if s.last
        .iter()
        .any(|v| v.is_some_and(|v| !(0..i64::MAX).contains(&v)))
    {
        return Err(ItemStateError::Export);
    }
    let mut seen = HashSet::new();
    if !s.items.iter().all(|r| seen.insert(r.id)) {
        return Err(ItemStateError::Export);
    }
    s.items.sort_by_key(|r| r.id);
    let mut seen = HashSet::new();
    if !s.attributes.iter().all(|r| seen.insert(r.id)) {
        return Err(ItemStateError::Export);
    }
    s.attributes.sort_by_key(|r| r.id);
    let mut seen = HashSet::new();
    if !s.equipment.iter().all(|r| seen.insert(r.id)) {
        return Err(ItemStateError::Export);
    }
    s.equipment.sort_by_key(|r| r.id);
    let mut seen = HashSet::new();
    if !s.storage.iter().all(|r| seen.insert(r.id)) {
        return Err(ItemStateError::Export);
    }
    s.storage.sort_by_key(|r| r.id);

    for r in &s.items {
        if r.expire_ticks
            .is_some_and(|v| SqlDateTime2::from_ticks(v).is_err())
        {
            return Err(ItemStateError::Export);
        }
    }
    let mut keys = HashSet::new();
    for r in &s.attributes {
        let mut key = r.attribute_bytes.as_bytes().to_vec();
        if !ascii(&key) || r.id < 1 || !s.last[0].is_some_and(|v| r.id <= v) {
            return Err(ItemStateError::Export);
        }
        while key.last() == Some(&b' ') {
            key.pop();
        }
        key.make_ascii_lowercase();
        if !keys.insert((r.item_id, key)) {
            return Err(ItemStateError::Export);
        }
    }
    for r in &s.storage {
        if r.id < 1
            || !s.last[1].is_some_and(|v| r.id <= v)
            || r.expire_units300
                .is_some_and(|v| SqlDateTime::from_units300(v).is_err())
        {
            return Err(ItemStateError::Export);
        }
    }
    Ok(s)
}
#[derive(Serialize)]
pub struct Summary {
    pub source_rows: [usize; 4],
    pub inserted: [usize; 4],
    pub verified: [usize; 4],
    pub identity_initialized: bool,
    pub character_creation_available: bool,
}
type IdentityState = (
    bool,
    Option<i64>,
    Option<i64>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);
pub async fn preserve(
    pool: &PgPool,
    s: &Snapshot,
    verify: bool,
) -> Result<Summary, ItemStateError> {
    tokio::time::timeout(Duration::from_secs(60),async{
 let mut tx=pool.begin().await?;
 sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
 sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
 let counts=[s.items.len(),s.attributes.len(),s.equipment.len(),s.storage.len()];
 let (initialized,a,b,ni,na,ne,ns):IdentityState=sqlx::query_as("SELECT initialized,attribute_last_value,storage_last_value,source_item_count,source_attribute_count,source_equipment_count,source_storage_count FROM game_item_identity WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
 if initialized&&([a,b]!=s.last||[ni,na,ne,ns]!=counts.map(|n|Some(n as i32))){return Err(ItemStateError::Conflict)}
 if verify&&!initialized{return Err(ItemStateError::Uninitialized)}
 sqlx::query("LOCK TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
 let insert=!verify&&!initialized;
 let inserted=[preserve_items(&mut tx,&s.items,insert).await?,preserve_attributes(&mut tx,&s.attributes,insert).await?,preserve_equipment(&mut tx,&s.equipment,insert).await?,preserve_storage(&mut tx,&s.storage,insert).await?];
 if insert {
 let totals:(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM game_items),(SELECT COUNT(*) FROM game_item_attributes),(SELECT COUNT(*) FROM game_equippables),(SELECT COUNT(*) FROM game_item_storage_status)").fetch_one(&mut *tx).await?;
 if [totals.0,totals.1,totals.2,totals.3]!=counts.map(|n|n as i64){return Err(ItemStateError::Conflict)}
 let next_a=s.last[0].map_or(1,|v|v+1);let next_b=s.last[1].map_or(1,|v|v+1);
 // Fixed identifiers and validated integers. RESTART is transactional, unlike setval.
 sqlx::query(&format!("ALTER SEQUENCE game_item_attribute_id RESTART WITH {next_a}")).execute(&mut *tx).await?;
 sqlx::query(&format!("ALTER SEQUENCE game_item_storage_id RESTART WITH {next_b}")).execute(&mut *tx).await?;
 sqlx::query("UPDATE game_item_identity SET initialized=TRUE,attribute_last_value=$1,storage_last_value=$2,source_item_count=$3,source_attribute_count=$4,source_equipment_count=$5,source_storage_count=$6 WHERE singleton").bind(s.last[0]).bind(s.last[1]).bind(counts[0] as i32).bind(counts[1] as i32).bind(counts[2] as i32).bind(counts[3] as i32).execute(&mut *tx).await?;
 }
 tx.commit().await?;
 Ok(Summary{source_rows:counts,inserted,verified:counts,identity_initialized:true,character_creation_available:false})
 }).await.map_err(|_|ItemStateError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), ItemStateError> {
    sqlx::query(&format!("SELECT {ITEMS_COLUMNS} FROM game_items LIMIT 0"))
        .execute(pool)
        .await?;
    sqlx::query(&format!(
        "SELECT {ATTRIBUTES_COLUMNS} FROM game_item_attributes LIMIT 0"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "SELECT {EQUIPMENT_COLUMNS} FROM game_equippables LIMIT 0"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "SELECT {STORAGE_COLUMNS} FROM game_item_storage_status LIMIT 0"
    ))
    .execute(pool)
    .await?;
    sqlx::query("SELECT singleton,initialized,attribute_last_value,storage_last_value,source_item_count,source_attribute_count,source_equipment_count,source_storage_count FROM game_item_identity LIMIT 0").execute(pool).await?;
    Ok(())
}
