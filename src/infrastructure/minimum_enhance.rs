//! EnhanceItemByDataProcessor for a bag equipment item of the selected first Fiona:
//! domain::enhance_item decides; this applies the consumed materials and fail rewards
//! through the give planner, the ENHANCE attribute, the binding, the durability loss and
//! Destroy (MoveToBuybackStorage into the RestoreStorage, or out of the inventory), then
//! answers UpdateInventoryInfo and EnhanceItemResultMessage. Equipped items (their stats
//! and equipment sync) and the event enhance are not migrated.
use super::{
    character_lobby::UserKey,
    item_enhance::Contents,
    minimum_assets::Assets,
    minimum_character::MinimumError,
    minimum_give::Request,
    minimum_inventory::RESTORE_COLLECTION,
    minimum_item_stat::{ANTIBIND, ENHANCE, RESTORED},
};
use crate::{
    domain::{
        character_stats::Arithmetic,
        enhance::{self, EnhanceInfo, Recipe, Rune, RuneExtra, StatRows},
        enhance_item::{self, Data, Features, Random, Refusal, Target},
        give_item::GiveError,
        item_attribute::{self, Attributes},
        item_durability::EquipmentDurability,
        map_partition::OutgoingPacket,
        shop_price::{self, Arch},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, EnhanceItem, EnhanceItemResult, Kind, Message, Slot},
        mission::StrDict,
    },
};
use sqlx::{PgPool, Postgres, Transaction};

/// EventType.EnhanceItemByData and ActionType.Enhance.
const EVENT_ENHANCE_ITEM_BY_DATA: i16 = 29;
const ACTION_ENHANCE: i16 = 14;
/// ItemLogType.GiveItem / DestroyItem.
const LOG_GIVE: i16 = 2;
const LOG_DESTROY: i16 = 3;

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

struct ItemData<'a> {
    contents: &'a Contents,
    class: &'a [u8],
    class_text: &'a str,
    ratio: f32,
    arch: Arch,
}
impl Data for ItemData<'_> {
    fn info(&self, level: i32) -> Option<&EnhanceInfo> {
        self.contents.info(self.class, level)
    }
    fn stat_rows(&self, level: i32) -> Option<StatRows<'_>> {
        self.contents.stat_rows(self.class, level)
    }
    fn server_ratio(&self, level: i32) -> Option<f32> {
        self.contents.server_ratio(&level.to_string(), level)
    }
    fn rune(&self, class: &str) -> Option<Rune> {
        self.contents.rune(class).copied()
    }
    fn rune_extra(&self, relation: i32) -> Option<RuneExtra> {
        self.contents.rune_extra(relation).copied()
    }
    fn bonus(&self, material: &str, level: i32) -> Option<f64> {
        self.contents.bonus(material, level)
    }
    fn instead(&self, material: &str, level: i32) -> Option<(String, i32)> {
        self.contents
            .instead(material, level)
            .map(|(c, n)| (c.to_owned(), n))
    }
    fn recipe(&self) -> Option<&Recipe> {
        self.contents.make_item_recipe(self.class_text)
    }
    fn price(&self, class: &str, num: i32) -> Option<i32> {
        if class == "gold" {
            shop_price::ceiling10(num, self.ratio, self.arch)
        } else {
            shop_price::ceiling(num, self.ratio, self.arch)
        }
    }
}

/// The two static System.Random instances of the processor (EnhanceRandom for the roll,
/// random for the durability loss).
pub struct EnhanceRandom {
    pub roll: crate::domain::item_colors::ColorRandom,
    pub other: crate::domain::item_colors::ColorRandom,
}
impl Random for EnhanceRandom {
    fn next_double(&mut self) -> f64 {
        self.roll.next_double()
    }
    fn next(&mut self, maximum: i32) -> Option<i32> {
        self.other.next(maximum).ok()
    }
}

fn result_packet(result: EnhanceItemResult) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: Kind::EnhanceItemResult.category(),
        body: wire::encode(
            Kind::EnhanceItemResult,
            Some(&Message::EnhanceItemResult(result)),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    })
}

