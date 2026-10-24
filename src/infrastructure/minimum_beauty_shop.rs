//! BeautyShopCustomizeProcessor for the selected first Fiona, paid from the laboratory cash
//! balance (DirectPickUp of the CashShopService, whose NISMS billing is replaced by
//! PostgreSQL): each item must be a CustomizeItemInfo with a CustomizePriceInfo of its Duration
//! and the requested Price; the part of the item category is unequipped and the item is
//! given (ExpireDateTime = now + Duration days for a positive Duration). The frontend answers
//! ItemFailMessage "GameUI_Heroes_Success" or "GameUI_Heroes_BeautyShopFail". Coupons, free
//! pass coupons, the free paintings, look change, avatar packages, painting positions and
//! Tir coin payment are not migrated and answer the failure.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::{
    domain::{character_stats::Arithmetic, item_attribute, map_partition::OutgoingPacket},
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, BeautyShopCustomize, Kind, Message},
    },
};
use sqlx::PgPool;

/// BeautyShopCustomizeProcessor.FreeItems.
const FREE_ITEMS: [&str; 3] = ["uni_painting38", "uni_painting39", "uni_painting40"];
/// EventType.BeautyShopCustomize / ActionType.CashShop / ItemLogType.GiveItem.
const EVENT_BEAUTY_SHOP_CUSTOMIZE: i16 = 8;
const ACTION_CASH_SHOP: i16 = 25;
const LOG_GIVE: i16 = 2;
/// TimeSpan ticks of one day.
const DAY_TICKS: i64 = 864_000_000_000;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// The answer and, when the Gear changed, the refreshed look.
pub struct Customized {
    pub packets: Vec<OutgoingPacket>,
    pub look: Option<OutgoingPacket>,
}

/// OnFail: ItemFailMessage "GameUI_Heroes_BeautyShopFail".
fn refused() -> Result<Customized, MinimumError> {
    Ok(Customized {
        packets: vec![super::minimum_equip::item_fail("BeautyShopFail")?],
        look: None,
    })
}

struct Planned {
    class: String,
    category: String,
    colors: [i32; 3],
    duration: i32,
}

