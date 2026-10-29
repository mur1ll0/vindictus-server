use std::collections::BTreeMap;

use serde::Deserialize;
use vindictus_server::domain::{
    character_stats::{Stats, empty_gear},
    equipped_skills::{Equipment, EquipmentError, apply},
    skill_runtime::{Argument, Cache},
};
#[derive(Deserialize)]
struct Item {
    item_class: String,
    equip_class: String,
    stats: [i32; 25],
    weight_bits: String,
    hp: i32,
    maximum: i32,
    part: i32,
    destroyed: i32,
    decoration: bool,
    category: String,
    category_fail: bool,
}
#[derive(Deserialize)]
struct Case {
    seed: i32,
    set: [i32; 25],
    items: Vec<Item>,
    stats: [i32; 25],
    weight_bits: String,
    hp: BTreeMap<i32, i32>,
    def: BTreeMap<i32, i32>,
    calls: Vec<String>,
    error: Option<String>,
}
#[derive(Deserialize)]
struct Fixture {
    x64: Vec<Case>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-equipped-skills.json")).unwrap()
}
fn text(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn bits(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}
const SKILLS: [&str; 23] = [
    "skill_cloth_mastery",
    "skill_light_armor_mastery",
    "skill_heavy_armor_mastery",
    "skill_plate_armor_mastery",
    "skill_fiona_longsword_mastery",
    "skill_lethita_dualsword_mastery",
    "skill_fiona_longhammer_mastery",
    "skill_lethita_dualspear_mastery",
    "skill_evy_staff_mastery",
    "skill_fiona_shield_mastery",
    "skill_fiona_largeshield_mastery",
    "skill_evy_scythe_mastery",
    "skill_kalok_battlepillar_mastery",
    "skill_kalok_blaster_mastery",
    "skill_kay_bow_mastery",
    "skill_kay_crossgun_mastery",
    "skill_vella_dualblade_mastery",
    "skill_hurk_greatsword_mastery",
    "skill_lynn_battleglaive_mastery",
    "skill_arisha_longblade_mastery",
    "skill_arisha_castlet_mastery",
    "skill_hagie_phantomdagger_mastery",
    "skill_lethita_critical_damage",
];
fn cache(seed: i32) -> Cache {
    let mut cache = Cache::default();
    for (n, skill) in SKILLS.iter().enumerate() {
        let rows: Vec<_> = [
            "def_bonus",
            "weight_reduction_bonus",
            "durability_bonus",
            "dmg_bonus",
        ]
        .iter()
        .enumerate()
        .map(|(k, key)| {
            let value = if seed == 0 {
                0
            } else {
                seed.wrapping_add((n as i32 + 1) * 17)
                    .wrapping_add((k as i32 + 1) * 101)
            };
            Argument::new(Some(text(key)), Some(value)).unwrap()
        })
        .collect();
        cache.update_arguments(&text(skill), Some(&rows)).unwrap();
    }
    cache
}
fn verify(cases: &[Case]) {
    for (index, c) in cases.iter().enumerate() {
        let set = Stats::from_values(c.set);
        let stats: Vec<_> = c
            .items
            .iter()
            .map(|i| Stats::from_values(i.stats))
            .collect();
        let items: Vec<_> = c
            .items
            .iter()
            .zip(&stats)
            .map(|(i, s)| Equipment {
                stats: s,
                item_class: &i.item_class,
                equip_class: &i.equip_class,
                weight: f64::from_bits(bits(&i.weight_bits)),
                armor_hp: i.hp,
                max_durability: i.maximum,
                costume_part: i.part,
                destroyed_def: i.destroyed,
                decoration: i.decoration,
            })
            .collect();
        let mut calls = Vec::new();
        let mut output = empty_gear(&Stats::default());
        let result = apply(&set, &items, &cache(c.seed), &mut output, |key| {
            calls.push(key.to_owned());
            let i = c.items.iter().find(|i| i.item_class == key).unwrap();
            if i.category_fail {
                Err(EquipmentError::Category)
            } else {
                Ok(i.category.clone())
            }
        });
        let error = match result {
            Ok(()) => None,
            Err(EquipmentError::Category) => Some("category"),
            Err(EquipmentError::Duplicate) => Some("duplicate"),
            Err(EquipmentError::Profile) => panic!("case {index}: unexpected admission error"),
        };
        assert_eq!(error, c.error.as_deref(), "case {index}: error");
        assert_eq!(output.stats.values(), &c.stats, "case {index}: stats");
        assert_eq!(
            output.weight.to_bits(),
            bits(&c.weight_bits),
            "case {index}: weight"
        );
        assert_eq!(output.armor_hp, c.hp, "case {index}: hp");
        assert_eq!(output.armor_def, c.def, "case {index}: def");
        assert_eq!(calls, c.calls, "case {index}: category order");
    }
}
#[test]
fn all_equipment_masteries_match_framework_x64() {
    let f = fixture();
    assert_eq!(f.x64.len(), 640);
    verify(&f.x64[..576]);
}
#[test]
fn duplicate_and_category_failures_retain_original_partial_outputs() {
    let f = fixture();
    assert!(
        f.x64[576..]
            .iter()
            .any(|c| c.error.as_deref() == Some("duplicate"))
    );
    assert!(
        f.x64[576..]
            .iter()
            .any(|c| c.error.as_deref() == Some("category"))
    );
    verify(&f.x64[576..]);
}
#[test]
fn empty_equipment_replaces_previous_outputs_without_category_queries() {
    let stats = Stats::from_values([19; 25]);
    let mut output = empty_gear(&Stats::from_values([-7; 25]));
    output.weight = 98.0;
    output.armor_hp.insert(4, 99);
    output.armor_def.insert(4, 88);
    apply(&stats, &[], &Cache::default(), &mut output, |_| {
        panic!("unexpected category")
    })
    .unwrap();
    assert_eq!(output.stats.values(), stats.values());
    assert_eq!(output.weight, 0.0);
    assert!(output.armor_hp.is_empty());
    assert!(output.armor_def.is_empty());
}
#[test]
fn unsupported_projection_is_rejected_without_replacing_previous_outputs() {
    let stats = Stats::from_values([19; 25]);
    for (name, weight) in [
        ("unicode_\u{e9}", 1.0),
        ("av\0atar_", 1.0),
        ("item", f64::NAN),
        ("item", f64::INFINITY),
    ] {
        let item = Equipment {
            stats: &stats,
            item_class: name,
            equip_class: "CLOTH",
            weight,
            armor_hp: 1,
            max_durability: 10000,
            costume_part: 4,
            destroyed_def: 0,
            decoration: false,
        };
        let mut output = empty_gear(&stats);
        output.weight = 98.0;
        assert_eq!(
            apply(
                &Stats::default(),
                &[item],
                &Cache::default(),
                &mut output,
                |_| panic!("unexpected category")
            ),
            Err(EquipmentError::Profile)
        );
        assert_eq!(output.weight, 98.0);
        assert_eq!(output.stats.values(), stats.values());
    }
}

#[test]
fn fixture_covers_each_mastery_with_nonzero_arguments() {
    let f = fixture();
    for class in [
        "CLOTH",
        "LIGHT_ARMOR",
        "HEAVY_ARMOR",
        "PLATE_ARMOR",
        "LONGSWORD",
        "DUALSWORD",
        "HAMMER",
        "DUALSPEAR",
        "STAFF",
        "SHIELD",
        "LARGESHIELD",
        "SCYTHE",
        "PILLAR",
        "BLASTER",
        "BOW",
        "CROSSGUN",
        "DUALBLADE",
        "GREATSWORD",
        "BATTLEGLAIVE",
        "LONGBLADE",
        "CASTLET",
        "PHANTOMDAGGER",
    ] {
        assert!(
            f.x64.iter().any(|c| c.seed != 0
                && c.items
                    .iter()
                    .any(|i| i.equip_class == class && !i.item_class.contains("avatar_"))),
            "missing nonzero mastery case: {class}"
        );
    }
}

#[test]
fn bastard_sword_takes_delia_mastery_like_greatsword_takes_hurk_mastery() {
    // Class 10 extension (not in the Framework fixture): the BASTARDSWORD mastery behaves as the
    // GREATSWORD one when both skills carry the same arguments.
    let weight = |equip_class: &str, skill: &str| {
        let mut cache = Cache::default();
        let rows: Vec<_> = [("weight_reduction_bonus", 40), ("dmg_bonus", 7)]
            .iter()
            .map(|(k, v)| Argument::new(Some(text(k)), Some(*v)).unwrap())
            .collect();
        cache.update_arguments(&text(skill), Some(&rows)).unwrap();
        let stats = Stats::default();
        let items = [Equipment {
            stats: &stats,
            item_class: "bastardsword_newbie",
            equip_class,
            weight: 41.8333,
            armor_hp: 1000,
            max_durability: 10000,
            costume_part: 6,
            destroyed_def: 0,
            decoration: false,
        }];
        let mut output = empty_gear(&Stats::default());
        apply(&Stats::default(), &items, &cache, &mut output, |_| Ok(String::new())).unwrap();
        (output.weight, output.stats.values().to_vec())
    };
    let delia = weight("BASTARDSWORD", "skill_delia_bastard_mastery");
    assert_eq!(delia, weight("GREATSWORD", "skill_hurk_greatsword_mastery"));
    assert_ne!(delia, weight("BASTARDSWORD", "skill_unrelated"));
}
