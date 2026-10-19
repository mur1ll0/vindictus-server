use serde_json::Value;
use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    initial_attendance::{AdmissionError, Loaded, Outcome},
    initial_social,
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
#[test]
fn existing_empty_and_failed_lookup_match_original_framework_order_and_state() {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-existing-attendance.json")).unwrap();
    assert_eq!(f["x86"], f["x64"]);
    let cases = f["x86"].as_array().unwrap();
    assert_eq!(cases.len(), 72);
    let mut queries = 0;
    for c in cases {
        let p = policy(c["enabled"].as_bool().unwrap(), c["version"].as_str());
        let social = initial_social::Loaded::load(31, 101, &p).unwrap();
        let mut loaded = Loaded::all_user(&social);
        if Loaded::enabled(&p).unwrap() {
            match loaded.initialize_newbie(
                false,
                &p,
                |_| Some((100, 200)),
                || -> Result<_, ()> { panic!("existing branch never samples UTC") },
            ) {
                Err(AdmissionError::Existing) => {
                    loaded
                        .resolve_existing_absence(c["query_mode"] != 0)
                        .unwrap();
                    queries += 1;
                }
                other => other.unwrap(),
            }
        }
        let trace = c["trace"].as_array().unwrap();
        let queried = trace.iter().any(|t| t == "existing-query");
        if queried {
            assert!(
                trace.iter().position(|t| t == "nullable-int").unwrap()
                    < trace.iter().position(|t| t == "existing-query").unwrap()
            );
        }
        let expected = if c["enabled"] == false {
            Outcome::Disabled
        } else if !queried {
            Outcome::CaughtIntegerOverflow
        } else if c["query_mode"] == 0 {
            Outcome::ExistingAbsent
        } else {
            Outcome::CaughtExistingLookupError
        };
        assert_eq!(loaded.outcome(), expected);
        assert_eq!(loaded.instance_count(), 1);
        assert_eq!(c["instances"], 1);
        assert_eq!(c["clock_calls"], 0);
        assert_eq!(c["unsupported"], 0);
        assert!(loaded.matches_social(&social));
        assert_eq!(
            c["caught"],
            if matches!(
                expected,
                Outcome::CaughtIntegerOverflow | Outcome::CaughtExistingLookupError
            ) {
                1
            } else {
                0
            }
        );
    }
    assert!(queries > 40);
}
#[test]
fn existing_resolution_requires_current_dependency_and_cannot_override_another_branch() {
    let p = policy(true, Some("7"));
    let social = initial_social::Loaded::load(31, 101, &p).unwrap();
    let mut loaded = Loaded::all_user(&social);
    assert!(loaded.resolve_existing_absence(false).is_err());
    assert!(matches!(
        loaded.initialize_newbie(false, &p, |_| None, || -> Result<_, ()> { panic!() }),
        Err(AdmissionError::Existing)
    ));
    loaded.resolve_existing_absence(false).unwrap();
    assert!(loaded.resolve_existing_absence(true).is_err());
    assert!(matches!(
        loaded.initialize_newbie(false, &p, |_| None, || -> Result<_, ()> { panic!() }),
        Err(AdmissionError::Existing)
    ));
    loaded
        .initialize_newbie(true, &p, |_| None, || -> Result<_, ()> { panic!() })
        .unwrap();
    assert!(loaded.resolve_existing_absence(false).is_err());
    assert_eq!(loaded.outcome(), Outcome::NoNewbie);
}
