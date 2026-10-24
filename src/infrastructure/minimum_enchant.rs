//! EnchantItemProcessor and EnchantSession for the selected first Fiona: the session
//! opens on a bag equipment item with an enchant scroll (and an optional indestructible
//! scroll), dice rolls (EnchantDiceInfo) raise the success ratio, and DoEnchant applies
//! the PREFIX/SUFFIX attribute or the failure (protection, destruction into the
//! RestoreStorage, durability loss). The session lives with the selected character.
//! Equipped targets, timed enchants and enchant_elixir_br are not migrated.
use super::{
    character_lobby::UserKey,
    minimum_assets::Assets,
    minimum_character::MinimumError,
    minimum_item_stat::{PREFIX, SUFFIX},
};
use crate::{
    domain::{
        enchant::{self, Protection, Random, result},
        give_item::GiveError,
        item_attribute::{self, Attributes},
        item_durability::EquipmentDurability,
        map_partition::OutgoingPacket,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, EnchantItem, EnchantItemResult, Kind, Message, Slot},
    },
};
use sqlx::{PgPool, Postgres, Transaction};

/// The open EnchantSession of the selected character.
pub struct Session {
    target: i64,
    scroll: i64,
    rune: i64,
    attribute: &'static str,
    enchant_class: String,
    current: i32,
    goal: i32,
    games_left: i32,
}

/// The three static System.Random instances: EnchantSession.random (DoEnchant),
/// enchantDiceRandom (RollDice) and enchantArgRandom (GetRandomArg).
pub struct EnchantRandom {
    pub session: crate::domain::item_colors::ColorRandom,
    pub dice: crate::domain::item_colors::ColorRandom,
    pub arg: crate::domain::item_colors::ColorRandom,
}
struct SessionRandom<'a>(&'a mut crate::domain::item_colors::ColorRandom);
impl Random for SessionRandom<'_> {
    fn next_double(&mut self) -> f64 {
        self.0.next_double()
    }
    fn next(&mut self, maximum: i32) -> Option<i32> {
        self.0.next(maximum).ok()
    }
}
/// Random.Next(minValue, maxValue) for a range that fits Int32.
fn next_range(
    random: &mut crate::domain::item_colors::ColorRandom,
    min: i32,
    max: i32,
) -> Option<i32> {
    let range = max.checked_sub(min)?;
    if range < 0 {
        return None;
    }
    Some(random.next(range).ok()?.wrapping_add(min))
}

