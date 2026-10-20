use serde_json::Value;
use vindictus_server::domain::{
    character_stats::Stats,
    equipped_gear::{self as gear, Item, Part, Projection},
    skill_runtime::Cache,
};
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn string(v: &Value) -> String {
    v.as_str().unwrap().to_owned()
}
fn stat(v: &Value) -> Stats {
    Stats::from_values(
        v.as_array()
            .unwrap()
            .iter()
            .map(int)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
    )
}
fn effects(id: &str, count: usize) -> Vec<(String, i32)> {
    match count {
        2 => vec![
            ("STR".into(), i32::MAX),
            ("DEF".into(), 7),
            (
                if id == "A" {
                    "synthetic_skill"
                } else {
                    "synthetic_skill_b"
                }
                .into(),
                3,
            ),
        ],
        3 => vec![("STR".into(), -1), ("HP".into(), 13)],
        _ => vec![],
    }
}
#[test]
fn normal_equipped_projection_set_counts_and_item_stats_match_original_framework() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-equipped-gear.json")).unwrap();
    for arch in ["x86", "x64"] {
        let cases = fixture[arch].as_array().unwrap();
        assert_eq!(cases.len(), 128);
        for (n, c) in cases.iter().enumerate() {
            let parts: Vec<_> = c["input"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| Part {
                    slot: int(&r["slot"]) as u8,
                    category: string(&r["category"]),
                    costume: r["costume"].as_i64().map(|n| n as i32),
                    decoration: r["decoration"].as_bool().unwrap(),
                    consumable: r["consumable"].as_bool().unwrap(),
                })
                .collect();
            let raw = c["input"]["items"].as_array().unwrap();
            let items: Vec<_> = raw
                .iter()
                .map(|i| Item {
                    id: i["id"].as_i64().unwrap(),
                    part: int(&i["part"]) as u8,
                    category: string(&i["category"]),
                    class: string(&i["item_class"]),
                    equip_class: string(&i["equip_class"]),
                    serial: int(&i["serial"]),
                    level: int(&i["level"]),
                    weight: int(&i["weight"]),
                    durability: int(&i["durability"]),
                    maximum: int(&i["maximum"]),
                    armor_hp: int(&i["hp"]),
                    stats: stat(&i["stats"]),
                    destroyed_def: int(&i["destroyed"]),
                    effect: i["effect"].as_i64().map(|n| n as i32),
                    disabled: i["disabled"].as_bool().unwrap(),
                    colors: i["colors"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(int)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                })
                .collect();
            let sets: Vec<Vec<Option<String>>> = raw
                .iter()
                .map(|i| {
                    i["sets"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| Some(string(s)))
                        .collect()
                })
                .collect();
            let setitems: Vec<_> = items
                .iter()
                .zip(&sets)
                .map(|(i, sets)| gear::SetItem {
                    class: &i.class,
                    set_ids: sets,
                    disabled: i.disabled,
                })
                .collect();
            let bonus = gear::set_bonus(&setitems, |id, count| Ok(effects(id, count))).unwrap();
            assert!(bonus.stats == stat(&c["set_stats"]), "{arch}/{n}");
            assert_eq!(
                serde_json::to_value(&bonus.skills).unwrap(),
                c["set_skills"]
            );
            let projection = Projection::load(&parts, &items).unwrap();
            assert_eq!(
                projection
                    .equipment
                    .iter()
                    .map(|(p, id)| serde_json::json!({"part":p,"id":id}))
                    .collect::<Vec<_>>(),
                *c["equipment"].as_array().unwrap()
            );
            assert_eq!(projection.infos.len(), c["infos"].as_array().unwrap().len());
            for (i, r) in projection.infos.iter().zip(c["infos"].as_array().unwrap()) {
                for (key, actual) in [
                    ("part", i.part),
                    ("costume", i.costume),
                    ("serial", i.serial),
                    ("level", i.level),
                    ("durability", i.durability),
                    ("maximum", i.maximum),
                    ("hp", i.armor_hp),
                    ("destroyed", i.destroyed_def),
                ] {
                    assert_eq!(actual, int(&r[key]), "{arch}/{n}/{key}");
                }
                assert_eq!(i.decoration, r["decoration"].as_bool().unwrap());
                assert_eq!(
                    i.colors,
                    r["colors"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(int)
                        .collect::<Vec<_>>()
                        .as_slice()
                );
                assert_eq!(
                    i.weight.to_bits(),
                    u64::from_str_radix(r["weight_bits"].as_str().unwrap(), 16).unwrap()
                );
                assert!(i.stats == stat(&r["stats"]));
                assert_eq!(i.class, string(&r["item_class"]));
                assert_eq!(i.equip_class, string(&r["equip_class"]));
                assert_eq!(i.effect, r["effect"].as_i64().map(|v| v as i32));
            }
            let result = projection
                .item_stats(&bonus.stats, &Cache::default())
                .unwrap();
            assert!(result.stats == stat(&c["item_stats"]));
            assert_eq!(
                result.weight.to_bits(),
                u64::from_str_radix(c["weight_bits"].as_str().unwrap(), 16).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&result.armor_hp).unwrap(),
                c["armor_hp"]
            );
            assert_eq!(
                serde_json::to_value(&result.armor_def).unwrap(),
                c["armor_def"]
            );
        }
    }
}
#[test]
fn ambiguous_sets_and_unresolved_equipment_are_refused() {
    let ids = vec![Some("A".into()), Some("B".into())];
    let items = [gear::SetItem {
        class: "synthetic",
        set_ids: &ids,
        disabled: false,
    }];
    assert!(gear::set_bonus(&items, |_, _| Ok(vec![("skill".into(), 1)])).is_err());
    assert!(
        gear::set_bonus(&items, |_, _| Ok(vec![
            ("DEF".into(), 1),
            ("DEF".into(), 2)
        ]))
        .is_err()
    );
    assert!(
        Projection::load(
            &[Part {
                slot: 1,
                category: "A".into(),
                costume: Some(1),
                decoration: false,
                consumable: false
            }],
            &[Item {
                id: 1,
                part: 2,
                category: "A".into(),
                class: "synthetic".into(),
                equip_class: "".into(),
                serial: 0,
                level: 0,
                colors: [0; 3],
                weight: 0,
                durability: 0,
                maximum: 0,
                armor_hp: 0,
                stats: Stats::default(),
                destroyed_def: 0,
                effect: None,
                disabled: false
            }]
        )
        .is_err()
    );
}
