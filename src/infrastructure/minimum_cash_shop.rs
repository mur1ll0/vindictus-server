//! CashShopService for the selected first Fiona, with the laboratory catalog and the NISMS
//! billing state replaced by PostgreSQL (migrations/0039_cash_shop.sql): QueryProductList,
//! CheckBalance, PurchaseItem into the character inventory (GameID per CID), InventoryInquiry,
//! InventoryPickup (InventoryPickupOnceResponse.ToGiveItem → GiveItem) and the wish list.
//! Gifts, refunds, the shared inventory, direct purchase in quests, Tir coins and mileage are
//! not migrated.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{give_item::GiveError, item_attribute, map_partition::OutgoingPacket},
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{
            self as wire, CashShopBalance, CashShopInventoryItem, CashShopPickUp, CashShopPurchase,
            Kind, Message, WishItem, WishListChange, WishListResponse, WishListSelectResponse,
        },
    },
};
use sqlx::PgPool;

/// Body limit of the catalog messages: the NISMS product array is sent in one message.
pub const CATALOG_MAX_BODY: usize = 4 * 1024 * 1024;
/// Nexon.Nisms.Result codes carried by CashShopFailMessage.
const RESULT_FAILED: i32 = 0;
const RESULT_NOT_ENOUGH_CASH: i32 = 12040;
/// WishListResult.
const WISH_SUCCESS: i32 = 1;
const WISH_NO_ITEM: i32 = 2;
const WISH_OVER_COUNT: i32 = -10;
/// WishListInsert refuses an insert at 30 rows.
const WISH_LIMIT: i64 = 30;
/// InventoryInquiry(GameID, ShowInventory.True, 1, 64): the first page of 64 orders.
const INVENTORY_PAGE: i64 = 64;
/// EventType.GiveItem / ActionType.CashShop / ItemLogType.GiveItem.
const EVENT_GIVE_ITEM: i16 = 40;
const ACTION_CASH_SHOP: i16 = 25;
const LOG_GIVE: i16 = 2;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|a| u16::from_le_bytes(*a))
        .collect();
    String::from_utf16(&units).map_err(|_| fail())
}
fn packet(kind: Kind, message: Message, limit: usize) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), limit)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
fn failure(reason: i32) -> Result<Vec<OutgoingPacket>, MinimumError> {
    Ok(vec![packet(
        Kind::CashShopFail,
        Message::CashShopFail(reason),
        DEFAULT_MAX_BODY,
    )?])
}

/// QueryProductList: CashShopCategoryListMessage then CashShopProductListMessage.
pub fn product_list(assets: &Assets) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let Some(contents) = assets.cash_shop.as_ref() else {
        return failure(RESULT_FAILED);
    };
    Ok(vec![
        packet(
            Kind::CashShopCategoryList,
            Message::CashShopCategoryList(Some(
                contents.categories().iter().cloned().map(Some).collect(),
            )),
            CATALOG_MAX_BODY,
        )?,
        packet(
            Kind::CashShopProductList,
            Message::CashShopProductList(Some(
                contents
                    .products()
                    .iter()
                    .map(|p| Some(p.element.clone()))
                    .collect(),
            )),
            CATALOG_MAX_BODY,
        )?,
    ])
}

async fn balance_of(executor: impl sqlx::PgExecutor<'_>, uid: i32) -> Result<i32, MinimumError> {
    Ok(
        sqlx::query_scalar("SELECT balance FROM game_cash_balances WHERE uid=$1")
            .bind(uid)
            .fetch_optional(executor)
            .await?
            .unwrap_or(0),
    )
}
fn balance_packet(balance: i32) -> Result<OutgoingPacket, MinimumError> {
    // CheckBalance without RefundlessBalance: CashShopBalanceMessage(Balance, 0).
    packet(
        Kind::CashShopBalance,
        Message::CashShopBalance(CashShopBalance {
            balance,
            refundless: 0,
        }),
        DEFAULT_MAX_BODY,
    )
}

/// BeginCheckBalance → CashShopBalanceMessage.
pub async fn balance(pool: &PgPool, uid: i32) -> Result<OutgoingPacket, MinimumError> {
    balance_packet(balance_of(pool, uid).await?)
}

