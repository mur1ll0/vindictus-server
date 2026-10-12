use serde_json::{Value, json};
use vindictus_server::domain::{
    feature_policy::FeaturePolicy, item_binding::*, item_class_ex, legacy_user_data::LegacyBytes,
};
fn bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn hex(value: &Value) -> Option<Vec<u8>> {
    value.as_str().map(|s| {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    })
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-item-binding.json")).unwrap()
}
struct Recorded {
    calls: Vec<String>,
    slots: i32,
    macro_enabled: bool,
    slot_error: bool,
}
impl BindingContext for Recorded {
    fn is_enabled(&mut self, key: Option<&str>) -> Result<bool, BindingError> {
        self.calls
            .push(format!("feature:{}", key.unwrap_or("<null>")));
        match key {
            Some("error") => Err(BindingError::Feature(
                vindictus_server::error::Error::FeatureExpression,
            )),
            Some("Macro_EnchantTradeRestriction") => Ok(self.macro_enabled),
            None | Some("" | "on") => Ok(true),
            _ => Ok(false),
        }
    }
    fn enchant_level(&mut self, key: Option<&[u8]>) -> Result<i32, BindingError> {
        self.calls.push(format!(
            "enchant:{}",
            key.map(|b| String::from_utf16(
                &b.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|u| u16::from_le_bytes([u[0], u[1]]))
                    .collect::<Vec<_>>()
            )
            .unwrap())
                .unwrap_or("<null>".into())
        ));
        if key == Some(bytes("study_low").as_slice()) {
            return Ok(6);
        }
        if key == Some(bytes("study_high").as_slice()) {
            return Ok(7);
        }
        Err(BindingError::MissingEnchant)
    }
    fn slot_count(&mut self) -> Result<i32, BindingError> {
        self.calls.push("slots".into());
        if self.slot_error {
            return Err(BindingError::Feature(
                vindictus_server::error::Error::FeatureIntegerOverflow,
            ));
        }
        Ok(self.slots)
    }
}
fn run(case: &Value) -> (Result<bool, BindingError>, Vec<String>) {
    let cls = hex(&case["cls_hex"]);
    let info = cls.as_deref().map(|item_class| BindingInfo {
        item_class,
        equippable: case["equippable"].as_bool().unwrap(),
        bind: case["bind"].as_str(),
        trade_on_bind: case["trade"].as_str(),
    });
    let raw = case["attrs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                hex(&a["name"]).unwrap(),
                hex(&a["value"]),
                a["arg"].as_i64().unwrap() as i32,
                a["null_element"].as_bool().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let entries = raw
        .iter()
        .map(|(name, value, arg, null)| BindingAttribute {
            name,
            element: if *null {
                None
            } else {
                Some(BindingElement {
                    value: value.as_deref(),
                    arg: *arg,
                })
            },
        })
        .collect::<Vec<_>>();
    let attrs = BindingAttributes::new(&entries).unwrap();
    let mut context = Recorded {
        calls: vec![],
        slots: case["slots"].as_i64().unwrap() as i32,
        macro_enabled: case["macro"].as_bool().unwrap(),
        slot_error: case["slot_error"].as_bool().unwrap(),
    };
    let result = if case["mode"] == "bind" {
        check_character_bind(case["bound"].as_bool().unwrap(), info, &attrs, &mut context)
    } else {
        check_high_level_equipment(info.unwrap(), &attrs, &mut context)
    };
    (result, context.calls)
}
fn pack(result: Result<bool, BindingError>) -> (Value, Value) {
    match result {
        Ok(value) => (json!(value), Value::Null),
        Err(error) => (
            Value::Null,
            json!(match error {
                BindingError::IntegerFormat => "IntegerFormat",
                BindingError::IntegerOverflow => "IntegerOverflow",
                BindingError::MissingEnchant => "MissingEnchant",
                BindingError::GemstoneRange => "GemstoneRange",
                BindingError::Feature(_) => "Dependency",
                BindingError::CultureUnsupported => "CultureUnsupported",
                _ => panic!("unexpected error: {error}"),
            }),
        ),
    }
}
#[test]
fn binding_matches_recovered_framework_results_errors_and_lazy_dependency_order() {
    let fixture = fixture();
    assert_eq!(fixture["profiles"].as_array().unwrap().len(), 2);
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["culture"], "en-US");
        assert_eq!(profile["cases"].as_array().unwrap().len(), 2017);
        assert_eq!(profile["gemstone_values"], json!([0, 1, 2, 3]));
        for (i, case) in profile["cases"].as_array().unwrap().iter().enumerate() {
            let (result, calls) = run(case);
            let (result, error) = pack(result);
            assert_eq!(
                (result, error, json!(calls)),
                (
                    case["result"].clone(),
                    case["error"].clone(),
                    case["calls"].clone()
                ),
                "architecture={} case={i}",
                profile["architecture"]
            );
        }
    }
}
#[test]
fn unsupported_cultural_classes_refuse_after_feature_checks_without_fabricated_decision() {
    for profile in fixture()["profiles"].as_array().unwrap() {
        for case in profile["unsupported"].as_array().unwrap() {
            let (result, calls) = run(case);
            assert!(matches!(result, Err(BindingError::CultureUnsupported)));
            assert_eq!(calls, vec!["feature:off", "feature:off"]);
        }
    }
}
fn policy(slots: &str) -> FeaturePolicy {
    FeaturePolicy::new([
        ("CurrentVer".into(), Some("3.09.00".into())),
        ("BraceletMaxSlotCount".into(), Some(slots.into())),
        ("enabled".into(), Some("1.1".into())),
    ])
    .unwrap()
}
#[test]
fn parsed_item_attributes_use_concrete_feature_and_exact_enchant_catalog_adapter() {
    let expression = LegacyBytes::<1024, true>::from_bytes(bytes(
        "study_equipment[PREFIX:study_low,SUFFIX:study_high,QUALITY:x(4)]",
    ))
    .unwrap();
    let parsed =
        item_class_ex::parse(Some(&expression), &mut item_class_ex::integer_only_argument).unwrap();
    let policy = policy("4");
    let low = bytes("study_low");
    let high = bytes("study_high");
    let mut context = CatalogBindingContext::new(&policy, &[(&low, 6), (&high, 7)]).unwrap();
    let info = BindingInfo {
        item_class: parsed.item_class.as_bytes(),
        equippable: true,
        bind: Some("disabled"),
        trade_on_bind: Some("disabled"),
    };
    assert!(
        check_parsed_character_bind(false, Some(info), &parsed.attributes, &mut context).unwrap()
    );
}
#[test]
fn concrete_adapter_keeps_null_empty_feature_semantics_and_lazy_slot_overflow() {
    let policy = policy("2147483648");
    let mut context = CatalogBindingContext::new(&policy, &[]).unwrap();
    let cls = bytes("study_equipment");
    let attrs = BindingAttributes::new(&[]).unwrap();
    let info = BindingInfo {
        item_class: &cls,
        equippable: true,
        bind: None,
        trade_on_bind: Some("disabled"),
    };
    assert!(check_character_bind(false, Some(info), &attrs, &mut context).unwrap());
    assert!(
        check_character_bind(
            false,
            Some(BindingInfo {
                bind: Some(""),
                ..info
            }),
            &attrs,
            &mut context
        )
        .unwrap()
    );
    assert!(matches!(
        check_character_bind(
            false,
            Some(BindingInfo {
                bind: Some("disabled"),
                ..info
            }),
            &attrs,
            &mut context
        ),
        Err(BindingError::Feature(_))
    ));
}
#[test]
fn enchant_catalog_duplicates_error_and_case_variants_remain_distinct() {
    let policy = policy("0");
    let a = bytes("study");
    let upper = bytes("STUDY");
    assert!(matches!(
        CatalogBindingContext::new(&policy, &[(&a, 6), (&a, 7)]),
        Err(BindingError::Duplicate)
    ));
    let mut context = CatalogBindingContext::new(&policy, &[(&a, 6), (&upper, 7)]).unwrap();
    assert_eq!(
        (
            context.enchant_level(Some(&a)).unwrap(),
            context.enchant_level(Some(&upper)).unwrap()
        ),
        (6, 7)
    );
}
#[test]
fn opaque_views_reject_duplicate_names_odd_utf16_and_oversized_data_without_values_in_errors() {
    let name = bytes("LOOK");
    let entry = BindingAttribute {
        name: &name,
        element: None,
    };
    assert!(matches!(
        BindingAttributes::new(&[entry, entry]),
        Err(BindingError::Duplicate)
    ));
    assert!(matches!(
        BindingAttributes::new(&[BindingAttribute {
            name: &[1],
            element: None
        }]),
        Err(BindingError::Bound)
    ));
    let oversized = vec![0; 1026];
    assert!(matches!(
        BindingAttributes::new(&[BindingAttribute {
            name: &name,
            element: Some(BindingElement {
                value: Some(&oversized),
                arg: 0
            })
        }]),
        Err(BindingError::Bound)
    ));
    assert!(matches!(
        BindingAttributes::new(&vec![entry; 257]),
        Err(BindingError::Bound)
    ));
    assert!(matches!(
        CatalogBindingContext::new(&policy("0"), &vec![(&name[..], 0); 8193]),
        Err(BindingError::Bound)
    ));
    assert!(!BindingError::Bound.to_string().contains("LOOK"));
}
#[test]
fn mobile_binding_does_not_apply_the_default_queue_currency_override() {
    let policy = policy("0");
    let mut context = CatalogBindingContext::new(&policy, &[]).unwrap();
    let gold = bytes("gold");
    let silver = bytes("silver_coin");
    let attrs = BindingAttributes::new(&[]).unwrap();
    for cls in [&gold, &silver] {
        let info = BindingInfo {
            item_class: cls,
            equippable: false,
            bind: Some("disabled"),
            trade_on_bind: Some("disabled"),
        };
        assert!(check_character_bind(true, Some(info), &attrs, &mut context).unwrap());
    }
    assert!(
        !check_character_bind(
            false,
            Some(BindingInfo {
                item_class: &gold,
                equippable: false,
                bind: Some("disabled"),
                trade_on_bind: Some("disabled")
            }),
            &attrs,
            &mut context
        )
        .unwrap()
    );
}