#[allow(clippy::too_many_arguments)]
pub async fn customize(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: &BeautyShopCustomize,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<Customized, MinimumError> {
    let contents = assets.cash_shop.as_ref().ok_or_else(fail)?;
    let customize = contents.customize();
    let profile = assets.equipment.as_ref().ok_or_else(fail)?;
    let items: Vec<_> = request.customize_items.iter().flatten().collect();
    // An empty request only updates the painting positions (UpdateBaseCostume).
    if items.is_empty() || items.len() > 16 || request.payment > 1 {
        return refused();
    }
    let mut planned = Vec::with_capacity(items.len());
    let mut total: i64 = 0;
    // A null CustomizeItemRequestInfo cannot be deserialized by the source either.
    if request
        .customize_items
        .iter()
        .flatten()
        .any(Option::is_none)
    {
        return refused();
    }
    for item in items.into_iter().flatten() {
        let (Some(class), Some(price)) = (item.item_class.as_deref(), item.price.as_deref()) else {
            return refused();
        };
        if FREE_ITEMS.contains(&class) || customize.coupons.contains_key(price) {
            return refused();
        }
        if !customize.items.contains_key(class) {
            return refused();
        }
        let key = utf16(class);
        let Some(indexed) = assets.stats_index.class(Some(&key)) else {
            return refused();
        };
        if !indexed.equippable {
            return refused();
        }
        let Some(selected) = customize
            .prices
            .get(class)
            .into_iter()
            .flatten()
            .find(|p| p.duration == item.duration && p.price.to_string() == price)
        else {
            return refused();
        };
        total += i64::from(selected.price);
        planned.push(Planned {
            class: class.to_owned(),
            category: profile.category(&assets.catalog, &key)?,
            colors: [item.color1, item.color2, item.color3],
            duration: item.duration,
        });
    }
    let total = i32::try_from(total).map_err(|_| fail())?;
    let parts = assets.appearance.normal_parts(&assets.catalog)?;
    let operation = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let journal =
        super::minimum_item_logs::named_journal(pool, log.flags, "BeautyShopCustomize").await;
    let mut ids = entity_ids.into_iter();
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
    // DirectPickUp: the NISMS purchase of the invoice (NotEnoughCash fails the operation).
    let debited: Option<i32> = sqlx::query_scalar(
        "UPDATE game_cash_balances SET balance=balance-$2 WHERE uid=$1 AND balance>=$2 RETURNING balance",
    )
    .bind(owner.uid)
    .bind(total)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(balance) = debited else {
        return refused();
    };
    // Unequip(GetEquippablePart(Category).FirstOrDefault()) for each item.
    let mut modified: Vec<i64> = Vec::new();
    for p in &planned {
        let Some(part) = parts
            .iter()
            .find(|x| !x.consumable && x.category == p.category)
            .map(|x| x.slot)
        else {
            continue;
        };
        let unequipped: Vec<i64> = sqlx::query_scalar("UPDATE game_equippables e SET part=NULL FROM game_items i WHERE i.id=e.id AND i.owner_id=$1 AND e.part=$2 RETURNING e.id")
            .bind(cid)
            .bind(i16::from(part))
            .fetch_all(&mut *tx)
            .await?;
        modified.extend(unequipped);
    }
    let keys: Vec<Vec<u8>> = planned.iter().map(|p| utf16(&p.class)).collect();
    let requests: Vec<_> = planned
        .iter()
        .zip(&keys)
        .map(|(p, key)| super::minimum_give::Request {
            class: key,
            count: 1,
            bound: false,
            equipment: (!p.colors.contains(&-1)).then_some(super::minimum_give::Equipment {
                colors: p.colors,
                reduced: 0,
                bonus: 0,
            }),
            attributes: &item_attribute::EMPTY,
        })
        .collect();
    let applied =
        match super::minimum_give::apply(&mut tx, owner, assets, cid, &[], &requests, &mut || {
            ids.next().ok_or_else(fail)
        })
        .await?
        {
            Ok(a) => a,
            Err(_) => return refused(),
        };
    // GiveItemRequest(customizeItem): ExpireDateTime = UtcNow.AddDays(Duration) when positive.
    let now = super::lab_clock::utc_now().map_err(|_| fail())?.ticks();
    let mut taken: Vec<i64> = Vec::new();
    for p in &planned {
        let key = utf16(&p.class);
        let created = applied
            .created
            .iter()
            .find(|c| c.class_utf16 == key && !taken.contains(&c.id))
            .ok_or_else(fail)?;
        taken.push(created.id);
        if p.duration > 0 {
            sqlx::query("UPDATE game_items SET expire_ticks=$2 WHERE id=$1")
                .bind(created.id)
                .bind(now + i64::from(p.duration) * DAY_TICKS)
                .execute(&mut *tx)
                .await?;
        }
    }
    modified.extend(taken);
    let gear =
        super::minimum_equip::refresh_gear(&mut tx, owner, assets, cid, config, arithmetic).await?;
    let inventory = super::minimum_inventory::snapshot_with_equipment(
        &mut tx,
        owner,
        assets,
        cid,
        Some(gear.equipment_info.clone()),
    )
    .await?;
    let mut slots = Vec::with_capacity(modified.len());
    for id in &modified {
        let slot = inventory
            .slots
            .as_ref()
            .and_then(|s| s.iter().flatten().find(|s| s.id == *id))
            .ok_or_else(fail)?;
        slots.push(Some(slot.clone()));
    }
    tx.commit().await?;
    super::minimum_item_logs::write_give(
        pool,
        cid,
        operation,
        EVENT_BEAUTY_SHOP_CUSTOMIZE,
        &applied,
        log,
    )
    .await;
    for key in &keys {
        super::minimum_item_logs::write_requested_ledger(
            pool,
            cid,
            journal,
            LOG_GIVE,
            ACTION_CASH_SHOP,
            key,
            1,
            log.flags,
        )
        .await;
    }
    let mut packets = vec![OutgoingPacket {
        category: Kind::UpdateInventoryInfo.category(),
        body: wire::encode(
            Kind::UpdateInventoryInfo,
            Some(&Message::UpdateInventoryInfo(Some(slots))),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    }];
    packets.extend(gear.packets);
    packets.push(OutgoingPacket {
        category: Kind::CashShopBalance.category(),
        body: wire::encode(
            Kind::CashShopBalance,
            Some(&Message::CashShopBalance(wire::CashShopBalance {
                balance,
                refundless: 0,
            })),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    });
    packets.push(super::minimum_equip::item_fail("Success")?);
    Ok(Customized {
        packets,
        look: Some(gear.look),
    })
}