/// SetItemAttribute(name, value, arg, arg2): `None` removes the row.
async fn set_attribute(
    tx: &mut Transaction<'_, Postgres>,
    item: i64,
    current: &Attributes,
    name: &str,
    value: Option<&str>,
) -> Result<(), MinimumError> {
    match (current.get(name), value) {
        (None, None) => Ok(()),
        (Some(_), None) => {
            sqlx::query("DELETE FROM game_item_attributes WHERE item_id=$1 AND attribute_bytes=$2")
                .bind(item)
                .bind(name.as_bytes())
                .execute(&mut **tx)
                .await?;
            Ok(())
        }
        (Some(old), Some(v)) if old.value == v && old.arg == 0 && old.arg2 == 0 => Ok(()),
        (Some(_), Some(v)) => {
            item_attribute::Attribute::from_stored(name.as_bytes(), v.as_bytes(), 0, 0)
                .map_err(|_| fail())?;
            sqlx::query("UPDATE game_item_attributes SET value_bytes=$3,arg=0,arg2=0 WHERE item_id=$1 AND attribute_bytes=$2")
                .bind(item)
                .bind(name.as_bytes())
                .bind(v.as_bytes())
                .execute(&mut **tx)
                .await?;
            Ok(())
        }
        (None, Some(v)) => {
            let row = Attributes::new(vec![
                item_attribute::Attribute::from_stored(name.as_bytes(), v.as_bytes(), 0, 0)
                    .map_err(|_| fail())?,
            ])
            .map_err(|_| fail())?;
            super::minimum_give::insert_attributes(tx, item, &row).await
        }
    }
}

/// ItemContents.CalcRequiredBuybackAP for an item without QUALITY/PREFIX/SUFFIX/LOOK.
fn buyback_ap(
    contents: &Contents,
    class: &str,
    level: i32,
    required_level: i32,
    category: &str,
) -> Result<i32, MinimumError> {
    let mut ap = 0_i32;
    for row in contents.anti_bind() {
        let value = match row.target.as_deref() {
            Some("Quality") => "2".to_owned(),
            Some("ItemLevel") => required_level.to_string(),
            Some("Prefix" | "Suffix") => "0".to_owned(),
            Some("Enhance") => level.to_string(),
            Some("IsWeapon") if category.eq_ignore_ascii_case("WEAPON") => String::new(),
            Some("IsArtifact") if category.eq_ignore_ascii_case("ARTIFACT") => String::new(),
            Some("ItemClass") => class.to_owned(),
            Some("Base") => String::new(),
            _ => continue,
        };
        if enhance::anti_bind_contains(row, &value.to_lowercase()).ok_or_else(fail)? {
            ap = ap.wrapping_add(row.ap);
        }
    }
    Ok(ap)
}

fn requests<'a>(rows: &'a [(Vec<u8>, i32)], empty: &'a Attributes) -> Vec<Request<'a>> {
    rows.iter()
        .map(|(class, count)| Request {
            class,
            count: *count,
            bound: false,
            equipment: None,
            attributes: empty,
        })
        .collect()
}

/// The updated slot of an item, or its deleted SlotInfo.
async fn slot_of(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    id: i64,
) -> Result<Slot, MinimumError> {
    Ok(
        super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
            .await?
            .slots
            .ok_or_else(fail)?
            .into_iter()
            .flatten()
            .find(|s| s.id == id)
            .unwrap_or_else(|| super::minimum_move::deleted_slot(id)),
    )
}

/// Locks the assets' enhance random for the processor.
pub async fn enhance(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: &EnhanceItem,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let mut random = assets.enhance_random.lock().await;
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        enhance_with(
            pool,
            owner,
            assets,
            cid,
            request,
            &mut *random,
            entity_ids,
            log,
        ),
    )
    .await
    .map_err(|_| fail())?
}

