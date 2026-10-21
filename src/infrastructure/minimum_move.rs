//! MoveInventoryItemProcessor for the selected first Fiona: BagStorage.SwapSlot
//! between available bag collections, persisted and synchronized with
//! UpdateInventoryInfo, including Item.Stack merging that can delete the moved item.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::map_partition::OutgoingPacket,
    protocol::{
        DEFAULT_MAX_BODY,
        frontend_control::{
            self as ctl, HeroesText, Kind as ControlKind, Message as ControlMessage,
        },
        inventory::{self as wire, Dict, Kind, Message, Move, Slot},
    },
};
use sqlx::PgPool;

pub enum Outcome {
    /// UpdateInventoryInfo with the moved items in RegisterChange order.
    Applied(OutgoingPacket),
    /// ItemOwner.SendDialog("GameUI_Heroes_Inventory_MoveItemFail"); nothing changed.
    Refused(OutgoingPacket),
    /// No state change and no frontend message.
    Unchanged,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}
/// StorageSetting.DefaultBagStorageIDs; other bag IDs need a storage status row,
/// which the snapshot loader verifies absent.
const DEFAULT_BAG: [i32; 3] = [0, 1, 6];
/// StorageSetting.ValidBagStorageIDs.
const VALID_BAG: [i32; 9] = [0, 1, 6, 2, 3, 4, 5, 7, 8];

#[derive(PartialEq, Eq)]
enum StorageType {
    Bag,
    Other(i32),
}
fn storage_type(collection: i32) -> StorageType {
    if VALID_BAG.contains(&collection) {
        StorageType::Bag
    } else {
        StorageType::Other(collection)
    }
}

pub fn move_fail() -> Result<OutgoingPacket, MinimumError> {
    let body = ctl::encode(
        ControlKind::System,
        Some(&ControlMessage::System {
            category: 4,
            text: Some(HeroesText {
                format: Some("GameUI_Heroes_Inventory_MoveItemFail"),
                parameters: Some(Vec::new()),
            }),
        }),
        DEFAULT_MAX_BODY,
    )
    .map_err(|_| fail())?;
    Ok(OutgoingPacket {
        category: ControlKind::System.category(),
        body,
    })
}

/// ItemTraitExtension.IsStackableWith for items whose attributes and expiration
/// are verified absent by the snapshot loader.
fn stackable(assets: &Assets, a: &Slot, b: &Slot) -> Result<Option<i32>, MinimumError> {
    let key = |s: &Slot| -> Vec<u8> {
        s.class
            .as_deref()
            .unwrap_or("")
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect()
    };
    let (ka, kb) = (key(a), key(b));
    let ca = assets.stats_index.class(Some(&ka)).ok_or_else(fail)?;
    let cb = assets.stats_index.class(Some(&kb)).ok_or_else(fail)?;
    if ca.equippable || cb.equippable || ka != kb {
        return Ok(None);
    }
    // Account storage (3) ignores character binding.
    if ca.storage != 3 && cb.storage != 3 && a.tradable != b.tradable {
        return Ok(None);
    }
    Ok(Some(ca.max_stack))
}

