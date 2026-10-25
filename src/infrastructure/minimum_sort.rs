//! SortInventoryProcessor for the selected first Fiona: ItemOwner.SortBagInventory (one
//! bag, or every available bag with -1: items whose InventoryFilterType matches a bag's
//! StorageTag first, then the rest in ValidBagStorageIDs order), SortCashInventory (100)
//! and SortStoryInventory (103), ordered by ItemOwner.ItemComparer, then ArrangeSlot and
//! the inventory Sync. The 30-second cooldown belongs to the frontend session. Strings
//! compare ordinally (the source uses culture comparison; the content keys are ASCII).
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::map_partition::OutgoingPacket,
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message},
    },
};
use sqlx::PgPool;
use std::cmp::Ordering;

/// StorageSetting.ValidBagStorageIDs.
const VALID_BAGS: [i16; 9] = [0, 1, 6, 2, 3, 4, 5, 7, 8];
const CASH: i32 = 100;
const STORY: i32 = 103;
const SLOTS: i32 = 48;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// The ItemComparer keys of one item.
struct Key {
    id: i64,
    collection: i16,
    slot: i32,
    equipped: bool,
    sort_order: String,
    set_id: Option<String>,
    category: Vec<u8>,
    level: i32,
    price: i32,
    class: String,
    filter: i32,
}

/// ItemContentsExtension.GetInventoryFilterType: Enum.Parse(InventoryFilterTypes), else 0.
fn filter_type(name: Option<&str>) -> i32 {
    match name.map(str::trim) {
        Some("NONE") => 0,
        Some("EQUIPMENT") => 1,
        Some("GOODS") => 2,
        Some("MATERIAL") => 3,
        Some("STORY") => 4,
        Some("ETC") => 5,
        Some(v) => v
            .parse::<i32>()
            .ok()
            .filter(|n| (0..=5).contains(n))
            .unwrap_or(0),
        None => 0,
    }
}