#[allow(clippy::too_many_arguments)]
async fn enhance_with(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: &EnhanceItem,
    random: &mut (dyn Random + Send),
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let policy = contents.policy().ok_or_else(fail)?;
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let arithmetic = assets.item_arithmetic.ok_or_else(fail)?;
    let arch = match arithmetic {
        Arithmetic::FrameworkX86 => Arch::X86,
        Arithmetic::FrameworkX64 => Arch::X64,
    };
    // A null AdditionalMaterial throws in GetEnhanceRuneInfo (Dictionary key).
    let additional = request.additional_material.as_deref().ok_or_else(fail)?;
    let material1 = request.material1.as_deref().unwrap_or("");
    let material2 = request.material2.as_deref().unwrap_or("");
    let operation = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let journal = super::minimum_item_logs::named_journal(pool, log.flags, "EnhanceItem").await;
    let mut ids = entity_ids.into_iter();
    let gold_class = text(assets.inventory.gold_class())?;
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
    let dialog = |r: Refusal| -> Result<Vec<OutgoingPacket>, MinimumError> {
        Ok(vec![super::minimum_equip::item_fail(r.name())?])
    };
    let row: Option<(Vec<u8>, i16)> = sqlx::query_as(
        "SELECT item_class_utf16,collection FROM game_items WHERE id=$1 AND owner_id=$2",
    )
    .bind(request.item_id)
    .bind(cid)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((class, collection)) = row else {
        return dialog(Refusal::NoEnhanceInfo);
    };
    let indexed = assets.stats_index.class(Some(&class)).ok_or_else(fail)?;
    if !indexed.equippable {
        return dialog(Refusal::NoEnhanceInfo);
    }
    let equipment: (Option<i16>, i32, i32) = sqlx::query_as(
        "SELECT part,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=$1",
    )
    .bind(request.item_id)
    .fetch_one(&mut *tx)
    .await?;
    // Equipped items also change the character stats and EquipmentInfo.
    if equipment.0.is_some() || !matches!(collection, 0 | 1 | 6) {
        return Err(fail());
    }
    let class_text = text(&class)?;
    let attributes = super::minimum_inventory::load_attributes(&mut tx, &[request.item_id])
        .await?
        .remove(&request.item_id)
        .unwrap_or_default();
    let before = super::minimum_item_stat::equipment(assets, &class, &attributes)?;
    let base = assets.stats_index.get(Some(&class)).ok_or_else(fail)?;
    let durability = EquipmentDurability::load(base, &before.stat, equipment.1, equipment.2);
    let target = Target {
        level: before.enhance_level,
        max_armor_broken: durability.has_durability() && durability.maximum() == 0,
        guild_price_bonus: 0,
    };
    let data = ItemData {
        contents,
        class: &class,
        class_text: &class_text,
        ratio: contents.craft_price_ratio().ok_or_else(fail)?,
        arch,
    };
    let features = Features {
        event_enhance_a_shot: policy.is_enabled(Some("Event_enhance_a_shot"))?,
        event_change_ratio: policy.is_enabled(Some("Event_enhance_change_ratio"))?,
        success_bonus: policy.get_nullable_integer("EnhanceSuccessBonus")?,
        success_bonus_limit: policy.get_nullable_integer("EnhanceSuccessBonus_Limit")?,
        gold_class: gold_class.clone(),
    };
    let now = super::lab_clock::utc_now().map_err(|_| fail())?.ticks();
    let useable: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(count),0)::BIGINT FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2 AND collection IN (0,1,6,100) AND (expire_ticks IS NULL OR expire_ticks>=$3)",
    )
    .bind(cid)
    .bind(utf16(additional))
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    let enhance_request = enhance_item::Request {
        material1,
        material2,
        additional,
        event: request.is_event_enhance_a_shot,
    };
    let prepared =
        match enhance_item::prepare(&enhance_request, &target, &data, &features, &|_| useable)
            .map_err(|_| fail())?
        {
            Ok(p) => p,
            Err(refusal) => return dialog(refusal),
        };
    let outcome = enhance_item::resolve(
        &prepared,
        &enhance_request,
        &target,
        &data,
        random,
        arithmetic,
    )
    .map_err(|_| fail())?;
    let prev_class =
        item_attribute::build_class_ex(&class_text, &attributes).map_err(|_| fail())?;
    // DestroyItem(materials) on success; CraftItem(materials, fail rewards) otherwise.
    let destroy_rows: Vec<(Vec<u8>, i32)> = outcome
        .destroy
        .iter()
        .map(|(c, n)| (utf16(c), *n))
        .collect();
    let give_rows: Vec<(Vec<u8>, i32)> = outcome.give.iter().map(|(c, n)| (utf16(c), *n)).collect();
    let empty = Attributes::default();
    let mut next_id = || ids.next().ok_or_else(fail);
    let applied = match super::minimum_give::apply(
        &mut tx,
        owner,
        assets,
        cid,
        &requests(&destroy_rows, &empty),
        &requests(&give_rows, &empty),
        &mut next_id,
    )
    .await?
    {
        Ok(a) => a,
        Err(GiveError::NotEnoughItem) => return dialog(Refusal::NotEnoughMaterial),
        // Leftovers of the source go to the mailbox, which is not migrated.
        Err(_) => return Err(fail()),
    };
    let mut slots = applied.slots.clone();
    let mut logs = vec![applied];
    if outcome.destroy_rune {
        let rune = [(utf16(additional), 1)];
        // ItemOwner.DestroyItem(class, 1): its result is ignored.
        if let Ok(a) = super::minimum_give::apply(
            &mut tx,
            owner,
            assets,
            cid,
            &requests(&rune, &empty),
            &[],
            &mut next_id,
        )
        .await?
        {
            slots.extend(a.slots.iter().cloned());
            logs.push(a);
        }
    }
    let mut restore_notice = false;
    let mut overflow = Vec::new();
    // EnhanceLevel setter: 0 removes the attribute.
    if outcome.level != target.level {
        let value = (outcome.level != 0).then(|| outcome.level.to_string());
        set_attribute(
            &mut tx,
            request.item_id,
            &attributes,
            ENHANCE,
            value.as_deref(),
        )
        .await?;
    }
    let mut current = super::minimum_inventory::load_attributes(&mut tx, &[request.item_id])
        .await?
        .remove(&request.item_id)
        .unwrap_or_default();
    let after = super::minimum_item_stat::equipment(assets, &class, &current)?;
    let mut state = EquipmentDurability::load(base, &after.stat, equipment.1, equipment.2);
    let (limit, _) = assets.inventory.limits()?;
    if outcome.max_durability_loss != 0 {
        state.set_maximum(
            state.maximum().wrapping_sub(outcome.max_durability_loss),
            limit,
        );
    }
    if !outcome.success && prepared.rune.is_none() {
        // item.Durability = durability (the value before the failure).
        state.set_current(durability.current());
    }
    sqlx::query(
        "UPDATE game_equippables SET reduced_durability=$2,max_durability_bonus=$3 WHERE id=$1",
    )
    .bind(request.item_id)
    .bind(state.reduced())
    .bind(state.bonus())
    .execute(&mut *tx)
    .await?;
    if outcome.bind {
        sqlx::query("UPDATE game_items SET tradable=false WHERE id=$1")
            .bind(request.item_id)
            .execute(&mut *tx)
            .await?;
    }
    if outcome.destroyed {
        let restore = policy.is_enabled(Some("RestoreItem"))?;
        let did_restored = current
            .get(RESTORED)
            .is_some_and(|a| enhance::level(Some(&a.value)) != 0);
        let economy_row = economy.get(indexed.source_index).ok_or_else(fail)?;
        let category = assets
            .equipment
            .as_ref()
            .ok_or_else(fail)?
            .category(&assets.catalog, &class)?;
        let ap = buyback_ap(
            contents,
            &class_text,
            outcome.level,
            economy_row.required_level,
            &category,
        )?;
        if restore && !did_restored && ap > 0 {
            // MoveToBuybackStorage: AntiBindCount = MaxAntiBind, bound, restored, then the
            // latest RestoreStorage slot and RemoveOverflowedItems.
            let max_anti_bind = contents.max_anti_bind(&class).ok_or_else(fail)?;
            let anti = (max_anti_bind != 0).then(|| max_anti_bind.to_string());
            set_attribute(
                &mut tx,
                request.item_id,
                &current,
                ANTIBIND,
                anti.as_deref(),
            )
            .await?;
            set_attribute(&mut tx, request.item_id, &current, RESTORED, Some("1")).await?;
            let latest: Option<i32> = sqlx::query_scalar(
                "SELECT max(slot) FROM game_items WHERE owner_id=$1 AND collection=$2",
            )
            .bind(cid)
            .bind(RESTORE_COLLECTION)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query("UPDATE game_items SET collection=$3,slot=$4,tradable=false WHERE id=$1 AND owner_id=$2")
                .bind(request.item_id)
                .bind(cid)
                .bind(RESTORE_COLLECTION)
                .bind(latest.map_or(0, |s| s.wrapping_add(1)))
                .execute(&mut *tx)
                .await?;
            let count = policy.get_integer("RestoreItemCount")?;
            let held: Vec<i64> = sqlx::query_scalar(
                "SELECT id FROM game_items WHERE owner_id=$1 AND collection=$2 ORDER BY slot,id",
            )
            .bind(cid)
            .bind(RESTORE_COLLECTION)
            .fetch_all(&mut *tx)
            .await?;
            let excess = held
                .len()
                .saturating_sub(usize::try_from(count).unwrap_or(0));
            if excess > 0 {
                for id in &held[..excess] {
                    super::minimum_give::delete_attributes(&mut tx, *id).await?;
                    sqlx::query("DELETE FROM game_equippables WHERE id=$1")
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                    sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
                        .bind(id)
                        .bind(cid)
                        .execute(&mut *tx)
                        .await?;
                    overflow.push(*id);
                }
                // ArrangeSlot(null): the remaining items take slots 0..n in order.
                for (n, id) in held[excess..].iter().enumerate() {
                    sqlx::query("UPDATE game_items SET slot=$2 WHERE id=$1")
                        .bind(id)
                        .bind(i32::try_from(n).map_err(|_| fail())?)
                        .execute(&mut *tx)
                        .await?;
                }
            }
        } else {
            restore_notice = restore;
            super::minimum_give::delete_attributes(&mut tx, request.item_id).await?;
            sqlx::query("DELETE FROM game_equippables WHERE id=$1")
                .bind(request.item_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
                .bind(request.item_id)
                .bind(cid)
                .execute(&mut *tx)
                .await?;
        }
        current = Attributes::default();
    }
    slots.push(Some(
        slot_of(&mut tx, owner, assets, cid, request.item_id).await?,
    ));
    for id in &overflow {
        slots.push(Some(super::minimum_move::deleted_slot(*id)));
    }
    let enhanced_class = if outcome.destroyed {
        String::new()
    } else {
        let after = super::minimum_inventory::load_attributes(&mut tx, &[request.item_id])
            .await?
            .remove(&request.item_id)
            .unwrap_or(current);
        item_attribute::build_class_ex(&class_text, &after).map_err(|_| fail())?
    };
    tx.commit().await?;
    for a in &logs {
        super::minimum_item_logs::write_give(
            pool,
            cid,
            operation,
            EVENT_ENHANCE_ITEM_BY_DATA,
            a,
            log,
        )
        .await;
    }
    for (class, count) in &destroy_rows {
        super::minimum_item_logs::write_requested_ledger(
            pool,
            cid,
            journal,
            LOG_DESTROY,
            ACTION_ENHANCE,
            class,
            *count,
            log.flags,
        )
        .await;
    }
    for (class, count) in &give_rows {
        super::minimum_item_logs::write_requested_ledger(
            pool,
            cid,
            journal,
            LOG_GIVE,
            ACTION_ENHANCE,
            class,
            *count,
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
    if let Some(result) = outcome.result {
        packets.push(result_packet(EnhanceItemResult {
            result: Some(result),
            item_id: request.item_id,
            prev_item_class: Some(prev_class),
            enhanced_item_class: Some(enhanced_class),
            fail_reward: StrDict(Some(outcome.fail_reward)),
        })?);
    }
    if restore_notice {
        packets.push(super::minimum_shop::system_message(
            0,
            "GameUI_Heroes_RestoreItem_System_Notify",
            &[],
        )?);
    }
    Ok(packets)
}
