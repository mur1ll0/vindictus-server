use serde_json::{Value, json};
use vindictus_server::{
    application::reward_inspection::inspect,
    domain::{
        dormant_rewards::plan_rewards, feature_policy::FeaturePolicy, legacy_user_data::SqlDateTime,
    },
    error::Error,
    infrastructure::reward_policy::{read_characters, read_policy},
};
const GOLD: &str = include_str!("fixtures/framework-reward-policy.json");
const POLICY: &str = include_str!("fixtures/synthetic-reward-policy.json");
fn feature_policy() -> FeaturePolicy {
    FeaturePolicy::new(
        [
            ("CurrentVer", Some("3.09.00")),
            ("a", Some("1.1")),
            ("b", Some("")),
            ("Case", Some("1")),
            ("single||", Some("1")),
            ("n", Some("9")),
            ("okint", Some(" +12 ")),
            ("invalidint", Some("x")),
            ("overflowint", Some("2147483648")),
        ]
        .map(|(k, v)| (k.to_owned(), v.map(str::to_owned))),
    )
    .unwrap()
}
#[test]
fn framework_expression_and_integer_decisions_match_both_architectures() {
    let gold: Value = serde_json::from_str(GOLD).unwrap();
    let policy = feature_policy();
    for profile in gold["profiles"].as_array().unwrap() {
        for case in profile["expressions"].as_array().unwrap() {
            let actual = policy.is_enabled(case["expression"].as_str());
            if case["error"].is_null() {
                assert_eq!(
                    actual.unwrap(),
                    case["enabled"].as_bool().unwrap(),
                    "{}",
                    case["expression"]
                );
            } else {
                assert!(matches!(
                    actual,
                    Err(Error::FeatureExpression) | Err(Error::FeatureVersionUnsupported)
                ));
            }
        }
        for case in profile["integers"].as_array().unwrap() {
            let integer_policy = FeaturePolicy::new([
                ("CurrentVer".into(), Some("3.09.00".into())),
                (
                    case["key"].as_str().unwrap().to_owned(),
                    case["input"].as_str().map(str::to_owned),
                ),
            ])
            .unwrap();
            let actual = integer_policy.get_integer(case["key"].as_str().unwrap());
            if case["error"].is_null() {
                assert_eq!(i64::from(actual.unwrap()), case["value"].as_i64().unwrap());
            } else {
                assert!(matches!(actual, Err(Error::FeatureIntegerOverflow)));
            }
        }
    }
}
#[test]
fn framework_text_version_comparisons_match_every_verified_culture() {
    let gold: Value = serde_json::from_str(GOLD).unwrap();
    for profile in gold["profiles"].as_array().unwrap() {
        for pair in profile["version_pairs"].as_array().unwrap() {
            let policy = FeaturePolicy::new([
                (
                    "CurrentVer".into(),
                    Some(pair["current"].as_str().unwrap().to_owned()),
                ),
                (
                    "threshold".into(),
                    Some(pair["value"].as_str().unwrap().to_owned()),
                ),
            ])
            .unwrap();
            assert_eq!(
                policy.is_enabled(Some("threshold")).unwrap(),
                pair["enabled"].as_bool().unwrap()
            );
        }
    }
    let policy = feature_policy();
    assert!(matches!(
        policy.is_enabled(Some("n")),
        Err(Error::FeatureVersionUnsupported)
    ));
    assert!(
        !FeaturePolicy::new([
            ("CurrentVer".into(), Some("3.09.00".into())),
            ("threshold".into(), Some("3.1".into()))
        ])
        .unwrap()
        .is_enabled(Some("threshold"))
        .unwrap()
    );
}
#[test]
fn sqlclient_datetime_callback_matches_framework_including_negative_days() {
    let gold: Value = serde_json::from_str(GOLD).unwrap();
    for profile in gold["profiles"].as_array().unwrap() {
        assert_eq!(profile["sqlclient_rows"], 908);
        for case in profile["dates"].as_array().unwrap() {
            let units = case["units"].as_str().unwrap().parse().unwrap();
            let expected: i64 = case["ticks"].as_str().unwrap().parse().unwrap();
            assert_eq!(
                SqlDateTime::from_units300(units)
                    .unwrap()
                    .framework_date_time()
                    .unwrap()
                    .ticks(),
                expected
            );
        }
    }
}
#[test]
fn full_reward_plan_matches_recovered_foreach_with_null_and_unpaired_utf16() {
    let gold: Value = serde_json::from_str(GOLD).unwrap();
    for profile in gold["profiles"].as_array().unwrap() {
        let package = read_policy(POLICY.as_bytes()).unwrap();
        let characters = read_characters(
            serde_json::to_vec(&profile["plan"]["characters"])
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        let characters: Vec<_> = characters.iter().map(|v| v.as_character()).collect();
        let inspection = inspect(&package, &characters).unwrap();
        assert_eq!(
            inspection.queued_calls as u64,
            profile["plan"]["count"].as_u64().unwrap()
        );
        assert_eq!(
            inspection.plan_sha256,
            profile["plan"]["digest"].as_str().unwrap()
        );
        let mut calls = 0;
        let plan = plan_rewards(&characters, &package.rewards, &package.policy, || {
            calls += 1;
            Ok(SqlDateTime::from_units300(calls).unwrap())
        })
        .unwrap();
        assert_eq!(calls as usize, plan.len());
        for (i, row) in plan.iter().enumerate() {
            assert_eq!(row.log_date.units300(), i as i64 + 1);
        }
        let mut reversed = characters;
        reversed.reverse();
        assert_ne!(
            inspection.plan_sha256,
            inspect(&package, &reversed).unwrap().plan_sha256
        );
    }
}
#[test]
fn private_readers_refuse_missing_nullable_columns_corrupt_hex_and_unbounded_input() {
    let original: Value = serde_json::from_str(POLICY).unwrap();
    for mutation in 0..7 {
        let mut value = original.clone();
        match mutation {
            0 => {
                value["features"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("value");
            }
            1 => value["rewards"][0]["item_utf16"] = json!("000"),
            2 => value["manifest"]["catalog_rows"] = json!(99),
            3 => value["manifest"]["region"] = json!("bad region"),
            4 => value["unexpected"] = json!(true),
            5 => value["rewards"][0]["num"] = json!(2147483648_i64),
            _ => value["manifest"]["database_sha256"] = json!("secret"),
        }
        assert!(read_policy(serde_json::to_vec(&value).unwrap().as_slice()).is_err());
    }
    assert!(read_policy(vec![b' '; 512 * 1024 + 1].as_slice()).is_err());
    assert!(read_characters(br#"[{"id":"1","level":1}]"#.as_slice()).is_err());
    assert!(read_characters(br#"[{"id":"01","level":1,"name_utf16":null}]"#.as_slice()).is_err());
    assert!(read_characters(br#"[{"id":"1","level":1,"name_utf16":"01"}]"#.as_slice()).is_err());
    assert!(
        read_characters(
            serde_json::to_vec(&vec![json!({"id":"1","level":1,"name_utf16":null}); 1001])
                .unwrap()
                .as_slice()
        )
        .is_err()
    );
}
#[test]
fn feature_case_overwrite_null_short_circuit_and_recursion_limits_are_explicit() {
    let policy = FeaturePolicy::new([
        ("CurrentVer".into(), Some("3.09.00".into())),
        ("a".into(), Some("1.1".into())),
        ("a".into(), None),
        ("A".into(), Some("1".into())),
    ])
    .unwrap();
    assert!(!policy.is_enabled(Some("a")).unwrap());
    assert!(policy.is_enabled(Some("A")).unwrap());
    assert!(policy.is_enabled(Some("A|| ")).unwrap());
    assert!(!policy.is_enabled(Some("a&& ")).unwrap());
    assert!(matches!(
        policy.is_enabled(Some(" \u{3000}")),
        Err(Error::FeatureExpression)
    ));
    assert!(matches!(
        policy.is_enabled(Some(&"!".repeat(34))),
        Err(Error::FeatureExpression)
    ));
}
#[test]
fn planner_preserves_duplicate_rows_signed_limits_and_failure_order() {
    let package = read_policy(POLICY.as_bytes()).unwrap();
    let characters =
        read_characters(br#"[{"id":"-1","level":20,"name_utf16":null}]"#.as_slice()).unwrap();
    let characters: Vec<_> = characters.iter().map(|v| v.as_character()).collect();
    let plan = plan_rewards(&characters, &package.rewards, &package.policy, || {
        Ok(SqlDateTime::from_units300(0).unwrap())
    })
    .unwrap();
    assert_eq!(plan.len(), 4);
    assert_eq!(plan[0].count, 2);
    assert_eq!(plan[1].count, -1);
    assert_eq!(plan[2].count, i32::MAX);
    assert_eq!(plan[3].count, 2);
    assert_eq!(
        plan[0].item_class_ex.as_bytes(),
        plan[3].item_class_ex.as_bytes()
    );
    assert!(matches!(
        plan_rewards(&characters, &package.rewards, &package.policy, || Err(
            Error::FeatureExpression
        )),
        Err(Error::FeatureExpression)
    ));
    let mut package = package;
    package.rewards[0].feature = Some(" ".into());
    package.rewards[0].min_level = 100;
    assert!(matches!(
        plan_rewards(&characters, &package.rewards, &package.policy, || Ok(
            SqlDateTime::from_units300(0).unwrap()
        )),
        Err(Error::FeatureExpression)
    ));
}
#[test]
fn planner_refuses_excessive_character_and_effect_counts() {
    let mut package = read_policy(POLICY.as_bytes()).unwrap();
    let single =
        read_characters(br#"[{"id":"1","level":20,"name_utf16":null}]"#.as_slice()).unwrap();
    let characters: Vec<_> = (0..1001).map(|_| single[0].as_character()).collect();
    assert!(matches!(
        plan_rewards(&characters, &package.rewards, &package.policy, || Ok(
            SqlDateTime::from_units300(0).unwrap()
        )),
        Err(Error::RewardPlanLimit)
    ));
    let base = package.rewards.remove(0);
    package.rewards = (0..10)
        .map(
            |_| vindictus_server::domain::dormant_rewards::DormantReward {
                item_class: vindictus_server::domain::legacy_user_data::LegacyBytes::from_bytes(
                    base.item_class.as_bytes().to_vec(),
                )
                .unwrap(),
                num: base.num,
                min_level: 0,
                max_level: 0,
                feature: None,
            },
        )
        .collect();
    assert!(matches!(
        plan_rewards(
            &characters[..1000],
            &package.rewards,
            &package.policy,
            || Ok(SqlDateTime::from_units300(0).unwrap())
        ),
        Err(Error::RewardPlanLimit)
    ));
}
