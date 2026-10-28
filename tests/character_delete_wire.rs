use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    character_delete::{self as wire, CancelResult, DeleteResult, Kind, Message},
    server_profile::REGISTERED_CLASSES,
};
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn expected(kind: Kind, f: &Value, null: bool) -> Option<Message> {
    if null {
        return None;
    }
    Some(match kind {
        Kind::Delete => Message::Delete {
            name: f["Name"].as_str().map(str::to_owned),
            character_sn: int(&f["CharacterSN"]),
        },
        Kind::Cancel => Message::Cancel {
            name: f["Name"].as_str().map(str::to_owned),
            character_sn: int(&f["CharacterSN"]),
        },
        Kind::DeleteResult => Message::DeleteResult {
            character_sn: int(&f["CharacterSN"]),
            result: int(&f["Result"]),
            remain_time_sec: int(&f["RemainTimeSec"]),
        },
        Kind::CancelResult => Message::CancelResult {
            character_sn: int(&f["CharacterSN"]),
            result: int(&f["Result"]),
        },
        Kind::SelectOrders => Message::SelectOrders(
            f["SelectOrders"]
                .as_array()
                .map(|v| v.iter().map(|n| n.as_u64().unwrap() as u8).collect()),
        ),
    })
}
#[test]
fn framework_character_delete_fields_bytes_envelopes_and_truncations_match_x86_x64() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/framework-character-delete-wire.json"
    ))
    .unwrap();
    assert_eq!(fixture["profiles"].as_array().unwrap().len(), 2);
    for profile in fixture["profiles"].as_array().unwrap() {
        let cases = profile["wire_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 594);
        for (n, case) in cases.iter().enumerate() {
            let kind = match case["kind"].as_str().unwrap() {
                "delete" => Kind::Delete,
                "delete_cancel" => Kind::Cancel,
                "delete_result" => Kind::DeleteResult,
                "delete_cancel_result" => Kind::CancelResult,
                "select_orders" => Kind::SelectOrders,
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
            assert_eq!(
                wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap(),
                message,
                "case {n}"
            );
            assert_eq!(
                wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap(),
                message,
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
fn character_delete_result_values_follow_the_recovered_enums() {
    assert_eq!(DeleteResult::SuccessDeleted as i32, 1);
    assert_eq!(DeleteResult::SuccessDeleting as i32, 2);
    assert_eq!(DeleteResult::FailRemainCashItems as i32, 5);
    assert_eq!(DeleteResult::FailNeedMoreWaiting as i32, 8);
    assert_eq!(DeleteResult::FailExcuteOp as i32, 12);
    assert_eq!(CancelResult::Success as i32, 1);
    assert_eq!(CancelResult::FailUnknown as i32, 5);
    let message = Message::Delete {
        name: None,
        character_sn: 1,
    };
    assert!(wire::encode(Kind::Cancel, Some(&message), DEFAULT_MAX_BODY).is_err());
    assert!(wire::decode_ingress(Kind::Delete, 258, &[0], DEFAULT_MAX_BODY).is_err());
}
