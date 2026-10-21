//! EquipItemProcessor for the selected first Fiona in town.
//! Slot transitions use the recovered EquippableSlot kernel. Item persistence,
//! CostumeManager.Save and the Sync/UpdateItemStat projection run in one transaction;
//! the original used separate contexts, so a failed costume save could not undo an
//! equipped item there. Constraint/slot refusals roll back instead of keeping the
//! unsaved in-memory unequip that the original could later persist.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::{
    domain::{
        character_stats::Arithmetic,
        equipment_slot::{self, Operation, Slot},
        item_constraint::{self, Character, Equipped},
        map_partition::OutgoingPacket,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        equipment::{self as equipment_wire, Message as EquipmentMessage},
        inventory::{self as inventory_wire, Dict, HeroesString, Kind as InventoryKind},
    },
};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};

pub enum Outcome {
    /// Packets in Sync order (617, 458) followed by UpdateItemStat (619, 447).
    /// The refreshed look replaces the selection look for later channel entries.
    Applied {
        packets: Vec<OutgoingPacket>,
        look: OutgoingPacket,
    },
    /// SendOperationFailedDialog; no item or costume state changed.
    Refused(OutgoingPacket),
    /// Source paths that save nothing and send no frontend message.
    Unchanged,
    /// Quick slot cleared through EquipItem(-1, part): QuickSlotInfo only.
    QuickSlots(OutgoingPacket),
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
/// ItemOwner.ModifiedItems is a HashSet enumerated in first-registration order.
fn touch(id: i64, modified: &mut Vec<i64>) {
    if !modified.contains(&id) {
        modified.push(id)
    }
}
fn operation_name(op: Operation) -> &'static str {
    match op {
        Operation::Success => "Success",
        Operation::InvalidItemClass => "InvalidItemClass",
        Operation::EquipSystem => "EquipItemFail_System",
        Operation::InvalidSlot => "EquipItemFail_InvalidSlot",
        Operation::InvalidCategory => "EquipItemFail_InvalidItemCategory",
        Operation::EnhanceSystem => "EnhanceItemFail_System",
    }
}
/// ItemOwner.SendOperationFailedDialog: ItemFailMessage(HeroesString("GameUI_Heroes_" + result)).
pub fn item_fail(result: &str) -> Result<OutgoingPacket, MinimumError> {
    let message = inventory_wire::Message::ItemFail(Some(HeroesString {
        format: Some(format!("GameUI_Heroes_{result}")),
        parameters: Some(Vec::new()),
    }));
    Ok(OutgoingPacket {
        category: InventoryKind::ItemFail.category(),
        body: inventory_wire::encode(InventoryKind::ItemFail, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

struct Gear {
    equipment: BTreeMap<u8, Slot>,
    quick: BTreeSet<u8>,
}
impl Gear {
    fn new(assets: &Assets) -> Result<Self, MinimumError> {
        let parts = assets.appearance.normal_parts(&assets.catalog)?;
        let locked: BTreeSet<i32> = assets
            .appearance
            .inventory_locked_parts()?
            .into_iter()
            .collect();
        let mut groups: Vec<(u8, bool, Vec<Vec<u8>>)> = Vec::new();
        for p in &parts {
            if let Some((_, _, rows)) = groups.iter_mut().find(|(slot, _, _)| *slot == p.slot) {
                rows.push(utf16(&p.category));
            } else {
                groups.push((p.slot, p.consumable, vec![utf16(&p.category)]));
            }
        }
        let mut gear = Self {
            equipment: BTreeMap::new(),
            quick: BTreeSet::new(),
        };
        for (slot, consumable, rows) in groups {
            if consumable {
                gear.quick.insert(slot);
                continue;
            }
            let expanded = locked.contains(&i32::from(slot));
            let built = Slot::new(slot, rows.into_iter().map(|c| (Some(c), expanded)))
                .map_err(|_| fail())?;
            gear.equipment.insert(slot, built);
        }
        Ok(gear)
    }
    fn part_of(&self, id: i64) -> Option<u8> {
        self.equipment
            .values()
            .find(|s| s.value().is_some_and(|i| i.id == id))
            .map(Slot::part)
    }
}

#[derive(sqlx::FromRow)]
struct Target {
    item_class_utf16: Vec<u8>,
    equippable: bool,
    part: Option<i16>,
    reduced_durability: Option<i32>,
    max_durability_bonus: Option<i32>,
}

fn slot_item(
    assets: &Assets,
    stats: &super::item_stats::StatsIndex,
    id: i64,
    class: &[u8],
    part: Option<u8>,
    reduced: i32,
    bonus: i32,
) -> Result<equipment_slot::Item, MinimumError> {
    let profile = assets.equipment.as_ref().ok_or_else(fail)?;
    let metadata = profile.metadata(&assets.catalog, class)?;
    let base = stats.get(Some(class)).ok_or_else(fail)?;
    let d = crate::domain::item_durability::EquipmentDurability::load(base, base, reduced, bonus);
    Ok(equipment_slot::Item {
        id,
        category: utf16(&metadata.category),
        equippable: true,
        equipped_part: part,
        resolved_part: part.map(i32::from),
        // Same BadStatus predicate as the equipped Gear loader.
        bad_status: d.has_durability() && d.maximum() >= 0 && d.current() == 0,
        enhance_other: false,
    })
}

/// Admission (selected character, city state and second password) belongs to the frontend.
#[allow(clippy::too_many_arguments)]
pub async fn equip(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    part: i32,
    config: QueryConfig,
    arithmetic: Arithmetic,
    log: super::minimum_item_logs::Context,
) -> Result<Outcome, MinimumError> {
    // EquipItemProcessor constructor: GetJournalID and SetItemOpLog(EquipItem). No
    // Num changes occur, so no item log rows follow.
    let _journal = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let (connected, level, class): (bool, i32, i32) = sqlx::query_as(
            "SELECT is_connected,level,class::integer FROM game_character_info WHERE id=$1",
        )
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
        if !connected {
            return Err(fail());
        }
        // Serialize item writers before the shared-mode reads of the loaders.
        sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        // Validates marker/profile/ownership and refuses unsupported item states.
        super::minimum_equipment::load(&mut tx, owner, assets, cid).await?;
        let mut gear = Gear::new(assets)?;
        let stats = &assets.stats_index;
        let equipped:Vec<(i64,Vec<u8>,i16,i32,i32)>=sqlx::query_as("SELECT i.id,i.item_class_utf16,e.part,e.reduced_durability,e.max_durability_bonus FROM game_items i JOIN game_equippables e ON e.id=i.id WHERE i.owner_id=$1 AND e.part IS NOT NULL ORDER BY i.id LIMIT 4097").bind(cid).fetch_all(&mut *tx).await?;
        for (id, class, p, reduced, bonus) in equipped {
            let p = u8::try_from(p).map_err(|_| fail())?;
            let item = slot_item(assets, stats, id, &class, Some(p), reduced, bonus)?;
            let slot = gear.equipment.get_mut(&p).ok_or_else(fail)?;
            if !slot.load(item).map_err(|_| fail())? {
                return Err(fail());
            }
        }
        let mut modified: Vec<i64> = Vec::new();
        let profile = assets.equipment.as_ref().ok_or_else(fail)?;
        let artifact_feature = assets.catalog.policy.is_enabled(Some("Artifact"))?;
        let mut quick = super::minimum_quickslot::load(&mut tx, assets, cid).await?;
        let mut quick_changes = Vec::new();
        // Gear.GetEquippedArtifactQuickSlot: first quick slot whose class is ARTIFACT
        // (skills and empty values have no ItemClassInfo).
        let artifact_slot = |quick: &crate::domain::quick_slot::QuickSlots| {
            quick
                .values()
                .find(|(_, class)| {
                    !class.is_empty()
                        && !class.starts_with("skill_")
                        && profile
                            .category(&assets.catalog, &utf16(class))
                            .is_ok_and(|c| c == "ARTIFACT")
                })
                .map(|(part, _)| part)
        };
        if item_id == -1 {
            // ItemOwner.Unequip(part) -> Gear.Unequip(part). Quick slots are verified
            // absent; a part outside both collections raises in the source.
            let Ok(p) = u8::try_from(part) else {
                return Err(fail());
            };
            if let Some(slot) = gear.equipment.get_mut(&p) {
                // ItemOwner.Unequip(16) also clears the artifact quick slot.
                if p == 16
                    && let Some(a) = artifact_slot(&quick)
                {
                    quick_changes.extend(quick.clear(i32::from(a)).flatten());
                }
                if let (Some(old), _) = slot.clear() {
                    touch(old.id, &mut modified);
                }
            } else if gear.quick.contains(&p) {
                // StackableSlot.Clear: only QuickSlotInfo is synchronized.
                let Some(Some(change)) = quick.clear(part) else {
                    tx.commit().await?;
                    return Ok(Outcome::Unchanged);
                };
                super::minimum_quickslot::persist(&mut tx, cid, &[change]).await?;
                let packet = super::minimum_quickslot::packet(&quick)?;
                tx.commit().await?;
                return Ok(Outcome::QuickSlots(packet));
            } else {
                return Err(fail());
            }
        } else {
            let target: Option<Target> = sqlx::query_as("SELECT i.item_class_utf16,e.id IS NOT NULL AS equippable,e.part,e.reduced_durability,e.max_durability_bonus FROM game_items i LEFT JOIN game_equippables e ON e.id=i.id WHERE i.id=$1 AND i.owner_id=$2").bind(item_id).bind(cid).fetch_optional(&mut *tx).await?;
            let Some(target) = target else {
                return Ok(Outcome::Refused(item_fail("EquipItemFail_System")?));
            };
            if target
                .item_class_utf16
                .starts_with(&utf16("cash_freepass_"))
            {
                // Attributes are verified absent, therefore no LOOK attribute exists.
                return Ok(Outcome::Unchanged);
            }
            if !target.equippable {
                // Constraint metadata is exported only for the bound equipment profile.
                return Err(fail());
            }
            let current = target
                .part
                .map(|p| u8::try_from(p).map_err(|_| fail()))
                .transpose()?;
            if current.is_some() && current.map(i32::from) != Some(part) {
                let q = current.ok_or_else(fail)?;
                let slot = gear.equipment.get_mut(&q).ok_or_else(fail)?;
                if !matches!(slot.clear(), (Some(old), _) if old.id == item_id) {
                    return Err(fail());
                }
                touch(item_id, &mut modified);
            }
            let item = slot_item(
                assets,
                stats,
                item_id,
                &target.item_class_utf16,
                gear.part_of(item_id),
                target.reduced_durability.ok_or_else(fail)?,
                target.max_durability_bonus.ok_or_else(fail)?,
            )?;
            let skills: Vec<(Vec<u8>, i32)> =
                sqlx::query_as("SELECT skill_utf16,rank FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097")
                    .bind(cid)
                    .fetch_all(&mut *tx)
                    .await?;
            let skills = skills
                .into_iter()
                .map(|(k, r)| {
                    let units = k.as_chunks::<2>().0.iter().map(|a| u16::from_le_bytes(*a)).collect::<Vec<_>>();
                    String::from_utf16(&units).map(|k| (k, r)).map_err(|_| fail())
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            // Enhance/enchant attributes are verified absent for every equipped item.
            let levels: Vec<Equipped> = gear
                .equipment
                .values()
                .filter(|s| s.value().is_some())
                .map(|_| Equipped {
                    enhance: 0,
                    prefix: 0,
                    suffix: 0,
                })
                .collect();
            let constraint = assets
                .equipment
                .as_ref()
                .ok_or_else(fail)?
                .constraint(&assets.catalog, &target.item_class_utf16)?;
            // The lab frontend announces CafeType 0; guild membership is not loaded.
            let character = Character {
                level,
                class,
                cafe_type: 0,
                cafe: false,
                guild_level: None,
                skills: &skills,
                equipped: Some(&levels),
            };
            if let Some(failure) = item_constraint::check(&character, &constraint) {
                return Ok(Outcome::Refused(item_fail(failure.name())?));
            }
            let Some(slot) = u8::try_from(part)
                .ok()
                .and_then(|p| gear.equipment.get_mut(&p))
            else {
                return Ok(Outcome::Refused(item_fail("EquipItemFail_InvalidSlot")?));
            };
            let checked = slot.check(Some(&item)).map_err(|_| fail())?;
            if checked != Operation::Success {
                return Ok(Outcome::Refused(item_fail(operation_name(checked))?));
            }
            // ItemOwner.Equip: artifact quick slot sampled before Unequip(part).
            let saved_artifact = artifact_slot(&quick);
            if part == 16
                && let Some(a) = saved_artifact
            {
                quick_changes.extend(quick.clear(i32::from(a)).flatten());
            }
            if let (Some(old), _) = slot.clear() {
                touch(old.id, &mut modified);
            }
            // EquipArtifactQuickSlot: only for an ARTIFACT item with the feature on.
            if artifact_feature
                && profile.category(&assets.catalog, &target.item_class_utf16)? == "ARTIFACT"
            {
                let class_text = String::from_utf16(
                    &target
                        .item_class_utf16
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|a| u16::from_le_bytes(*a))
                        .collect::<Vec<_>>(),
                )
                .map_err(|_| fail())?;
                // Gear.Equip(part, itemClass) results are ignored by the source; an
                // empty equipment part fails as InvalidSlot, so quick slots are tried.
                let target_slot = match saved_artifact {
                    Some(a) => Some(a),
                    None => quick
                        .empty_for("ARTIFACT")
                        .first()
                        .copied()
                        .or_else(|| quick.max_part()),
                };
                if let Some(t) = target_slot
                    && quick.equipped(&class_text).is_none()
                    && let Ok(change) = quick.set(i32::from(t), &class_text, "ARTIFACT")
                {
                    quick_changes.push(change);
                }
            }
            let (result, _) = slot.set(Some(item)).map_err(|_| fail())?;
            if result != Operation::Success {
                return Err(fail());
            }
            touch(item_id, &mut modified);
        }
        if modified.is_empty() && quick_changes.is_empty() {
            tx.commit().await?;
            return Ok(Outcome::Unchanged);
        }
        // Gear.Unequip(part) runs UnequipConsumablesByConstraint over the quick slot
        // classes; only classes without enhance/enchant requirements are supported.
        // Skill entries have no ItemClassInfo and are skipped, as in the source.
        for class in quick.classes().filter(|c| !c.starts_with("skill_")) {
            let constraint = profile.constraint(&assets.catalog, &utf16(class))?;
            if constraint.required_enhance > 0
                || constraint.required_enchant > 0
                || class.starts_with("awakening_atk_potion")
                || class.starts_with("awakening_def_potion")
            {
                return Err(fail());
            }
        }
        for id in &modified {
            let part = gear.part_of(*id).map(i16::from);
            let changed = sqlx::query("UPDATE game_equippables e SET part=$2 FROM game_items i WHERE e.id=$1 AND i.id=e.id AND i.owner_id=$3")
                .bind(id)
                .bind(part)
                .bind(cid)
                .execute(&mut *tx)
                .await?
                .rows_affected();
            if changed != 1 {
                return Err(fail());
            }
        }
        let view =
            super::minimum_selection::view(&mut tx, owner, assets, cid, config, arithmetic, true, 0)
                .await?;
        let (before, after) = view.costumes.as_ref().ok_or_else(fail)?;
        super::character_costumes::persist_item_costumes(&mut tx, cid, before, after)
            .await
            .map_err(|_| fail())?;
        let inventory = super::minimum_inventory::snapshot_with_equipment(
            &mut tx,
            owner,
            assets,
            cid,
            Some(view.equipment.clone()),
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
        let update = inventory_wire::Message::UpdateInventoryInfo(Some(slots));
        let equipment = EquipmentMessage::EquipmentInfo(Dict(Some(view.equipment.clone())));
        let packets = vec![
            OutgoingPacket {
                category: InventoryKind::UpdateInventoryInfo.category(),
                body: inventory_wire::encode(
                    InventoryKind::UpdateInventoryInfo,
                    Some(&update),
                    DEFAULT_MAX_BODY,
                )
                .map_err(|_| fail())?
                .to_vec(),
            },
            OutgoingPacket {
                category: equipment.kind().category(),
                body: equipment_wire::encode(equipment.kind(), Some(&equipment), DEFAULT_MAX_BODY)
                    .map_err(|_| fail())?
                    .to_vec(),
            },
            view.stat,
            OutgoingPacket {
                category: crate::protocol::character_list::COSTUME_UPDATE_CATEGORY,
                body: crate::protocol::character_list::encode_costume_update(
                    Some(view.costume.as_ref()),
                    DEFAULT_MAX_BODY,
                )
                .map_err(|_| fail())?
                .to_vec(),
            },
        ];
        let mut packets = packets;
        if !quick_changes.is_empty() {
            // Sync: QuickSlots after inventory, equipment and UpdateItemStat.
            super::minimum_quickslot::persist(&mut tx, cid, &quick_changes).await?;
            packets.push(super::minimum_quickslot::packet(&quick)?);
        }
        tx.commit().await?;
        Ok(Outcome::Applied {
            packets,
            look: view.look,
        })
    })
    .await
    .map_err(|_| fail())?
}
