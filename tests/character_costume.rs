use serde_json::Value;
use vindictus_server::{
    domain::character_costume::{BodyArithmetic, Costume, CostumeSet},
    infrastructure::character_costumes,
};
fn row(v: &Value) -> Costume {
    let opt = |k: &str| v[k].as_i64().map(|x| x as i32);
    Costume {
        is_base: v["IsBase"].as_bool().unwrap(),
        kind: v["Type"].as_u64().unwrap() as u8,
        key: v["Key"].as_u64().unwrap() as u8,
        serial: opt("CostumeSN"),
        colors: [opt("C1"), opt("C2"), opt("C3")],
        gloss: v["Gloss"].as_u64().map(|x| x as u8),
        effect: opt("Effect"),
    }
}
#[test]
fn body_extras_and_alias_updates_match_both_framework_profiles() {
    let v: Value = serde_json::from_str(include_str!("fixtures/framework-costume.json")).unwrap();
    assert_eq!(v["conversion_architecture_disagreements"], 57);
    for (index, arithmetic) in [BodyArithmetic::FrameworkX86, BodyArithmetic::FrameworkX64]
        .into_iter()
        .enumerate()
    {
        let p = &v["profiles"][index];
        for case in p["floats"].as_array().unwrap() {
            let bits = u32::from_str_radix(case["bits"].as_str().unwrap(), 16).unwrap();
            let result = Costume::body(
                case["key"].as_i64().unwrap() as i32,
                f32::from_bits(bits),
                arithmetic,
            );
            assert_eq!(
                result.serial,
                Some(case["sn"].as_i64().unwrap() as i32),
                "profile{index},bits{bits:08x}"
            );
            assert_eq!(result.key, case["storedKey"].as_u64().unwrap() as u8);
        }
        let mut ordered = CostumeSet::default();
        for key in [2, 1, 0, 2] {
            ordered.set(Costume::extra(key, key + 10)).unwrap();
        }
        assert_eq!(
            ordered.rows().copied().collect::<Vec<_>>(),
            p["ordered"]
                .as_array()
                .unwrap()
                .iter()
                .map(row)
                .collect::<Vec<_>>()
        );
        for case in p["extras"].as_array().unwrap() {
            assert_eq!(
                Costume::extra(case["key"].as_i64().unwrap() as i32, -123),
                row(&case["row"])
            );
        }
        for case in p["updates"].as_array().unwrap() {
            let expected = row(&case["row"]);
            let original = Costume {
                serial: Some(-12),
                colors: [None, Some(-1), Some(i32::MIN)],
                gloss: Some(255),
                effect: Some(91),
                ..expected
            };
            let mut set = CostumeSet::load([original]).unwrap();
            set.set(Costume {
                effect: Some(19),
                ..expected
            })
            .unwrap();
            assert_eq!(set.rows().copied().collect::<Vec<_>>(), vec![expected]);
            assert_eq!(case["same"], true);
            assert_eq!(case["rows"], 1);
        }
    }
}
#[test]
fn costume_sets_reject_duplicate_loaded_keys_and_explicit_bounds() {
    let c = Costume::extra(0, 0);
    assert!(CostumeSet::load([c, c]).is_err());
    let mut set = CostumeSet::default();
    for n in 0..CostumeSet::MAX_ROWS {
        set.set(Costume {
            kind: (n / 256) as u8,
            key: n as u8,
            ..c
        })
        .unwrap();
    }
    set.set(c).unwrap();
    assert!(set.set(Costume { kind: 9, ..c }).is_err());
}
#[test]
fn costume_export_requires_every_nullable_field_and_valid_identity_profile() {
    let mut v = serde_json::json!({"format_version":1,"source":"study_heroes.costume","source_last_value":"7","rows":[{"row_id":"1","cid":"99","is_base":true,"costume_type":3,"costume_key":0,"costume_sn":null,"c1":null,"c2":null,"c3":null,"gloss":null,"effect":null}]});
    let read =
        |v: &Value| character_costumes::read_snapshot(serde_json::to_vec(v).unwrap().as_slice());
    assert!(read(&v).is_ok());
    for key in ["costume_sn", "c1", "c2", "c3", "gloss", "effect"] {
        let mut bad = v.clone();
        bad["rows"][0].as_object_mut().unwrap().remove(key);
        assert!(read(&bad).is_err());
    }
    v["source_last_value"] = Value::Null;
    assert!(read(&v).is_err());
    v["source_last_value"] = serde_json::json!("7");
    v["rows"][0]["costume_key"] = serde_json::json!(256);
    assert!(read(&v).is_err());
    v["rows"][0]["costume_key"] = serde_json::json!(0);
    let copy = v["rows"][0].clone();
    v["rows"].as_array_mut().unwrap().push(copy);
    assert!(read(&v).is_err());
}
