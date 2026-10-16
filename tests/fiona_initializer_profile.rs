use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vindictus_server::{
    domain::{feature_policy::FeaturePolicy, legacy_user_data::SqlDateTime2},
    infrastructure::fiona_initializer_profile::Profile,
};

// All values are synthetic. The manifest contains original code paths/digests only.
fn synthetic() -> Value {
    let code: Value =
        serde_json::from_str(include_str!("../src/domain/fiona_initializer_sources.json")).unwrap();
    let implementations = code
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| *k != "feature_matrix")
        .map(|(k, v)| (k.clone(), v["sha256"].clone()))
        .collect::<serde_json::Map<_, _>>();
    let flags = [
        ("AutoItemLogging", true),
        ("ProcessAfterDeleteItem", false),
        ("InGameGuild", true),
        ("GuildScore", false),
        ("RandomMission", false),
        ("AddTitleFromPreset", true),
        ("DisableCharacterCreation", false),
        ("ServerTrouble", false),
        ("DisableLogging", false),
        ("Attendance_Newbie", true),
    ];
    let mut features = flags
        .iter()
        .map(|(k, b)| json!({"key":k,"value":if *b {"synthetic-on"}else{"synthetic-off"}}))
        .collect::<Vec<_>>();
    features.extend([
        json!({"key":"CurrentVer","value":"synthetic-current"}),
        json!({"key":"AttendanceEvent_Newbie","value":"7"}),
        json!({"key":"Event_newbie_character_preset_item","value":null}),
    ]);
    json!({"format_version":1,"source":"heroesContents.first-fiona-initializer","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"state_sha256":"c".repeat(64),"implementations":implementations,"features":features,"enabled_keys":flags.iter().map(|(k,_)|*k).collect::<Vec<_>>(),"base_stats":[1,2,3,4,5,6],"levels":[{"level":1,"required_exp":81,"feature":null}],"presets":[{"title_id":17,"class_restriction":2},{"title_id":18,"class_restriction":0}],"title_info":[{"title_id":17,"class_restriction":99,"auto_give_level":1,"feature":null},{"title_id":18,"class_restriction":2,"auto_give_level":1,"feature":null}],"skills":[{"id_utf16":"530079006e00","rank":2,"class_restriction":2,"use_type":0,"level_constraint":1,"rank_feature":null,"constraints":[],"next_rank":{"required_ap":9,"require_unlock":false},"status_effect_types":[null,null,null],"item_callback":true,"stat_callback":false,"meditation":false,"argument_rows":2,"argument_sha256":"d".repeat(64)}],"attendance":[{"version":7,"date_from_ticks":"100","date_until_ticks":"200"}]})
}
fn encoded(profile: &Value) -> (Vec<u8>, Vec<u8>) {
    let input = serde_json::to_vec(profile).unwrap();
    let code: Value =
        serde_json::from_str(include_str!("../src/domain/fiona_initializer_sources.json")).unwrap();
    let evidence = json!({"format_version":1,"source":"Framework.FeatureMatrix.IsEnable","profile_sha256":format!("{:x}",Sha256::digest(&input)),"database_sha256":profile["database_sha256"],"config_sha256":profile["config_sha256"],"feature_matrix_sha256":code["feature_matrix"]["sha256"],"profiles":["Framework48-x86-en-US","Framework48-x64-en-US"],"pairs":[{"current":"synthetic-current","value":"synthetic-on","enabled":true},{"current":"synthetic-current","value":"synthetic-off","enabled":false}]});
    (input, serde_json::to_vec(&evidence).unwrap())
}
fn read(v: &Value) -> Result<Profile, vindictus_server::error::Error> {
    let (a, b) = encoded(v);
    Profile::read(a.as_slice(), b.as_slice())
}
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
    let (mut input, evidence) = encoded(&v);
    input.push(b' ');
    assert!(Profile::read(input.as_slice(), evidence.as_slice()).is_err());
    for field in [
        "profile_sha256",
        "database_sha256",
        "config_sha256",
        "feature_matrix_sha256",
    ] {
        let (a, b) = encoded(&v);
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
