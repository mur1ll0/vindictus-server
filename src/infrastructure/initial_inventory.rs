//! Insert-only empty Fiona inventory storage; not SaveWithoutSync or TCP Create.
use crate::{
    domain::{game_user::LegacyNameKey, initial_inventory::PreparedInventory},
    infrastructure::character_lobby::UserKey,
};
use sqlx::PgPool;
use std::{collections::BTreeMap, time::Duration};
use thiserror::Error;
#[derive(Debug, Error)]
pub enum InventoryWriteError {
    #[error("initial inventory database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("initial inventory does not own the synthetic Fiona revision")]
    Ownership,
    #[error(transparent)]
    Admission(#[from] super::fiona_character_state::StateError),
    #[error("initial inventory conflicts with stored state; nothing repaired")]
    Conflict,
    #[error("item reference preservation is not initialized")]
    Uninitialized,
    #[error("initial inventory operation timed out; verify original attempt before retry")]
    Timeout,
}
#[derive(sqlx::FromRow)]
struct Stored {
    id: i64,
    item_class_utf16: Vec<u8>,
    count: i32,
    collection: i16,
    slot: i32,
    expire_ticks: Option<i64>,
    tradable: bool,
    checksum: Option<i32>,
}
#[derive(sqlx::FromRow)]
struct Equippable {
    id: i64,
    part: Option<i16>,
    color1: i32,
    color2: i32,
    color3: i32,
    reduced_durability: i32,
    max_durability_bonus: i32,
}
pub async fn save(
    pool: &PgPool,
    key: &UserKey<'_>,
    inventory: &PreparedInventory,
) -> Result<usize, InventoryWriteError> {
    save_inner(pool, key, inventory, false).await
}
pub(crate) async fn save_first_fiona(
    pool: &PgPool,
    key: &UserKey<'_>,
    inventory: &PreparedInventory,
) -> Result<usize, InventoryWriteError> {
    save_inner(pool, key, inventory, true).await
}
async fn save_inner(
    pool: &PgPool,
    key: &UserKey<'_>,
    inventory: &PreparedInventory,
    first: bool,
) -> Result<usize, InventoryWriteError> {
    let owner = LegacyNameKey::parse(key.name).map_err(|_| InventoryWriteError::Ownership)?;
    if inventory.items().len() > 4096 {
        return Err(InventoryWriteError::Conflict);
    }
    tokio::time::timeout(Duration::from_secs(15),async{
  let mut tx=if first {super::fiona_character_state::owned_first_fiona(pool,key,inventory.owner()).await?}else{pool.begin().await?};
  sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
  sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
  let ready:bool=sqlx::query_scalar("SELECT initialized FROM game_item_identity WHERE singleton FOR SHARE").fetch_one(&mut *tx).await?;
  if !ready{return Err(InventoryWriteError::Uninitialized);}
  let actual:Option<(String,i64)>=sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE").bind(key.uid).fetch_optional(&mut *tx).await?;
  let synthetic:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)").bind(key.uid).bind(owner.as_str()).fetch_one(&mut *tx).await?;
  if actual!=Some((owner.as_str().to_owned(),key.revision))||!synthetic{return Err(InventoryWriteError::Ownership);}
  sqlx::query("LOCK TABLE game_character_info IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let owned:Option<i64>=sqlx::query_scalar("SELECT id FROM game_character_info WHERE id=$1 AND uid=$2 AND class BETWEEN 0 AND 9 FOR UPDATE").bind(inventory.owner()).bind(key.uid).fetch_optional(&mut *tx).await?;
  if owned.is_none(){return Err(InventoryWriteError::Ownership);}
  let collection:Option<i64>=sqlx::query_scalar("SELECT owner_id FROM game_item_collections WHERE owner_id=$1 FOR UPDATE").bind(inventory.owner()).fetch_optional(&mut *tx).await?;
  if collection.is_none(){return Err(InventoryWriteError::Conflict);}
  // Match preservation's global table-lock order before examining orphan children.
  sqlx::query("LOCK TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let storage:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_item_storage_status WHERE owner_id=$1)").bind(inventory.owner()).fetch_one(&mut *tx).await?;
  if storage{return Err(InventoryWriteError::Conflict);}
  let ids=inventory.items().iter().map(|r|r.id()).collect::<Vec<_>>();
  let attributes:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_item_attributes WHERE item_id=ANY($1))").bind(&ids).fetch_one(&mut *tx).await?;
  if attributes{return Err(InventoryWriteError::Conflict);}
  let old:Vec<Stored>=sqlx::query_as("SELECT id,item_class_utf16,count,collection,slot,expire_ticks,tradable,checksum FROM game_items WHERE owner_id=$1 ORDER BY id LIMIT 4097 FOR UPDATE").bind(inventory.owner()).fetch_all(&mut *tx).await?;
  let equipment:Vec<Equippable>=sqlx::query_as("SELECT id,part,color1,color2,color3,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=ANY($1) FOR UPDATE").bind(&ids).fetch_all(&mut *tx).await?;
  if !old.is_empty(){
   if old.len()!=inventory.items().len(){return Err(InventoryWriteError::Conflict);}
   let equips=equipment.into_iter().map(|e|(e.id,e)).collect::<BTreeMap<_,_>>();
   for(a,b)in old.iter().zip(inventory.items()){
    if a.id!=b.id()||a.item_class_utf16!=b.class()||a.count!=b.count()||a.collection!=i16::from(b.location().collection)||a.slot!=b.location().slot||a.expire_ticks!=b.expiry()||!a.tradable||a.checksum!=b.checksum(){return Err(InventoryWriteError::Conflict);}
    match (equips.get(&a.id),b.equipment()){
     (None,None)=>{},
     (Some(x),Some(y))if x.part.is_none()&&[x.color1,x.color2,x.color3]==y.colors&&x.reduced_durability==y.reduced&&x.max_durability_bonus==y.bonus=>{},
     _=>return Err(InventoryWriteError::Conflict),
    }
   }
   tx.commit().await?;return Ok(0);
  }
  if !equipment.is_empty(){return Err(InventoryWriteError::Conflict);}
  for row in inventory.items(){
   sqlx::query("INSERT INTO game_items(id,item_class_utf16,owner_id,count,collection,slot,expire_ticks,tradable,checksum) VALUES($1,$2,$3,$4,$5,$6,$7,true,$8)")
    .bind(row.id()).bind(row.class()).bind(inventory.owner()).bind(row.count()).bind(i16::from(row.location().collection)).bind(row.location().slot).bind(row.expiry()).bind(row.checksum()).execute(&mut *tx).await?;
   if let Some(e)=row.equipment(){
    sqlx::query("INSERT INTO game_equippables(id,part,color1,color2,color3,reduced_durability,max_durability_bonus) VALUES($1,NULL,$2,$3,$4,$5,$6)")
     .bind(row.id()).bind(e.colors[0]).bind(e.colors[1]).bind(e.colors[2]).bind(e.reduced).bind(e.bonus).execute(&mut *tx).await?;
   }
  }
  tx.commit().await?;Ok(inventory.items().len())
 }).await.map_err(|_|InventoryWriteError::Timeout)?
}