/// Laboratory credit for the account of the given (or the only) initialized first Fiona.
pub async fn lab_credit(pool: &PgPool, cid: Option<i64>, amount: i32) -> Result<i32, MinimumError> {
    let owners: Vec<i32> = sqlx::query_scalar(
        "SELECT uid FROM game_minimum_character_initialization WHERE initialized AND ($1::BIGINT IS NULL OR cid=$1) LIMIT 2",
    )
    .bind(cid)
    .fetch_all(pool)
    .await?;
    let [uid] = owners.as_slice() else {
        return Err(fail());
    };
    credit(pool, *uid, amount).await
}

/// Laboratory credit (the NISMS charge): adds `amount` to the account balance.
pub async fn credit(pool: &PgPool, uid: i32, amount: i32) -> Result<i32, MinimumError> {
    if amount <= 0 {
        return Err(fail());
    }
    Ok(sqlx::query_scalar(
        "INSERT INTO game_cash_balances(uid,balance) VALUES($1,$2) ON CONFLICT(uid) DO UPDATE SET balance=game_cash_balances.balance+EXCLUDED.balance RETURNING balance",
    )
    .bind(uid)
    .bind(amount)
    .fetch_one(pool)
    .await?)
}

type OrderRow = (i32, i32, Vec<u8>, i16, i16, i16, Option<Vec<u8>>);

/// InventoryInquiry → CashShopInventoryMessage of the open orders.
async fn inventory_packet(
    executor: impl sqlx::PgExecutor<'_>,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    let rows: Vec<OrderRow> = sqlx::query_as(
        "SELECT order_no,product_no,item_class_utf16,pieces,expire_days,remain_quantity,attribute3_utf16 FROM game_cash_orders WHERE cid=$1 AND remain_quantity>0 ORDER BY order_no LIMIT $2",
    )
    .bind(cid)
    .bind(INVENTORY_PAGE)
    .fetch_all(executor)
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for (order_no, product_no, class, pieces, expire, remain, attribute3) in rows {
        let class = text(&class)?;
        let attribute3 = attribute3.as_deref().map(text).transpose()?;
        // CashShopInventoryElement(Order): ItemClassEx from ProductAttribute3.
        let class_ex = match attribute3.as_deref().filter(|a| !a.is_empty()) {
            Some(a) => {
                let (_, attributes) =
                    item_attribute::parse_class_ex(&format!("x{a}")).map_err(|_| fail())?;
                item_attribute::build_class_ex(&class, &attributes).map_err(|_| fail())?
            }
            None => class.clone(),
        };
        items.push(Some(CashShopInventoryItem {
            order_no,
            product_no,
            item_class: Some(format!("{class}{}", attribute3.unwrap_or_default())),
            item_class_ex: Some(class_ex),
            count: pieces,
            r: Some("-1".into()),
            g: Some("-1".into()),
            b: Some("-1".into()),
            expire,
            is_gift: false,
            sender_message: Some(String::new()),
            remain_quantity: remain,
        }));
    }
    packet(
        Kind::CashShopInventory,
        Message::CashShopInventory(Some(items)),
        DEFAULT_MAX_BODY,
    )
}

pub async fn inventory(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let out = inventory_packet(&mut *tx, cid).await?;
    tx.commit().await?;
    Ok(out)
}

