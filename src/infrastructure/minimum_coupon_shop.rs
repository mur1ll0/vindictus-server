//! Coupon shops for the entered first Fiona: QueryCouponShopItemInfoProcessor (the
//! restriction state of each restricted item of a shop version) and
//! BuyCouponShopItemProcessor (feature checks, IsAvailableFor, ShopBuyRestrictSet, then
//! CraftItem: destroy the version's price item and give the reward). The restriction write
//! and the trade share one transaction, so a CraftFail also undoes the restriction
//! (TryShopBuyRestrictRollback).
use super::{
    character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError,
    minimum_give::Request,
};
use crate::domain::{
    coupon_shop::{self as coupon, BuyPlan, Item},
    give_item::GiveError,
    map_partition::OutgoingPacket,
    shop::{BUY_FAILED, BUY_FAILED_UNIQUE, BuyResult},
};
use crate::protocol::{
    DEFAULT_MAX_BODY,
    coupon_shop::{self as cwire, ItemInfoResult, Restriction},
    inventory::{self as wire, BuyItemResult, Kind, Message},
};
use sqlx::{PgPool, Postgres, Transaction};

/// EventType.BuyCouponShopItem and ActionType.CouponShop of the item log.
const EVENT_BUY_COUPON_SHOP_ITEM: i16 = 11;
const ACTION_COUPON_SHOP: i16 = 144;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn result_packet(r: BuyResult) -> Result<OutgoingPacket, MinimumError> {
    let message = Message::BuyItemResult(BuyItemResult {
        result: r.result,
        item_class: Some(r.item_class),
        item_count: r.item_count,
        price_item_class: Some(r.price_item_class),
        price_item_count: r.price_count,
        restriction_order: r.restriction_order,
        restriction_count: r.restriction_count,
    });
    Ok(OutgoingPacket {
        category: Kind::BuyItemResult.category(),
        body: wire::encode(Kind::BuyItemResult, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
fn info_packet(
    shop_version: i16,
    rows: Vec<(i16, Restriction)>,
) -> Result<OutgoingPacket, MinimumError> {
    let message = cwire::Message::ItemInfoResult(ItemInfoResult {
        shop_version,
        restrictions: Some(rows.into_iter().map(|(o, r)| (o, Some(r))).collect()),
    });
    Ok(OutgoingPacket {
        category: cwire::Kind::ItemInfoResult.category(),
        body: cwire::encode(
            cwire::Kind::ItemInfoResult,
            Some(&message),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    })
}

/// ShopBuyRestrictGet: the (PurchaseTime ticks, Count) of one item.
async fn restrict_row(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    item: &Item,
    lock: bool,
) -> Result<Option<(i64, i32)>, MinimumError> {
    let sql = if lock {
        "SELECT purchase_ticks,count FROM game_shop_buy_restrict WHERE cid=$1 AND shop_item_id=$2 FOR UPDATE"
    } else {
        "SELECT purchase_ticks,count FROM game_shop_buy_restrict WHERE cid=$1 AND shop_item_id=$2"
    };
    Ok(sqlx::query_as(sql)
        .bind(cid)
        .bind(item.shop_item_id())
        .fetch_optional(&mut **tx)
        .await?)
}

/// ShopSession.TryShopBuyRestrictGet over the version's items: the restricted ones by Order.
async fn restrictions(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    items: &[Item],
    now: i64,
) -> Result<Vec<(i16, Restriction)>, MinimumError> {
    let mut out: Vec<(i16, Restriction)> = Vec::new();
    for item in items.iter().filter(|i| i.is_restricted()) {
        let (buyable_count, next_reset_ticks_diff) =
            item.restriction(restrict_row(tx, cid, item, false).await?, now);
        // Dictionary.Add: a repeated Order throws in the source.
        if out.iter().any(|(o, _)| *o == item.order) {
            return Err(fail());
        }
        out.push((
            item.order,
            Restriction {
                buyable_count,
                next_reset_ticks_diff,
            },
        ));
    }
    Ok(out)
}

/// QueryCouponShopItemInfoProcessor: CouponShopItemInfoResultMessage, or nothing for a
/// version without items (FailMessage, logged by the proxy).
pub async fn query(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    shop_version: i16,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let Some(items) = npc.coupons.items(shop_version) else {
        tracing::warn!("coupon shop version without items; values omitted");
        return Ok(Vec::new());
    };
    let now = super::minimum_status_effect::now_ticks()?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let rows = restrictions(&mut tx, cid, items, now).await?;
    tx.commit().await?;
    Ok(vec![info_packet(shop_version, rows)?])
}

/// BuyCouponShopItemProcessor. Admission (selected InGame character) belongs to the frontend.
#[allow(clippy::too_many_arguments)]
pub async fn buy(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: cwire::ItemBuy,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let base: i32 = {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let class: i32 =
            sqlx::query_scalar("SELECT class::integer FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        class
    };
    let restriction_of = |class: &str| {
        let key = utf16(class);
        let index = assets.stats_index.class(Some(&key))?;
        let economy = assets.economy.as_ref()?.get(index.source_index)?;
        Some(
            1_i32
                .checked_shl(u32::try_from(base).unwrap_or(32))
                .unwrap_or(0)
                & economy.class_restriction
                != 0,
        )
    };
    let (version, item, give, price) = match npc.coupons.plan(
        &npc.policy,
        request.shop_id,
        request.order,
        request.count,
        restriction_of,
        |class| assets.stats_index.class(Some(&utf16(class))).is_some(),
    ) {
        BuyPlan::Silent => {
            tracing::warn!("coupon shop buy refused before the trade; values omitted");
            return Ok(Vec::new());
        }
        BuyPlan::Refused(r) => return Ok(vec![result_packet(r)?]),
        BuyPlan::Trade {
            version,
            item,
            give,
            price,
        } => (version.clone(), item.clone(), give, price),
    };
    let flags = log.flags;
    let operation = super::minimum_item_logs::operation_journal(pool, flags).await;
    let journal = super::minimum_item_logs::named_journal(pool, flags, "CouponShopItem").await;
    let (class, attributes) =
        crate::domain::item_attribute::parse_class_ex(&item.reward_item_class)
            .map_err(|_| fail())?;
    let item_key = utf16(&class);
    let price_key = utf16(&version.price_item_class);
    // MakeGiveItemRequest(checkBind: true), then IsCharacterBind.
    let bind = assets.stats_index.bind(&item_key).ok_or_else(fail)?;
    let bound = item.is_character_bind || assets.catalog.policy.is_enabled(bind)?;
    let now = super::minimum_status_effect::now_ticks()?;
    let mut ids = entity_ids.into_iter();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        // ShopSession.TryShopBuyRestrictSet before CraftItem.
        let mut current = -1;
        if item.is_restricted() {
            let row = restrict_row(&mut tx, cid, &item, true).await?;
            match coupon::restrict_set(
                row,
                item.period_start(now),
                request.count,
                item.restrict_count,
                now,
            ) {
                Some(((purchase, count), result)) => {
                    sqlx::query("INSERT INTO game_shop_buy_restrict(cid,shop_item_id,purchase_ticks,count) VALUES($1,$2,$3,$4) ON CONFLICT (cid,shop_item_id) DO UPDATE SET purchase_ticks=EXCLUDED.purchase_ticks,count=EXCLUDED.count")
                        .bind(cid)
                        .bind(item.shop_item_id())
                        .bind(purchase)
                        .bind(count)
                        .execute(&mut *tx)
                        .await?;
                    current = result;
                }
                None => return Ok::<_, MinimumError>(Err(None)),
            }
        }
        let applied = super::minimum_give::apply(
            &mut tx,
            owner,
            assets,
            cid,
            &[Request {
                class: &price_key,
                count: price,
                bound: false,
                equipment: None,
                attributes: &crate::domain::item_attribute::EMPTY,
            }],
            &[Request {
                class: &item_key,
                count: give,
                bound,
                equipment: None,
                attributes: &attributes,
            }],
            &mut || ids.next().ok_or_else(fail),
        )
        .await?;
        match applied {
            Ok(applied) => {
                let refreshed = if item.is_restricted() {
                    let items = npc.coupons.items(request.shop_id).unwrap_or(&[]);
                    Some(restrictions(&mut tx, cid, items, now).await?)
                } else {
                    None
                };
                tx.commit().await?;
                Ok(Ok((applied, current, refreshed)))
            }
            // The transaction drops: the restriction write rolls back with the trade.
            Err(e) => Ok(Err(Some(e))),
        }
    })
    .await
    .map_err(|_| fail())??;
    let (applied, current, refreshed) = match outcome {
        Ok(v) => v,
        // BuyRestrictionCheckFail.
        Err(None) => {
            return Ok(vec![result_packet(coupon::CouponShops::refused(
                &version, &item, BUY_FAILED,
            ))?]);
        }
        Err(Some(GiveError::Unsupported)) => return Err(fail()),
        Err(Some(e)) => {
            // CraftFail: ToBuyItemResult (UniqueConstraint keeps its own code).
            let code = if e == GiveError::UniqueConstraint {
                BUY_FAILED_UNIQUE
            } else {
                BUY_FAILED
            };
            return Ok(vec![result_packet(coupon::CouponShops::refused(
                &version, &item, code,
            ))?]);
        }
    };
    super::minimum_item_logs::write_give(
        pool,
        cid,
        operation,
        EVENT_BUY_COUPON_SHOP_ITEM,
        &applied,
        log,
    )
    .await;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        3,
        ACTION_COUPON_SHOP,
        &price_key,
        price,
        flags,
    )
    .await;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        2,
        ACTION_COUPON_SHOP,
        &item_key,
        give,
        flags,
    )
    .await;
    // Save → Sync (UpdateInventoryInfo), the result, then the refreshed restrictions when
    // the version has any.
    let mut packets = vec![
        OutgoingPacket {
            category: Kind::UpdateInventoryInfo.category(),
            body: wire::encode(
                Kind::UpdateInventoryInfo,
                Some(&Message::UpdateInventoryInfo(Some(applied.slots))),
                DEFAULT_MAX_BODY,
            )
            .map_err(|_| fail())?
            .to_vec(),
        },
        result_packet(coupon::CouponShops::succeeded(
            &version, &item, give, price, current,
        ))?,
    ];
    if let Some(rows) = refreshed.filter(|r| !r.is_empty()) {
        packets.push(info_packet(request.shop_id, rows)?);
    }
    Ok(packets)
}
