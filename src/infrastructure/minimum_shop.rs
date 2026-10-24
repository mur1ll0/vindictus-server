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
                equipment: None,
                attributes: &crate::domain::item_attribute::EMPTY,
            }],
            &[Request {
                class: &item_key,
                count: give_count,
                bound,
                equipment: None,
                attributes: &crate::domain::item_attribute::EMPTY,
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

/// ItemOwner.BuybackList entry: the sold item clone (in memory; lost on logout).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuybackEntry {
    pub class_utf16: Vec<u8>,
    pub class: String,
    pub count: i32,
    pub sell_price: i32,
    pub bound: bool,
    pub equipment: Option<super::minimum_give::Equipment>,
    /// Item.Attribute of the clone (ToRequestInfo keeps it).
    pub attributes: crate::domain::item_attribute::Attributes,
}
impl BuybackEntry {
    /// ItemClassExBuilder.Build(ItemClass, Attribute).
    fn class_ex(&self) -> Result<String, MinimumError> {
        crate::domain::item_attribute::build_class_ex(&self.class, &self.attributes)
            .map_err(|_| fail())
    }
}

pub(super) fn system_message(
    category: u8,
    format: &str,
    parameters: &[String],
) -> Result<OutgoingPacket, MinimumError> {
    use crate::protocol::frontend_control::{
        self as ctl, HeroesText, Kind as ControlKind, Message as ControlMessage,
    };
    let body = ctl::encode(
        ControlKind::System,
        Some(&ControlMessage::System {
            category,
            text: Some(HeroesText {
                format: Some(format),
                parameters: Some(parameters.iter().map(|p| Some(p.as_str())).collect()),
            }),
        }),
        DEFAULT_MAX_BODY,
    )
    .map_err(|_| fail())?;
    Ok(OutgoingPacket {
        category: ControlKind::System.category(),
        body: body.to_vec(),
    })
}
/// SystemMessageCategory.Dialog.
const DIALOG: u8 = 4;

/// GetBuybackInfoList: newest first, Order is the list index, unit price
/// SellPrice × BuybackPriceRatio.
pub fn buyback_list(
    assets: &Assets,
    list: &[BuybackEntry],
) -> Result<OutgoingPacket, MinimumError> {
    let ratio = assets
        .npc
        .as_ref()
        .ok_or_else(fail)?
        .policy
        .get_integer("BuybackPriceRatio")?;
    let infos = list
        .iter()
        .enumerate()
        .rev()
        .map(|(i, e)| {
            let colors = e.equipment.map_or([-1; 3], |q| q.colors);
            Ok(Some(wire::BuybackInfo {
                shop_id: Some("Buyback".to_owned()),
                order: i as i32,
                item_class: Some(e.class_ex()?),
                item_count: e.count,
                color1: colors[0],
                color2: colors[1],
                color3: colors[2],
                price_item_class: Some("gold".to_owned()),
                price_count: e.sell_price.wrapping_mul(ratio),
            }))
        })
        .collect::<Result<_, MinimumError>>()?;
    packet(
        Kind::BuybackListResult,
        Message::BuybackListResult(Some(infos)),
    )
}

#[derive(sqlx::FromRow)]
struct SoldRow {
    item_class_utf16: Vec<u8>,
    count: i32,
    collection: i16,
    tradable: bool,
}

/// SellItemProcessor for a town character (the SUBWEAPON battle check needs IsInGame).
/// A sale whose gold cannot be given is refused whole; the source would still remove
/// the sold items.
#[allow(clippy::too_many_arguments)]
pub async fn sell(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    count: i32,
    buyback: &mut Vec<BuybackEntry>,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let mut ids = entity_ids.into_iter();
    let gold = assets.inventory.gold_class().to_vec();
    let flags = log.flags;
    let operation = super::minimum_item_logs::operation_journal(pool, flags).await;
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
        sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None).await?;
        let Some(row): Option<SoldRow> = sqlx::query_as(
            "SELECT item_class_utf16,count,collection,tradable FROM game_items WHERE id=$1 AND owner_id=$2",
        )
        .bind(item_id)
        .bind(cid)
        .fetch_optional(&mut *tx)
        .await?
        else {
            // GetItem == null: FailMessage only.
            return Ok(Err(None));
        };
        // Bag (0/1/6) and Cash (100) are sellable; AccountStorage (gold) is not.
        if !matches!(row.collection, 0 | 1 | 6 | 100) {
            return Ok(Err(None));
        }
        let indexed = assets.stats_index.class(Some(&row.item_class_utf16)).ok_or_else(fail)?;
        let info = economy.get(indexed.source_index).ok_or_else(fail)?;
        if count <= 0 || row.count < count {
            return Ok(Err(Some("Heroes_System_Item_Error_Cannot_Sell_NotEnoughItem")));
        }
        let equipped: Option<Option<i16>> =
            sqlx::query_scalar("SELECT part FROM game_equippables WHERE id=$1")
                .bind(item_id)
                .fetch_optional(&mut *tx)
                .await?;
        let class_text = String::from_utf16(
            &row.item_class_utf16
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes(*b))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| fail())?;
        let quick = super::minimum_quickslot::load(&mut tx, assets, cid).await?;
        let on_quick_slot = super::minimum_quickslot::info(&quick)
            .classes
            .0
            .iter()
            .flatten()
            .any(|(_, c)| c.as_deref() == Some(class_text.as_str()));
        if info.indestructible == 1 || equipped.flatten().is_some() || on_quick_slot {
            return Ok(Err(Some("Heroes_System_Item_Error_Cannot_Sell_EquippedItem")));
        }
        let remaining = row.count - count;
        let equipment = if indexed.equippable {
            let e: (i32, i32, i32, i32, i32) = sqlx::query_as("SELECT color1,color2,color3,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=$1")
                .bind(item_id)
                .fetch_one(&mut *tx)
                .await?;
            Some(super::minimum_give::Equipment {
                colors: [e.0, e.1, e.2],
                reduced: e.3,
                bonus: e.4,
            })
        } else {
            None
        };
        let attributes = super::minimum_inventory::load_attributes(&mut tx, &[item_id])
            .await?
            .remove(&item_id)
            .unwrap_or_default();
        if remaining == 0 {
            super::minimum_give::delete_attributes(&mut tx, item_id).await?;
            sqlx::query("DELETE FROM game_equippables WHERE id=$1").bind(item_id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2").bind(item_id).bind(cid).execute(&mut *tx).await?;
        } else {
            sqlx::query("UPDATE game_items SET count=$3 WHERE id=$1 AND owner_id=$2").bind(item_id).bind(cid).bind(remaining).execute(&mut *tx).await?;
        }
        let price = info.sell_price.wrapping_mul(count);
        let applied = super::minimum_give::apply(
            &mut tx,
            owner,
            assets,
            cid,
            &[],
            &[Request {
                class: &gold,
                count: price,
                bound: false,
                equipment: None,
                attributes: &crate::domain::item_attribute::EMPTY,
            }],
            &mut || ids.next().ok_or_else(fail),
        )
        .await?;
        let Ok(applied) = applied else {
            return Err(fail());
        };
        let sold = if remaining == 0 {
            super::minimum_move::deleted_slot(item_id)
        } else {
            super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
                .await?
                .slots
                .ok_or_else(fail)?
                .into_iter()
                .flatten()
                .find(|s| s.id == item_id)
                .ok_or_else(fail)?
        };
        tx.commit().await?;
        Ok(Ok((applied, sold, row, class_text, price, equipment, info.sell_price, attributes)))
    })
    .await
    .map_err(|_| fail())??;
    let (applied, sold, row, class_text, price, equipment, sell_price, attributes) = match outcome {
        Ok(v) => v,
        Err(None) => return Err(fail()),
        Err(Some(dialog)) => return Ok(vec![system_message(DIALOG, dialog, &[])?]),
    };
    let journal = super::minimum_item_logs::named_journal(pool, flags, "SellItem").await;
    let mut changes = applied.clone_for_logs();
    changes.changes.insert(
        0,
        super::minimum_item_logs::NumChange {
            item_id,
            class_utf16: row.item_class_utf16.clone(),
            delta: -count,
        },
    );
    super::minimum_item_logs::write_give(pool, cid, operation, EVENT_SELL_ITEM, &changes, log)
        .await;
    let mut packets = vec![packet(
        Kind::UpdateInventoryInfo,
        Message::UpdateInventoryInfo(Some(
            std::iter::once(Some(sold)).chain(applied.slots).collect(),
        )),
    )?];
    if npc.policy.is_enabled(Some("Buyback"))? {
        let cap = usize::try_from(npc.policy.get_integer("BuybackListCount")?).unwrap_or(0);
        if buyback.len() >= cap && !buyback.is_empty() {
            buyback.remove(0);
        }
        buyback.push(BuybackEntry {
            class_utf16: row.item_class_utf16.clone(),
            class: class_text.clone(),
            count,
            sell_price,
            bound: !row.tradable,
            equipment,
            attributes,
        });
        packets.push(buyback_list(assets, buyback)?);
    }
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        3,
        ACTION_SHOP,
        &row.item_class_utf16,
        count,
        flags,
    )
    .await;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        2,
        ACTION_SHOP,
        &gold,
        price,
        flags,
    )
    .await;
    packets.push(system_message(
        0,
        "GameUI_Heroes_Chat_Shop_Sell",
        &[class_text, count.to_string(), price.to_string()],
    )?);
    Ok(packets)
}