/// QueryCashShopPurchaseItem → BeginPurchaseItem (character inventory) → EndPurchaseItem
/// (CashShopFailMessage on a failed result) → BeginCheckBalance.
pub async fn purchase(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: &CashShopPurchase,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let contents = assets.cash_shop.as_ref().ok_or_else(fail)?;
    // The common (shared) inventory is not migrated.
    if request.is_for_common_inven {
        return Err(fail());
    }
    let Some(product) = contents.product(request.product_no) else {
        return failure(RESULT_FAILED);
    };
    if request.quantity <= 0 {
        return failure(RESULT_FAILED);
    }
    let total = i64::from(request.quantity) * i64::from(product.element.sale_price);
    let total = i32::try_from(total).map_err(|_| fail())?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let connected: bool =
        sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    if !connected {
        return Err(fail());
    }
    let debited: Option<i32> = sqlx::query_scalar(
        "UPDATE game_cash_balances SET balance=balance-$2 WHERE uid=$1 AND balance>=$2 RETURNING balance",
    )
    .bind(owner.uid)
    .bind(total)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(balance) = debited else {
        return failure(RESULT_NOT_ENOUGH_CASH);
    };
    let attribute = |a: &Option<String>| a.as_deref().map(utf16);
    sqlx::query("INSERT INTO game_cash_orders(uid,cid,product_no,item_class_utf16,pieces,expire_days,remain_quantity,price,attribute0_utf16,attribute1_utf16,attribute2_utf16,attribute3_utf16) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
        .bind(owner.uid)
        .bind(cid)
        .bind(request.product_no)
        .bind(&product.class_utf16)
        .bind(product.element.product_pieces)
        .bind(product.element.product_expire)
        .bind(request.quantity)
        .bind(total)
        .bind(attribute(&request.attribute0))
        .bind(attribute(&request.attribute1))
        .bind(attribute(&request.attribute2))
        .bind(attribute(&request.attribute3))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(vec![balance_packet(balance)?])
}

/// Entity IDs the pick-up may need (one per non-stacking unit), bounded.
pub fn entity_ids_needed(request: &CashShopPickUp) -> usize {
    usize::try_from(request.quantity).map_or(1, |q| q.clamp(1, 256))
}

type PickRow = (
    Vec<u8>,
    i16,
    i16,
    i16,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
);

/// QueryCashShopItemPickUp → InventoryPickup → GiveItem(OperationFail) → the inventory again.
/// A failed pick-up keeps the order (EndInventoryPickUp_OnFail) and answers nothing.
pub async fn pick_up(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: &CashShopPickUp,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if request.quantity <= 0 {
        return Err(fail());
    }
    let operation = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let journal = super::minimum_item_logs::named_journal(pool, log.flags, "GiveItem").await;
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
    let row: Option<PickRow> = sqlx::query_as(
        "SELECT item_class_utf16,pieces,expire_days,remain_quantity,attribute0_utf16,attribute1_utf16,attribute2_utf16,attribute3_utf16 FROM game_cash_orders WHERE order_no=$1 AND product_no=$2 AND cid=$3 FOR UPDATE",
    )
    .bind(request.order_no)
    .bind(request.product_no)
    .bind(cid)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((class, pieces, expire, remain, a0, a1, a2, a3)) = row else {
        return Err(fail());
    };
    if remain < request.quantity || expire != 0 {
        // Expiring products need ExpireDateTime on the give request (not migrated).
        return Err(fail());
    }
    let class = text(&class)?;
    let (name, attributes) = match a3
        .as_deref()
        .map(text)
        .transpose()?
        .filter(|a| !a.is_empty())
    {
        Some(a) => {
            let (_, attributes) =
                item_attribute::parse_class_ex(&format!("x{a}")).map_err(|_| fail())?;
            (class.clone(), attributes)
        }
        None => (class.clone(), item_attribute::Attributes::default()),
    };
    // ProductAttribute0-2: ParseInt or -1.
    let color = |a: &Option<Vec<u8>>| -> Result<i32, MinimumError> {
        Ok(match a.as_deref().map(text).transpose()? {
            Some(t) => t.trim().parse().unwrap_or(-1),
            None => -1,
        })
    };
    let colors = [color(&a0)?, color(&a1)?, color(&a2)?];
    let key = utf16(&name);
    let indexed = assets.stats_index.class(Some(&key)).ok_or_else(fail)?;
    let equipment =
        (indexed.equippable && !colors.contains(&-1)).then_some(super::minimum_give::Equipment {
            colors,
            reduced: 0,
            bonus: 0,
        });
    let count = i32::from(request.quantity) * i32::from(pieces);
    let give = [super::minimum_give::Request {
        class: &key,
        count,
        bound: false,
        equipment,
        attributes: &attributes,
    }];
    let applied =
        match super::minimum_give::apply(&mut tx, owner, assets, cid, &[], &give, &mut || {
            ids.next().ok_or_else(fail)
        })
        .await?
        {
            Ok(a) => a,
            Err(GiveError::Unsupported | GiveError::InvalidItemClass) => return Err(fail()),
            // GiveItem.FailMethodEnum.OperationFail: the order stays, nothing is sent.
            Err(_) => return Ok(Vec::new()),
        };
    sqlx::query("UPDATE game_cash_orders SET remain_quantity=remain_quantity-$2 WHERE order_no=$1")
        .bind(request.order_no)
        .bind(request.quantity)
        .execute(&mut *tx)
        .await?;
    let inventory = inventory_packet(&mut *tx, cid).await?;
    tx.commit().await?;
    super::minimum_item_logs::write_give(pool, cid, operation, EVENT_GIVE_ITEM, &applied, log)
        .await;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        LOG_GIVE,
        ACTION_CASH_SHOP,
        &key,
        count,
        log.flags,
    )
    .await;
    Ok(vec![
        packet(
            Kind::UpdateInventoryInfo,
            Message::UpdateInventoryInfo(Some(applied.slots)),
            DEFAULT_MAX_BODY,
        )?,
        inventory,
    ])
}

