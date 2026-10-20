use serde_json::{Value, json};
use vindictus_server::domain::{
    character_costume::{Costume, CostumeSet},
    character_load::{self, BaseLoad},
    character_stats::{Arithmetic, Field, Stats, empty_gear},
    equipped_gear::{Info, Projection},
    skill_runtime::{Argument, Cache},
};
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn info(v: &Value) -> Info {
    Info {
        part: 0,
        costume: int(&v["CostumePartNum"]),
        decoration: v["IsDecoration"].as_bool().unwrap(),
        serial: int(&v["CostumeSN"]),
        colors: [int(&v["Color1"]), int(&v["Color2"]), int(&v["Color3"])],
        level: 0,
        weight: 0.0,
        durability: int(&v["Durability"]),
        maximum: 0,
        armor_hp: 0,
        stats: Stats::default(),
        destroyed_def: 0,
        effect: v["Effect"].as_i64().map(|n| n as i32),
        class: String::new(),
        equip_class: String::new(),
    }
}
fn costume(c: &Costume) -> Value {
    json!({"IsBase":c.is_base,"Type":c.kind,"Key":c.key,"CostumeSN":c.serial,"C1":c.colors[0],"C2":c.colors[1],"C3":c.colors[2],"Gloss":c.gloss,"Effect":c.effect})
}
#[test]
fn equipment_costumes_match_original_x64_conversion_alias_and_entityset_order() {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-equipped-costume.json")).unwrap();
    let cases = f["x64"].as_array().unwrap();
    assert_eq!(cases.len(), 256);
    for (n, c) in cases.iter().enumerate() {
        assert_eq!(
            costume(&info(&c["info"]).costume_x64()),
            c["converted"],
            "conversion {n}"
        );
        let mut rows = CostumeSet::load([
            Costume {
                is_base: true,
                kind: 0,
                key: 1,
                serial: Some(3),
                colors: [Some(1), Some(2), Some(3)],
                gloss: None,
                effect: None,
            },
            Costume {
                is_base: false,
                kind: 0,
                key: 2,
                serial: Some(9),
                colors: [None; 3],
                gloss: None,
                effect: Some(91),
            },
            Costume {
                is_base: false,
                kind: 0,
                key: 3,
                serial: Some(11),
                colors: [None; 3],
                gloss: None,
                effect: Some(0),
            },
        ])
        .unwrap();
        Projection {
            infos: c["steps"].as_array().unwrap().iter().map(info).collect(),
            equipment: vec![],
        }
        .load_costumes_x64(&mut rows)
        .unwrap();
        assert_eq!(
            rows.rows().map(costume).collect::<Vec<_>>(),
            *c["rows"].as_array().unwrap(),
            "rows {n}"
        );
    }
}
fn bits(v: &Value) -> u64 {
    u64::from_str_radix(v.as_str().unwrap(), 16).unwrap()
}
fn text(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[test]
fn nonempty_equipment_load_matches_original_framework_x64() {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-equipped-load.json")).unwrap();
    let cases = f["x64"].as_array().unwrap();
    assert_eq!(cases.len(), 256);
    for (n, c) in cases.iter().enumerate() {
        let mut cache = Cache::default();
        cache
            .update_arguments(
                &text("skill_weight_mastery"),
                Some(&[Argument::new(
                    Some(text("max_weight_increament_bonus")),
                    Some(int(&c["bonus"])),
                )
                .unwrap()]),
            )
            .unwrap();
        let mut stats = Stats::default();
        stats.set(Field::Str, int(&c["item_value"]));
        let mut items = empty_gear(&stats);
        items.weight = f64::from_bits(bits(&c["weight_bits"]));
        let result = character_load::equipped_x64(
            items,
            stats.clone(),
            &cache,
            BaseLoad::from_bits(bits(&c["base_bits"])).unwrap(),
            Arithmetic::FrameworkX64,
        )
        .unwrap();
        assert_eq!(
            result.medium.to_bits(),
            bits(&c["medium_bits"]),
            "medium {n}"
        );
        assert_eq!(result.heavy.to_bits(), bits(&c["heavy_bits"]), "heavy {n}");
        assert_eq!(result.penalty, int(&c["penalty"]), "penalty {n}");
        assert!(result.item_stats == Some(stats));
        assert!(!result.equipped_empty);
    }
    assert!(
        character_load::equipped_x64(
            empty_gear(&Stats::default()),
            Stats::default(),
            &Cache::default(),
            BaseLoad::from_bits(1_f64.to_bits()).unwrap(),
            Arithmetic::FrameworkX86
        )
        .is_err()
    );
}
