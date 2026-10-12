use serde_json::{Value, json};
use vindictus_server::domain::{
    item_class_ex::{self, ItemAttribute, ItemClassExError},
    item_location::ItemLocation,
    legacy_user_data::LegacyBytes,
};
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-item-parser.json")).unwrap()
}
fn value(text: &Value) -> Option<LegacyBytes<1024, true>> {
    text.as_str().map(|s| {
        LegacyBytes::from_bytes(
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect(),
        )
        .unwrap()
    })
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn attribute(a: &ItemAttribute) -> Value {
    json!({"name_hex":hex(a.name.as_bytes()),"value_hex":hex(a.value.as_bytes()),"arg":a.arg,"arg2":a.arg2})
}
fn units(value: &LegacyBytes<1024, true>) -> Vec<u16> {
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect()
}
fn controlled(text: &[u16]) -> i32 {
    if let Some(value) = item_class_ex::try_integer_argument(text) {
        return value;
    }
    (text.len() * 17 + text.iter().map(|u| usize::from(*u)).sum::<usize>() % 101) as i32
}
#[test]
fn parser_build_and_argument_call_order_match_source_in_both_architectures_and_five_cultures() {
    for profile in fixture()["profiles"].as_array().unwrap() {
        for culture in profile["profiles"].as_array().unwrap() {
            for case in culture["cases"].as_array().unwrap() {
                let input = value(&case["input_hex"]);
                let mut calls = Vec::new();
                let mut resolve = |text: &[u16]| {
                    calls.push(hex(&text
                        .iter()
                        .flat_map(|u| u.to_le_bytes())
                        .collect::<Vec<_>>()));
                    Ok(controlled(text))
                };
                let result = match case["mode"].as_str().unwrap() {
                    "class" => item_class_ex::parse(input.as_ref(), &mut resolve).map(|parsed| {
                        let built =
                            item_class_ex::build(&parsed.item_class, &parsed.attributes).unwrap();
                        (
                            Some(hex(parsed.item_class.as_bytes())),
                            parsed.attributes,
                            Some(hex(built.as_bytes())),
                        )
                    }),
                    "attributes" => item_class_ex::parse_attributes(input.as_ref(), &mut resolve)
                        .map(|a| (None, a, None)),
                    "single" => item_class_ex::parse_single_attribute(input.as_ref(), &mut resolve)
                        .map(|a| (None, a.into_iter().collect(), None)),
                    _ => unreachable!(),
                };
                assert_eq!(json!(calls), case["arguments"], "{case}");
                if case["error"] == true {
                    assert!(
                        matches!(
                            result,
                            Err(ItemClassExError::NullInput | ItemClassExError::DuplicateAttribute)
                        ),
                        "{case}"
                    );
                } else {
                    let (class, attrs, built) = result.unwrap();
                    assert_eq!(json!(class), case["class_hex"], "{case}");
                    assert_eq!(
                        json!(attrs.iter().map(attribute).collect::<Vec<_>>()),
                        case["attrs"],
                        "{case}"
                    );
                    assert_eq!(json!(built), case["built_hex"], "{case}");
                }
            }
        }
    }
}
#[test]
fn integer_argument_primitive_matches_framework_tryparse_in_five_cultures() {
    for profile in fixture()["profiles"].as_array().unwrap() {
        for culture in profile["profiles"].as_array().unwrap() {
            for case in culture["numbers"].as_array().unwrap() {
                let input = value(&case["input_hex"]);
                let result = input
                    .map(|v| units(&v))
                    .and_then(|text| item_class_ex::try_integer_argument(&text));
                assert_eq!(json!(result), case["result"], "{case}");
                assert_eq!(result.is_some(), case["success"] == true, "{case}");
            }
        }
    }
}
#[test]
fn parsed_attribute_equality_ignores_order_but_preserves_case_values_and_both_arguments() {
    for profile in fixture()["profiles"].as_array().unwrap() {
        for culture in profile["profiles"].as_array().unwrap() {
            for case in culture["pairs"].as_array().unwrap() {
                let l = value(&case["left_hex"]);
                let r = value(&case["right_hex"]);
                let l = item_class_ex::parse(l.as_ref(), |t| Ok(controlled(t))).unwrap();
                let r = item_class_ex::parse(r.as_ref(), |t| Ok(controlled(t))).unwrap();
                assert_eq!(item_class_ex::same(&l, &r), case["same"] == true, "{case}");
            }
        }
    }
}
#[test]
fn item_location_null_sentinel_matches_source_and_signed_slot_boundaries() {
    for profile in fixture()["profiles"].as_array().unwrap() {
        for case in profile["locations"].as_array().unwrap() {
            let location = ItemLocation {
                collection: case["collection"].as_u64().unwrap().try_into().unwrap(),
                slot: case["slot"].as_i64().unwrap().try_into().unwrap(),
            };
            assert_eq!(location.is_null(), case["is_null"] == true, "{case}");
            assert_eq!(
                location == ItemLocation::NULL,
                case["equal_null"] == true,
                "{case}"
            );
        }
    }
}
#[test]
fn unresolved_date_or_relative_arguments_fail_closed_in_integer_only_adapter() {
    let input = LegacyBytes::from_bytes(
        "item[A:x(2020-01-02)]"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        item_class_ex::parse(Some(&input), item_class_ex::integer_only_argument),
        Err(ItemClassExError::ArgumentUnsupported)
    ));
    let input = LegacyBytes::from_bytes(
        "item[A:x(1sec)]"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        item_class_ex::parse(Some(&input), item_class_ex::integer_only_argument),
        Err(ItemClassExError::ArgumentUnsupported)
    ));
}
#[test]
fn builder_stays_bounded_even_when_external_argument_resolution_expands_output() {
    let input = LegacyBytes::from_bytes(
        "x".repeat(512)
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    let empty = LegacyBytes::from_bytes(vec![]).unwrap();
    let parsed = item_class_ex::parse(Some(&input), item_class_ex::integer_only_argument).unwrap();
    assert_eq!(
        item_class_ex::build(&parsed.item_class, &[])
            .unwrap()
            .as_bytes(),
        input.as_bytes()
    );
    assert!(matches!(
        item_class_ex::build(
            &input,
            &[ItemAttribute {
                name: empty,
                value: LegacyBytes::from_bytes(vec![]).unwrap(),
                arg: i32::MIN,
                arg2: i32::MAX
            }]
        ),
        Err(ItemClassExError::Bound)
    ));
}
