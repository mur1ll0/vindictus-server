use serde_json::Value;
use vindictus_server::domain::legacy_user_data::{LegacyUserDataError, SqlDateTime2};
#[test]
fn datetime_parameter_matches_framework_and_live_sqlclient() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-datetime-parameters.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        for case in profile["cases"].as_array().unwrap() {
            let ticks = case["ticks"].as_str().unwrap().parse().unwrap();
            let result = SqlDateTime2::from_ticks(ticks).unwrap().sql_parameter();
            if let Some(expected) = case["units300"].as_str() {
                assert_eq!(
                    result.unwrap().units300(),
                    expected.parse::<i64>().unwrap(),
                    "ticks={ticks}"
                );
            } else {
                assert_eq!(
                    result,
                    Err(LegacyUserDataError::DateTimeRange),
                    "ticks={ticks}"
                );
            }
        }
    }
}
