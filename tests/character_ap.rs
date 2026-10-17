use serde::Deserialize;
use vindictus_server::domain::{
    character_ap::{ApError, Inputs, Snapshot, State, untrained_interval},
    legacy_user_data::SqlDateTime2,
    skill_runtime::{Argument, Cache},
};
#[derive(Deserialize)]
struct Case {
    op: i32,
    ap: i32,
    last: String,
    now: String,
    bonus: i32,
    max: i32,
    second_max: i32,
    value: i32,
    exceed: bool,
    clock_fail: bool,
    policy_fail_at: i32,
    interval_fail: bool,
    interval: i32,
    error: bool,
    result: serde_json::Value,
    final_ap: i32,
    final_last: String,
    final_bonus: i32,
    clock_calls: i32,
    policy_calls: i32,
    interval_calls: i32,
}
#[derive(Deserialize)]
struct Fixture {
    x86: Vec<Case>,
    x64: Vec<Case>,
}
struct Provider<'a> {
    case: &'a Case,
    clocks: i32,
    policies: i32,
    intervals: i32,
}
fn date(s: &str) -> SqlDateTime2 {
    SqlDateTime2::from_ticks(s.parse().unwrap()).unwrap()
}
impl Inputs for Provider<'_> {
    fn utc(&mut self) -> Result<SqlDateTime2, ApError> {
        self.clocks += 1;
        if self.case.clock_fail {
            Err(ApError::Clock)
        } else {
            Ok(date(&self.case.now))
        }
    }
    fn maximum(&mut self) -> Result<i32, ApError> {
        self.policies += 1;
        if self.policies == self.case.policy_fail_at {
            return Err(ApError::Maximum);
        };
        Ok(if self.policies == 1 {
            self.case.max
        } else {
            self.case.second_max
        })
    }
    fn interval(&mut self) -> Result<i32, ApError> {
        self.intervals += 1;
        if self.case.interval_fail {
            Err(ApError::Interval)
        } else {
            Ok(self.case.interval)
        }
    }
}
#[test]
fn ap_transitions_match_original_framework_on_both_architectures_and_preserve_partial_failures() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-character-ap.json")).unwrap();
    for (arch, cases) in [("x86", fixture.x86), ("x64", fixture.x64)] {
        assert_eq!(cases.len(), 448);
        for (i, c) in cases.iter().enumerate() {
            let mut state = State::restore(Snapshot {
                ap: c.ap,
                last_update: date(&c.last),
                bonus_seconds: c.bonus,
            });
            let mut provider = Provider {
                case: c,
                clocks: 0,
                policies: 0,
                intervals: 0,
            };
            let result = match c.op {
                0 => state.update(&mut provider).map(|_| serde_json::Value::Null),
                1 => state
                    .recalculate(&mut provider)
                    .map(|_| serde_json::Value::Null),
                2 => state
                    .use_ap(c.value, &mut provider)
                    .map(serde_json::Value::from),
                _ => state
                    .give(c.value, c.exceed, &mut provider)
                    .map(serde_json::Value::from),
            };
            assert_eq!(result.is_err(), c.error, "{arch}/{i}");
            if let Ok(value) = result {
                assert_eq!(value, c.result, "{arch}/{i}");
            }
            let s = state.snapshot();
            assert_eq!(
                (s.ap, s.last_update.ticks(), s.bonus_seconds),
                (c.final_ap, c.final_last.parse().unwrap(), c.final_bonus),
                "{arch}/{i}"
            );
            assert_eq!(
                (provider.clocks, provider.policies, provider.intervals),
                (c.clock_calls, c.policy_calls, c.interval_calls),
                "{arch}/{i}"
            );
        }
    }
}
struct Untouched;
impl Inputs for Untouched {
    fn utc(&mut self) -> Result<SqlDateTime2, ApError> {
        panic!("disabled interval sampled UTC")
    }
    fn maximum(&mut self) -> Result<i32, ApError> {
        panic!("disabled interval queried maximum")
    }
    fn interval(&mut self) -> Result<i32, ApError> {
        Ok(0)
    }
}
#[test]
fn constructor_recalculation_without_meditation_preserves_source_clock_and_policy_guards() {
    let start = SqlDateTime2::from_ticks(100).unwrap();
    let mut state = State::fresh(9, start);
    assert_eq!(state.snapshot().bonus_seconds, -1);
    state.update(&mut Untouched).unwrap();
    state.recalculate(&mut Untouched).unwrap();
    assert_eq!(
        (
            state.snapshot().ap,
            state.snapshot().last_update.ticks(),
            state.snapshot().bonus_seconds
        ),
        (9, 100, 0)
    );
    assert!(state.use_ap(3, &mut Untouched).unwrap());
    assert_eq!(state.snapshot().ap, 6);
}
#[test]
fn no_meditation_interval_follows_original_early_return_and_refuses_unported_float_branch() {
    let text = |s: &str| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-character-ap.json")).unwrap();
    for arch in ["prefix_x86", "prefix_x64"] {
        for case in fixture[arch].as_array().unwrap() {
            let mut cache = Cache::default();
            let reduction = case["reduction"].as_i64().unwrap() as i32;
            cache
                .update_arguments(
                    &text("skill_meditation"),
                    Some(&[Argument::new(
                        Some(text("meditation_interval_reduction")),
                        Some(reduction),
                    )
                    .unwrap()]),
                )
                .unwrap();
            let result = untrained_interval(&cache);
            assert_eq!(result.is_err(), case["error"].as_bool().unwrap());
            if let Ok(n) = result {
                assert_eq!(n, case["result"].as_i64().unwrap() as i32);
                assert_eq!(case["float_reads"], 0);
            }
        }
    }
    let mut cache = Cache::default();
    assert_eq!(untrained_interval(&cache), Ok(0));
    let skill = text("skill_meditation");
    cache.update_arguments(&skill, Some(&[])).unwrap();
    assert_eq!(untrained_interval(&cache), Err(ApError::Meditation));
    cache
        .update_arguments(
            &skill,
            Some(&[Argument::new(Some(text("meditation_interval_reduction")), Some(-1)).unwrap()]),
        )
        .unwrap();
    assert_eq!(untrained_interval(&cache), Ok(0));
}
