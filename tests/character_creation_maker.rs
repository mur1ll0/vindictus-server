use vindictus_server::{
    application::character_creation::name_result,
    domain::{character_creation::*, legacy_user_data::SqlDateTime2},
    infrastructure::character_name_check::NameCheckResult as N,
};
#[test]
fn maker_results_and_name_mapping_match_recovered_framework() {
    let f: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/framework-character-maker-results.json"
    ))
    .unwrap();
    let values = [
        CreateResult::Unknown,
        CreateResult::Success,
        CreateResult::InvalidName,
        CreateResult::DuplicatedName,
        CreateResult::ReservedName,
        CreateResult::ForbiddenName,
        CreateResult::ChangedCharacterName,
        CreateResult::CharacterLimit,
        CreateResult::WebConnectionError,
        CreateResult::CreateCharacterStopped,
        CreateResult::MakeEntityFailed,
        CreateResult::CustomizeOperationFailed,
        CreateResult::CostumeSaveFailed,
        CreateResult::InitCharacterFailed,
        CreateResult::OverLimitUsingPreset,
        CreateResult::PremiumSlotLimit,
    ];
    for profile in f["profiles"].as_array().unwrap() {
        assert_eq!(profile["values"].as_object().unwrap().len(), 16);
        for value in values {
            assert_eq!(profile["values"][format!("{value:?}")], value as i32);
        }
        for case in profile["cases"].as_array().unwrap() {
            let result = match case["input"].as_i64().unwrap() {
                1 => name_result(N::Success),
                2 => name_result(N::InvalidName),
                3 => name_result(N::DuplicatedName),
                4 => name_result(N::ReservedName),
                5 => name_result(N::ForbiddenName),
                6 => name_result(N::ChangedCharacterName),
                _ => CreateResult::Unknown,
            };
            assert_eq!(case["result"], result as i32);
        }
    }
}
#[test]
fn dormant_utc_minus_seven_days_keeps_measured_sqlclient_parameter_rounding() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-datetime-parameters.json")).unwrap();
    let mut checked = 0;
    for p in f["profiles"].as_array().unwrap() {
        for c in p["cases"].as_array().unwrap() {
            let ticks = c["ticks"].as_str().unwrap().parse::<i64>().unwrap();
            let Ok(utc) = SqlDateTime2::from_ticks(ticks + 7 * 864_000_000_000) else {
                continue;
            };
            let result = dormant_parameter(utc);
            if let Some(units) = c["units300"].as_str() {
                assert_eq!(result.unwrap().units300(), units.parse::<i64>().unwrap());
            } else {
                assert!(result.is_err())
            }
            checked += 1;
        }
    }
    assert!(checked > 100);
    assert!(dormant_parameter(SqlDateTime2::from_ticks(0).unwrap()).is_err());
}