fn wish_response(kind: Kind, result: i32, cid: i64) -> Result<OutgoingPacket, MinimumError> {
    let r = WishListResponse { result, cid };
    let message = match kind {
        Kind::WishListInsertResponse => Message::WishListInsertResponse(r),
        _ => Message::WishListDeleteResponse(r),
    };
    packet(kind, message, DEFAULT_MAX_BODY)
}

/// WishListSelect → WishListSelectResponseMessage (NOITEM for an empty list).
pub async fn wish_list(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
    request_cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    if request_cid != cid {
        return Err(fail());
    }
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let rows: Vec<(i32, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT product_no,product_name_utf16 FROM game_cash_wishlist WHERE cid=$1 ORDER BY product_no LIMIT 64",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut items = Vec::with_capacity(rows.len());
    for (product_no, name) in rows {
        items.push(Some(WishItem {
            cid,
            product_no,
            product_name: name.as_deref().map(text).transpose()?,
        }));
    }
    packet(
        Kind::WishListSelectResponse,
        Message::WishListSelectResponse(WishListSelectResponse {
            result: if items.is_empty() {
                WISH_NO_ITEM
            } else {
                WISH_SUCCESS
            },
            cid: request_cid,
            product_info: Some(items),
        }),
        DEFAULT_MAX_BODY,
    )
}

/// WishListInsert: each product must exist (NOITEM stops); the 31st row is OVERCOUNT; a
/// duplicate fails in the procedure and the loop goes on.
pub async fn wish_insert(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: &WishListChange,
) -> Result<OutgoingPacket, MinimumError> {
    let contents = assets.cash_shop.as_ref().ok_or_else(fail)?;
    if request.cid != cid {
        return Err(fail());
    }
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut result = WISH_SUCCESS;
    for product_no in request.products.iter().flatten().take(64) {
        let Some(product) = contents.product(*product_no) else {
            result = WISH_NO_ITEM;
            break;
        };
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM game_cash_wishlist WHERE cid=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
        if count >= WISH_LIMIT {
            result = WISH_OVER_COUNT;
            break;
        }
        let name = product.element.product_id.as_deref().map(utf16);
        sqlx::query("INSERT INTO game_cash_wishlist(cid,product_no,product_name_utf16) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
            .bind(cid)
            .bind(product_no)
            .bind(name)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    wish_response(Kind::WishListInsertResponse, result, request.cid)
}

/// WishListDelete: ProductNo 0 deletes nothing.
pub async fn wish_delete(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
    request: &WishListChange,
) -> Result<OutgoingPacket, MinimumError> {
    if request.cid != cid {
        return Err(fail());
    }
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    for product_no in request.products.iter().flatten().take(64) {
        if *product_no == 0 {
            continue;
        }
        sqlx::query("DELETE FROM game_cash_wishlist WHERE cid=$1 AND product_no=$2")
            .bind(cid)
            .bind(product_no)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    wish_response(Kind::WishListDeleteResponse, WISH_SUCCESS, request.cid)
}

/// RequestSecuredOperationMessage (after the second password): below
/// TradeAndCashShopLevelLimit a Dialog SystemMessage, otherwise SecuredOperationMessage
/// (Operation, LockedTimeInSeconds 0), which opens the cash shop or trade dialog.
pub async fn secured_operation(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    operation: i32,
) -> Result<OutgoingPacket, MinimumError> {
    let contents = assets.cash_shop.as_ref().ok_or_else(fail)?;
    let limit = contents
        .customize()
        .policy
        .get_integer("TradeAndCashShopLevelLimit")?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let level: i32 = sqlx::query_scalar("SELECT level FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    if level < limit {
        return super::minimum_shop::system_message(
            4,
            "GameUI_Heroes_TradeAndCashShopLevelLimit_Desc",
            &[limit.to_string()],
        );
    }
    packet(
        Kind::SecuredOperation,
        Message::SecuredOperation(wire::SecuredOperation {
            operation,
            locked_time_in_seconds: 0,
        }),
        DEFAULT_MAX_BODY,
    )
}
