use serde_json::{Value, json};
use vindictus_server::{
    domain::{
        feature_policy::FeaturePolicy,
        initial_attendance::{Loaded, Outcome},
        initial_social,
        legacy_user_data::SqlDateTime2,
    },
    error::Error,
};
fn policy(enabled: bool, version: Option<&str>) -> FeaturePolicy {
    FeaturePolicy::with_recorded_comparisons(
        [
            ("CurrentVer".into(), Some("a".into())),
            ("InGameGuild".into(), Some("on".into())),
            (
                "Attendance_Newbie".into(),
                Some(if enabled { "on" } else { "off" }.into()),
            ),
            ("AttendanceEvent_Newbie".into(), version.map(str::to_owned)),
        ],
        [
            ("a".into(), "on".into(), true),
            ("a".into(), "off".into(), false),
        ],
    )
    .unwrap()
}
fn compare(arch: &str) {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-attendance.json")).unwrap();
    let cases = f[arch].as_array().unwrap();
    assert_eq!(cases.len(), 96);
    let mut counters = [0usize; 3];
    for c in cases {
        let p = policy(c["enabled"].as_bool().unwrap(), c["version"].as_str());
        let social = initial_social::Loaded::load(31, 101, &p).unwrap();
        let mut out = Loaded::all_user(&social);
        assert!(out.matches_social(&social));
        let enabled = Loaded::enabled(&p).unwrap();
        let mut clocks = 0;
        let result = if enabled {
            let first = !matches!(c["user_mode"].as_i64().unwrap(), 0 | 1);
            out.initialize_newbie(
                first,
                &p,
                |v| {
                    if v == 7 && c["has_period"] == true {
                        Some((100, 200))
                    } else {
                        None
                    }
                },
                || {
                    clocks += 1;
                    if c["fail_clock"] == clocks {
                        Err("clock")
                    } else {
                        Ok(SqlDateTime2::from_ticks(
                            c["times"][(clocks - 1) as usize].as_i64().unwrap(),
                        )
                        .unwrap())
                    }
                },
            )
        } else {
            Ok(())
        };
        assert_eq!(clocks, c["clock_calls"].as_u64().unwrap());
        let trace = c["trace"].as_array().unwrap();
        assert_eq!(trace[0], "item-context");
        assert_eq!(trace[1], "feature");
        if enabled {
            assert!(
                trace.iter().position(|s| s == "Count").unwrap()
                    < trace.iter().position(|s| s == "nullable-int").unwrap()
            );
            assert_eq!(
                trace.iter().filter(|s| *s == "First").count(),
                usize::from(c["user_mode"] != 0)
            );
        } else {
            assert!(
                !trace
                    .iter()
                    .any(|s| s == "Count" || s == "nullable-int" || s == "clock")
            );
        }
        if c["supported"] == false {
            assert!(result.is_err());
            counters[0] += 1;
            continue;
        }
        result.unwrap();
        assert_eq!(out.instance_count(), c["instances"]);
        assert_eq!(
            matches!(
                out.outcome(),
                Outcome::CaughtIntegerOverflow | Outcome::CaughtClockError
            ),
            c["caught"] == 1
        );
        if out.outcome() == Outcome::CaughtClockError {
            counters[1] += 1;
        }
        if out.outcome() == Outcome::CaughtIntegerOverflow {
            counters[2] += 1;
        }
        let a = out.all_user_state();
        let actual = json!({"cid":a.cid(),"uid":a.uid(),"version":a.event_version(),"from_ticks":a.date_from_ticks(),"until_ticks":a.date_until_ticks(),"callback":a.db_destroyed_callback_present(),"attendance_count":a.attendance_count(),"bonus_count":a.bonus_count(),"last_error":a.last_feature_error(),"parent_matches":a.context_matches_parent(),"context_present":true});
        assert_eq!(actual, c["expected"]);
    }
    assert!(counters.into_iter().all(|c| c > 0));
}
#[test]
fn initial_attendance_matches_framework_x86() {
    compare("x86");
}
#[test]
fn initial_attendance_matches_framework_x64() {
    compare("x64");
}
#[test]
fn attendance_refuses_unknown_feature_and_keeps_nullable_integer_contract() {
    let p = FeaturePolicy::new([
        ("CurrentVer".into(), Some("unknown".into())),
        ("Attendance_Newbie".into(), Some("unmeasured".into())),
    ])
    .unwrap();
    assert!(matches!(
        Loaded::enabled(&p),
        Err(Error::FeatureVersionUnsupported)
    ));
    for (v, expected) in [
        (None, None),
        (Some(""), None),
        (Some("bad"), None),
        (Some("0"), Some(0)),
        (Some(" +7\0"), Some(7)),
        (Some("\u{00a0}7"), None),
    ] {
        assert_eq!(
            policy(true, v)
                .get_nullable_integer("AttendanceEvent_Newbie")
                .unwrap(),
            expected
        );
    }
    assert!(matches!(
        policy(true, Some("2147483648")).get_nullable_integer("AttendanceEvent_Newbie"),
        Err(Error::FeatureIntegerOverflow)
    ));
}
