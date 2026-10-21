//! Quick slots of the selected first Fiona: loader for ToQuickSlotInfo and the
//! EquipBundleProcessor (assign or clear an item class), synchronized with
//! QuickSlotInfoMessage, including skill quick slots ("skill_" classes).
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        item_constraint::{self, Character, Equipped},
        map_partition::OutgoingPacket,
        quick_slot::{Change, QuickSlots},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Bundle, Dict, Kind, Message},
    },
};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};

pub enum Outcome {
    /// QuickSlotInfoMessage after Save/Sync.
    Applied(OutgoingPacket),
    /// SendOperationFailedDialog (ItemFailMessage); nothing changed.
    Refused(OutgoingPacket),
    /// Success without a registered change: no Sync message.
    Unchanged,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    if raw.len() > 100 || !raw.len().is_multiple_of(2) {
        return Err(fail());
    }
    String::from_utf16(
        &raw.as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}

/// Gear constructor: consumable parts in content order with their StackableSlot
/// lock, then GetQuickSlots rows. Expanded-slot rows are verified absent.
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
) -> Result<QuickSlots, MinimumError> {
    let parts = assets.appearance.normal_parts(&assets.catalog)?;
    let locked: BTreeSet<i32> = assets
        .appearance
        .inventory_locked_parts()?
        .into_iter()
        .collect();
    let mut groups: Vec<(u8, bool, Vec<String>)> = Vec::new();
    for p in &parts {
        if let Some((_, consumable, rows)) = groups.iter_mut().find(|(s, _, _)| *s == p.slot) {
            if *consumable {
                rows.push(p.category.clone());
            }
        } else if p.consumable {
            groups.push((p.slot, true, vec![p.category.clone()]));
        } else {
            groups.push((p.slot, false, Vec::new()));
        }
    }
    let mut slots = QuickSlots::new(
        groups
            .into_iter()
            .filter(|(_, consumable, _)| *consumable)
            .map(|(slot, _, categories)| (slot, locked.contains(&i32::from(slot)), categories)),
    )
    .map_err(|_| fail())?;
    let rows: Vec<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT item_class_utf16,slot FROM game_quick_slots WHERE owner_id=$1 ORDER BY row_id LIMIT 257",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 256 {
        return Err(fail());
    }
    for (class, slot) in rows {
        slots.load(slot, text(&class)?).map_err(|_| fail())?;
    }
    Ok(slots)
}

pub(super) fn info(slots: &QuickSlots) -> wire::QuickSlots {
    wire::QuickSlots {
        classes: Dict(Some(slots.info())),
    }
}

pub(super) async fn persist(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    changes: &[Change],
) -> Result<(), MinimumError> {
    for change in changes {
        let n = match change {
            Change::Insert { part, class } => sqlx::query(
                "INSERT INTO game_quick_slots(owner_id,item_class_utf16,slot) VALUES($1,$2,$3)",
            )
            .bind(cid)
            .bind(utf16(class))
            .bind(i32::from(*part))
            .execute(&mut **tx)
            .await?
            .rows_affected(),
            Change::Update { part, class } => sqlx::query(
                "UPDATE game_quick_slots SET item_class_utf16=$3 WHERE owner_id=$1 AND slot=$2",
            )
            .bind(cid)
            .bind(i32::from(*part))
            .bind(utf16(class))
            .execute(&mut **tx)
            .await?
            .rows_affected(),
            Change::Delete { part } => {
                sqlx::query("DELETE FROM game_quick_slots WHERE owner_id=$1 AND slot=$2")
                    .bind(cid)
                    .bind(i32::from(*part))
                    .execute(&mut **tx)
                    .await?
                    .rows_affected()
            }
        };
        if n != 1 {
            return Err(fail());
        }
    }
    Ok(())
}

