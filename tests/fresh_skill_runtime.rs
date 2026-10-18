use serde::Deserialize;
use vindictus_server::domain::{
    fresh_skill_runtime::Detached,
    legacy_user_data::SqlDateTime2,
    skill_multicast::{self, Call, Grant, Runtime},
    skill_runtime::{Argument, Cache},
};
#[derive(Deserialize)]
struct Case {
    key: String,
    rank: i32,
    item: bool,
    stat: bool,
    arg: i32,
    ap: i32,
    last_ticks: String,
    bonus: i32,
    rank_sum: i32,
    arg_value: i32,
    item_loaded: bool,
    final_loaded: bool,
    unexpected_calls: usize,
}
#[derive(Deserialize)]
struct Results {
    cases: Vec<Case>,
    counterfactual_hits: Vec<usize>,
}
#[derive(Deserialize)]
struct Fixture {
    x86: Results,
    x64: Results,
}
fn text(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn verify(f: Results) {
    assert_eq!(f.cases.len(), 128);
    assert_eq!(f.counterfactual_hits.len(), 4);
    assert!(
        f.counterfactual_hits.iter().all(|n| *n > 0),
        "oracle did not detect loaded stats/connection/observer"
    );
    for c in f.cases {
        let key = text(&c.key);
        assert!(Detached::admits(&key));
        let date = SqlDateTime2::from_ticks(c.last_ticks.parse().unwrap()).unwrap();
        let mut runtime = Detached::new(c.ap, date, false).unwrap();
        let mut cache = Cache::default();
        let args = [Argument::new(Some(text("A")), Some(c.arg)).unwrap()];
        skill_multicast::after_grant(
            &mut cache,
            Grant {
                key: &key,
                rank: c.rank,
                required_ap: 27,
                arguments: &args,
                item_skill: c.item,
                stat_skill: c.stat,
            },
            &mut runtime,
        )
        .unwrap();
        for call in [Call::PushAp, Call::PushSkillList, Call::PushUpdateStat] {
            runtime.call(call, &cache).unwrap();
        }
        assert_eq!(
            (
                runtime.ap().ap,
                runtime.ap().last_update.ticks(),
                runtime.ap().bonus_seconds
            ),
            (c.ap, date.ticks(), c.bonus)
        );
        assert_eq!(
            (cache.rank_sum(), cache.argument(&key, &text("A"), -123)),
            (c.rank_sum, c.arg_value)
        );
        assert!(!c.item_loaded && !c.final_loaded && c.unexpected_calls == 0);
    }
}
#[test]
fn framework_x86_complete_guarded_methods_match_detached_skill_context() {
    let f: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-fresh-skills.json")).unwrap();
    verify(f.x86);
}
#[test]
fn framework_x64_complete_guarded_methods_match_detached_skill_context() {
    let f: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-fresh-skills.json")).unwrap();
    verify(f.x64);
}
#[test]
fn active_guild_or_meditation_are_not_silently_treated_as_detached_noops() {
    let date = SqlDateTime2::from_ticks(638000000000000000).unwrap();
    assert!(Detached::new(0, date, true).is_err());
    assert!(!Detached::admits(&text("skill_meditation")));
    assert!(Detached::admits(&text("Skill_meditation")));
    let mut runtime = Detached::new(0, date, false).unwrap();
    assert!(
        runtime
            .call(Call::RecalculateAp, &Cache::default())
            .is_err()
    );
    assert_eq!(runtime.ap().bonus_seconds, -1);
}
