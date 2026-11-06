use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    guild_newbie::{self as wire, Kind},
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
        "find_newbie_recommend_guild" => Kind::FindNewbieRecommendGuild,
        "find_newbie_recommend_guild_result" => Kind::FindNewbieRecommendGuildResult,
        "request_newbie_recommend_guild" => Kind::RequestNewbieRecommendGuild,
        "notice_newbie_recommend" => Kind::NoticeNewbieRecommend,
        "direct_purchase_guild_item" => Kind::DirectPurchaseGuildItem,
        "direct_purchase_guild_item_result" => Kind::DirectPurchaseGuildItemResult,
        _ => panic!("unknown fixture type"),
    }
}

/// System.Object's GUID: the envelope of a null unregistered message.
const NULL_OBJECT: u128 = 0x81c5fe01_027c_3e1c_98d5_da9c9862aa21;

#[test]
fn framework_guild_newbie_messages_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-guild-newbie-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 17);
    let null_object = uuid::Uuid::from_u128(NULL_OBJECT);
    for (n, case) in cases.iter().enumerate() {
        let kind = kind(case["kind"].as_str().unwrap());
        let body = bytes(&case["body"]);
        let typed = bytes(&case["typed_body"]);
        let category = case["category"].as_i64().unwrap() as i32;
        if kind.category() == 0 {
            assert_eq!(category, 0, "case {n}");
            if typed[..16] == *null_object.as_bytes() {
                assert_eq!(
                    (&typed[16..], case["is_null"].as_bool()),
                    (&[0u8][..], Some(true))
                );
                continue;
            }
            assert_eq!(&typed[..16], kind.type_id().as_bytes(), "case {n}");
        } else if case["mode"] == "registered-typed" {
            assert_eq!(category, kind.category());
        }
        let payload = if kind.category() == 0 {
            &typed[16..]
        } else {
            &typed[..]
        };
        let decoded = wire::decode(kind, payload, DEFAULT_MAX_BODY)
            .unwrap_or_else(|e| panic!("case {n} {:?}: {e:?}", case["kind"]));
        assert_eq!(decoded.is_none(), case["fields"].is_null(), "case {n}");
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            payload,
            "case {n}"
        );
        if let Some(m) = &decoded {
            // The sent packet is the typed form: GUID envelope when unregistered.
            assert_eq!(
                wire::packet(m, DEFAULT_MAX_BODY).unwrap(),
                (kind.category(), typed.clone()),
                "case {n}"
            );
        }
        if Kind::INGRESS.contains(&kind) {
            assert_eq!(wire::ingress_kind(category, &body), Some(kind), "case {n}");
            assert_eq!(
                wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap(),
                decoded,
                "ingress case {n}"
            );
        }
    }
    // The recommended guild's InGameGuildInfo keeps its fields.
    let case = cases
        .iter()
        .rev()
        .find(|c| c["kind"] == "find_newbie_recommend_guild_result" && !c["fields"].is_null())
        .unwrap();
    let typed = bytes(&case["typed_body"]);
    let Some(wire::Message::FindNewbieRecommendGuildResult(found)) =
        wire::decode(Kind::FindNewbieRecommendGuildResult, &typed[16..], DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("no FindNewbieRecommendGuildResultMessage")
    };
    let info = found.info.unwrap();
    assert_eq!((info.guild_sn, info.member_count, info.is_newbie_recommend), (7, 2, true));
}