/// EventType.EnchantItem.
const EVENT_ENCHANT_ITEM: i16 = 28;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(b: &[u8]) -> Result<String, MinimumError> {
    String::from_utf16(
        &b.as_chunks::<2>()
            .0
            .iter()
            .map(|v| u16::from_le_bytes(*v))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
/// EnchantItemResultMessage(result, current, goal, rolledDice, successRatio).
fn result_packet(
    r: i32,
    current: i32,
    goal: i32,
    dice: &str,
    ratio: f32,
) -> Result<OutgoingPacket, MinimumError> {
    packet(
        Kind::EnchantItemResult,
        Message::EnchantItemResult(EnchantItemResult {
            current_value: current,
            goal_value: goal,
            result: r,
            rolled_dice: Some(dice.to_owned()),
            current_success_ratio: (ratio * 100.0) as i32,
        }),
    )
}

#[derive(sqlx::FromRow)]
struct Row {
    item_class_utf16: Vec<u8>,
    count: i32,
    collection: i16,
    expire_ticks: Option<i64>,
}
async fn item(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    id: i64,
) -> Result<Option<Row>, MinimumError> {
    Ok(sqlx::query_as(
        "SELECT item_class_utf16,count,collection,expire_ticks FROM game_items WHERE id=$1 AND owner_id=$2",
    )
    .bind(id)
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?)
}
fn expired(row: &Row, now: i64) -> bool {
    row.expire_ticks.is_some_and(|t| t < now)
}

/// The target's Item.Stat, durability and IsMaxArmorBroken.
async fn target_state(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    id: i64,
    class: &[u8],
) -> Result<(Attributes, EquipmentDurability, bool), MinimumError> {
    let (part, reduced, bonus): (Option<i16>, i32, i32) = sqlx::query_as(
        "SELECT part,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await?;
    // An equipped target also changes the character stats and EquipmentInfo.
    if part.is_some() {
        return Err(fail());
    }
    let attributes = super::minimum_inventory::load_attributes(tx, &[id])
        .await?
        .remove(&id)
        .unwrap_or_default();
    let context = super::minimum_item_stat::context(tx, cid).await?;
    let stat =
        super::minimum_item_stat::equipment_with(assets, class, &attributes, Some(&context))?;
    let base = assets.stats_index.get(Some(class)).ok_or_else(fail)?;
    let durability = EquipmentDurability::load(base, &stat.stat, reduced, bonus);
    let broken = durability.has_durability() && durability.maximum() == 0;
    Ok((attributes, durability, broken))
}

/// EnchantSession.NewSession; `None` is OpenSessionFail.
async fn open(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    request: &EnchantItem,
) -> Result<Option<Session>, MinimumError> {
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let Some(target) = item(tx, cid, request.target_item_id).await? else {
        return Ok(None);
    };
    let Some(scroll) = item(tx, cid, request.enchant_scroll_item_id).await? else {
        return Ok(None);
    };
    if !text(&scroll.item_class_utf16)?.starts_with("enchant_scroll") {
        return Ok(None);
    }
    // GetEquipItemInfo() of a non-equipment target is null: the source throws.
    let enchantable = contents
        .enchantable(&target.item_class_utf16)
        .ok_or_else(fail)?;
    if enchantable == 0 {
        return Ok(None);
    }
    let scroll_attributes =
        super::minimum_inventory::load_attributes(tx, &[request.enchant_scroll_item_id])
            .await?
            .remove(&request.enchant_scroll_item_id)
            .unwrap_or_default();
    let (attribute, value) = match (scroll_attributes.get(PREFIX), scroll_attributes.get(SUFFIX)) {
        (Some(p), None) => (PREFIX, p.value.clone()),
        (None, Some(s)) => (SUFFIX, s.value.clone()),
        _ => return Ok(None),
    };
    let Some(info) = contents.enchant(&value) else {
        return Ok(None);
    };
    if !matches!(target.collection, 0 | 1 | 6) {
        return Err(fail());
    }
    let (_, _, broken) = target_state(
        tx,
        assets,
        cid,
        request.target_item_id,
        &target.item_class_utf16,
    )
    .await?;
    if !info.ignore_max_armor && broken {
        return Ok(None);
    }
    let category = assets
        .equipment
        .as_ref()
        .ok_or_else(fail)?
        .category(&assets.catalog, &target.item_class_utf16)?;
    let equip_class = assets
        .equip_class
        .get(&target.item_class_utf16)
        .cloned()
        .flatten();
    if !enchant::is_available_to(
        info,
        &text(&target.item_class_utf16)?,
        &category,
        equip_class.as_deref(),
    ) {
        return Ok(None);
    }
    Ok(Some(Session {
        target: request.target_item_id,
        scroll: request.enchant_scroll_item_id,
        rune: request.indestructible_scroll_item_id,
        attribute,
        enchant_class: value,
        current: 0,
        goal: info.mini_game,
        games_left: 5,
    }))
}

/// Item.Num-- of one specific item (attributes and equippable go with the last unit).
async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    id: i64,
) -> Result<(), MinimumError> {
    let count: i32 = sqlx::query_scalar("SELECT count FROM game_items WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(cid)
        .fetch_one(&mut **tx)
        .await?;
    if count <= 1 {
        super::minimum_enhance::delete_item(tx, cid, id).await
    } else {
        sqlx::query("UPDATE game_items SET count=count-1 WHERE id=$1 AND owner_id=$2")
            .bind(id)
            .bind(cid)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

async fn slot_of(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    ids: &[i64],
) -> Result<Vec<Option<Slot>>, MinimumError> {
    let slots: Vec<Slot> =
        super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
            .await?
            .slots
            .ok_or_else(fail)?
            .into_iter()
            .flatten()
            .collect();
    Ok(ids
        .iter()
        .map(|id| {
            Some(
                slots
                    .iter()
                    .find(|s| s.id == *id)
                    .cloned()
                    .unwrap_or_else(|| super::minimum_move::deleted_slot(*id)),
            )
        })
        .collect())
}

/// EnchantItemMessage after the second password, in town.
#[allow(clippy::too_many_arguments)]
pub async fn enchant(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    session: &mut Option<Session>,
    request: &EnchantItem,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let policy = contents.policy().ok_or_else(fail)?;
    let bonus = policy.get_integer("EnchantSuccessBonus")?;
    let mut random = assets.enchant_random.lock().await;
    let operation = super::minimum_item_logs::operation_journal(pool, log.flags).await;
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
    super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None).await?;
    let now = super::lab_clock::utc_now().map_err(|_| fail())?.ticks();
    let mut packets = Vec::new();
    if request.open_new_session {
        if let Some(scroll) = item(&mut tx, cid, request.enchant_scroll_item_id).await?
            && expired(&scroll, now)
        {
            return Ok(vec![result_packet(
                result::SCROLL_EXPIRED,
                -1,
                -1,
                "",
                0.0,
            )?]);
        }
        if let Some(rune) = item(&mut tx, cid, request.indestructible_scroll_item_id).await?
            && expired(&rune, now)
        {
            return Ok(vec![result_packet(result::RUNE_EXPIRED, -1, -1, "", 0.0)?]);
        }
        *session = open(&mut tx, assets, cid, request).await?;
    }
    let Some(open) = session.as_mut() else {
        return Ok(vec![result_packet(
            result::OPEN_SESSION_FAIL,
            -1,
            -1,
            "",
            0.0,
        )?]);
    };
    let info = contents
        .enchant(&open.enchant_class)
        .ok_or_else(fail)?
        .clone();
    let ratio = enchant::success_ratio(&info, open.current, open.goal, bonus);
    let dice = request.dice_item_class.as_deref().unwrap_or("");
    let mut restore_notice = false;
    match dice {
        "CancelEnchant" => {
            *session = None;
            packets.push(result_packet(result::CANCELED, -1, -1, "", 0.0)?);
        }
        "DoEnchant" => {
            let scroll = item(&mut tx, cid, open.scroll).await?;
            let target = item(&mut tx, cid, open.target).await?;
            let outcome = match (scroll, target) {
                (None, _) => Err(result::INVALID_SCROLL_ITEM),
                (_, None) => Err(result::INVALID_TARGET_ITEM),
                (Some(_), Some(t)) if t.count != 1 => Err(result::INVALID_TARGET_ITEM),
                (Some(_), Some(t)) => Ok(t),
            };
            let target_row = match outcome {
                Ok(t) => t,
                Err(r) => {
                    tx.commit().await?;
                    return Ok(vec![result_packet(r, -1, -1, "", 0.0)?]);
                }
            };
            let class = target_row.item_class_utf16.clone();
            let (attributes, durability, broken) =
                target_state(&mut tx, assets, cid, open.target, &class).await?;
            if broken {
                tx.commit().await?;
                return Ok(vec![result_packet(
                    result::INVALID_TARGET_ITEM,
                    -1,
                    -1,
                    "",
                    0.0,
                )?]);
            }
            // Timed enchants (Duration > 0) set an expiring Arg2: not migrated.
            if info.duration > 0 {
                return Err(fail());
            }
            let rune = item(&mut tx, cid, open.rune).await?;
            let rune_class = rune
                .as_ref()
                .map(|r| text(&r.item_class_utf16))
                .transpose()?;
            let protection = Protection::of(rune_class.as_deref());
            let destructible = policy.is_enabled(Some("Enchant_DestructibleItems"))?;
            let decided = enchant::do_enchant(
                &info,
                ratio,
                protection,
                destructible,
                &mut SessionRandom(&mut random.session),
            )
            .ok_or_else(fail)?;
            let class_text = text(&class)?;
            let target_ex =
                item_attribute::build_class_ex(&class_text, &attributes).map_err(|_| fail())?;
            if let Some(dialog) = decided.dialog {
                let mut parameters = vec![target_ex.clone()];
                if decided.result == result::SCROLL_PROTECTED {
                    let scroll_attributes =
                        super::minimum_inventory::load_attributes(&mut tx, &[open.scroll])
                            .await?
                            .remove(&open.scroll)
                            .unwrap_or_default();
                    let scroll_row = item(&mut tx, cid, open.scroll).await?.ok_or_else(fail)?;
                    parameters.push(
                        item_attribute::build_class_ex(
                            &text(&scroll_row.item_class_utf16)?,
                            &scroll_attributes,
                        )
                        .map_err(|_| fail())?,
                    );
                }
                packets.push(super::minimum_shop::system_message(4, dialog, &parameters)?);
            }
            let mut changed = Vec::new();
            if decided.consume_scroll {
                consume(&mut tx, cid, open.scroll).await?;
                changed.push(open.scroll);
            }
            if decided.consume_rune && rune.is_some() {
                consume(&mut tx, cid, open.rune).await?;
                changed.push(open.rune);
            }
            let mut overflow = Vec::new();
            if decided.enchanted {
                let arg = next_range(&mut random.arg, info.min_arg, info.max_arg.wrapping_add(1))
                    .ok_or_else(fail)?;
                super::minimum_enhance::set_attribute_args(
                    &mut tx,
                    open.target,
                    &attributes,
                    open.attribute,
                    Some(&info.class),
                    arg,
                    0,
                )
                .await?;
                if decided.bind {
                    sqlx::query("UPDATE game_items SET tradable=false WHERE id=$1")
                        .bind(open.target)
                        .execute(&mut *tx)
                        .await?;
                }
            } else if decided.destroyed {
                let context = super::minimum_item_stat::context(&mut tx, cid).await?;
                let stat = super::minimum_item_stat::equipment_with(
                    assets,
                    &class,
                    &attributes,
                    Some(&context),
                )?;
                let (notice, removed) = super::minimum_enhance::destroy_or_restore(
                    &mut tx,
                    assets,
                    cid,
                    open.target,
                    &class,
                    &attributes,
                    (stat.enhance_level, stat.prefix_level, stat.suffix_level),
                )
                .await?;
                restore_notice = notice;
                overflow = removed;
            } else if decided.result == result::ENCHANT_FAILED {
                let before = durability.current();
                let mut state = durability;
                let (limit, _) = assets.inventory.limits()?;
                if decided.max_durability_loss != 0 {
                    state.set_maximum(
                        state.maximum().wrapping_sub(decided.max_durability_loss),
                        limit,
                    );
                }
                state.set_current(before);
                sqlx::query("UPDATE game_equippables SET reduced_durability=$2,max_durability_bonus=$3 WHERE id=$1")
                    .bind(open.target)
                    .bind(state.reduced())
                    .bind(state.bonus())
                    .execute(&mut *tx)
                    .await?;
            }
            changed.push(open.target);
            changed.extend(overflow);
            let slots = slot_of(&mut tx, owner, assets, cid, &changed).await?;
            tx.commit().await?;
            packets.push(packet(
                Kind::UpdateInventoryInfo,
                Message::UpdateInventoryInfo(Some(slots)),
            )?);
            if decided.result == result::ENCHANT_SUCCESS && info.level >= 7 {
                *session = None;
            }
            packets.push(result_packet(decided.result, -1, -1, "", 0.0)?);
            if restore_notice {
                packets.push(super::minimum_shop::system_message(
                    0,
                    "GameUI_Heroes_RestoreItem_System_Notify",
                    &[],
                )?);
            }
            return Ok(packets);
        }
        _ => {
            // RollDice(itemClassEx).
            if open.games_left <= 0 {
                packets.push(result_packet(
                    result::NO_MORE_ROLL_AVAILABLE,
                    open.current,
                    open.goal,
                    "",
                    ratio,
                )?);
            } else if let Some((min, max)) = contents.enchant_dice(dice) {
                let value =
                    next_range(&mut random.dice, min, max.wrapping_add(1)).ok_or_else(fail)?;
                let class = utf16(dice);
                let empty = Attributes::default();
                let request = super::minimum_give::Request {
                    class: &class,
                    count: 1,
                    bound: false,
                    equipment: None,
                    attributes: &empty,
                };
                let mut next = || ids.next().ok_or_else(fail);
                match super::minimum_give::apply(
                    &mut tx,
                    owner,
                    assets,
                    cid,
                    std::slice::from_ref(&request),
                    &[],
                    &mut next,
                )
                .await?
                {
                    Ok(applied) => {
                        open.games_left -= 1;
                        open.current = open.current.wrapping_add(value);
                        let ratio = enchant::success_ratio(&info, open.current, open.goal, bonus);
                        tx.commit().await?;
                        super::minimum_item_logs::write_give(
                            pool,
                            cid,
                            operation,
                            EVENT_ENCHANT_ITEM,
                            &applied,
                            log,
                        )
                        .await;
                        packets.push(packet(
                            Kind::UpdateInventoryInfo,
                            Message::UpdateInventoryInfo(Some(applied.slots)),
                        )?);
                        packets.push(result_packet(
                            result::ROLL_SUCCESS,
                            open.current,
                            open.goal,
                            dice,
                            ratio,
                        )?);
                        return Ok(packets);
                    }
                    Err(GiveError::NotEnoughItem) => packets.push(result_packet(
                        result::NO_DICE_ITEM,
                        open.current,
                        open.goal,
                        "",
                        ratio,
                    )?),
                    Err(_) => return Err(fail()),
                }
            } else {
                // enchant_elixir_br[VALUE:n] selects by its VALUE attribute: not migrated.
                if crate::domain::mission::item_class(dice) == "enchant_elixir_br" {
                    return Err(fail());
                }
                packets.push(result_packet(
                    result::INVALID_DICE_CLASS,
                    open.current,
                    open.goal,
                    "",
                    ratio,
                )?);
            }
        }
    }
    tx.commit().await?;
    Ok(packets)
}
