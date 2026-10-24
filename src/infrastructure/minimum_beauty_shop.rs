//! BeautyShopCustomizeProcessor for the selected first Fiona. Cash payments use the
//! laboratory balance (DirectPickUp of the CashShopService, whose NISMS billing is replaced by
//! PostgreSQL); Tir payments destroy tir_coin. Per item: CustomizeItemInfo, the
//! CustomizePriceInfo of its Duration and Price or a CustomizeCouponInfo (value coupons,
//! unlimited pass and free pass), the free paintings; then UpdateBaseCostume (painting
//! positions), the used coupons, UpdateFaceCostume for the look change, the free pass
//! coupon equipped with its LOOK, the parts of the other items unequipped, GiveItem and the
//! painting COSTUMELOC. The frontend answers ItemFailMessage "GameUI_Heroes_Success" or
//! "GameUI_Heroes_BeautyShopFail".
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::{
    domain::{character_stats::Arithmetic, item_attribute, map_partition::OutgoingPacket},
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, BeautyShopCustomize, CustomizeItemRequest, Kind, Message},
    },
};
use sqlx::{PgPool, Postgres, Transaction};

/// BeautyShopCustomizeProcessor.FreeItems.
const FREE_ITEMS: [&str; 3] = ["uni_painting38", "uni_painting39", "uni_painting40"];
/// EventType.BeautyShopCustomize / ActionType.CashShop / ItemLogType.GiveItem.
const EVENT_BEAUTY_SHOP_CUSTOMIZE: i16 = 8;
const ACTION_CASH_SHOP: i16 = 25;
const LOG_GIVE: i16 = 2;
/// TimeSpan ticks of one second.
const SECOND_TICKS: f64 = 10_000_000.0;
/// ItemClassConst.TirCoin.
const TIR_COIN: &str = "tir_coin";
/// SystemMessageCategory.Dialog.
const DIALOG: u8 = 4;
/// Gear parts of the face and body paintings (SetCostumeLocation).
const FACE_PAINTING_PART: i16 = 25;
const BODY_PAINTING_PART: i16 = 26;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// The answer and, when the Gear or the costume changed, the refreshed look.
pub struct Customized {
    pub packets: Vec<OutgoingPacket>,
    pub look: Option<OutgoingPacket>,
}

/// OnFail: ItemFailMessage "GameUI_Heroes_BeautyShopFail" after any earlier dialog.
fn refused(mut packets: Vec<OutgoingPacket>) -> Result<Customized, MinimumError> {
    packets.push(super::minimum_equip::item_fail("BeautyShopFail")?);
    Ok(Customized {
        packets,
        look: None,
    })
}

/// A planned GiveItemRequest(customizeItem, selectedCouponItemID).
struct Give {
    class: String,
    category: Option<String>,
    colors: [i32; 3],
    duration: i32,
    coupon_item_id: i64,
    look_change: bool,
    package: bool,
}

/// CostumeManager.SetCostume of a base row: an existing (Type, Key) takes CostumeSN, C1-C3
/// and Gloss; otherwise a new row.
async fn set_base_costume(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    kind: i16,
    key: i16,
    sn: Option<i32>,
    colors: [Option<i32>; 3],
    gloss: Option<i16>,
) -> Result<(), MinimumError> {
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT row_id FROM game_costumes WHERE cid=$1 AND is_base AND costume_type=$2 AND costume_key=$3 ORDER BY row_id LIMIT 2 FOR UPDATE",
    )
    .bind(cid)
    .bind(kind)
    .bind(key)
    .fetch_all(&mut **tx)
    .await?;
    match rows.as_slice() {
        [row] => {
            sqlx::query(
                "UPDATE game_costumes SET costume_sn=$2,c1=$3,c2=$4,c3=$5,gloss=$6 WHERE row_id=$1",
            )
            .bind(row)
            .bind(sn)
            .bind(colors[0])
            .bind(colors[1])
            .bind(colors[2])
            .bind(gloss)
            .execute(&mut **tx)
            .await?;
        }
        [] => {
            sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect) VALUES(nextval('game_costume_row_id'),$1,true,$2,$3,$4,$5,$6,$7,$8,NULL)")
                .bind(cid)
                .bind(kind)
                .bind(key)
                .bind(sn)
                .bind(colors[0])
                .bind(colors[1])
                .bind(colors[2])
                .bind(gloss)
                .execute(&mut **tx)
                .await?;
        }
        // Duplicate keys would make the source dictionary choice observable.
        _ => return Err(fail()),
    }
    Ok(())
}
type CostumeRow = (
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i16>,
);
async fn base_costume(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    kind: i16,
    key: i16,
) -> Result<Option<CostumeRow>, MinimumError> {
    let rows: Vec<CostumeRow> = sqlx::query_as(
        "SELECT costume_sn,c1,c2,c3,gloss FROM game_costumes WHERE cid=$1 AND is_base AND costume_type=$2 AND costume_key=$3 ORDER BY row_id LIMIT 2",
    )
    .bind(cid)
    .bind(kind)
    .bind(key)
    .fetch_all(&mut **tx)
    .await?;
    match rows.as_slice() {
        [] => Ok(None),
        [row] => Ok(Some(*row)),
        _ => Err(fail()),
    }
}
/// CostumeManager.LoadExtraData(key, value): a Type 2 base row with CostumeSN only.
async fn extra(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    key: i16,
    value: i32,
) -> Result<(), MinimumError> {
    set_base_costume(tx, cid, 2, key, Some(value), [None; 3], None).await
}

