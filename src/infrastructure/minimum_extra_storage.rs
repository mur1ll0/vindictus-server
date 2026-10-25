//! UseExtraStorageProcessor for the selected first Fiona: a Storage Chest Expansion Ticket
//! opens one of the extra bags (2, 3, 4, 5, 7, 8). The permanent tickets OpenStorage
//! without ExpireTime; the timed ones ExtendStorage, whose expiring bags the inventory
//! does not migrate yet, so they are refused without changes. A refusal answers nothing
//! (FailMessage without a frontend message), like the source.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::map_partition::OutgoingPacket,
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message, UseExtraStorage},
    },
};
use sqlx::PgPool;

/// UseExtraStorageProcessor.Tickets with ExtendTime -1 (OpenStorage without expiry).
const PERMANENT_TICKETS: [&str; 3] = [
    "warehouse_ticket_free",
    "warehouse_ticket_free_gift",
    "warehouse_ticket_free_gift_accounttrade",
];
/// StorageTicket.StorageIDs of every ticket.
const TICKET_STORAGES: [u8; 6] = [2, 3, 4, 5, 7, 8];

pub enum Outcome {
    /// UpdateInventoryInfo (the ticket) then UpdateStorageInfo, as ItemOwner.Sync sends.
    Applied(Vec<OutgoingPacket>),
    /// FailMessage: no frontend message and nothing saved.
    Refused,
}

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

/// Admission (selection and UseSecondPassword) belongs to the frontend.
pub async fn use_extra_storage(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: UseExtraStorage,
) -> Result<Outcome, MinimumError> {
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let connected: bool =
        sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    if !connected {
        return Err(fail());
    }
    sqlx::query(
        "LOCK TABLE game_items,game_equippables,game_item_storage_status IN SHARE ROW EXCLUSIVE MODE",
    )
    .execute(&mut *tx)
    .await?;
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
        .cloned()
    else {
        return Ok(Outcome::Refused);
    };
    let class = item.class.clone().unwrap_or_default();
    // Tickets.TryGetValue and IsExpired; timed tickets need expiring bags (not migrated).
    if !PERMANENT_TICKETS.contains(&class.as_str()) || item.expireable {
        return Ok(Outcome::Refused);
    }
    // IExpirableStorage of the bag type, StorageIDs and IsUnlimited (an opened bag).
    if !TICKET_STORAGES.contains(&request.storage_id)
        || super::minimum_inventory::bags(&mut tx, cid)
            .await?
            .contains(&i16::from(request.storage_id))
    {
        return Ok(Outcome::Refused);
    }
    let deleted = item.count == 1;
    if deleted {
        sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
            .bind(item.id)
            .bind(cid)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE game_items SET count=count-1 WHERE id=$1 AND owner_id=$2 AND count>1")
            .bind(item.id)
            .bind(cid)
            .execute(&mut *tx)
            .await?;
    }
    // OpenStorage(StorageID, null): a new ItemStorageStatus row without ExpireTime.
    sqlx::query("INSERT INTO game_item_storage_status(id,owner_id,storage_type,storage,expire_units300,storage_name_utf16,storage_tag) VALUES(nextval('game_item_storage_id'),$1,$2,$3,NULL,'',0)")
        .bind(cid)
        .bind(super::minimum_inventory::STORAGE_TYPE_BAG)
        .bind(i16::from(request.storage_id))
        .execute(&mut *tx)
        .await?;
    let slot = if deleted {
        super::minimum_move::deleted_slot(item.id)
    } else {
        let after =
            super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
                .await?;
        after
            .slots
            .as_ref()
            .and_then(|s| s.iter().flatten().find(|s| s.id == item.id))
            .cloned()
            .ok_or_else(fail)?
    };
    let packets = vec![
        packet(
            Kind::UpdateInventoryInfo,
            Message::UpdateInventoryInfo(Some(vec![Some(slot)])),
        )?,
        packet(
            Kind::UpdateStorageInfo,
            Message::UpdateStorageInfo(Some(
                super::minimum_inventory::storage_infos(&mut tx, cid).await?,
            )),
        )?,
    ];
    tx.commit().await?;
    Ok(Outcome::Applied(packets))
}
