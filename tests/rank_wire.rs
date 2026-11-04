use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    rank::{self as wire, Kind},
};

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}

#[test]
fn framework_rank_messages_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-rank-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 141);
    let kinds = [
        ("query_random_rank", Kind::QueryRandomRank),
        ("random_rank", Kind::RandomRank),
        ("query_rank_alarm", Kind::QueryRankAlarm),
        ("rank_alarm", Kind::RankAlarm),
        ("query_rank_all", Kind::QueryRankAll),
        ("query_rank_character", Kind::QueryRankCharacter),
        ("query_rank_favorites", Kind::QueryRankFavorites),
        ("query_rank_goal", Kind::QueryRankGoal),
        ("query_rank_heavy_favorites", Kind::QueryRankHeavyFavorites),
        ("query_rank", Kind::QueryRank),
        ("query_rank_other", Kind::QueryRankOther),
        ("rank_character", Kind::RankCharacter),
        ("rank_favorites", Kind::RankFavorites),
        ("rank_goal", Kind::RankGoal),
        ("rank_heavy_favorites", Kind::RankHeavyFavorites),
        ("rank_all", Kind::RankAll),
        ("rank_info", Kind::RankInfo),
        ("rank_other", Kind::RankOther),
        ("set_rank_favorite", Kind::SetRankFavorite),
        ("set_rank_goal", Kind::SetRankGoal),
    ];
    for (n, case) in cases.iter().enumerate() {
        let name = case["kind"].as_str().unwrap();
        let kind = kinds.iter().find(|(k, _)| *k == name).expect("kind").1;
        let category = case["category"].as_i64().unwrap() as i32;
        if case["mode"] == "registered-typed" {
            assert_eq!(category, kind.category());
        }
        let typed = bytes(&case["typed_body"]);
        let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
        assert_eq!(decoded.is_none(), case["fields"].is_null(), "case {n}");
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            typed.as_slice(),
            "case {n}"
        );
        if Kind::INGRESS.contains(&kind) {
            assert_eq!(
                wire::decode_ingress(kind, category, &bytes(&case["body"]), DEFAULT_MAX_BODY)
                    .unwrap(),
                decoded,
                "ingress case {n}"
            );
        }
    }
    // Field mapping of the nested results (RankCharacterInfo, first non-null case).
    let case = cases
        .iter()
        .find(|c| c["kind"] == "rank_character" && !c["fields"].is_null())
        .unwrap();
    let Some(wire::Message::RankCharacter(r)) = wire::decode(
        Kind::RankCharacter,
        &bytes(&case["typed_body"]),
        DEFAULT_MAX_BODY,
    )
    .unwrap() else {
        panic!("no RankCharacterInfo")
    };
    let theirs = case["fields"]["RankResult"].as_array().unwrap();
    for (ours, t) in r.rank_result.unwrap().iter().zip(theirs) {
        let o = ours.as_ref().unwrap();
        assert_eq!(i64::from(o.event_id), t["EventID"].as_i64().unwrap());
        assert_eq!(i64::from(o.rank), t["Rank"].as_i64().unwrap());
        assert_eq!(i64::from(o.rank_prev), t["RankPrev"].as_i64().unwrap());
        assert_eq!(o.score, t["Score"].as_i64().unwrap());
        assert_eq!(o.character_name.as_deref(), t["CharacterName"].as_str());
    }
    assert_eq!(
        i64::from(r.period_type),
        case["fields"]["PeriodType"].as_i64().unwrap()
    );
}