/// ItemOwner.GetItems(itemClass).ItemCount().
async fn held(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    class: &str,
) -> Result<i64, MinimumError> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(sum(count),0)::BIGINT FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2",
    )
    .bind(cid)
    .bind(utf16(class))
    .fetch_one(&mut **tx)
    .await?)
}

type ItemRow = (Vec<u8>, i32, Option<i64>);
async fn owned_item(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    id: i64,
) -> Result<Option<(String, i32, Option<i64>)>, MinimumError> {
    let row: Option<ItemRow> = sqlx::query_as(
        "SELECT item_class_utf16,count,expire_ticks FROM game_items WHERE id=$1 AND owner_id=$2",
    )
    .bind(id)
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|(class, count, expire)| {
        let units: Vec<u16> = class
            .as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect();
        Ok((
            String::from_utf16(&units).map_err(|_| fail())?,
            count,
            expire,
        ))
    })
    .transpose()
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
    let Some(items) = request.customize_items.as_ref() else {
        return refused(Vec::new());
    };
    // A null CustomizeItemRequestInfo cannot be deserialized by the source either.
    let Some(items) = items
        .iter()
        .cloned()
        .collect::<Option<Vec<CustomizeItemRequest>>>()
    else {
        return refused(Vec::new());
    };
    if items.len() > 32 {
        return refused(Vec::new());
    }
    let avatar_beauty = customize.policy.is_enabled(Some("AvatarBeauty"))?;
    let tir_payment = request.payment == 2;
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
    let mut dialogs = Vec::new();
    let mut gives: Vec<Give> = Vec::new();
    let mut used_coupons: Vec<String> = Vec::new();
    let mut total: i64 = 0;
    for mut item in items {
        let (Some(class), Some(price)) = (item.item_class.clone(), item.price.clone()) else {
            return refused(dialogs);
        };
        let coupon = customize.coupons.get(&price).cloned();
        let Some(customize_category) = customize.items.get(&class) else {
            return refused(dialogs);
        };
        let key = utf16(&class);
        let Some(indexed) = assets.stats_index.class(Some(&key)) else {
            return refused(dialogs);
        };
        let (class_category, class_restriction) =
            customize.classes.get(&key).cloned().ok_or_else(fail)?;
        let look_change =
            class_category.as_deref() == Some("APPEARANCE") && class == "lookchange_item";
        let package = avatar_beauty && class_category.as_deref() == Some("AVATAR_PACKAGE");
        if !indexed.equippable && !look_change && !package {
            return refused(dialogs);
        }
        let all_prices = customize.prices.get(&class).cloned().unwrap_or_default();
        let prices: Vec<_> = all_prices
            .iter()
            .filter(|p| p.duration == item.duration)
            .copied()
            .collect();
        let valid_check: Vec<_> = all_prices.iter().filter(|p| p.price > 0).copied().collect();
        let free_pass = price.starts_with("cash_freepass_") && coupon.is_some();
        let unlimited = price.starts_with("cash_unlimitedpass_") && coupon.is_some();
        if !free_pass && !unlimited && prices.is_empty() {
            return refused(dialogs);
        }
        let free_item = FREE_ITEMS.contains(&class.as_str());
        let mut selected = None;
        if !free_item {
            match &coupon {
                Some((coupon_category, coupon_price)) => {
                    let mut correct = false;
                    if coupon_category == customize_category
                        && held(&mut tx, cid, &price).await? > 0
                    {
                        if free_pass {
                            if let Some((coupon_class, _, _)) =
                                owned_item(&mut tx, cid, item.coupon_item_id).await?
                                && coupon_class.starts_with("cash_freepass_")
                            {
                                let coupon_restriction = customize
                                    .classes
                                    .get(&utf16(&coupon_class))
                                    .map_or(0, |c| c.1);
                                correct = coupon_restriction & class_restriction != 0;
                            }
                        } else if unlimited {
                            correct = owned_item(&mut tx, cid, item.coupon_item_id)
                                .await?
                                .is_some_and(|(c, _, _)| c.starts_with("cash_unlimitedpass_"));
                        } else {
                            for p in &prices {
                                if p.price > 0 && p.price <= *coupon_price {
                                    selected = Some(*p);
                                    // IsValidCoupon: no other positive price between it and the coupon.
                                    correct = !valid_check.iter().any(|q| {
                                        q != p && q.price > p.price && q.price <= *coupon_price
                                    });
                                    if !correct {
                                        dialogs.push(super::minimum_shop::system_message(
                                            DIALOG,
                                            "GameUI_Heroes_BeautyShopToolTip_InvalidCoupon",
                                            &[],
                                        )?);
                                    }
                                    break;
                                }
                                if p.price < 0 && p.price == *coupon_price {
                                    selected = Some(*p);
                                    correct = true;
                                    break;
                                }
                            }
                        }
                    }
                    if !correct {
                        return refused(dialogs);
                    }
                }
                None => {
                    let Some(p) = prices.iter().find(|p| p.price.to_string() == price) else {
                        return refused(dialogs);
                    };
                    selected = Some(*p);
                }
            }
        }
        let mut coupon_item_id = -1;
        if free_item {
        } else if coupon.is_none() {
            let selected = selected.ok_or_else(fail)?;
            if tir_payment
                && customize.tir_ignored.iter().any(|t| {
                    t.item == class
                        && t.duration.is_none_or(|d| d == selected.duration)
                        && t.price.is_none_or(|v| v == selected.price)
                        && (t.feature.as_deref() == Some("")
                            || customize
                                .policy
                                .is_enabled(t.feature.as_deref())
                                .unwrap_or(false))
                })
            {
                // CashshopTirCoinResultMessage lists it; no customize item of the content is
                // in Cashshop_Tircoin_notsell_list.
                return refused(dialogs);
            }
            total += i64::from(selected.price);
        } else if free_pass {
            coupon_item_id = item.coupon_item_id;
        } else {
            used_coupons.push(price.clone());
            if unlimited {
                item.duration = 0;
            }
        }
        gives.push(Give {
            class,
            category: if indexed.equippable {
                Some(profile.category(&assets.catalog, &key)?)
            } else {
                None
            },
            colors: [item.color1, item.color2, item.color3],
            duration: item.duration,
            coupon_item_id,
            look_change,
            package,
        });
    }
    let total = i32::try_from(total).map_err(|_| fail())?;
    let mut balance = None;
    if total > 0 {
        if request.payment <= 1 {
            // DirectPickUp: the NISMS purchase of the invoice (NotEnoughCash fails it).
            let debited: Option<i32> = sqlx::query_scalar(
                "UPDATE game_cash_balances SET balance=balance-$2 WHERE uid=$1 AND balance>=$2 RETURNING balance",
            )
            .bind(owner.uid)
            .bind(total)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(b) = debited else {
                return refused(dialogs);
            };
            balance = Some(b);
        } else if !tir_payment {
            return refused(dialogs);
        }
    }
    let mut destroys: Vec<(Vec<u8>, i32)> = used_coupons.iter().map(|c| (utf16(c), 1)).collect();
    if tir_payment && total > 0 {
        destroys.push((utf16(TIR_COIN), total));
    }
    // UpdateBaseCostume(EquippedItems: empty, painting positions).
    for (key, value) in [
        (3, request.painting_pos_x),
        (4, request.painting_pos_y),
        (6, request.painting_size),
        (5, request.painting_rotation),
        (8, request.body_painting_pos_x),
        (9, request.body_painting_pos_y),
        (11, request.body_painting_size),
        (10, request.body_painting_rotation),
        (12, request.body_painting_side),
        (13, request.body_painting_clip),
        (14, request.body_painting_mode),
    ] {
        extra(&mut tx, cid, key, value).await?;
    }
    // UpdateFaceCostume for a look change with known eye and skin colors and eyebrow class.
    if gives.iter().any(|g| g.look_change) {
        let eye = customize
            .colors
            .contains(&("eye".to_owned(), request.eye_color));
        let skin = customize
            .colors
            .contains(&("skin".to_owned(), request.skin_color));
        let eyebrow = request
            .eyebrow_item_class
            .as_deref()
            .map(utf16)
            .filter(|k| assets.stats_index.class(Some(k)).is_some());
        if let (true, true, Some(eyebrow)) = (eye, skin, eyebrow) {
            let costume_type = profile.metadata(&assets.catalog, &eyebrow)?.serial;
            let sss = customize
                .skin_sss
                .get(&request.skin_color)
                .copied()
                .unwrap_or(0);
            let face = base_costume(&mut tx, cid, 0, 2).await?.ok_or_else(fail)?;
            set_base_costume(
                &mut tx,
                cid,
                0,
                2,
                face.0,
                [Some(request.skin_color), Some(sss), Some(request.eye_color)],
                face.4,
            )
            .await?;
            let brow = base_costume(&mut tx, cid, 1, 0).await?.ok_or_else(fail)?;
            set_base_costume(
                &mut tx,
                cid,
                1,
                0,
                Some(costume_type),
                [Some(request.eyebrow_color), brow.2, brow.3],
                brow.4,
            )
            .await?;
            extra(&mut tx, cid, 2, request.bust_value).await?;
            extra(&mut tx, cid, 1, request.height_value).await?;
            extra(&mut tx, cid, 0, request.shineness_value).await?;
            for (key, bits) in request.body_shape_info.0.iter().flatten() {
                let value = f32::from_bits(*bits as u32);
                let key = i16::try_from(*key).map_err(|_| fail())?;
                if !(0..=255).contains(&key) {
                    return Err(fail());
                }
                set_base_costume(
                    &mut tx,
                    cid,
                    3,
                    key,
                    Some((value * 1000f32) as i32),
                    [None; 3],
                    None,
                )
                .await?;
            }
        }
    }
    let mut modified: Vec<i64> = Vec::new();
    let now = super::lab_clock::utc_now().map_err(|_| fail())?.ticks();
    let part_of = |category: &str| {
        parts
            .iter()
            .find(|x| !x.consumable && x.category == category)
            .map(|x| i16::from(x.slot))
    };
    // The free pass coupon takes the LOOK, the colors and, unless already expiring, its
    // duration (10 minutes for 0 days), and is equipped in the part of the look's category.
    for g in gives.iter().filter(|g| g.coupon_item_id >= 0) {
        let Some((coupon_class, _, expire)) = owned_item(&mut tx, cid, g.coupon_item_id).await?
        else {
            return refused(dialogs);
        };
        let Some(part) = g.category.as_deref().and_then(part_of) else {
            continue;
        };
        let current = super::minimum_inventory::load_attributes(&mut tx, &[g.coupon_item_id])
            .await?
            .remove(&g.coupon_item_id)
            .unwrap_or_default();
        super::minimum_enhance::set_attribute_args(
            &mut tx,
            g.coupon_item_id,
            &current,
            "LOOK",
            Some(&g.class),
            0,
            0,
        )
        .await?;
        sqlx::query("UPDATE game_equippables SET color1=$2,color2=$3,color3=$4 WHERE id=$1")
            .bind(g.coupon_item_id)
            .bind(g.colors[0])
            .bind(g.colors[1])
            .bind(g.colors[2])
            .execute(&mut *tx)
            .await?;
        if expire.is_none() {
            let days = coupon_class
                .split('_')
                .nth(2)
                .and_then(|d| d.parse::<f64>().ok())
                .unwrap_or(0.0);
            let days = if days as i32 == 0 { 1.0 / 144.0 } else { days };
            sqlx::query("UPDATE game_items SET expire_ticks=$2 WHERE id=$1")
                .bind(g.coupon_item_id)
                .bind(now + (days * 86_400.0 * SECOND_TICKS) as i64)
                .execute(&mut *tx)
                .await?;
        }
        let unequipped: Vec<i64> = sqlx::query_scalar("UPDATE game_equippables e SET part=NULL FROM game_items i WHERE i.id=e.id AND i.owner_id=$1 AND e.part=$2 AND e.id<>$3 RETURNING e.id")
            .bind(cid)
            .bind(part)
            .bind(g.coupon_item_id)
            .fetch_all(&mut *tx)
            .await?;
        modified.extend(unequipped);
        sqlx::query("UPDATE game_equippables SET part=$2 WHERE id=$1")
            .bind(g.coupon_item_id)
            .bind(part)
            .execute(&mut *tx)
            .await?;
        modified.push(g.coupon_item_id);
    }
    // The remaining items: the look change is not given; the part of each other non-package
    // item is unequipped (part 0 is never unequipped by the source).
    let given: Vec<&Give> = gives
        .iter()
        .filter(|g| g.coupon_item_id < 0 && !g.look_change)
        .collect();
    for g in &given {
        if g.package {
            continue;
        }
        let Some(part) = g.category.as_deref().and_then(part_of).filter(|p| *p != 0) else {
            continue;
        };
        let unequipped: Vec<i64> = sqlx::query_scalar("UPDATE game_equippables e SET part=NULL FROM game_items i WHERE i.id=e.id AND i.owner_id=$1 AND e.part=$2 RETURNING e.id")
            .bind(cid)
            .bind(part)
            .fetch_all(&mut *tx)
            .await?;
        modified.extend(unequipped);
    }
    let keys: Vec<Vec<u8>> = given.iter().map(|g| utf16(&g.class)).collect();
    let destroy_requests: Vec<_> = destroys
        .iter()
        .map(|(class, count)| super::minimum_give::Request {
            class,
            count: *count,
            bound: false,
            equipment: None,
            attributes: &item_attribute::EMPTY,
        })
        .collect();
    let give_requests: Vec<_> = given
        .iter()
        .zip(&keys)
        .map(|(g, key)| super::minimum_give::Request {
            class: key,
            count: 1,
            bound: false,
            equipment: (g.category.is_some() && !g.colors.contains(&-1)).then_some(
                super::minimum_give::Equipment {
                    colors: g.colors,
                    reduced: 0,
                    bonus: 0,
                },
            ),
            attributes: &item_attribute::EMPTY,
        })
        .collect();
    let applied = match super::minimum_give::apply(
        &mut tx,
        owner,
        assets,
        cid,
        &destroy_requests,
        &give_requests,
        &mut || ids.next().ok_or_else(fail),
    )
    .await?
    {
        Ok(a) => a,
        // DestroyItem(coupon or tir_coin) or GiveItem failed: Undo.
        Err(_) => return refused(dialogs),
    };
    // GiveItemRequest(customizeItem): ExpireDateTime = UtcNow.AddDays(Duration) when positive.
    let mut taken: Vec<i64> = Vec::new();
    for (g, key) in given.iter().zip(&keys) {
        let created = applied
            .created
            .iter()
            .find(|c| c.class_utf16 == *key && !taken.contains(&c.id))
            .ok_or_else(fail)?;
        taken.push(created.id);
        if g.duration > 0 {
            sqlx::query("UPDATE game_items SET expire_ticks=$2 WHERE id=$1")
                .bind(created.id)
                .bind(now + i64::from(g.duration) * 86_400 * SECOND_TICKS as i64)
                .execute(&mut *tx)
                .await?;
        }
    }
    modified.extend(taken);
    modified.extend(
        applied
            .slots
            .iter()
            .flatten()
            .map(|s| s.id)
            .filter(|id| !modified.contains(id))
            .collect::<Vec<_>>(),
    );
    // SetCostumeLocation of the equipped face (25) and body (26) paintings.
    for (part, value) in [
        (
            FACE_PAINTING_PART,
            format!(
                "{}/{}/{}/{}",
                request.painting_pos_x,
                request.painting_pos_y,
                request.painting_size,
                request.painting_rotation
            ),
        ),
        (
            BODY_PAINTING_PART,
            format!(
                "{}/{}/{}/{}/{}/{}/{}",
                request.body_painting_pos_x,
                request.body_painting_pos_y,
                request.body_painting_size,
                request.body_painting_rotation,
                request.body_painting_side,
                request.body_painting_clip,
                request.body_painting_mode
            ),
        ),
    ] {
        let equipped: Option<i64> = sqlx::query_scalar("SELECT e.id FROM game_equippables e JOIN game_items i ON i.id=e.id WHERE i.owner_id=$1 AND e.part=$2")
            .bind(cid)
            .bind(part)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(id) = equipped {
            let current = super::minimum_inventory::load_attributes(&mut tx, &[id])
                .await?
                .remove(&id)
                .unwrap_or_default();
            super::minimum_enhance::set_attribute_args(
                &mut tx,
                id,
                &current,
                "COSTUMELOC",
                Some(&value),
                0,
                0,
            )
            .await?;
            if !modified.contains(&id) {
                modified.push(id);
            }
        }
    }
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
            .cloned()
            .unwrap_or_else(|| super::minimum_move::deleted_slot(*id));
        slots.push(Some(slot));
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
    let mut packets = dialogs;
    packets.push(OutgoingPacket {
        category: Kind::UpdateInventoryInfo.category(),
        body: wire::encode(
            Kind::UpdateInventoryInfo,
            Some(&Message::UpdateInventoryInfo(Some(slots))),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    });
    packets.extend(gear.packets);
    if let Some(balance) = balance {
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
    }
    packets.push(super::minimum_equip::item_fail("Success")?);
    Ok(Customized {
        packets,
        look: Some(gear.look),
    })
}
