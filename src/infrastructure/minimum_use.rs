//! UseInventoryItemProcessor for the selected first Fiona. Only the capsule_ap
//! branch (UseUseableItem + CharacterImpl.GiveAP) is supported; other usable item
//! types are rejected without changes until their branches are migrated.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::map_partition::OutgoingPacket,
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message, UseItem},
    },
};
use sqlx::PgPool;

pub enum Outcome {
    /// Frontend messages in emission order: UseInventoryItemOk, the GiveAP result
    /// (APMessage or ItemFail), then the queued inventory Sync (UpdateInventoryInfo).
    Applied(Vec<OutgoingPacket>),
    /// Source failure without a frontend message (unknown item ID).
    Unchanged,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// `capsule_ap` amount: the VALUE attribute (verified absent, so 0) and then
/// int.TryParse(ItemClass.Substring(10)); a failed parse yields 0.
fn capsule_amount(class: &str) -> Option<i32> {
    let suffix = class.strip_prefix("capsule_ap")?;
    Some(suffix.parse::<i32>().unwrap_or(0))
}

/// Admission (selection and UseSecondPassword) belongs to the frontend.
pub async fn use_item(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: UseItem,
    log: super::minimum_item_logs::Context,
) -> Result<Outcome, MinimumError> {
    // UseInventoryItemProcessor constructor: GetJournalID (UseInventoryItem).
    let journal = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let (outcome, used) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
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
        // Validates ownership/profile; attributes and expiration are verified absent,
        // so IsUsable (uptime passed, not expired, not broken) holds for non-equipment.
        let before =
            super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
                .await?;
        let Some(item) = before
            .slots
            .as_ref()
            .ok_or_else(fail)?
            .iter()
            .flatten()
            .find(|s| s.id == request.item_id)
        else {
            // GetItem == null: FailMessage only.
            return Ok((Outcome::Unchanged, None));
        };
        let class = item.class.clone().unwrap_or_default();
        let key: Vec<u8> = class.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let indexed = assets.stats_index.class(Some(&key)).ok_or_else(fail)?;
        let Some(amount) = capsule_amount(&class).filter(|_| !indexed.equippable) else {
            return Err(fail());
        };
        // Run: ItemLog.AddItemJournal("UseItem") once the item and BadStatus checks pass.
        let use_journal = super::minimum_item_logs::use_item_journal(pool, log.flags).await;
        // UseUseableItem: DestroyItem(Useable(item), 1) and Save before onSuccess.
        let deleted = item.count == 1;
        if deleted {
            sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
                .bind(item.id)
                .bind(cid)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("UPDATE game_items SET count=count-1 WHERE id=$1 AND owner_id=$2")
                .bind(item.id)
                .bind(cid)
                .execute(&mut *tx)
                .await?;
        }
        let mut packets = vec![OutgoingPacket {
            category: Kind::UseInventoryItemOk.category(),
            body: wire::encode(
                Kind::UseInventoryItemOk,
                Some(&Message::UseInventoryItemOk(Some(class.clone()))),
                DEFAULT_MAX_BODY,
            )
            .map_err(|_| fail())?
            .to_vec(),
        }];
        if amount == 0 {
            // The item is already consumed when the amount resolves to zero.
            packets.push(super::minimum_equip::item_fail(
                "UseItemFail_NotUseableItem",
            )?);
        } else {
            let ap = super::minimum_ap::give_exceeding(&mut tx, assets, cid, amount).await?;
            packets.push(super::minimum_ap::packet(ap)?);
        }
        let slot = if deleted {
            super::minimum_move::deleted_slot(item.id)
        } else {
            let after = super::minimum_inventory::snapshot_with_equipment(
                &mut tx, owner, assets, cid, None,
            )
            .await?;
            after
                .slots
                .as_ref()
                .and_then(|s| s.iter().flatten().find(|s| s.id == item.id))
                .cloned()
                .ok_or_else(fail)?
        };
        let update = Message::UpdateInventoryInfo(Some(vec![Some(slot)]));
        packets.push(OutgoingPacket {
            category: Kind::UpdateInventoryInfo.category(),
            body: wire::encode(Kind::UpdateInventoryInfo, Some(&update), DEFAULT_MAX_BODY)
                .map_err(|_| fail())?
                .to_vec(),
        });
        tx.commit().await?;
        let change = super::minimum_item_logs::NumChange {
            item_id: item.id,
            class_utf16: key.clone(),
            delta: -1,
        };
        Ok((Outcome::Applied(packets), Some((change, use_journal))))
    })
    .await
    .map_err(|_| fail())??;
    if let Some((change, use_journal)) = used {
        // Save → LogManager.Write (Num −1), then UseUseableItem's ledger.
        super::minimum_item_logs::write_changes(
            pool,
            cid,
            journal,
            super::minimum_item_logs::USE_INVENTORY_ITEM,
            std::slice::from_ref(&change),
            log,
        )
        .await;
        super::minimum_item_logs::write_use_ledger(pool, cid, use_journal, &change, 1, log.flags)
            .await;
    }
    Ok(outcome)
}
