//! Read-only initial inventory snapshot for the initialized, unequipped Fiona scope.
//! Expanded/shared states are refused until their loaders exist. Item attributes load;
//! equipment attributes other than ENHANCE are refused by Item.Stat.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        item_attribute::{self, Attribute, Attributes},
        item_durability::EquipmentDurability,
        map_partition::OutgoingPacket,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Dict, Inventory, Kind, Message, Slot, Storage},
    },
};
use sqlx::{PgPool, Postgres, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
#[derive(sqlx::FromRow)]
struct Item {
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
struct Equipment {
    id: i64,
    part: Option<i16>,
    color1: i32,
    color2: i32,
    color3: i32,
    reduced_durability: i32,
    max_durability_bonus: i32,
}
fn text(b: &[u8]) -> Result<String, MinimumError> {
    if !b.len().is_multiple_of(2) || b.len() > 2048 {
        return Err(MinimumError::Conflict);
    }
    String::from_utf16(
        &b.as_chunks::<2>()
            .0
            .iter()
            .map(|v| u16::from_le_bytes(*v))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| MinimumError::Conflict)
}
fn wire_attribute(a: &Attribute) -> Option<wire::Attribute> {
    Some(wire::Attribute {
        name: Some(a.name.clone()),
        value: Some(a.value.clone()),
        arg: a.arg,
        arg2: a.arg2,
    })
}
/// Item.Attribute of a SlotInfo, as the attribute view IsStackableWith compares.
pub(super) fn slot_attributes(slot: &Slot) -> Vec<(&str, &str, i32)> {
    slot.attributes
        .iter()
        .flatten()
        .flatten()
        .map(|a| {
            (
                a.name.as_deref().unwrap_or(""),
                a.value.as_deref().unwrap_or(""),
                a.arg,
            )
        })
        .collect()
}
/// ItemLocation.RestoreLocation collection.
pub(super) const RESTORE_COLLECTION: i16 = 102;
/// item_id, attribute_bytes, value_bytes, arg, arg2.
type AttributeRow = (i64, Vec<u8>, Vec<u8>, i32, i32);
/// Item.LoadAttribute: the ItemAttribute rows of the items, in ID (insertion) order;
/// a repeated AttributeName fails like Dictionary.Add.
pub(super) async fn load_attributes(
    tx: &mut Transaction<'_, Postgres>,
    ids: &[i64],
) -> Result<BTreeMap<i64, Attributes>, MinimumError> {
    let rows: Vec<AttributeRow> = sqlx::query_as(
        "SELECT item_id,attribute_bytes,value_bytes,arg,arg2 FROM game_item_attributes WHERE item_id=ANY($1) ORDER BY id LIMIT 65537",
    )
    .bind(ids)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 65536 {
        return Err(MinimumError::Conflict);
    }
    let mut grouped: BTreeMap<i64, Vec<Attribute>> = BTreeMap::new();
    for (item, name, value, arg, arg2) in rows {
        grouped.entry(item).or_default().push(
            Attribute::from_stored(&name, &value, arg, arg2).map_err(|_| MinimumError::Conflict)?,
        );
    }
    grouped
        .into_iter()
        .map(|(id, list)| {
            Ok((
                id,
                Attributes::new(list).map_err(|_| MinimumError::Conflict)?,
            ))
        })
        .collect()
}
/// Handler admission uses the current selected CID; storage independently rechecks ownership.
pub async fn query(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let packet = load(&mut tx, owner, assets, cid, None).await?;
        tx.commit().await?;
        Ok(packet)
    })
    .await
    .map_err(|_| MinimumError::Conflict)?
}
/// Caller retains the same character lock for selection and initial delivery.
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    equipment_info: Option<Vec<(i32, i64)>>,
) -> Result<OutgoingPacket, MinimumError> {
    let inventory = snapshot_with_equipment(tx, owner, assets, cid, equipment_info).await?;
    let body = wire::encode(
        Kind::InventoryInfo,
        Some(&Message::InventoryInfo(inventory)),
        DEFAULT_MAX_BODY,
    )
    .map_err(|_| MinimumError::Conflict)?
    .to_vec();
    Ok(OutgoingPacket {
        category: Kind::InventoryInfo.category(),
        body,
    })
}
/// ItemOwner.ToInventoryInfo projection; SlotInfo rows are also Sync's ToSlotInfo.
/// `equipment` is the EquipmentInfo already projected by the caller from the same
/// locked transaction; None loads and validates Gear here.
pub(super) async fn snapshot_with_equipment(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    equipment_info: Option<Vec<(i32, i64)>>,
) -> Result<Inventory, MinimumError> {
    project(tx, owner, assets, cid, equipment_info, true).await
}
/// ToSlotInfo(ignoreRestoreStorage: false) of the RestoreStorage items.
pub(super) async fn restore_slots(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Vec<Slot>, MinimumError> {
    Ok(project(tx, owner, assets, cid, None, false)
        .await?
        .slots
        .ok_or(MinimumError::Conflict)?
        .into_iter()
        .flatten()
        .filter(|s| s.tab == i32::from(RESTORE_COLLECTION))
        .collect())
}
async fn project(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    equipment_info: Option<Vec<(i32, i64)>>,
    ignore_restore: bool,
) -> Result<Inventory, MinimumError> {
    let marker:Option<(i32,Vec<u8>,bool)>=sqlx::query_as("SELECT uid,profile_sha256,initialized FROM game_minimum_character_initialization WHERE cid=$1 FOR SHARE").bind(cid).fetch_optional(&mut **tx).await?;
    if marker != Some((owner.uid, assets.initializer.fingerprint().to_vec(), true)) {
        return Err(MinimumError::Conflict);
    }
    sqlx::query("LOCK TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status IN SHARE MODE").execute(&mut **tx).await?;
    let opened = bags(tx, cid).await?;
    let rows:Vec<Item>=sqlx::query_as("SELECT id,item_class_utf16,count,collection,slot,expire_ticks,tradable,checksum FROM game_items WHERE owner_id=$1 ORDER BY id LIMIT 4097").bind(cid).fetch_all(&mut **tx).await?;
    if rows.is_empty() || rows.len() > 4096 {
        return Err(MinimumError::Conflict);
    }
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let mut attributes = load_attributes(tx, &ids).await?;
    let equipment:Vec<Equipment>=sqlx::query_as("SELECT id,part,color1,color2,color3,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=ANY($1)").bind(&ids).fetch_all(&mut **tx).await?;
    let mut equipment: BTreeMap<_, _> = equipment.into_iter().map(|e| (e.id, e)).collect();
    let stats = &assets.stats_index;
    let mut slots = Vec::with_capacity(rows.len());
    let mut positions = BTreeSet::new();
    // StorageSetting: collection 103 is the StoryStorage only with StoryTabInInventory.
    let story_tab = assets
        .catalog
        .policy
        .is_enabled(Some("StoryTabInInventory"))?;
    let now = super::lab_clock::utc_now()
        .map_err(|_| MinimumError::Conflict)?
        .ticks();
    let owner_context = super::minimum_item_stat::context(tx, cid).await?;
    for row in rows {
        let bag = opened.contains(&row.collection);
        // AccountStorage: one item per class at AccountLocation (101, -1).
        let account = row.collection == 101;
        let story =
            story_tab && row.collection == i16::from(crate::domain::give_item::STORY_COLLECTION);
        // RestoreStorage (102): bag items moved there by MoveToBuybackStorage.
        let restore = row.collection == RESTORE_COLLECTION;
        if row.count <= 0
            || !(bag || account || story || restore || row.collection == 100)
            || (account != (row.slot == -1))
            || (!account && row.slot < 0)
            || ((bag || story) && row.slot >= 48)
            || !positions.insert((row.collection, row.slot))
        {
            return Err(MinimumError::Conflict);
        }
        let class = stats
            .class(Some(&row.item_class_utf16))
            .ok_or(MinimumError::Conflict)?;
        if class.storage
            != if bag || restore {
                1
            } else if account {
                3
            } else if story {
                5
            } else {
                2
            }
        {
            return Err(MinimumError::Conflict);
        }
        if assets.inventory.is_gold(&row.item_class_utf16) {
            let key = row
                .item_class_utf16
                .as_chunks::<2>()
                .0
                .iter()
                .map(|v| u16::from_le_bytes(*v))
                .collect::<Vec<_>>();
            if row.checksum
                != Some(crate::domain::initial_inventory::gold_checksum(
                    row.id,
                    Some(cid),
                    &key,
                    row.count,
                ))
            {
                return Err(MinimumError::Conflict);
            }
        } else if row.checksum.is_some() {
            return Err(MinimumError::Conflict);
        }
        let equippable = class.equippable;
        let attributes = attributes.remove(&row.id).unwrap_or_default();
        // MIN_UPTIME needs ToFormatableDateTime for MinUpTimeDateTimeDiff; equipment
        // attributes go through Item.Stat (only ENHANCE is migrated).
        if !equippable && attributes.get(item_attribute::MIN_UPTIME).is_some() {
            return Err(MinimumError::Conflict);
        }
        // SlotInfo Prefix/SuffixEnchantStatus (StatBonusStatus of the entities).
        let mut status = (Vec::new(), Vec::new());
        let (colors, durability, maximum, bonus) = match (equippable, equipment.remove(&row.id)) {
            (false, None) => (vec![-1; 3], 0, 0, 0),
            (true, Some(e)) => {
                if e.part.is_some() && assets.equipment.is_none() {
                    return Err(MinimumError::Conflict);
                }
                let base = stats
                    .get(Some(&row.item_class_utf16))
                    .ok_or(MinimumError::Conflict)?;
                let item = super::minimum_item_stat::equipment_with(
                    assets,
                    &row.item_class_utf16,
                    &attributes,
                    Some(&owner_context),
                )?;
                let d = EquipmentDurability::load(
                    base,
                    &item.stat,
                    e.reduced_durability,
                    e.max_durability_bonus,
                );
                status = (item.prefix_status, item.suffix_status);
                (
                    vec![e.color1, e.color2, e.color3],
                    d.current(),
                    d.maximum(),
                    d.bonus(),
                )
            }
            _ => return Err(MinimumError::Conflict),
        };
        if restore && ignore_restore {
            // ToSlotInfo(ignoreRestoreStorage: true): the empty 13-argument SlotInfo.
            slots.push(Some(super::minimum_move::deleted_slot(row.id)));
            continue;
        }
        slots.push(Some(Slot {
            id: row.id,
            class: Some(text(&row.item_class_utf16)?),
            count: if equippable { 1 } else { row.count },
            tab: i32::from(row.collection),
            slot: row.slot,
            // SlotInfo: ExpireDateTime ticks and their difference from UtcNow.
            expireable: row.expire_ticks.is_some(),
            expire_diff: row.expire_ticks.map_or(-1, |t| t.wrapping_sub(now)),
            expire_ticks: row.expire_ticks.unwrap_or(-1),
            colors: Some(colors),
            durability,
            maximum,
            bonus,
            tradable: row.tradable,
            expander: Some(String::new()),
            // SlotInfo.Attributes = Item.Attribute.Values (insertion order).
            attributes: Some(attributes.as_slice().iter().map(wire_attribute).collect()),
            prefix: Dict(Some(status.0)),
            suffix: Dict(Some(status.1)),
            // TradeManager.UpdatePrice initializes an empty cache before its query. Missing
            // market entries retain the SlotInfo default zeros; trading remains deferred.
            min_price: 0,
            max_price: 0,
            avg_price: 0,
            min_up_diff: if equippable { -1 } else { 0 },
        }));
    }
    if !equipment.is_empty() {
        return Err(MinimumError::Conflict);
    }
    let locked = locked_parts(tx, assets, cid).await?;
    let quick = super::minimum_quickslot::load(tx, assets, cid).await?;
    let equipment_info = match equipment_info {
        Some(e) => e,
        None => {
            super::minimum_equipment::load(tx, owner, assets, cid)
                .await?
                .projection
                .equipment
        }
    };
    let inventory = Inventory {
        storages: Some(storage_infos(tx, cid).await?),
        slots: Some(slots),
        equipment: Dict(Some(equipment_info)),
        quick_slots: Some(super::minimum_quickslot::info(&quick)),
        unequippable: Some(locked),
    };
    Ok(inventory)
}

/// DBLoader.GetExpandedSlots (ExpandSlotInfo.Slot in row order).
pub(super) async fn expanded_slots(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<i32>, MinimumError> {
    let rows: Vec<i32> = sqlx::query_scalar(
        "SELECT slot FROM game_expanded_slots WHERE cid=$1 ORDER BY row_id LIMIT 257",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 256 {
        return Err(MinimumError::Conflict);
    }
    Ok(rows)
}

/// Gear.ToUnequippableParts after the Gear constructor: the parts locked by
/// IsExpandedSlot that no ExpandSlotInfo row unlocked (gearSlot.Load).
pub(super) async fn locked_parts(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
) -> Result<Vec<i32>, MinimumError> {
    let expanded = expanded_slots(tx, cid).await?;
    Ok(assets
        .appearance
        .inventory_locked_parts()?
        .into_iter()
        .filter(|p| !expanded.contains(p))
        .collect())
}

/// StorageSetting.ValidBagStorageIDs (also the BagStorage empty-slot order).
const VALID_BAGS: [i16; 9] = [0, 1, 6, 2, 3, 4, 5, 7, 8];
/// StorageSetting.DefaultBagStorageIDs.
const DEFAULT_BAGS: [i16; 3] = [0, 1, 6];
/// StorageType.Bag.
pub(super) const STORAGE_TYPE_BAG: i16 = 1;

/// ItemStorageStatus: StorageType, Storage, ExpireTime, StorageName, StorageTag.
type StatusRow = (i16, i16, Option<i64>, Vec<u8>, i32);
/// BagStorage.Load(ItemStorageStatus) rows: the bags opened by OpenStorage without an
/// ExpireTime, with their StorageName. Expiring or tagged storages (ExtendStorage,
/// UpdateStorageSetting) and other storage types are not migrated and refused.
async fn opened_bags(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<BTreeMap<i16, String>, MinimumError> {
    let rows: Vec<StatusRow> = sqlx::query_as(
        "SELECT storage_type,storage,expire_units300,storage_name_utf16,storage_tag FROM game_item_storage_status WHERE owner_id=$1 ORDER BY id LIMIT 17",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let mut out = BTreeMap::new();
    for (kind, storage, expire, name, tag) in rows {
        if kind != STORAGE_TYPE_BAG
            || !VALID_BAGS.contains(&storage)
            || DEFAULT_BAGS.contains(&storage)
            || expire.is_some()
            || tag != 0
        {
            return Err(MinimumError::Conflict);
        }
        let units: Vec<u16> = name
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .collect();
        let name = String::from_utf16(&units).map_err(|_| MinimumError::Conflict)?;
        if out.insert(storage, name).is_some() {
            return Err(MinimumError::Conflict);
        }
    }
    Ok(out)
}

/// The available bag collections (BagStorage.IsAvailable) in ValidBagStorageIDs order.
pub(super) async fn bags(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<i16>, MinimumError> {
    let opened = opened_bags(tx, cid).await?;
    Ok(VALID_BAGS
        .into_iter()
        .filter(|b| DEFAULT_BAGS.contains(b) || opened.contains_key(b))
        .collect())
}

/// ItemStorage.ToStorageInfo: BagStorage.ToStorageInfo in ValidBagStorageIDs order
/// (default bags and unlimited opened ones are available with LastSecsToExpire -1).
pub(super) async fn storage_infos(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<Option<Storage>>, MinimumError> {
    let opened = opened_bags(tx, cid).await?;
    Ok(VALID_BAGS
        .into_iter()
        .filter_map(|b| {
            let name = match opened.get(&b) {
                Some(n) => n.clone(),
                None if DEFAULT_BAGS.contains(&b) => String::new(),
                None => return None,
            };
            Some(Some(Storage {
                id: u8::try_from(b).ok()?,
                available: 1,
                last_seconds: -1,
                name: Some(name),
                tag: 0,
            }))
        })
        .collect())
}
