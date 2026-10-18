use serde_json::{Value, json};
use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    initial_social,
    initial_status::{Loaded, Profile},
    legacy_user_data::SqlDateTime2,
};
fn social(cid: i64) -> initial_social::Loaded {
    let policy = FeaturePolicy::with_recorded_comparisons(
        [
            ("CurrentVer".into(), Some("a".into())),
            ("InGameGuild".into(), Some("b".into())),
        ],
        [("a".into(), "b".into(), true)],
    )
    .unwrap();
    initial_social::Loaded::load(31, cid, &policy).unwrap()
}
fn compare(arch: &str) {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-status.json")).unwrap();
    let cases = fixture[arch]["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 32);
    for c in cases {
        let cid = c["cid"].as_i64().unwrap();
        let social = social(cid);
        let mut profile = Profile::default();
        let mut calls = 0;
        let result = Loaded::load(
            &social,
            || {
                calls += 1;
                if c["mode"] == 1 {
                    Err("clock")
                } else {
                    Ok(SqlDateTime2::from_ticks(100).unwrap())
                }
            },
            &mut profile,
        );
        assert_eq!(calls, c["clock_calls"]);
        assert_eq!(profile.calls(), c["profile_calls"]);
        assert!(
            profile.min_ms() >= 0.0
                && profile.max_ms() >= profile.min_ms()
                && profile.total_ms() >= profile.max_ms()
        );
        assert_eq!(profile.average_ms(), profile.total_ms());
        assert_eq!(c["profile_valid"], true);
        for key in [
            "submits",
            "stat_calculations",
            "pushes",
            "frontend",
            "micro",
            "reloads",
        ] {
            assert_eq!(c[key], 0);
        }
        if !c["success"].as_bool().unwrap() {
            assert!(matches!(result, Err("clock")));
            continue;
        }
        let out = result.unwrap();
        let status = out.status();
        assert_eq!(status.uid(), 31);
        assert_eq!(status.cid(), cid);
        assert!(out.matches_social(&social));
        assert_eq!(out.campfire().uid(), 31);
        assert_eq!(out.campfire().cid(), cid);
        assert!(status.save_succeeded());
        let stats: serde_json::Map<String, Value> = status
            .stats()
            .entries()
            .map(|(k, v)| (k.into(), json!(v)))
            .collect();
        let actual = json!({"types":status.type_count(),"tags":status.tag_count(),"need_db":status.need_db_submit(),"need_stat":status.need_update_stat(),"updated":status.updated_count(),"stats":stats,"same_context":out.shares_character_context(),"campfire":out.campfire().status()});
        assert_eq!(actual, c["expected"]);
    }
    // Counterfactuals execute the original connected Save, outside the detached
    // native type. These ensure the oracle would detect suppressing these effects.
    let probes = fixture[arch]["probes"].as_array().unwrap();
    assert_eq!(probes.len(), 3);
    assert_eq!(probes[0]["frontend"], 1);
    assert_eq!(probes[0]["micro"], 0);
    assert_eq!(probes[0]["clock_calls"], 2);
    assert_eq!(probes[1]["frontend"], 1);
    assert_eq!(probes[1]["micro"], 1);
    assert_eq!(probes[2]["frontend"], 1);
    assert_eq!(probes[2]["micro"], 0);
    assert_eq!(probes[2]["reloads"], 1);
    // Save failure is caught and ignored by original constructor; Town still runs.
    assert_eq!(probes[2]["success"], true);
}
#[test]
fn initial_status_matches_framework_x86() {
    compare("x86");
}
#[test]
fn initial_status_matches_framework_x64() {
    compare("x64");
}
#[test]
fn profiling_retains_failed_and_successful_constructor_calls() {
    let social = social(1);
    let mut p = Profile::default();
    assert_eq!(p.calls(), 0);
    assert_eq!(p.min_ms(), 0.0);
    assert_eq!(p.average_ms(), 0.0);
    assert!(Loaded::load(&social, || Err::<SqlDateTime2, _>("clock"), &mut p).is_err());
    assert_eq!(p.calls(), 1);
    let out = Loaded::load(
        &social,
        || Ok::<_, &str>(SqlDateTime2::from_ticks(100).unwrap()),
        &mut p,
    )
    .unwrap();
    assert_eq!(p.calls(), 2);
    assert!(p.total_ms() >= p.max_ms());
    assert_eq!(p.average_ms(), p.total_ms() / 2.0);
    assert!(out.matches_social(&social));
    assert!(!out.matches_social(&self::social(1)));
}
