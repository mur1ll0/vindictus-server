use serde::Deserialize;
use vindictus_server::domain::{
    skill_multicast::{self, Call, DispatchError, Grant, Runtime},
    skill_runtime::{Argument, Cache},
};
#[derive(Deserialize, Debug, PartialEq, Eq)]
struct Event {
    name: String,
    sum: i32,
    arg: i32,
}
#[derive(Deserialize)]
struct Case {
    key: String,
    rank: i32,
    item: bool,
    stat: bool,
    kind: usize,
    fail: usize,
    error: bool,
    rank_sum: i32,
    arg: i32,
    events: Vec<Event>,
}
#[derive(Deserialize)]
struct Fixture {
    x86: Vec<Case>,
    x64: Vec<Case>,
}
fn text(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
struct Recorder<'a> {
    key: &'a [u8],
    rank: i32,
    fail: usize,
    events: Vec<Event>,
}
impl Runtime for Recorder<'_> {
    type Error = ();
    fn call(&mut self, call: Call<'_>, cache: &Cache) -> Result<(), ()> {
        let name = match call {
            Call::RecalculateAp => "ap",
            Call::PushAp => "ap-push",
            Call::GuildSkillUp { key, rank } => {
                assert_eq!((key, rank), (self.key, self.rank));
                "guild"
            }
            Call::PushSkillList => "skills",
            Call::CalculateItemStat => "item",
            Call::CalculateFinalStat => "final",
            Call::PushUpdateStat => "stat-push",
            Call::SkillUpdated => "skill",
            Call::MicroPlayUpdated => "micro",
        };
        // RequiredAP is established before the multicast, including cache/provider errors.
        assert_eq!(cache.required_ap(self.key), Some(27));
        self.events.push(Event {
            name: name.into(),
            sum: cache.rank_sum(),
            arg: cache.argument(self.key, &text("A"), -123),
        });
        if self.fail == self.events.len() {
            Err(())
        } else {
            Ok(())
        }
    }
}
fn verify(cases: Vec<Case>) {
    assert_eq!(cases.len(), 256);
    for (i, c) in cases.into_iter().enumerate() {
        let key = text(&c.key);
        let mut args = vec![
            Argument::new(
                if c.kind == 2 { None } else { Some(text("A")) },
                if c.kind == 3 { None } else { Some(i as i32) },
            )
            .unwrap(),
        ];
        if c.kind == 1 {
            args.push(Argument::new(Some(text("A")), Some(9)).unwrap());
        }
        let mut cache = Cache::default();
        let mut runtime = Recorder {
            key: &key,
            rank: c.rank,
            fail: c.fail,
            events: Vec::new(),
        };
        let result = skill_multicast::after_grant(
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
        );
        assert_eq!(result.is_err(), c.error, "case {i}");
        assert_eq!(cache.required_ap(&key), Some(27), "case {i}");
        assert_eq!(cache.base_rank(&key), c.rank, "case {i}");
        assert_eq!(cache.rank_sum(), c.rank_sum, "case {i}");
        assert_eq!(cache.argument(&key, &text("A"), -123), c.arg, "case {i}");
        assert_eq!(runtime.events, c.events, "case {i}");
        if matches!(c.kind, 1..=3) {
            assert!(matches!(result, Err(DispatchError::Cache(_))));
        }
    }
}
#[test]
fn original_framework_x86_subscribers_match_cache_and_partial_failure_order() {
    let f: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-skill-multicast.json")).unwrap();
    verify(f.x86);
}
#[test]
fn original_framework_x64_subscribers_match_cache_and_partial_failure_order() {
    let f: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-skill-multicast.json")).unwrap();
    verify(f.x64);
}
