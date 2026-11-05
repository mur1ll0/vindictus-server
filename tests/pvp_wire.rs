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

#[test]
fn framework_free_match_report_matches_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-pvp-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 18);
    for (n, case) in cases.iter().enumerate() {
        assert_eq!(case["kind"], "free_match_report");
        let kind = Kind::FreeMatchReport;
        let category = case["category"].as_i64().unwrap() as i32;
        if case["mode"] == "registered-typed" {
            assert_eq!(category, kind.category());
        }
        let typed = bytes(&case["typed_body"]);
        let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
        assert_eq!(decoded.is_none(), case["fields"].is_null(), "case {n}");
        if let Some(wire::Message::FreeMatchReport(r)) = &decoded {
            assert_eq!(
                (i64::from(r.winner_tag), i64::from(r.loser_tag)),
                (
                    case["fields"]["WinnerTag"].as_i64().unwrap(),
                    case["fields"]["LoserTag"].as_i64().unwrap()
                ),
                "case {n}"
            );
        }
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            typed.as_slice(),
            "case {n}"
        );
        assert_eq!(
            wire::decode_ingress(kind, category, &bytes(&case["body"]), DEFAULT_MAX_BODY).unwrap(),
            decoded,
            "ingress case {n}"
        );
    }
}

#[test]
fn free_match_tags_resolve_to_the_play_players() {
    use vindictus_server::infrastructure::pvp::resolve;
    let players = [(0, 11), (1, 22)];
    assert_eq!(resolve(&players, 1, 0), Some((22, 11)));
    assert_eq!(resolve(&players, 2, 0), None);
    assert_eq!(resolve(&players, 0, 5), None);
}