/// ItemOwner.ItemComparer.
fn compare(p: &Key, q: &Key) -> Ordering {
    if p.equipped != q.equipped {
        return if p.equipped {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    let order = |k: &Key| {
        if k.set_id.is_some() {
            "200.00".to_owned()
        } else {
            k.sort_order.clone()
        }
    };
    let (a, b) = (order(p), order(q));
    if a != b {
        return a.cmp(&b);
    }
    match (&p.set_id, &q.set_id) {
        (Some(s), Some(t)) => {
            if s == t {
                return p.category.cmp(&q.category);
            }
            if p.level != q.level {
                return q.level.cmp(&p.level);
            }
            s.cmp(t)
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => {
            if p.level != q.level {
                return q.level.cmp(&p.level);
            }
            if p.price != q.price {
                return q.price.cmp(&p.price);
            }
            p.class.cmp(&q.class)
        }
    }
}

/// SortInventory(TargetInventory): Ok(None) when the source fails without a frontend
/// message (an unavailable bag, the story tab without StoryTabInInventory).
pub async fn sort(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    target: i32,
) -> Result<Option<Vec<OutgoingPacket>>, MinimumError> {
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let before =
        super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
            .await?;
    let available = super::minimum_inventory::bags(&mut tx, cid).await?;
    let story_tab = assets
        .catalog
        .policy
        .is_enabled(Some("StoryTabInInventory"))
        .map_err(|_| fail())?;
    let bag_sort = target != CASH && target != STORY;
    if (bag_sort && target != -1 && !i16::try_from(target).is_ok_and(|t| available.contains(&t)))
        || (target == STORY && !story_tab)
    {
        return Ok(None);
    }
    let equipped: Vec<i64> = sqlx::query_scalar(
        "SELECT e.id FROM game_equippables e JOIN game_items i ON i.id=e.id WHERE i.owner_id=$1 AND e.part IS NOT NULL",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    let mut keys = Vec::new();
    for s in before.slots.as_ref().ok_or_else(fail)?.iter().flatten() {
        let collection = i16::try_from(s.tab).map_err(|_| fail())?;
        let selected = if bag_sort {
            available.contains(&collection) && (target == -1 || s.tab == target)
        } else {
            s.tab == target
        };
        if !selected {
            continue;
        }
        let class = s.class.clone().unwrap_or_default();
        let utf16: Vec<u8> = class.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let indexed = assets.stats_index.class(Some(&utf16)).ok_or_else(fail)?;
        let info = economy.get(indexed.source_index).ok_or_else(fail)?;
        keys.push(Key {
            id: s.id,
            collection,
            slot: s.slot,
            equipped: equipped.contains(&s.id),
            sort_order: info.sort_order.clone().unwrap_or_default(),
            // Item.IsSetItem: an equippable class with a SetInfo.
            set_id: info.set_id.clone().filter(|_| indexed.equippable),
            category: info
                .category
                .as_ref()
                .map(|c| c.as_bytes().to_vec())
                .unwrap_or_default(),
            level: info.level,
            price: info.price,
            filter: filter_type(info.inventory_filter.as_deref()),
            class,
        });
    }
    keys.sort_by(compare);
    // ArrangeSlot map: item → (collection, slot).
    let mut placed: Vec<(i64, i16, i32)> = Vec::with_capacity(keys.len());
    if bag_sort && target == -1 {
        // Every bag's StorageTag is 0 (UpdateStorageSetting tags are not migrated).
        let mut used: std::collections::BTreeMap<i16, i32> = std::collections::BTreeMap::new();
        let mut done = std::collections::BTreeSet::new();
        for k in &keys {
            for bag in VALID_BAGS.into_iter().filter(|b| available.contains(b)) {
                if k.filter != 0 {
                    break;
                }
                let n = used.entry(bag).or_default();
                if *n != SLOTS {
                    placed.push((k.id, bag, *n));
                    *n += 1;
                    done.insert(k.id);
                    break;
                }
            }
        }
        for k in keys.iter().filter(|k| !done.contains(&k.id)) {
            let bag = VALID_BAGS
                .into_iter()
                .filter(|b| available.contains(b))
                .find(|b| used.get(b).is_none_or(|n| *n != SLOTS))
                .ok_or_else(fail)?;
            let n = used.entry(bag).or_default();
            placed.push((k.id, bag, *n));
            *n += 1;
        }
    } else {
        let collection = i16::try_from(target).map_err(|_| fail())?;
        for (n, k) in keys.iter().enumerate() {
            placed.push((k.id, collection, i32::try_from(n).map_err(|_| fail())?));
        }
    }
    let moved: Vec<(i64, i16, i32)> = placed
        .into_iter()
        .filter(|(id, c, s)| {
            keys.iter()
                .find(|k| k.id == *id)
                .is_some_and(|k| (k.collection, k.slot) != (*c, *s))
        })
        .collect();
    for (id, collection, slot) in &moved {
        sqlx::query("UPDATE game_items SET collection=$3,slot=$4 WHERE id=$1 AND owner_id=$2")
            .bind(id)
            .bind(cid)
            .bind(collection)
            .bind(slot)
            .execute(&mut *tx)
            .await?;
    }
    let after =
        super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
            .await?;
    tx.commit().await?;
    if moved.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let slots: Vec<Option<wire::Slot>> = after
        .slots
        .as_ref()
        .ok_or_else(fail)?
        .iter()
        .flatten()
        .filter(|s| moved.iter().any(|(id, _, _)| *id == s.id))
        .cloned()
        .map(Some)
        .collect();
    let update = Message::UpdateInventoryInfo(Some(slots));
    Ok(Some(vec![OutgoingPacket {
        category: Kind::UpdateInventoryInfo.category(),
        body: wire::encode(Kind::UpdateInventoryInfo, Some(&update), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    }]))
}
