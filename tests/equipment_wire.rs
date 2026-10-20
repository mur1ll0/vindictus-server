use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    equipment::{self as wire, Kind, Message},
    inventory::Dict,
    server_profile::REGISTERED_CLASSES,
};
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn expected(kind: Kind, fields: &Value, null: bool) -> Option<Message> {
    if null {
        return None;
    }
    Some(match kind {
        Kind::EquipItem => Message::EquipItem {
            item_id: fields["ItemID"].as_i64().unwrap(),
            part: fields["PartID"].as_i64().unwrap() as i32,
        },
        Kind::EquipmentInfo => {
            Message::EquipmentInfo(Dict(fields["EquipInfos"].as_array().map(|rows| {
                rows.iter()
                    .map(|r| {
                        (
                            r["key"].as_i64().unwrap() as i32,
                            r["value"].as_i64().unwrap(),
                        )
                    })
                    .collect()
            })))
        }
    })
}
#[test]
fn framework_equipment_fields_bytes_envelopes_and_truncations_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-equipment-wire.json")).unwrap();
    assert_eq!(fixture["profiles"].as_array().unwrap().len(), 2);
    for profile in fixture["profiles"].as_array().unwrap() {
        let cases = profile["wire_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 90);
        for (n, case) in cases.iter().enumerate() {
            let kind = match case["kind"].as_str().unwrap() {
                "equip_item" => Kind::EquipItem,
                "equipment_info" => Kind::EquipmentInfo,
                _ => panic!("unknown fixture type"),
            };
            let category = case["category"].as_i64().unwrap() as i32;
            let body = bytes(&case["body"]);
            let typed = bytes(&case["typed_body"]);
            let message = expected(kind, &case["fields"], case["is_null"].as_bool().unwrap());
            assert!(
                REGISTERED_CLASSES
                    .iter()
                    .any(|r| r.category_id == kind.category() && r.type_id == kind.type_id())
            );
            assert!(
                wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap() == message,
                "case {n}"
            );
            assert!(
                wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap() == message,
                "case {n}"
            );
            assert_eq!(
                wire::encode(kind, message.as_ref(), DEFAULT_MAX_BODY)
                    .unwrap()
                    .as_slice(),
                typed.as_slice(),
                "case {n}"
            );
            for cut in 0..body.len() {
                assert!(
                    wire::decode_ingress(kind, category, &body[..cut], DEFAULT_MAX_BODY).is_err(),
                    "case {n}, cut {cut}"
                );
            }
            let mut trailing = body.clone();
            trailing.push(0);
            assert!(wire::decode_ingress(kind, category, &trailing, DEFAULT_MAX_BODY).is_err());
            assert!(wire::decode_ingress(kind, category, &body, body.len() - 1).is_err());
        }
    }
}
#[test]
fn equipment_bounds_duplicate_keys_and_mismatched_types_are_refused() {
    let kind = Kind::EquipmentInfo;
    for entries in [
        vec![(0, 1), (0, 2)],
        (0..4097).map(|n| (n, i64::from(n))).collect(),
    ] {
        assert!(
            wire::encode(
                kind,
                Some(&Message::EquipmentInfo(Dict(Some(entries)))),
                DEFAULT_MAX_BODY
            )
            .is_err()
        );
    }
    let message = Message::EquipItem {
        item_id: -1,
        part: 255,
    };
    assert!(wire::encode(kind, Some(&message), DEFAULT_MAX_BODY).is_err());
    assert!(wire::encode(Kind::EquipItem, Some(&message), 12).is_err());
    assert!(wire::decode_ingress(Kind::EquipItem, 458, &[0], DEFAULT_MAX_BODY).is_err());
    let mut body = kind.type_id().as_bytes().to_vec();
    body.push(0);
    assert!(wire::decode_ingress(Kind::EquipItem, 0, &body, DEFAULT_MAX_BODY).is_err());
    let mut duplicate = vec![1, 3];
    for value in [1_i64, 2] {
        duplicate.extend(0_i32.to_be_bytes());
        duplicate.extend(value.to_be_bytes());
    }
    assert!(wire::decode(kind, &duplicate, DEFAULT_MAX_BODY).is_err());
}
