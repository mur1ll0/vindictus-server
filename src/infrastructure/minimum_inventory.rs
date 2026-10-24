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
    let marker:Option<(i32,Vec<u8>,bool)>=sqlx::query_as("SELECT uid,profile_sha256,initialized FROM game_minimum_character_initialization WHERE cid=$1 FOR SHARE").bind(cid).fetch_optional(&mut **tx).await?;
    if marker != Some((owner.uid, assets.initializer.fingerprint().to_vec(), true)) {
        return Err(MinimumError::Conflict);
    }
    super::item_loader_state::require_expanded_absent(tx, cid)
        .await
        .map_err(|_| MinimumError::Conflict)?;
    sqlx::query("LOCK TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status IN SHARE MODE").execute(&mut **tx).await?;
    let storage: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_item_storage_status WHERE owner_id=$1)",
    )
    .bind(cid)
    .fetch_one(&mut **tx)
    .await?;
    if storage {
        return Err(MinimumError::Conflict);
    }
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
    for row in rows {
        let bag = matches!(row.collection, 0 | 1 | 6);
        // AccountStorage: one item per class at AccountLocation (101, -1).
        let account = row.collection == 101;
        let story =
            story_tab && row.collection == i16::from(crate::domain::give_item::STORY_COLLECTION);
        if row.count <= 0
            || !(bag || account || story || row.collection == 100)
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
            != if bag {
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
        let (colors, durability, maximum, bonus) = match (equippable, equipment.remove(&row.id)) {
            (false, None) => (vec![-1; 3], 0, 0, 0),
            (true, Some(e)) => {
                if e.part.is_some() && assets.equipment.is_none() {
                    return Err(MinimumError::Conflict);
                }
                let base = stats
                    .get(Some(&row.item_class_utf16))
                    .ok_or(MinimumError::Conflict)?;
                let item = super::minimum_item_stat::equipment(
                    assets,
                    &row.item_class_utf16,
                    &attributes,
                )?;
                let d = EquipmentDurability::load(
                    base,
                    &item.stat,
                    e.reduced_durability,
                    e.max_durability_bonus,
                );
                (
                    vec![e.color1, e.color2, e.color3],
                    d.current(),
                    d.maximum(),
                    d.bonus(),
                )
            }
            _ => return Err(MinimumError::Conflict),
        };
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
            prefix: Dict(Some(Vec::new())),
            suffix: Dict(Some(Vec::new())),
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
    let locked = assets.appearance.inventory_locked_parts()?;
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
        storages: Some(
            [0, 1, 6]
                .into_iter()
                .map(|id| {
                    Some(Storage {
                        id,
                        available: 1,
                        last_seconds: -1,
                        name: Some(String::new()),
                        tag: 0,
                    })
                })
                .collect(),
        ),
        slots: Some(slots),
        equipment: Dict(Some(equipment_info)),
        quick_slots: Some(super::minimum_quickslot::info(&quick)),
        unequippable: Some(locked),
    };
    Ok(inventory)
}
