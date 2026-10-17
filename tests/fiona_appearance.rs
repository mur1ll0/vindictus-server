mod support_fiona {
    include!("fixtures/fiona_inventory_support.rs");
}
use serde_json::{Value, json};
use support_fiona::*;
use vindictus_server::{
    domain::{
        character_costume::{BodyArithmetic, Costume},
        feature_policy::FeaturePolicy,
    },
    error::Error,
    infrastructure::{
        fiona_appearance::Profile, fiona_inventory_profile, item_catalog, item_colors,
    },
};
fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}
include!("fixtures/fiona_appearance_support.rs");
fn prepare(
    v: &Value,
    t: &Template<'_>,
    profile: Option<BodyArithmetic>,
) -> Result<Vec<Costume>, Error> {
    let (c, _, h, p) = fixtures();
    Profile::read(bytes(v).as_slice())?.prepare(
        t,
        &fiona_inventory_profile::Profile::read(bytes(&p).as_slice())?,
        &item_catalog::read_package(bytes(&c).as_slice())?,
        &item_colors::Package::read(bytes(&h).as_slice())?,
        profile,
    )
}
#[test]
fn appearance_composes_inner_alias_skin_sss_extras_and_body_in_source_order() {
    let rows = prepare(&fixture(), &template(), None).unwrap();
    assert_eq!(rows.len(), 10);
    assert_eq!(
        (rows[0].kind, rows[0].key, rows[0].serial),
        (0, 4, Some(29))
    );
    assert_eq!(rows[0].colors, [Some(7), Some(8), Some(9)]);
    assert_eq!((rows[0].gloss, rows[0].effect), (Some(255), Some(0)));
    assert_eq!(rows[1].colors, [Some(10), Some(30), Some(20)]);
    assert_eq!((rows[1].gloss, rows[1].effect), (None, None));
    assert_eq!(
        rows[2..9].iter().map(|r| r.key).collect::<Vec<_>>(),
        [2, 1, 0, 3, 4, 5, 6]
    );
    assert_eq!(
        (rows[9].kind, rows[9].key, rows[9].serial),
        (3, 9, Some(500))
    );
    let mut v = fixture();
    v["parts"][0]["costume_part"] = json!(-1);
    assert_eq!(prepare(&v, &template(), None).unwrap().len(), 9);
}
#[test]
fn appearance_preserves_source_cash_filters_and_rejects_ambiguous_content() {
    let base = fixture();
    for field in ["is_cash", "cash_type", "feature"] {
        let mut v = base.clone();
        v["custom"][0][field] = match field {
            "is_cash" => json!(true),
            "cash_type" => json!("OTHER"),
            _ => json!("missing"),
        };
        assert!(prepare(&v, &template(), None).is_err());
    }
    for field in ["parts", "custom", "colors"] {
        let mut v = base.clone();
        let row = v[field][0].clone();
        v[field].as_array_mut().unwrap().push(row);
        assert!(prepare(&v, &template(), None).is_err());
    }
    let mut v = base.clone();
    v["database_sha256"] = json!("d".repeat(64));
    assert!(prepare(&v, &template(), None).is_err());
    for field in base.as_object().unwrap().keys() {
        let mut v = base.clone();
        v.as_object_mut().unwrap().remove(field);
        assert!(
            Profile::read(bytes(&v).as_slice()).is_err(),
            "missing {field}"
        );
    }
    let mut t = template();
    t.character_class = 2;
    assert!(prepare(&base, &t, None).is_err());
    t = template();
    t.items = None;
    assert!(prepare(&base, &t, None).is_err());
}
#[test]
fn appearance_requires_explicit_arithmetic_when_measured_architectures_disagree() {
    let mut t = template();
    let value = (1..10000)
        .map(|i| i as f32 / 10000.0)
        .find(|&v| {
            Costume::body(9, v, BodyArithmetic::FrameworkX86)
                != Costume::body(9, v, BodyArithmetic::FrameworkX64)
        })
        .unwrap();
    t.body_shape = Some(vec![(9, value.to_bits())]);
    assert!(matches!(
        prepare(&fixture(), &t, None),
        Err(Error::AppearanceArithmetic)
    ));
    let x = prepare(&fixture(), &t, Some(BodyArithmetic::FrameworkX86)).unwrap();
    let y = prepare(&fixture(), &t, Some(BodyArithmetic::FrameworkX64)).unwrap();
    assert_ne!(x.last(), y.last());
    t.body_shape = Some(vec![(9, 0), (9, 0)]);
    assert!(prepare(&fixture(), &t, None).is_err());
}
#[test]
fn appearance_source_features_have_explicit_bounds_and_measured_version_pairs() {
    let rows = |n| {
        std::iter::once(("CurrentVer".to_owned(), Some("3.09.00".to_owned())))
            .chain((1..n).map(|i| (format!("feature{i}"), None)))
    };
    assert!(FeaturePolicy::new(rows(128)).is_ok());
    assert!(FeaturePolicy::new(rows(129)).is_err());
    assert!(FeaturePolicy::extended_source(rows(256)).is_ok());
    assert!(FeaturePolicy::extended_source(rows(257)).is_err());
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-appearance-features.json")).unwrap();
    let pairs = fixture["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 5);
    for pair in pairs {
        let p = FeaturePolicy::extended_source([
            (
                "CurrentVer".to_owned(),
                Some(pair["current"].as_str().unwrap().to_owned()),
            ),
            (
                "candidate".to_owned(),
                Some(pair["value"].as_str().unwrap().to_owned()),
            ),
        ])
        .unwrap();
        assert_eq!(
            p.is_enabled(Some("candidate")).unwrap(),
            pair["enabled"].as_bool().unwrap()
        );
    }
}
#[test]
fn appearance_projection_matches_original_framework_for_256_boundary_cases() {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/framework-fiona-appearance.json")).unwrap();
    let cases = oracle["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 256);
    for case in cases {
        let mut v = fixture();
        v["equips"][0]["costume_type"] = case["serial"].clone();
        v["inner_colors"] = case["colors"].clone();
        v["parts"][0]["costume_part"] = case["part"].clone();
        v["parts"][0]["decoration"] = case["decoration"].clone();
        let part = v["parts"][0].clone();
        v["parts"] = json!(vec![part; case["count"].as_u64().unwrap() as usize]);
        let mut t = template();
        t.items = Some(vec![]);
        let result = prepare(&v, &t, None);
        if !case["accepted"].as_bool().unwrap() {
            assert!(result.is_err());
            continue;
        }
        let result = result.unwrap();
        let expected = &case["costume"];
        if expected.is_null() {
            assert_eq!(result.len(), 9);
            continue;
        }
        let actual = result[0];
        assert_eq!(
            (actual.is_base, actual.kind, actual.key),
            (
                expected["IsBase"].as_bool().unwrap(),
                expected["Type"].as_u64().unwrap() as u8,
                expected["Key"].as_u64().unwrap() as u8
            )
        );
        assert_eq!(
            actual.serial,
            Some(expected["CostumeSN"].as_i64().unwrap() as i32)
        );
        assert_eq!(
            actual.colors,
            ["C1", "C2", "C3"].map(|k| Some(expected[k].as_i64().unwrap() as i32))
        );
        assert_eq!(
            actual.gloss,
            Some(expected["Gloss"].as_u64().unwrap() as u8)
        );
        assert_eq!(
            actual.effect,
            Some(expected["Effect"].as_i64().unwrap() as i32)
        );
    }
}

#[test]
fn initial_gear_requires_pinned_sources_consumable_metadata_and_unique_slot_category() {
    let (c, _, _, _) = fixtures();
    let catalog = item_catalog::read_package(bytes(&c).as_slice()).unwrap();
    let mut v = fixture();
    v["gear_sources"]["gear"] = json!("f".repeat(64));
    assert!(Profile::read(bytes(&v).as_slice()).is_err());
    let mut v = fixture();
    v["parts"][0].as_object_mut().unwrap().remove("consumable");
    assert!(Profile::read(bytes(&v).as_slice()).is_err());
    let mut v = fixture();
    let duplicate = v["parts"][0].clone();
    v["parts"].as_array_mut().unwrap().push(duplicate);
    assert!(
        Profile::read(bytes(&v).as_slice())
            .unwrap()
            .initial_gear(&catalog)
            .is_err()
    );
    let mut v = fixture();
    v["parts"][0]["category_utf16"] = Value::Null;
    assert!(
        Profile::read(bytes(&v).as_slice())
            .unwrap()
            .initial_gear(&catalog)
            .is_err()
    );
    let mut v = fixture();
    v["parts"][0]["equip_part"] = json!(256);
    assert!(
        Profile::read(bytes(&v).as_slice())
            .unwrap()
            .initial_gear(&catalog)
            .is_err()
    );
}
#[test]
fn initial_gear_keeps_source_slot_order_and_zero_set_bonus_with_avatar_flags() {
    let (c, _, _, _) = fixtures();
    let catalog = item_catalog::read_package(bytes(&c).as_slice()).unwrap();
    let mut v = fixture();
    v["parts"] = json!([
        {"category_utf16":key("a"),"equip_part":9,"costume_part":8,"decoration":false,"consumable":false},
        {"category_utf16":key("b"),"equip_part":1,"costume_part":7,"decoration":true,"consumable":false},
        {"category_utf16":key("c"),"equip_part":9,"costume_part":8,"decoration":true,"consumable":false},
        {"category_utf16":key("d"),"equip_part":5,"costume_part":4,"decoration":false,"consumable":true}
    ]);
    for enabled in [Value::Null, json!("")] {
        for f in v["features"].as_array_mut().unwrap() {
            if f["key"] == "Avatar" || f["key"] == "AvatarSlotBonus" {
                f["value"] = enabled.clone();
            }
        }
        let gear = Profile::read(bytes(&v).as_slice())
            .unwrap()
            .initial_gear(&catalog)
            .unwrap();
        let rows: Vec<_> = gear
            .rows()
            .iter()
            .map(|r| (r.part, r.costume_part, r.decoration))
            .collect();
        assert_eq!(rows, vec![(9, 8, false), (1, 7, true)]);
        assert_eq!(gear.set_bonus().values(), &[0; 25]);
        assert!(gear.set_skills().is_empty());
    }
}
