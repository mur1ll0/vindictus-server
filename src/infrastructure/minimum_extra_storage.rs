//! UseExtraStorageProcessor for the selected first Fiona: a Storage Chest Expansion Ticket
//! opens one of the extra bags (2, 3, 4, 5, 7, 8). The permanent tickets OpenStorage
//! without ExpireTime; the timed ones ExtendStorage (an unavailable bag opens until now
//! plus the time, an available expiring one adds the time). A refusal answers nothing
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

/// UseExtraStorageProcessor.Tickets: ItemClass → ExtendTime in seconds (-1 permanent).
const TICKETS: [(&str, i64); 20] = [
    ("warehouse_ticket_test", 300),
    ("warehouse_ticket_5", 432_000),
    ("warehouse_ticket_7", 604_800),
    ("warehouse_ticket_10", 864_000),
    ("warehouse_ticket_10_cash", 864_000),
    ("warehouse_ticket_15", 1_296_000),
    ("warehouse_ticket_30", 2_592_000),
    ("warehouse_ticket_60", 5_184_000),
    ("warehouse_ticket_90", 7_776_000),
    ("warehouse_ticket_100", 8_640_000),
    ("warehouse_ticket_180", 15_552_000),
    ("warehouse_ticket_180_gift", 15_552_000),
    ("warehouse_ticket_500", 43_200_000),
    ("warehouse_ticket_500_event", 43_200_000),
    ("warehouse_ticket_free", -1),
    ("warehouse_ticket_free_gift", -1),
    ("warehouse_ticket_30_gift", 2_592_000),
    ("warehouse_ticket_500_gift", 43_200_000),
    ("warehouse_ticket_180_gift_accounttrade", 15_552_000),
    ("warehouse_ticket_free_gift_accounttrade", -1),
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
    // Tickets.TryGetValue and IsExpired (a ticket with an expiry is not migrated).
    let Some(&(_, extend)) = TICKETS.iter().find(|(c, _)| *c == class) else {
        return Ok(Outcome::Refused);
    };
    if item.expireable {
        return Ok(Outcome::Refused);
    }
    // IExpirableStorage of the bag type, StorageIDs and IsUnlimited.
    let storage = i16::from(request.storage_id);
    let status = super::minimum_inventory::opened_bags(&mut tx, cid)
        .await?
        .remove(&storage);
    if !TICKET_STORAGES.contains(&request.storage_id)
        || status.as_ref().is_some_and(|s| s.expire_units300.is_none())
    {
        return Ok(Outcome::Refused);
    }
    let now = super::minimum_inventory::now_units300()?;
    // OpenStorage(id, null) / ExtendStorage(id, ExtendTime): IsAvailable opens from now,
    // an available expiring bag adds the time; both cap at 9999-12-01 23:59:59.
    let expire = if extend < 0 {
        None
    } else {
        let base = match status.as_ref().and_then(|s| s.expire_units300) {
            Some(e) if now <= e => e,
            _ => now,
        };
        Some(
            base.saturating_add(extend.saturating_mul(300))
                .min(super::minimum_inventory::MAX_BAG_EXPIRE_UNITS300),
        )
    };
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
    if status.is_some() {
        sqlx::query("UPDATE game_item_storage_status SET expire_units300=$3 WHERE owner_id=$1 AND storage_type=$2 AND storage=$4")
            .bind(cid)
            .bind(super::minimum_inventory::STORAGE_TYPE_BAG)
            .bind(expire)
            .bind(storage)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("INSERT INTO game_item_storage_status(id,owner_id,storage_type,storage,expire_units300,storage_name_utf16,storage_tag) VALUES(nextval('game_item_storage_id'),$1,$2,$3,$4,'',0)")
            .bind(cid)
            .bind(super::minimum_inventory::STORAGE_TYPE_BAG)
            .bind(storage)
            .bind(expire)
            .execute(&mut *tx)
            .await?;
    }
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
