use serde_json::Value;
use vindictus_server::domain::character_name::{EN_EU_NAMING_RULE, NameRule, NameRuleError};

#[test]
fn recovered_en_eu_name_rule_matches_framework_nullable_utf16_length_and_repetition() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-character-name.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["pattern"], EN_EU_NAMING_RULE);
        assert_eq!(profile["cases"].as_array().unwrap().len(), 1050);
        for case in profile["cases"].as_array().unwrap() {
            let name = case["name"]
                .as_str()
                .map(|s| s.encode_utf16().collect::<Vec<_>>());
            let rule = NameRule::recovered(
                profile["pattern"].as_str().unwrap(),
                case["max_bytes"].as_i64().unwrap() as i32,
                case["repeating"].as_i64().unwrap() as i32,
            )
            .unwrap();
            let actual = rule.check_valid_name(name.as_deref());
            if case["error"] == "ArgumentNullException" {
                assert_eq!(actual, Err(NameRuleError::NullName));
            } else {
                assert!(case["error"].is_null());
                assert_eq!(actual, Ok(case["result"].as_bool().unwrap()));
            }
        }
    }
}
#[test]
fn name_rule_refuses_unrecovered_pattern_and_does_not_trim_or_repair_input() {
    assert!(matches!(
        NameRule::recovered(".*", 20, -1),
        Err(NameRuleError::UnsupportedRule)
    ));
    let rule = NameRule::recovered(EN_EU_NAMING_RULE, 20, -1).unwrap();
    for raw in [
        vec![65, 66, 67, 68, 32],
        vec![65, 66, 67, 68, 13, 10],
        vec![65, 66, 0xd800, 67, 68],
        vec![65, 66, 0xdc00, 67, 68],
    ] {
        assert_eq!(rule.check_valid_name(Some(&raw)), Ok(false));
    }
    assert_eq!(rule.check_valid_name(Some(&[65, 66, 67, 68, 10])), Ok(true));
    assert_eq!(
        NameRule::recovered(EN_EU_NAMING_RULE, 4, -1)
            .unwrap()
            .check_valid_name(Some(&[65, 66, 67, 68, 10])),
        Ok(false)
    );
}
