use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    pvp::{self as wire, Kind},
};

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}

fn kind(name: &str) -> Kind {
    match name {
        "query_channel_list" => Kind::QueryChannelList,
        "channel_list" => Kind::ChannelList,
        "register" => Kind::Register,
        "unregister" => Kind::Unregister,
        "confirm_join" => Kind::ConfirmJoin,
        "register_result" => Kind::RegisterResult,
        "command" => Kind::Command,
        "kicked" => Kind::Kicked,
        "report" => Kind::Report,
        "team_info" => Kind::TeamInfo,
        "game_hosted" => Kind::GameHosted,
        "host_info" => Kind::HostInfo,
        "member_info" => Kind::MemberInfo,
        "arena_result_info" => Kind::ArenaResultInfo,
        "penalty" => Kind::Penalty,
        "register_game_waiting_queue" => Kind::RegisterGameWaitingQueue,
        "unregister_game_waiting_queue" => Kind::UnregisterGameWaitingQueue,
        _ => panic!("unknown fixture type"),
    }
}

/// System.Object's GUID: the envelope of a null unregistered message.
const NULL_OBJECT: u128 = 0x81c5fe01_027c_3e1c_98d5_da9c9862aa21;

#[test]
fn framework_pvp_match_messages_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-pvp-match-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 57);
    let null_object = uuid::Uuid::from_u128(NULL_OBJECT);
    for (n, case) in cases.iter().enumerate() {
        let kind = kind(case["kind"].as_str().unwrap());
        let typed = bytes(&case["typed_body"]);
        let generic = bytes(&case["generic_body"]);
        let category = case["category"].as_i64().unwrap() as i32;
        assert_eq!(category, kind.category(), "case {n}");
        let is_null = case["is_null"].as_bool().unwrap();
        if kind.category() == 0 {
            if typed[..16] == *null_object.as_bytes() {
                assert!(is_null, "case {n}");
                assert_eq!(&typed[16..], &[0u8][..]);
                continue;
            }
            assert_eq!(&typed[..16], kind.type_id().as_bytes(), "case {n}");
        }
        let payload = if kind.category() == 0 {
            &typed[16..]
        } else {
            &typed[..]
        };
        let decoded = wire::decode(kind, payload, DEFAULT_MAX_BODY)
            .unwrap_or_else(|e| panic!("case {n} {:?}: {e:?}", case["kind"]));
        assert_eq!(decoded.is_none(), is_null, "case {n}");
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            payload,
            "case {n}"
        );
        if let Some(m) = &decoded {
            assert_eq!(
                wire::packet(m, DEFAULT_MAX_BODY).unwrap(),
                (kind.category(), typed.clone()),
                "case {n}"
            );
        }
        // The client may send the unregistered generic form of any message.
        assert_eq!(case["generic_category"], 0, "case {n}");
        let ingress = wire::decode_ingress(kind, 0, &generic, DEFAULT_MAX_BODY)
            .unwrap_or_else(|e| panic!("case {n} generic: {e:?}"));
        assert_eq!(ingress, decoded, "case {n}");
        if kind.category() != 0 {
            assert_eq!(
                wire::decode_ingress(kind, kind.category(), &typed, DEFAULT_MAX_BODY).unwrap(),
                decoded,
                "case {n}"
            );
        }
    }
}
