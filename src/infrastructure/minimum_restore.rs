//! RestoreStorage of the selected first Fiona: QueryBuybackList(Restoreitem) →
//! RestoreItemListMessage (ToSlotInfo(ignoreRestoreStorage: false), price "AP" from
//! CalcRequiredBuybackAP) and the BuyItemProcessor "Buyback_byAP" branch: UseAP, then
//! BringFromBuybackStorage (GiveItem of the item's request clone, the restore item
//! destroyed, ArrangeSlot) with BuyItemResult and the new list.
use super::{
    character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError,
    minimum_inventory::RESTORE_COLLECTION,
};
use crate::{
    domain::{
        character_stats::Arithmetic, give_item::GiveError, item_attribute,
        map_partition::OutgoingPacket, shop,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message, RestoreItemInfo, Slot},
    },
};
use sqlx::{PgPool, Postgres, Transaction};

/// ActionType.RestoreItem / EventType.BuyItem.
const ACTION_RESTORE_ITEM: i16 = 64;
const EVENT_BUY_ITEM: i16 = 12;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// A restore item: its full SlotInfo and CalcRequiredBuybackAP.
struct Entry {
    slot: Slot,
    ap: i32,
}

async fn entries(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Vec<Entry>, MinimumError> {
    let mut slots: Vec<Slot> =
        super::minimum_inventory::restore_slots(tx, owner, assets, cid).await?;
    // RestoreStorage.ItemList is ordered by slot.
    slots.sort_by_key(|s| s.slot);
    let mut out = Vec::with_capacity(slots.len());
    for slot in slots {
        let class: Vec<u8> = slot
            .class
            .as_deref()
            .unwrap_or("")
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let attributes = super::minimum_inventory::load_attributes(tx, &[slot.id])
            .await?
            .remove(&slot.id)
            .unwrap_or_default();
        let ap = super::minimum_enhance::buyback_ap_of(assets, &class, &attributes)?;
        out.push(Entry { slot, ap });
    }
    Ok(out)
}

fn list_packet(entries: Vec<Entry>) -> Result<OutgoingPacket, MinimumError> {
    packet(
        Kind::RestoreItemList,
        Message::RestoreItemList(Some(
            entries
                .into_iter()
                .map(|e| {
                    Some(RestoreItemInfo {
                        slot: Some(e.slot),
                        price_type: Some("AP".to_owned()),
                        price_value: e.ap,
                    })
                })
                .collect(),
        )),
    )
}

/// QueryRestoreItemListMessage after the second password.
pub async fn list(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let entries = entries(&mut tx, owner, assets, cid).await?;
    tx.commit().await?;
    list_packet(entries)
}

fn result(r: i32, class: String, count: i32, ap: i32) -> Result<OutgoingPacket, MinimumError> {
    packet(
        Kind::BuyItemResult,
        Message::BuyItemResult(wire::BuyItemResult {
            result: r,
            item_class: Some(class),
            item_count: count,
            price_item_class: Some("ap".to_owned()),
            price_item_count: ap,
            restriction_order: -1,
            restriction_count: -1,
        }),
    )
}

/// BuyItemMessage("Buyback_byAP", Order, Count) with RestoreItem enabled.
#[allow(clippy::too_many_arguments)]
pub async fn restore(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    order: i32,
    count: i32,
    entity_ids: Vec<i64>,
    arithmetic: Arithmetic,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let mut ids = entity_ids.into_iter();
    let operation = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let journal = super::minimum_item_logs::named_journal(pool, log.flags, "RestoreItem").await;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let list = entries(&mut tx, owner, assets, cid).await?;
    // restoreItems[Order]: an index outside the list fails without a message.
    let Some(selected) = usize::try_from(order).ok().and_then(|i| list.get(i)) else {
        return Err(fail());
    };
    let id = selected.slot.id;
    let ap = selected.ap;
    let row: (Vec<u8>, i32, bool, Option<i64>) = sqlx::query_as(
        "SELECT item_class_utf16,count,tradable,expire_ticks FROM game_items WHERE id=$1 AND owner_id=$2 AND collection=$3",
    )
    .bind(id)
    .bind(cid)
    .bind(RESTORE_COLLECTION)
    .fetch_one(&mut *tx)
    .await?;
    let attributes = super::minimum_inventory::load_attributes(&mut tx, &[id])
        .await?
        .remove(&id)
        .unwrap_or_default();
    let class_text = selected.slot.class.clone().unwrap_or_default();
    let class_ex = item_attribute::build_class_ex(&class_text, &attributes).map_err(|_| fail())?;
    // UseAP, PushAPMessage and SubmitChanges come before BringFromBuybackStorage.
    let Some(ap_state) =
        super::minimum_ap::use_ap(&mut tx, owner, assets, cid, arithmetic, ap).await?
    else {
        tx.commit().await?;
        return Ok(vec![result(shop::BUY_FAILED, class_ex, count, ap)?]);
    };
    let mut packets = vec![super::minimum_ap::packet(ap_state)?];
    // GiveItem(item.ToRequestInfo()): an ExpireDateTime request is not migrated.
    if row.3.is_some() {
        return Err(fail());
    }
    let equipment: Option<(i32, i32, i32, i32, i32)> = sqlx::query_as(
        "SELECT color1,color2,color3,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let request = super::minimum_give::Request {
        class: &row.0,
        count: row.1,
        bound: !row.2,
        equipment: equipment.map(|e| super::minimum_give::Equipment {
            colors: [e.0, e.1, e.2],
            reduced: e.3,
            bonus: e.4,
        }),
        attributes: &attributes,
    };
    let mut next = || ids.next().ok_or_else(fail);
    let applied = match super::minimum_give::apply(
        &mut tx,
        owner,
        assets,
        cid,
        &[],
        std::slice::from_ref(&request),
        &mut next,
    )
    .await?
    {
        Ok(a) => a,
        // NoEmptySlot/UniqueConstraint mail the request in the source (not migrated).
        Err(GiveError::NoEmptySlot | GiveError::UniqueConstraint) => return Err(fail()),
        Err(_) => {
            tx.commit().await?;
            packets.push(result(shop::BUY_FAILED, class_ex, count, ap)?);
            return Ok(packets);
        }
    };
    super::minimum_give::delete_attributes(&mut tx, id).await?;
    sqlx::query("DELETE FROM game_equippables WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(cid)
        .execute(&mut *tx)
        .await?;
    // ArrangeSlot(null).
    let remaining: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM game_items WHERE owner_id=$1 AND collection=$2 ORDER BY slot,id",
    )
    .bind(cid)
    .bind(RESTORE_COLLECTION)
    .fetch_all(&mut *tx)
    .await?;
    for (n, item) in remaining.iter().enumerate() {
        sqlx::query("UPDATE game_items SET slot=$2 WHERE id=$1")
            .bind(item)
            .bind(i32::try_from(n).map_err(|_| fail())?)
            .execute(&mut *tx)
            .await?;
    }
    let after = entries(&mut tx, owner, assets, cid).await?;
    tx.commit().await?;
    super::minimum_item_logs::write_give(pool, cid, operation, EVENT_BUY_ITEM, &applied, log).await;
    for kind in [3_i16, 2] {
        super::minimum_item_logs::write_requested_ledger(
            pool,
            cid,
            journal,
            kind,
            ACTION_RESTORE_ITEM,
            &row.0,
            row.1,
            log.flags,
        )
        .await;
    }
    let mut slots = applied.slots;
    slots.push(Some(super::minimum_move::deleted_slot(id)));
    packets.push(packet(
        Kind::UpdateInventoryInfo,
        Message::UpdateInventoryInfo(Some(slots)),
    )?);
    packets.push(result(shop::BUY_SUCCEEDED, class_ex, count, ap)?);
    packets.push(list_packet(after)?);
    Ok(packets)
}