/// Item.ToSlotInfo for Num == 0: the 13-argument SlotInfo constructor with an empty
/// class, zero location, no expiration, no attributes and default colors.
pub(super) fn deleted_slot(id: i64) -> Slot {
    Slot {
        id,
        class: Some(String::new()),
        count: 0,
        tab: 0,
        slot: 0,
        expireable: false,
        expire_diff: -1,
        expire_ticks: -1,
        colors: Some(vec![-1; 3]),
        durability: 0,
        maximum: 0,
        bonus: 0,
        tradable: false,
        // Same GetExpirationDateExpander value the snapshot uses for these items.
        expander: Some(String::new()),
        attributes: Some(Vec::new()),
        prefix: Dict(Some(Vec::new())),
        suffix: Dict(Some(Vec::new())),
        min_price: 0,
        max_price: 0,
        avg_price: 0,
        min_up_diff: 0,
    }
}
/// Admission (selection and second password) belongs to the frontend.
pub async fn move_item(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: Move,
    log: super::minimum_item_logs::Context,
) -> Result<Outcome, MinimumError> {
    // MoveInventoryItemProcessor constructor: GetJournalID (MoveInventoryItem).
    let journal = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let (outcome, changes) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let connected: bool =
            sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?;
        if !connected {
            return Err(fail());
        }
        sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        // Validates marker/profile/ownership and refuses unsupported item states.
        let before =
            super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
                .await?;
        let slots: Vec<&Slot> = before
            .slots
            .as_ref()
            .ok_or_else(fail)?
            .iter()
            .flatten()
            .collect();
        let Some(item) = slots.iter().find(|s| s.id == request.item_id).copied() else {
            return Ok((Outcome::Refused(move_fail()?), Vec::new()));
        };
        let to_collection = i32::from(request.storage);
        let from = storage_type(item.tab);
        if from != storage_type(to_collection) {
            return Ok((Outcome::Refused(move_fail()?), Vec::new()));
        }
        if from != StorageType::Bag {
            // CashStorage swap rules are not part of this slice.
            return Err(fail());
        }
        // StorageSetting.IsValid(Bag): 0 <= slot < 48. The source then loads the moved
        // item into an invalid slot and still saves; refusing avoids persisting that.
        if !(0..48).contains(&request.slot) {
            return Ok((Outcome::Refused(move_fail()?), Vec::new()));
        }
        let occupant = slots
            .iter()
            .find(|s| s.tab == to_collection && s.slot == request.slot)
            .copied();
        // SwapSlot availability: the moved item needs an available target and the
        // occupant an available source.
        if !DEFAULT_BAG.contains(&to_collection)
            || (occupant.is_some() && !DEFAULT_BAG.contains(&item.tab))
        {
            return Ok((Outcome::Refused(move_fail()?), Vec::new()));
        }
        if occupant.is_some_and(|o| o.id == item.id) {
            // Same location: the source would stack an item onto itself.
            return Ok((Outcome::Unchanged, Vec::new()));
        }
        // SwapSlot: `occupant.Stack(item)` first; a full or unrelated occupant swaps.
        let mut changed: Vec<i64> = Vec::new();
        let mut deleted = None;
        let mut log_changes = Vec::new();
        let stack = match occupant {
            Some(o) => stackable(assets, o, item)?
                .filter(|max| o.count < *max)
                .map(|max| (o, max)),
            None => None,
        };
        if let Some((o, max_stack)) = stack {
            let class: Vec<u8> = item
                .class
                .as_deref()
                .unwrap_or("")
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            if assets.inventory.is_gold(&class) {
                // Gold carries a count-dependent checksum; not in this slice.
                return Err(fail());
            }
            let Some((occupant_count, moving_count)) =
                crate::domain::item_stack::stack(o.count, item.count, max_stack)
            else {
                return Err(fail());
            };
            let amount = occupant_count - o.count;
            // Item.Num setters register the occupant first, then the moved item.
            sqlx::query("UPDATE game_items SET count=count+$2 WHERE id=$1 AND owner_id=$3")
                .bind(o.id)
                .bind(amount)
                .bind(cid)
                .execute(&mut *tx)
                .await?;
            changed.push(o.id);
            // Item.Num setters log the occupant's increase, then the moved decrease.
            log_changes.push(super::minimum_item_logs::NumChange {
                item_id: o.id,
                class_utf16: class.clone(),
                delta: amount,
            });
            log_changes.push(super::minimum_item_logs::NumChange {
                item_id: item.id,
                class_utf16: class.clone(),
                delta: -amount,
            });
            if moving_count == 0 {
                // Num = 0: RegisterDelete removes it from storage and destroys the row.
                sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
                    .bind(item.id)
                    .bind(cid)
                    .execute(&mut *tx)
                    .await?;
                deleted = Some(item.id);
            } else {
                sqlx::query("UPDATE game_items SET count=count-$2 WHERE id=$1 AND owner_id=$3")
                    .bind(item.id)
                    .bind(amount)
                    .bind(cid)
                    .execute(&mut *tx)
                    .await?;
            }
            changed.push(item.id);
        } else {
            let mut moved = vec![(item.id, to_collection, request.slot)];
            if let Some(o) = occupant {
                moved.push((o.id, item.tab, item.slot));
            }
            for (id, collection, slot) in &moved {
                let n = sqlx::query(
                    "UPDATE game_items SET collection=$2,slot=$3 WHERE id=$1 AND owner_id=$4",
                )
                .bind(id)
                .bind(i16::try_from(*collection).map_err(|_| fail())?)
                .bind(slot)
                .bind(cid)
                .execute(&mut *tx)
                .await?
                .rows_affected();
                if n != 1 {
                    return Err(fail());
                }
                changed.push(*id);
            }
        }
        let after =
            super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
                .await?;
        let updated = changed
            .iter()
            .map(|id| {
                if deleted == Some(*id) {
                    return Ok(Some(deleted_slot(*id)));
                }
                after
                    .slots
                    .as_ref()
                    .and_then(|s| s.iter().flatten().find(|s| s.id == *id))
                    .cloned()
                    .map(Some)
                    .ok_or_else(fail)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let message = Message::UpdateInventoryInfo(Some(updated));
        let packet = OutgoingPacket {
            category: Kind::UpdateInventoryInfo.category(),
            body: wire::encode(Kind::UpdateInventoryInfo, Some(&message), DEFAULT_MAX_BODY)
                .map_err(|_| fail())?
                .to_vec(),
        };
        tx.commit().await?;
        Ok((Outcome::Applied(packet), log_changes))
    })
    .await
    .map_err(|_| fail())??;
    super::minimum_item_logs::write_changes(
        pool,
        cid,
        journal,
        super::minimum_item_logs::MOVE_INVENTORY_ITEM,
        &changes,
        log,
    )
    .await;
    Ok(outcome)
}
