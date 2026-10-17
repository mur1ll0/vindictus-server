use serde_json::{Value, json};
use vindictus_server::domain::{feature_policy::FeaturePolicy, legacy_user_data::SqlDateTime2};
use vindictus_server::infrastructure::fiona_initializer_profile::Profile;

mod fixture {
    include!("fixtures/initializer_profile_support.rs");
}
use fixture::{read, synthetic};
#[test]
fn private_profile_preserves_source_projection_and_guards_active_attendance() {
    let p = read(&synthetic()).unwrap();
    assert_eq!(p.source_state(), &[0xcc; 32]);
    assert_eq!(p.level_up_exp(), 81);
    // SyncTitlePreset retains existing class restrictions instead of overwriting from content.
    assert_eq!(p.preset_ids(), [17]);
    assert_eq!(p.skills().len(), 1);
    let s = &p.skills()[0];
    assert_eq!(s.key(), [83, 0, 121, 0, 110, 0]);
    assert_eq!(s.rank(), 2);
    assert_eq!(s.required_ap(), 9);
    assert!(s.item_callback());
    assert!(!s.stat_callback());
    for now in [99, 201] {
        assert!(
            p.validate_attendance_window(SqlDateTime2::from_ticks(now).unwrap())
                .is_ok()
        );
    }
    for now in [100, 150, 200] {
        assert!(
            p.validate_attendance_window(SqlDateTime2::from_ticks(now).unwrap())
                .is_err()
        );
    }
    let mut v = synthetic();
    v["levels"] = json!([]);
    v["skills"][0]["next_rank"] = Value::Null;
    let p = read(&v).unwrap();
    assert_eq!(p.level_up_exp(), 0);
    assert_eq!(p.skills()[0].required_ap(), -1);
}
#[test]
fn private_profile_rejects_changed_evidence_and_unsupported_effects() {
    let v = synthetic();
    let (mut input, evidence) = fixture::encoded(&v);
    input.push(b' ');
    assert!(Profile::read(input.as_slice(), evidence.as_slice()).is_err());
    for field in [
        "profile_sha256",
        "database_sha256",
        "config_sha256",
        "feature_matrix_sha256",
    ] {
        let (a, b) = fixture::encoded(&v);
        let mut e: Value = serde_json::from_slice(&b).unwrap();
        e[field] = json!("0".repeat(64));
        let b = serde_json::to_vec(&e).unwrap();
        assert!(
            Profile::read(a.as_slice(), b.as_slice()).is_err(),
            "{field}"
        );
    }
    let mut cases = Vec::new();
    let mut x = v.clone();
    x["implementations"]["stats"] = json!("0".repeat(64));
    cases.push(x);
    let mut x = v.clone();
    x["skills"][0]["constraints"] = json!([{"id_utf16":"4100","rank":1}]);
    cases.push(x);
    let mut x = v.clone();
    x["skills"][0]["status_effect_types"] = json!(["synthetic-effect", null, null]);
    cases.push(x);
    let mut x = v.clone();
    x["skills"][0]["next_rank"]["require_unlock"] = json!(true);
    cases.push(x);
    let mut x = v.clone();
    x["skills"][0]["meditation"] = json!(true);
    cases.push(x);
    let mut x = v.clone();
    x["skills"][0]["rank"] = json!(i32::MAX);
    cases.push(x);
    let mut x = v.clone();
    x["presets"] = json!([]);
    cases.push(x);
    let mut x = v.clone();
    x["skills"][0]
        .as_object_mut()
        .unwrap()
        .remove("rank_feature");
    cases.push(x);
    let mut x = v.clone();
    x["features"][0]["value"] = json!("unmeasured");
    cases.push(x);
    let mut x = v.clone();
    x["unexpected"] = json!(true);
    cases.push(x);
    for (i, x) in cases.iter().enumerate() {
        assert!(read(x).is_err(), "case {i}");
    }
}
#[test]
fn recorded_comparisons_are_bounded_conflict_checked_and_instance_local() {
    let rows = || {
        vec![
            ("CurrentVer".into(), Some("synthetic-current".into())),
            ("Flag".into(), Some("synthetic-on".into())),
        ]
    };
    assert!(
        FeaturePolicy::new(rows())
            .unwrap()
            .is_enabled(Some("Flag"))
            .is_err()
    );
    let record = || {
        (
            "synthetic-current".to_owned(),
            "synthetic-on".to_owned(),
            true,
        )
    };
    assert!(
        FeaturePolicy::with_recorded_comparisons(rows(), [record()])
            .unwrap()
            .is_enabled(Some("Flag"))
            .unwrap()
    );
    assert!(
        FeaturePolicy::new(rows())
            .unwrap()
            .is_enabled(Some("Flag"))
            .is_err()
    );
    assert!(
        FeaturePolicy::with_recorded_comparisons(
            rows(),
            [
                record(),
                ("synthetic-current".into(), "synthetic-on".into(), false)
            ]
        )
        .is_err()
    );
    assert!(
        FeaturePolicy::with_recorded_comparisons(
            rows(),
            [("other-current".into(), "synthetic-on".into(), true)]
        )
        .is_err()
    );
    assert!(
        FeaturePolicy::with_recorded_comparisons(rows(), std::iter::repeat_with(record).take(129))
            .is_err()
    );
}

#[test]
fn initializer_arguments_are_required_bounded_and_preserved_for_runtime_provider_checks() {
    use vindictus_server::domain::skill_runtime::{Cache, CacheError};
    let p = read(&synthetic()).unwrap();
    let skill = &p.skills()[0];
    let mut cache = Cache::default();
    cache
        .after_grant(
            skill.key(),
            skill.rank(),
            skill.required_ap(),
            skill.arguments(),
        )
        .unwrap();
    assert_eq!(cache.argument(skill.key(), &[65, 0], 0), 7);
    for mode in 0..4 {
        let mut v = synthetic();
        match mode {
            0 => {
                v["skills"][0].as_object_mut().unwrap().remove("arguments");
            }
            1 => v["skills"][0]["argument_rows"] = json!(3),
            2 => v["skills"][0]["arguments"][0]["key_utf16"] = json!("0"),
            _ => v["skills"][0]["arguments"][0]["key_utf16"] = json!("00".repeat(1026)),
        };
        assert!(read(&v).is_err());
    }
    let mut v = synthetic();
    v["skills"][0]["arguments"][0]["value"] = json!(null);
    let p = read(&v).unwrap();
    let skill = &p.skills()[0];
    assert_eq!(
        cache.after_grant(
            skill.key(),
            skill.rank(),
            skill.required_ap(),
            skill.arguments()
        ),
        Err(CacheError::Null)
    );
}

#[test]
fn maximum_ap_policy_requires_exported_key_and_keeps_source_parsing_lazy() {
    let mut v = synthetic();
    assert_eq!(read(&v).unwrap().maximum_ap().unwrap(), -1);
    for field in v["features"].as_array_mut().unwrap() {
        if field["key"] == "APMax" {
            field["value"] = json!("2147483648");
        }
    }
    let p = read(&v).unwrap();
    assert!(p.maximum_ap().is_err());
    v["features"]
        .as_array_mut()
        .unwrap()
        .retain(|f| f["key"] != "APMax");
    let p = read(&v).unwrap();
    assert!(p.maximum_ap().is_err());
}
