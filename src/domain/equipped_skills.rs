//! ApplyItemSkills for bounded ASCII-identifier equipment projections, measured Framework48-x64.
//! Caller supplies the original EquippedItemInfo projection and lazy item-category lookup.
//! No item loader, set-bonus resolver, actor, SQL or network operation is implied.
use super::{
    character_stats::{Field, ItemResult, Stats, empty_gear},
    skill_runtime::Cache,
};
use thiserror::Error;

pub struct Equipment<'a> {
    pub stats: &'a Stats,
    pub item_class: &'a str,
    pub equip_class: &'a str,
    pub weight: f64,
    pub armor_hp: i32,
    pub max_durability: i32,
    pub costume_part: i32,
    pub destroyed_def: i32,
    pub decoration: bool,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EquipmentError {
    #[error("equipment projection exceeds the admitted ASCII/finite profile")]
    Profile,
    #[error("equipment category provider failed or returned no category")]
    Category,
    #[error("equipment armor map contains a duplicate part")]
    Duplicate,
}
fn argument(cache: &Cache, skill: &str, keyword: &str) -> i32 {
    let text = |s: &str| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    cache.argument(&text(skill), &text(keyword), 0)
}
fn mastery(class: &str) -> Option<(&'static str, bool, bool)> {
    Some(match class {
        "CLOTH" => ("skill_cloth_mastery", true, true),
        "LIGHT_ARMOR" => ("skill_light_armor_mastery", true, true),
        "HEAVY_ARMOR" => ("skill_heavy_armor_mastery", true, true),
        "PLATE_ARMOR" => ("skill_plate_armor_mastery", true, true),
        "LONGSWORD" => ("skill_fiona_longsword_mastery", false, false),
        "DUALSWORD" => ("skill_lethita_dualsword_mastery", false, false),
        "HAMMER" => ("skill_fiona_longhammer_mastery", false, false),
        "DUALSPEAR" => ("skill_lethita_dualspear_mastery", false, false),
        "STAFF" => ("skill_evy_staff_mastery", false, false),
        "SHIELD" => ("skill_fiona_shield_mastery", false, true),
        "LARGESHIELD" => ("skill_fiona_largeshield_mastery", false, true),
        "SCYTHE" => ("skill_evy_scythe_mastery", false, false),
        "PILLAR" => ("skill_kalok_battlepillar_mastery", false, false),
        "BLASTER" => ("skill_kalok_blaster_mastery", false, false),
        "BOW" => ("skill_kay_bow_mastery", false, false),
        "CROSSGUN" => ("skill_kay_crossgun_mastery", false, false),
        "DUALBLADE" => ("skill_vella_dualblade_mastery", false, false),
        "GREATSWORD" => ("skill_hurk_greatsword_mastery", false, false),
        "BATTLEGLAIVE" => ("skill_lynn_battleglaive_mastery", false, false),
        "LONGBLADE" => ("skill_arisha_longblade_mastery", false, false),
        "CASTLET" => ("skill_arisha_castlet_mastery", false, false),
        "PHANTOMDAGGER" => ("skill_hagie_phantomdagger_mastery", false, false),
        // Class 10 (Delia): absent from the 1.69 C# reference; the 1.69 content maps the
        // BASTARDSWORD weight bonus to this skill (scripts/expression/expression_table_base.txt).
        "BASTARDSWORD" => ("skill_delia_bastard_mastery", false, false),
        _ => return None,
    })
}
/// Mutates source out-parameters in order. Duplicate/category errors retain outputs
/// from earlier items, including the current item's stats/weight before a duplicate.
/// Admission errors are separate from source failures and do not reset outputs.
/// ASCII identifier admission excludes culture-ignorable control characters and avoids
/// pretending Rust substring matching implements culture-sensitive
/// String.IndexOf for arbitrary Unicode. Category lookup occurs only for "avatar_".
pub fn apply(
    set: &Stats,
    items: &[Equipment<'_>],
    cache: &Cache,
    output: &mut ItemResult,
    mut category: impl FnMut(&str) -> Result<String, EquipmentError>,
) -> Result<(), EquipmentError> {
    if items.len() > 128
        || items.iter().any(|i| {
            !i.weight.is_finite()
                || !i
                    .item_class
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || !i.equip_class.is_ascii()
                || i.item_class.len() > 100
                || i.equip_class.len() > 100
        })
    {
        return Err(EquipmentError::Profile);
    }
    *output = empty_gear(set);
    for item in items {
        let mut stats = item.stats.clone();
        let mut weight = item.weight;
        let mut hp = item.armor_hp;
        let avatar = if item.item_class.contains("avatar_") {
            let value = category(item.item_class)?;
            if !value.is_ascii() || value.len() > 256 {
                return Err(EquipmentError::Profile);
            }
            value
                .get(..7)
                .is_some_and(|s| s.eq_ignore_ascii_case("avatar_"))
        } else {
            false
        };
        if !avatar && let Some((skill, armor, durability)) = mastery(item.equip_class) {
            if armor {
                stats.set(
                    Field::Def,
                    stats
                        .get(Field::Def)
                        .wrapping_add(argument(cache, skill, "def_bonus")),
                );
            }
            weight -= f64::from(argument(cache, skill, "weight_reduction_bonus"));
            if durability {
                hp = hp.wrapping_mul(100_i32.wrapping_add(argument(
                    cache,
                    skill,
                    "durability_bonus",
                ))) / 100;
            }
            if item.equip_class == "DUALSWORD" {
                stats.set(
                    Field::CritFactor,
                    argument(cache, "skill_lethita_critical_damage", "dmg_bonus"),
                );
            }
        }
        if weight < 0.0 {
            weight = 0.0;
        }
        output.stats = output.stats.add(&stats);
        output.weight += weight;
        // The original uses Single multiplication and an unchecked Int32 subtraction.
        let bonus = 0.4_f32 * item.max_durability.wrapping_sub(10000).max(0) as f32;
        hp = hp.wrapping_add(bonus as i32);
        if item.costume_part != -1 && item.costume_part != 2 && !item.decoration {
            if output.armor_hp.contains_key(&item.costume_part) {
                return Err(EquipmentError::Duplicate);
            }
            output.armor_hp.insert(item.costume_part, hp);
            if output.armor_def.contains_key(&item.costume_part) {
                return Err(EquipmentError::Duplicate);
            }
            output.armor_def.insert(
                item.costume_part,
                stats.get(Field::Def).wrapping_sub(item.destroyed_def),
            );
        }
    }
    Ok(())
}