/// EventType.SellItem.
const EVENT_SELL_ITEM: i16 = 98;
/// ActionType.Buyback.
const ACTION_BUYBACK: i16 = 48;

/// BuyItemProcessor "Buyback" branch: SellPrice × BuybackPriceRatio × Count × item Count
/// gold for the sold clone (colors, durability and binding kept).
#[allow(clippy::too_many_arguments)]
pub async fn buy_back(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    order: i32,
    count: i32,
    buyback: &mut Vec<BuybackEntry>,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    if count <= 0 || !npc.policy.is_enabled(Some("Buyback"))? {
        return Err(fail());
    }
    // BuybackList[Order]: an index outside the list throws (no message).
    let entry = usize::try_from(order)
        .ok()
        .and_then(|i| buyback.get(i))
        .cloned()
        .ok_or_else(fail)?;
    let ratio = npc.policy.get_integer("BuybackPriceRatio")?;
    let flags = log.flags;
    let operation = super::minimum_item_logs::operation_journal(pool, flags).await;
    let journal = super::minimum_item_logs::named_journal(pool, flags, "Buyback").await;
    let failed = |result: i32, item_count: i32, price: i32| {
        result_packet(BuyResult {
            result,
            item_class: entry.class.clone(),
            item_count,
            price_item_class: "gold".to_owned(),
            price_count: price,
            restriction_order: -1,
            restriction_count: -1,
        })
    };
    if ratio < 1 {
        return Ok(vec![failed(
            shop::BUY_FAILED,
            entry.count,
            entry.sell_price,
        )?]);
    }
    let price = entry
        .sell_price
        .wrapping_mul(ratio)
        .wrapping_mul(count)
        .wrapping_mul(entry.count);
    let gold = assets.inventory.gold_class().to_vec();
    let mut ids = entity_ids.into_iter();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let applied = super::minimum_give::apply(
            &mut tx,
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
            &[Request {
                class: &entry.class_utf16,
                count: entry.count,
                bound: entry.bound,
                equipment: entry.equipment,
                attributes: &entry.attributes,
            }],
            &mut || ids.next().ok_or_else(fail),
        )
        .await?;
        if applied.is_ok() {
            tx.commit().await?;
        }
        Ok::<_, MinimumError>(applied)
    })
    .await
    .map_err(|_| fail())??;
    let applied = match outcome {
        Ok(a) => a,
        Err(GiveError::Unsupported) => return Err(fail()),
        Err(e) => {
            let code = if e == GiveError::UniqueConstraint {
                shop::BUY_FAILED_UNIQUE
            } else {
                shop::BUY_FAILED
            };
            return Ok(vec![failed(
                code,
                count,
                entry.sell_price.wrapping_mul(ratio).wrapping_mul(count),
            )?]);
        }
    };
    if let Some(i) = buyback.iter().position(|e| *e == entry) {
        buyback.remove(i);
    }
    super::minimum_item_logs::write_give(pool, cid, operation, EVENT_BUY_ITEM, &applied, log).await;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        3,
        ACTION_BUYBACK,
        &gold,
        price,
        flags,
    )
    .await;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        2,
        ACTION_BUYBACK,
        &entry.class_utf16,
        entry.count,
        flags,
    )
    .await;
    Ok(vec![
        packet(
            Kind::UpdateInventoryInfo,
            Message::UpdateInventoryInfo(Some(applied.slots)),
        )?,
        buyback_list(assets, buyback)?,
        result_packet(BuyResult {
            result: shop::BUY_SUCCEEDED,
            item_class: entry.class_ex()?,
            item_count: count.wrapping_mul(entry.count),
            price_item_class: "gold".to_owned(),
            price_count: price,
            restriction_order: -1,
            restriction_count: -1,
        })?,
    ])
}

/// ItemOwner.SendDialog(message): SystemMessage(Dialog, HeroesString(message)).
pub(super) fn dialog(message: &str) -> Result<OutgoingPacket, MinimumError> {
    system_message(DIALOG, message, &[])
}
