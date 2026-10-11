use serde_json::Value;
use vindictus_server::domain::{
    legacy_user_data::{SqlDateTime, SqlDateTime2},
    login_effects::*,
};
fn tick(value: &Value) -> SqlDateTime2 {
    SqlDateTime2::from_ticks(value.as_str().unwrap().parse().unwrap()).unwrap()
}
fn optional_tick(value: &Value) -> Option<SqlDateTime2> {
    (!value.is_null()).then(|| tick(value))
}
fn actions(plan: &LoginEffectPlan) -> Vec<String> {
    plan.actions()
        .iter()
        .map(|effect| match effect {
            LoginEffect::SetFriendRecommendFlag(flag) => format!("friend:{flag}"),
            LoginEffect::InsertNewUser => "create_user".into(),
            LoginEffect::ReplaceAchievementOwner => "achievement".into(),
            LoginEffect::ReleaseDormant => "release".into(),
            LoginEffect::SetDormantUser(date) => format!("dormant:{}", date.ticks()),
            LoginEffect::SendDormantRewards => "rewards".into(),
            LoginEffect::UpdateDormantEvent(date) => format!(
                "event:{}",
                date.map_or_else(|| "null".into(), |date| date.ticks().to_string())
            ),
            LoginEffect::ReadDormantEvent => "read_event".into(),
        })
        .collect()
}
#[test]
fn recovered_framework_effect_order_and_flags_match_both_profiles() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-login-effects.json")).unwrap();
    assert_eq!(fixture["profiles"].as_array().unwrap().len(), 2);
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["cases"].as_array().unwrap().len(), 330);
        for case in profile["cases"].as_array().unwrap() {
            let input = &case["input"];
            let bool_field = |key: &str| input[key].as_bool().unwrap();
            let integer = |key: &str| input[key].as_i64().unwrap() as i32;
            let plan = plan_login_effects(
                &EffectInputs {
                    new_user_required: bool_field("new_user"),
                    friend_recommend_flag: input["flag"].as_i64().map(|flag| flag as i32),
                    latest_for_loyalty: optional_tick(&input["latest_loyalty"]),
                    dormant_since: optional_tick(&input["since"]),
                    latest_for_dormancy: optional_tick(&input["latest_dormant"]),
                },
                EffectFeatures {
                    friend_loyalty: bool_field("friend_loyalty"),
                    friend_return_days: integer("friend_days"),
                    dormant_account: bool_field("dormant"),
                    dormant_new_account_days: integer("new_days"),
                    dormant_account_days: integer("dormant_days"),
                    dormant_account_v2: bool_field("v2"),
                },
                EffectClockSamples {
                    loyalty_now: tick(&input["loyalty_now"]),
                    dormant_elapsed_now: tick(&input["dormant_now"]),
                    latest_elapsed_now: tick(&input["latest_now"]),
                    set_dormant_now: tick(&input["set_now"]),
                    set_event_now: tick(&input["event_now"]),
                },
            );
            let expected: Vec<_> = case["output"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|action| action.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(actions(&plan), expected, "{}", input["label"]);
            assert!(plan.actions().len() <= 8);
            let events: Vec<_> = input["events"].as_array().map_or_else(Vec::new, |rows| {
                rows.iter()
                    .map(|date| (!date.is_null()).then(|| SqlDateTime::from_units300(0).unwrap()))
                    .collect()
            });
            let flags = plan.complete(Some(&events)).unwrap();
            assert_eq!(
                flags.is_dormant,
                case["output"]["is_dormant"].as_bool().unwrap(),
                "{}",
                input["label"]
            );
            assert_eq!(
                flags.is_return,
                case["output"]["is_return"].as_bool().unwrap(),
                "{}",
                input["label"]
            );
        }
    }
}
#[test]
fn recovered_reward_predicate_matches_signed_unbounded_and_disabled_cases() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-login-effects.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["rewards"].as_array().unwrap().len(), 686);
        for case in profile["rewards"].as_array().unwrap() {
            let integer = |key: &str| case[key].as_i64().unwrap() as i32;
            assert_eq!(
                dormant_reward_matches(
                    integer("level"),
                    integer("min"),
                    integer("max"),
                    case["enabled"].as_bool().unwrap()
                ),
                case["matched"].as_bool().unwrap()
            );
        }
    }
}
#[test]
fn latest_connected_date_matches_sql_null_case_and_max_semantics() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../docs/evidence/database/login-effects-oracle.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 9);
    let date = |value: &Value| {
        value
            .as_str()
            .map(|text| SqlDateTime::from_units300(text.parse().unwrap()).unwrap())
    };
    for case in fixture["cases"].as_array().unwrap() {
        // SQL JSON_QUERY emits NULL for the empty rowset case; it still exercises MAX(empty).
        if case["rows"].is_null() {
            assert_eq!(case["case_id"].as_i64(), Some(0));
        }
        let rows: Vec<_> = case["rows"].as_array().map_or_else(Vec::new, |rows| {
            rows.iter()
                .map(|row| LatestConnection {
                    last_connect: date(&row["connect_units300"]),
                    last_cafe_logout: date(&row["cafe_units300"]),
                })
                .collect()
        });
        assert_eq!(
            latest_connected_date(&rows),
            date(&case["latest_units300"]),
            "{}",
            case["case_id"]
        );
    }
}
fn sample_features() -> EffectFeatures {
    EffectFeatures {
        friend_loyalty: false,
        friend_return_days: 30,
        dormant_account: true,
        dormant_new_account_days: 1,
        dormant_account_days: 30,
        dormant_account_v2: true,
    }
}
fn clocks(now: SqlDateTime2) -> EffectClockSamples {
    EffectClockSamples {
        loyalty_now: now,
        dormant_elapsed_now: now,
        latest_elapsed_now: now,
        set_dormant_now: now,
        set_event_now: now,
    }
}
#[test]
fn release_dormant_keeps_pre_release_elapsed_time_for_requalification() {
    const DAY: i64 = 864_000_000_000;
    let now = SqlDateTime2::from_ticks(100 * DAY).unwrap();
    let plan = plan_login_effects(
        &EffectInputs {
            new_user_required: false,
            friend_recommend_flag: None,
            latest_for_loyalty: None,
            dormant_since: Some(SqlDateTime2::from_ticks(now.ticks() - 2 * DAY).unwrap()),
            latest_for_dormancy: Some(SqlDateTime2::from_ticks(now.ticks() - 31 * DAY).unwrap()),
        },
        sample_features(),
        clocks(now),
    );
    assert_eq!(
        plan.actions(),
        &[LoginEffect::ReleaseDormant, LoginEffect::ReadDormantEvent]
    );
    assert_eq!(
        plan.complete(Some(&[])).unwrap(),
        EffectFlags {
            is_dormant: false,
            is_return: false
        }
    );
}
#[test]
fn v2_completion_requires_actual_post_write_event_read_even_after_setting_event() {
    const DAY: i64 = 864_000_000_000;
    let now = SqlDateTime2::from_ticks(100 * DAY).unwrap();
    let plan = plan_login_effects(
        &EffectInputs {
            new_user_required: false,
            friend_recommend_flag: None,
            latest_for_loyalty: None,
            dormant_since: None,
            latest_for_dormancy: Some(SqlDateTime2::from_ticks(now.ticks() - 31 * DAY).unwrap()),
        },
        sample_features(),
        clocks(now),
    );
    assert_eq!(
        plan.complete(None),
        Err(EffectCompletionError::MissingEventRead)
    );
    // Legacy set/update already makes IsReturn true even if the subsequent query is empty.
    assert_eq!(
        plan.complete(Some(&[])).unwrap(),
        EffectFlags {
            is_dormant: true,
            is_return: true
        }
    );
}
