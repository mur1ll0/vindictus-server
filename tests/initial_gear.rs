use serde::Deserialize;
use std::collections::BTreeMap;
use vindictus_server::domain::{
    character_stats::{Stats, empty_gear},
    equipped_skills::EquipmentError,
    initial_gear::{Projection, SlotPart},
    skill_runtime::Cache,
};
#[derive(Deserialize)]
struct Row {
    slot: u8,
    costume_part: Option<i32>,
    decoration: bool,
    consumable: bool,
}
#[derive(Deserialize)]
struct Input {
    rows: Vec<Row>,
}
#[derive(Deserialize)]
struct Info {
    part: i32,
    costume_part: i32,
    decoration: bool,
    serial: i32,
    colors: [i32; 3],
    level: i32,
    weight_bits: String,
    durability: i32,
    maximum: i32,
    hp: i32,
    equip_class: String,
    item_class: String,
    avatar: bool,
    stats: [i32; 25],
    destroyed: i32,
    effect: Option<i32>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    gear_slots: usize,
    rows: Vec<Info>,
    set: [i32; 25],
    set_skill_count: usize,
    unexpected_provider_calls: usize,
    item_stats: [i32; 25],
    item_weight_bits: String,
    hp: BTreeMap<i32, i32>,
    def: BTreeMap<i32, i32>,
    duplicate: bool,
}
#[derive(Deserialize)]
struct Fixture {
    x86: Vec<Case>,
    x64: Vec<Case>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-initial-gear.json")).unwrap()
}
fn verify(cases: Vec<Case>) {
    assert_eq!(cases.len(), 160);
    for (i, c) in cases.into_iter().enumerate() {
        let rows: Vec<_> = c
            .input
            .rows
            .into_iter()
            .map(|r| SlotPart {
                slot: r.slot,
                costume_part: r.costume_part,
                decoration: r.decoration,
                consumable: r.consumable,
            })
            .collect();
        let p = Projection::from_empty_slots(&rows).unwrap();
        assert_eq!(p.gear_slots(), c.gear_slots, "case {i}: slots");
        assert_eq!(p.rows().len(), c.rows.len(), "case {i}: info count");
        for (a, b) in p.rows().iter().zip(c.rows) {
            assert_eq!(
                (a.part, a.costume_part, a.decoration),
                (b.part, b.costume_part, b.decoration),
                "case {i}: info"
            );
            // EmptyInfo represents the typed empty-slot initializer; preserve these
            // measured defaults when a future wire/actor adapter materializes the DTO.
            assert_eq!(b.serial, -1);
            assert_eq!(b.colors, [0; 3]);
            assert_eq!(b.level, 0);
            assert_eq!(b.weight_bits, "0000000000000000");
            assert_eq!(b.durability, 0);
            assert_eq!(b.maximum, 0);
            assert_eq!(b.hp, 0);
            assert!(b.equip_class.is_empty());
            assert!(b.item_class.is_empty());
            assert!(!b.avatar);
            assert_eq!(b.stats, [0; 25]);
            assert_eq!(b.destroyed, 0);
            assert_eq!(b.effect, None);
        }
        assert_eq!(p.set_bonus().values(), &c.set);
        assert_eq!(p.set_skills().len(), c.set_skill_count);
        assert_eq!(c.unexpected_provider_calls, 0);
        let mut result = empty_gear(&Stats::default());
        let outcome = p.apply_item_stats(&Cache::default(), &mut result);
        assert_eq!(
            outcome,
            if c.duplicate {
                Err(EquipmentError::Duplicate)
            } else {
                Ok(())
            },
            "case {i}: duplicate"
        );
        assert_eq!(result.stats.values(), &c.item_stats);
        assert_eq!(
            format!("{:016x}", result.weight.to_bits()),
            c.item_weight_bits
        );
        assert_eq!(result.armor_hp, c.hp, "case {i}: hp");
        assert_eq!(result.armor_def, c.def, "case {i}: def");
    }
}
#[test]
fn empty_gear_projection_and_item_maps_match_original_framework_x86() {
    verify(fixture().x86);
}
#[test]
fn empty_gear_projection_and_item_maps_match_original_framework_x64() {
    verify(fixture().x64);
}
#[test]
fn unequipped_gear_is_not_an_empty_equipped_info_list() {
    let gear = Projection::from_empty_slots(&[SlotPart {
        slot: 1,
        costume_part: Some(4),
        decoration: false,
        consumable: false,
    }])
    .unwrap();
    let actual = gear.item_stats(&Cache::default()).unwrap();
    assert_eq!(actual.armor_hp.get(&4), Some(&0));
    assert_eq!(actual.armor_def.get(&4), Some(&0));
    assert!(empty_gear(&Stats::default()).armor_hp.is_empty());
}
