use serde_json::{Value, json};
use vindictus_server::{
    domain::{feature_policy::FeaturePolicy, initial_social::Loaded},
    error::Error,
};
fn policy(enabled: bool) -> FeaturePolicy {
    FeaturePolicy::with_recorded_comparisons(
        [
            ("CurrentVer".into(), Some("social-current".into())),
            ("InGameGuild".into(), Some("social-flag".into())),
        ],
        [("social-current".into(), "social-flag".into(), enabled)],
    )
    .unwrap()
}
fn compare(arch: &str) {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-social.json")).unwrap();
    let cases = fixture[arch].as_array().unwrap();
    assert_eq!(cases.len(), 32);
    for case in cases {
        assert_eq!(case["feature_queries"], 1);
        let enabled = case["enabled"].as_bool().unwrap();
        let out = Loaded::load(
            i32::try_from(case["uid"].as_i64().unwrap()).unwrap(),
            case["cid"].as_i64().unwrap(),
            &policy(enabled),
        );
        if !enabled {
            assert!(!case["supported"].as_bool().unwrap());
            assert_eq!(case["external_branch_calls"], 1);
            assert!(matches!(out, Err(Error::FionaInitializerProfile)));
            continue;
        }
        assert!(case["supported"].as_bool().unwrap());
        assert_eq!(case["external_branch_calls"], 0);
        let out = out.unwrap();
        let actual = json!({"whisper_uid":out.whisper().uid(),"whisper_cid":out.whisper().cid(),"guild_uid":out.guild().uid(),"guild_cid":out.guild().cid(),"same_player":out.shares_player_context(),"same_character":out.whisper().shares_character_player_context(),"guild_info_loaded":out.guild().info_loaded(),"pending":out.guild().pending_procedure_count()});
        assert_eq!(actual, case["expected"]);
    }
}
#[test]
fn social_matches_framework_x86() {
    compare("x86");
}
#[test]
fn social_matches_framework_x64() {
    compare("x64");
}
#[test]
fn social_refuses_missing_or_unknown_feature_evidence() {
    let missing =
        FeaturePolicy::new([("CurrentVer".into(), Some("social-current".into()))]).unwrap();
    assert!(matches!(
        Loaded::load(1, 2, &missing),
        Err(Error::FionaInitializerProfile)
    ));
    let unknown = FeaturePolicy::new([
        ("CurrentVer".into(), Some("social-current".into())),
        ("InGameGuild".into(), Some("not-recorded".into())),
    ])
    .unwrap();
    assert!(matches!(
        Loaded::load(1, 2, &unknown),
        Err(Error::FeatureVersionUnsupported)
    ));
}
