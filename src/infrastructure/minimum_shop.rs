//! BuyItemProcessor (normal NPC shop branch) for the selected first Fiona: the price
//! item is destroyed and the bought item given by one CraftItem in one transaction,
//! then Save's inventory Sync and the BuyItemResultMessage.
use super::{
    character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError,
    minimum_give::Request,
};
use crate::{
    domain::{
        give_item::GiveError,
        map_partition::OutgoingPacket,
        shop::{self, BuyPlan, BuyResult},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, BuyItemResult, Kind, Message},
    },
};
use sqlx::PgPool;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
fn result_packet(r: BuyResult) -> Result<OutgoingPacket, MinimumError> {
    packet(
        Kind::BuyItemResult,
        Message::BuyItemResult(BuyItemResult {
            result: r.result,
            item_class: Some(r.item_class),
            item_count: r.item_count,
            price_item_class: Some(r.price_item_class),
            price_item_count: r.price_count,
            restriction_order: r.restriction_order,
            restriction_count: r.restriction_count,
        }),
    )
}

/// EventType.BuyItem; ItemLogType Destroy/Give with ActionType.Shop.
const EVENT_BUY_ITEM: i16 = 12;
const ACTION_SHOP: i16 = 10;

/// Upper bound of new item rows a buy can create (one per MaxStack bundle).
pub fn entity_ids_needed(assets: &Assets, shop_id: &str, order: i32, count: i32) -> usize {
    let (Some(shops), true) = (assets.shops.as_ref(), count > 0) else {
        return 0;
    };
    let Some(row) = shops.get(shop_id, order) else {
        return 0;
    };
    let key = utf16(&row.item_class);
    let max = assets
        .stats_index
        .class(Some(&key))
        .map_or(1, |c| c.max_stack.max(1));
    let total = i64::from(row.count) * i64::from(count);
    usize::try_from((total + i64::from(max) - 1) / i64::from(max))
        .unwrap_or(0)
        .min(4096)
}

/// Admission (selected InGame character, UseSecondPassword) belongs to the frontend.
#[allow(clippy::too_many_arguments)]
pub async fn buy(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    shop_id: &str,
    order: i32,
    count: i32,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let shops = assets.shops.as_ref().ok_or_else(fail)?;
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let row = match shop::plan(shops, &npc.policy, shop_id, order, count, |class| {
        assets.stats_index.class(Some(&utf16(class))).is_some()
    }) {
        BuyPlan::Silent | BuyPlan::Unsupported => return Err(fail()),
        BuyPlan::Refused(r) => return Ok(vec![result_packet(r)?]),
        BuyPlan::Trade { row, .. } => row.clone(),
    };
    let flags = log.flags;
    // BuyItemProcessor constructor GetJournalID; AddItemJournal("BuyItem").
    let operation = super::minimum_item_logs::operation_journal(pool, flags).await;
    let journal = super::minimum_item_logs::named_journal(pool, flags, "BuyItem").await;
    let give_count = row.count.wrapping_mul(count);
    let price = row.price_count.wrapping_mul(count);
    let item_key = utf16(&row.item_class);
    let price_key = utf16(&row.price_item_class);
    // MakeGiveItemRequest(checkBind: true), then ShopInfo.CharacterBind.
    let bind = assets.stats_index.bind(&item_key).ok_or_else(fail)?;
    let bound = row.character_bind || assets.catalog.policy.is_enabled(bind)?;
    let mut ids = entity_ids.into_iter();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let connected: bool =
            sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?;
        if !connected {
            return Err(fail());
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
            }],
            &[Request {
                class: &item_key,
                count: give_count,
                bound,
            }],
            &mut || ids.next().ok_or_else(fail),
        )
        .await?;
        match applied {
            Ok(applied) => {
                tx.commit().await?;
                Ok(Ok(applied))
            }
            Err(e) => Ok(Err(e)),
        }
    })
    .await
    .map_err(|_| fail())??;
    let applied = match outcome {
        Ok(applied) => applied,
        Err(GiveError::Unsupported) => return Err(fail()),
        Err(e) => {
            // CraftFail: ToBuyItemResult (UniqueConstraint keeps its own code).
            let code = if e == GiveError::UniqueConstraint {
                shop::BUY_FAILED_UNIQUE
            } else {
                shop::BUY_FAILED
            };
            return Ok(vec![result_packet(BuyResult::failed(&row, code))?]);
        }
    };
    super::minimum_item_logs::write_give(pool, cid, operation, EVENT_BUY_ITEM, &applied, log).await;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        3,
        ACTION_SHOP,
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
        ACTION_SHOP,
        &item_key,
        give_count,
        flags,
    )
    .await;
    // Save → Sync (UpdateInventoryInfo), then Comm.SendFrontendMessage(result).
    Ok(vec![
        packet(
            Kind::UpdateInventoryInfo,
            Message::UpdateInventoryInfo(Some(applied.slots)),
        )?,
        result_packet(BuyResult::succeeded(&row, count))?,
    ])
}

/// QueryShopItemInfoProcessor: feature-enabled rows of the shop with a buy restriction.
pub fn has_restricted_rows(assets: &Assets, shop_id: &str) -> bool {
    let (Some(shops), Some(npc)) = (assets.shops.as_ref(), assets.npc.as_ref()) else {
        return false;
    };
    shops.rows_of(shop_id).any(|r| {
        r.restriction_count > 0 && npc.policy.is_enabled(r.feature.as_deref()).unwrap_or(true)
    })
}
