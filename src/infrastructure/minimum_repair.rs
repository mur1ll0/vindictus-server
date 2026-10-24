//! ItemImpl.RepairItem / QueryRepairPrice for the selected first Fiona: the gold of every
//! RepairableItemSelector item is destroyed and their durability restored in one
//! transaction, then Save's Sync. Gear.ApplyBadStatus has nothing to clear while
//! broken-equipment status effects are not migrated.
use super::{
    character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError,
    minimum_give::Request,
};
use crate::{domain::repair, protocol::inventory::Slot};
use sqlx::{Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// RepairItem selector flags (RepairItemMessage or the NPC do_repair command).
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection<'a> {
    pub item_ids: &'a [i64],
    pub all_equipped: bool,
    pub all_broken: bool,
    pub all_repairable: bool,
}

/// Repairable items with their RepairPrice, in inventory order.
async fn repairable(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    selection: Selection<'_>,
) -> Result<Vec<(Slot, i32)>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let slots = super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
        .await?
        .slots
        .ok_or_else(fail)?;
    let parts: Vec<(i64, Option<i16>)> = sqlx::query_as(
        "SELECT e.id,e.part FROM game_equippables e JOIN game_items i ON i.id=e.id WHERE i.owner_id=$1",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let mut out = Vec::new();
    for slot in slots.into_iter().flatten() {
        // IsRepairable: MaxDurability - Durability > 0.
        if slot.maximum.wrapping_sub(slot.durability) <= 0 {
            continue;
        }
        let equipped = parts.iter().any(|(id, p)| *id == slot.id && p.is_some());
        let class_key: Vec<u8> = slot
            .class
            .as_deref()
            .unwrap_or_default()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        // IsBroken: HasDurability (EquipItemInfo.MaxDurability > 0), MaxDurability >= 0
        // and Durability == 0.
        let has_durability = assets
            .stats_index
            .class(Some(&class_key))
            .is_some_and(|c| c.equippable)
            && assets
                .stats_index
                .get(Some(&class_key))
                .is_some_and(|s| s.fields()[0] > 0);
        let broken = has_durability && slot.maximum >= 0 && slot.durability == 0;
        if !(selection.item_ids.contains(&slot.id)
            || (selection.all_equipped && equipped)
            || (selection.all_broken && broken)
            || selection.all_repairable)
        {
            continue;
        }
        let class = slot.class.clone().unwrap_or_default();
        let key: Vec<u8> = class.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let indexed = assets.stats_index.class(Some(&key)).ok_or_else(fail)?;
        let price = economy.get(indexed.source_index).ok_or_else(fail)?.price;
        let equip_class = assets.equip_class.get(&key).cloned().flatten();
        let cost = repair::repair_price(
            &npc.repair,
            slot.durability,
            slot.maximum,
            price,
            equip_class.as_deref(),
            {
                let (prefix, suffix) = super::minimum_item_stat::slot_enchant_levels(assets, &slot);
                (
                    super::minimum_item_stat::slot_enhance_level(&slot),
                    super::minimum_item_stat::slot_quality_level(&slot),
                    prefix,
                    suffix,
                )
            },
        );
        out.push((slot, cost));
    }
    Ok(out)
}

/// QueryRepairPrice(null, addAllRepairableItems: true).
pub(super) async fn price_of_all(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<i32, MinimumError> {
    let items = repairable(
        tx,
        owner,
        assets,
        cid,
        Selection {
            all_repairable: true,
            ..Default::default()
        },
    )
    .await?;
    Ok(items.iter().fold(0i32, |sum, (_, c)| sum.wrapping_add(*c)))
}

pub struct Repaired {
    pub slots: Vec<Option<Slot>>,
    pub gold: Vec<u8>,
    pub price: i32,
    pub applied: super::minimum_give::Applied,
}

/// RepairItem inside the caller's transaction. `Ok(None)`: the gold could not be paid.
pub(super) async fn repair(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    selection: Selection<'_>,
) -> Result<Option<Repaired>, MinimumError> {
    sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await?;
    let items = repairable(tx, owner, assets, cid, selection).await?;
    let price = items.iter().fold(0i32, |sum, (_, c)| sum.wrapping_add(*c));
    let gold = assets.inventory.gold_class().to_vec();
    let applied = super::minimum_give::apply(
        tx,
        owner,
        assets,
        cid,
        &[Request {
            class: &gold,
            count: price,
            bound: false,
            equipment: None,
            attributes: &crate::domain::item_attribute::EMPTY,
        }],
        &[],
        &mut || Err(fail()),
    )
    .await?;
    let Ok(applied) = applied else {
        return Ok(None);
    };
    for (slot, _) in &items {
        // Item.Repair: Durability = MaxDurability (no reduction left).
        sqlx::query("UPDATE game_equippables SET reduced_durability=0 WHERE id=$1")
            .bind(slot.id)
            .execute(&mut **tx)
            .await?;
    }
    let after = super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
        .await?
        .slots
        .ok_or_else(fail)?;
    let mut slots = applied.slots.clone();
    for (slot, _) in &items {
        slots.push(after.iter().flatten().find(|s| s.id == slot.id).cloned());
    }
    Ok(Some(Repaired {
        slots,
        gold,
        price,
        applied,
    }))
}

/// EventType.RepairItem and ActionType.Repair.
const EVENT_REPAIR_ITEM: i16 = 85;
const ACTION_REPAIR: i16 = 15;

/// RepairItemProcessor (RepairItemMessage, HideNotEnoughGold: true): the not-enough-gold
/// dialog or the repaired items' Sync.
pub async fn repair_item(
    pool: &sqlx::PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    selection: Selection<'_>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<crate::domain::map_partition::OutgoingPacket>, MinimumError> {
    use crate::protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message},
    };
    let flags = log.flags;
    let operation = super::minimum_item_logs::operation_journal(pool, flags).await;
    let journal = super::minimum_item_logs::named_journal(pool, flags, "RepairItem").await;
    let repaired = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let connected: bool =
            sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?;
        if !connected {
            return Err(fail());
        }
        let repaired = repair(&mut tx, owner, assets, cid, selection).await?;
        if repaired.is_some() {
            tx.commit().await?;
        }
        Ok::<_, MinimumError>(repaired)
    })
    .await
    .map_err(|_| fail())??;
    let Some(repaired) = repaired else {
        return Ok(vec![super::minimum_shop::dialog(
            "GameUI_Heroes_RepairFail_NotEnoughGold",
        )?]);
    };
    super::minimum_item_logs::write_give(
        pool,
        cid,
        operation,
        EVENT_REPAIR_ITEM,
        &repaired.applied,
        log,
    )
    .await;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        3,
        ACTION_REPAIR,
        &repaired.gold,
        repaired.price,
        flags,
    )
    .await;
    Ok(vec![crate::domain::map_partition::OutgoingPacket {
        category: Kind::UpdateInventoryInfo.category(),
        body: wire::encode(
            Kind::UpdateInventoryInfo,
            Some(&Message::UpdateInventoryInfo(Some(repaired.slots))),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    }])
}