pub(super) fn packet(slots: &QuickSlots) -> Result<OutgoingPacket, MinimumError> {
    let message = Message::QuickSlotInfo(Some(info(slots)));
    Ok(OutgoingPacket {
        category: Kind::QuickSlotInfo.category(),
        body: wire::encode(Kind::QuickSlotInfo, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// Admission (selected character, town and second password) belongs to the frontend.
pub async fn equip_bundle(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: Bundle,
) -> Result<Outcome, MinimumError> {
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
        sqlx::query("LOCK TABLE game_quick_slots IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        // Validates marker/profile/ownership and the Gear state used by Unequip.
        let gear = super::minimum_equipment::load(&mut tx, owner, assets, cid).await?;
        let mut slots = load(&mut tx, assets, cid).await?;
        let part = request.quick_slot;
        let item_class = request.class.ok_or_else(fail)?;
        let mut changes = Vec::new();
        if item_class.is_empty() {
            // ItemOwner.Unequip(part) → Gear.Unequip(part): equipment parts and
            // UnequipConsumablesByConstraint are outside this slice; unknown parts raise.
            match slots.clear(part) {
                Some(change) => changes.extend(change),
                None => return Err(fail()),
            }
        } else {
            if item_class.starts_with("skill_") {
                // EquipBundleProcessor: Character.Skills.GetSkill, then ItemOwner.EquipSkill.
                let key: Vec<u8> = item_class
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect();
                let owned: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM game_skills WHERE cid=$1 AND skill_utf16=$2)",
                )
                .bind(cid)
                .bind(&key)
                .fetch_one(&mut *tx)
                .await?;
                if !owned {
                    return Ok(Outcome::Refused(super::minimum_equip::item_fail(
                        "ConstraintFail_InvalidData",
                    )?));
                }
                if slots.equipped(&item_class).map(i32::from) == Some(part) {
                    return Ok(Outcome::Unchanged);
                }
                let contents = assets.skills.as_ref().ok_or_else(fail)?;
                // The bound profile covers every owned skill; absence is not evidence of
                // a missing SkillInfo row.
                let content = contents.get(&item_class).ok_or_else(fail)?;
                if content.use_type.is_none_or(|use_type| use_type == 0) {
                    return Ok(Outcome::Refused(super::minimum_equip::item_fail(
                        "ConstraintFail_InvalidData",
                    )?));
                }
                if let Some(previous) = slots.equipped(&item_class) {
                    changes.extend(slots.clear(i32::from(previous)).flatten());
                }
                match slots.set_skill(part, &item_class) {
                    Ok(change) => changes.push(change),
                    Err(refusal) => {
                        return Ok(Outcome::Refused(super::minimum_equip::item_fail(
                            refusal.name(),
                        )?));
                    }
                }
                persist(&mut tx, cid, &changes).await?;
                let packet = packet(&slots)?;
                tx.commit().await?;
                return Ok(Outcome::Applied(packet));
            }
            if slots.equipped(&item_class).map(i32::from) == Some(part) {
                return Ok(Outcome::Unchanged);
            }
            let key = utf16(&item_class);
            if assets.stats_index.class(Some(&key)).is_none() {
                return Ok(Outcome::Refused(super::minimum_equip::item_fail(
                    "ConstraintFail_InvalidClass",
                )?));
            }
            let profile = assets.equipment.as_ref().ok_or_else(fail)?;
            // Classes outside the bound profile have no exported constraint/category.
            let constraint = profile.constraint(&assets.catalog, &key)?;
            let category = profile.category(&assets.catalog, &key)?;
            let skills: Vec<(Vec<u8>, i32)> = sqlx::query_as(
                "SELECT skill_utf16,rank FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097",
            )
            .bind(cid)
            .fetch_all(&mut *tx)
            .await?;
            let skills = skills
                .into_iter()
                .map(|(k, r)| text(&k).map(|k| (k, r)))
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let levels: Vec<Equipped> = gear
                .projection
                .equipment
                .iter()
                .map(|_| Equipped {
                    enhance: 0,
                    prefix: 0,
                    suffix: 0,
                })
                .collect();
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
                return Ok(Outcome::Refused(super::minimum_equip::item_fail(
                    failure.name(),
                )?));
            }
            if category == "ARTIFACT" {
                // Artifact quick slots depend on the equipped part 16; not in this slice.
                return Err(fail());
            }
            if let Some(previous) = slots.equipped(&item_class) {
                changes.extend(slots.clear(i32::from(previous)).flatten());
            }
            match slots.set(part, &item_class, &category) {
                Ok(change) => changes.push(change),
                Err(refusal) => {
                    return Ok(Outcome::Refused(super::minimum_equip::item_fail(
                        refusal.name(),
                    )?));
                }
            }
        }
        if changes.is_empty() {
            tx.commit().await?;
            return Ok(Outcome::Unchanged);
        }
        persist(&mut tx, cid, &changes).await?;
        let packet = packet(&slots)?;
        tx.commit().await?;
        Ok(Outcome::Applied(packet))
    })
    .await
    .map_err(|_| fail())?
}
