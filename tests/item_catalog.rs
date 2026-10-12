use serde_json::{Value, json};
use vindictus_server::{
    application::item_catalog_inspection,
    domain::{
        feature_policy::FeaturePolicy,
        item_binding::{CatalogBindingContext, check_parsed_character_bind},
        item_catalog::*,
        item_class_ex,
        legacy_user_data::LegacyBytes,
    },
    infrastructure::item_catalog::{self, Package},
};
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-item-catalog.json")).unwrap()
}
fn export(raw: &Value) -> Value {
    let mut raw = raw.clone();
    raw["format_version"] = json!(1);
    raw["source"] = json!("heroesContents.item-catalog-projection");
    raw["manifest"] = json!({"database_sha256":"0".repeat(64),"config_sha256":"1".repeat(64),"region":"EN-EU-X-GM","override_region":null,"equip_rows":raw["equips"].as_array().unwrap().len(),"class_rows":raw["classes"].as_array().unwrap().len(),"extra_rows":raw["extras"].as_array().unwrap().len()});
    raw
}
fn package(raw: &Value) -> Package {
    item_catalog::read_package(serde_json::to_vec(&export(raw)).unwrap().as_slice()).unwrap()
}
fn views(package: &Package) -> (Vec<EquipRow<'_>>, Vec<ClassRow<'_>>, Vec<ExtraRow<'_>>) {
    let equips = package
        .equips
        .iter()
        .map(|r| EquipRow {
            item_class: r.item_class.as_ref().map(LegacyBytes::as_bytes),
            feature: r.feature.as_deref(),
        })
        .collect();
    let classes = package
        .classes
        .iter()
        .map(|r| ClassRow {
            item_class: r.item_class.as_ref().map(LegacyBytes::as_bytes),
            feature: r.feature.as_deref(),
            max_stack: r.max_stack,
            storage: r.storage,
            bind: r.bind.as_deref(),
            trade_on_bind: r.trade_on_bind.as_deref(),
        })
        .collect();
    let extras = package
        .extras
        .iter()
        .map(|r| ExtraRow {
            item_class: r.item_class.as_ref().map(LegacyBytes::as_bytes),
            feature: r.feature.as_deref(),
        })
        .collect();
    (equips, classes, extras)
}
#[test]
fn catalog_matches_framework_winners_storage_counts_digest_errors_and_feature_order() {
    for profile in fixture()["profiles"].as_array().unwrap() {
        assert_eq!(profile["culture"], "en-US");
        assert_eq!(profile["cases"].as_array().unwrap().len(), 391);
        for (i, case) in profile["cases"].as_array().unwrap().iter().enumerate() {
            let package = package(&case["package"]);
            let (equips, classes, extras) = views(&package);
            let mut calls = vec![];
            let result = ItemCatalog::load(&equips, &classes, &extras, &mut |expression| {
                let at = calls.len();
                let phase = if at < equips.len() {
                    1
                } else if at < equips.len() + classes.len() {
                    2
                } else if at < equips.len() + classes.len() + extras.len() {
                    3
                } else {
                    4
                };
                calls.push(format!("{phase}:{}", expression.unwrap_or("<null>")));
                Ok(package.policy.is_enabled(expression)?)
            });
            let (summary, error) = match result {
                Ok(_) => (
                    serde_json::to_value(item_catalog_inspection::inspect(&package).unwrap())
                        .unwrap(),
                    Value::Null,
                ),
                Err(error) => (
                    Value::Null,
                    json!(match error {
                        CatalogError::NullKey => "NullKey",
                        CatalogError::Feature(_) => "Feature",
                        CatalogError::Bound => panic!("unexpected bound"),
                    }),
                ),
            };
            assert_eq!(
                (summary, error, json!(calls)),
                (
                    case["observed"]["summary"].clone(),
                    case["observed"]["error"].clone(),
                    case["observed"]["calls"].clone()
                ),
                "architecture={} case={i}",
                profile["architecture"]
            );
        }
    }
}
#[test]
fn catalog_additional_finite_version_pairs_match_all_recovered_cultures_and_architectures() {
    let additional: Vec<Value> = serde_json::from_str(include_str!(
        "../src/domain/feature_catalog_version_pairs.json"
    ))
    .unwrap();
    assert_eq!(additional.len(), 24);
    for profile in fixture()["profiles"].as_array().unwrap() {
        assert_eq!(profile["version_pairs"].as_array().unwrap().len(), 120);
        for pair in profile["version_pairs"].as_array().unwrap() {
            let policy = FeaturePolicy::new([
                (
                    "CurrentVer".into(),
                    Some(pair["current"].as_str().unwrap().into()),
                ),
                (
                    "threshold".into(),
                    Some(pair["value"].as_str().unwrap().into()),
                ),
            ])
            .unwrap();
            assert_eq!(
                json!(policy.is_enabled(Some("threshold")).unwrap()),
                pair["enabled"]
            );
        }
    }
    assert!(
        FeaturePolicy::new([
            ("CurrentVer".into(), Some("3.09.00".into())),
            ("threshold".into(), Some("3.04.02".into()))
        ])
        .unwrap()
        .is_enabled(Some("threshold"))
        .is_err()
    );
}
#[test]
fn effective_catalog_supplies_exact_equipment_membership_to_parsed_binding() {
    let raw = json!({"features":[{"key":"CurrentVer","value":"3.09.00"},{"key":"enabled","value":"1.1"},{"key":"BraceletMaxSlotCount","value":"0"}],"equips":[{"item_utf16":"73007400750064007900","feature":"enabled"}],"classes":[{"item_utf16":"73007400750064007900","feature":"enabled","max_stack":1,"storage":5,"bind":"disabled","trade_on_bind":"disabled"},{"item_utf16":"67006f006c006400","feature":null,"max_stack":100,"storage":0,"bind":"disabled","trade_on_bind":"disabled"}],"extras":[]});
    let package = package(&raw);
    let (eq, classes, extra) = views(&package);
    let catalog = ItemCatalog::load_with_policy(&eq, &classes, &extra, &package.policy).unwrap();
    let expression = LegacyBytes::<1024, true>::from_bytes(
        "study[ENHANCE:9]"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    let parsed =
        item_class_ex::parse(Some(&expression), &mut item_class_ex::integer_only_argument).unwrap();
    let item = catalog.get(Some(parsed.item_class.as_bytes())).unwrap();
    assert!(item.binding.equippable);
    assert_eq!(item.storage, 1);
    let mut context = CatalogBindingContext::new(&package.policy, &[]).unwrap();
    assert!(
        check_parsed_character_bind(false, Some(item.binding), &parsed.attributes, &mut context)
            .unwrap()
    );
    assert!(catalog.get(None).is_none());
    assert!(
        catalog
            .get(Some(&[b'S', 0, b'T', 0, b'U', 0, b'D', 0, b'Y', 0]))
            .is_none()
    );
}
#[test]
fn private_projection_reader_rejects_missing_nullable_fields_and_damaged_manifest_without_content_in_errors()
 {
    let fixture = fixture();
    let original = export(&fixture["profiles"][0]["cases"][0]["package"]);
    for (path, field) in [
        ("equips", "feature"),
        ("equips", "item_utf16"),
        ("classes", "bind"),
        ("classes", "trade_on_bind"),
    ] {
        let mut value = original.clone();
        value[path][0].as_object_mut().unwrap().remove(field);
        assert!(
            item_catalog::read_package(serde_json::to_vec(&value).unwrap().as_slice()).is_err()
        );
    }
    for field in ["equip_rows", "class_rows", "extra_rows"] {
        let mut value = original.clone();
        value["manifest"][field] = json!(999999);
        assert!(
            item_catalog::read_package(serde_json::to_vec(&value).unwrap().as_slice()).is_err()
        );
    }
    let mut value = original.clone();
    value["classes"][0]["item_utf16"] = json!("zzzz");
    let error = item_catalog::read_package(serde_json::to_vec(&value).unwrap().as_slice())
        .err()
        .unwrap();
    assert!(!error.to_string().contains("zzzz"));
    let mut value = original;
    value["unexpected"] = json!(true);
    assert!(item_catalog::read_package(serde_json::to_vec(&value).unwrap().as_slice()).is_err());
}
#[test]
fn catalog_and_input_stream_bounds_are_explicit_before_allocation_or_evaluation() {
    let row = EquipRow {
        item_class: Some(&[1, 0]),
        feature: None,
    };
    let mut calls = 0;
    assert!(matches!(
        ItemCatalog::load(&vec![row; 50001], &[], &[], &mut |_| {
            calls += 1;
            Ok(true)
        }),
        Err(CatalogError::Bound)
    ));
    assert_eq!(calls, 0);
    assert!(matches!(
        ItemCatalog::load(
            &[EquipRow {
                item_class: Some(&[1]),
                feature: None
            }],
            &[],
            &[],
            &mut |_| Ok(true)
        ),
        Err(CatalogError::Bound)
    ));
    let too_large = vec![b' '; 16 * 1024 * 1024 + 1];
    assert!(item_catalog::read_package(too_large.as_slice()).is_err());
}
#[test]
fn canonical_projection_digest_is_stable_and_sensitive_to_the_selected_row_order() {
    let original = fixture()["profiles"][0]["cases"][0]["package"].clone();
    let a = package(&original);
    assert_eq!(
        item_catalog_inspection::inspect(&a)
            .unwrap()
            .projection_sha256,
        item_catalog_inspection::inspect(&a)
            .unwrap()
            .projection_sha256
    );
    let mut changed = original;
    changed["classes"].as_array_mut().unwrap().push(json!({"item_utf16":"73007400750064007900","feature":null,"max_stack":1,"storage":2,"bind":null,"trade_on_bind":"off"}));
    assert_ne!(
        item_catalog_inspection::inspect(&a)
            .unwrap()
            .projection_sha256,
        item_catalog_inspection::inspect(&package(&changed))
            .unwrap()
            .projection_sha256
    );
}
