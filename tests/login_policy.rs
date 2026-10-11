use serde_json::Value;
use vindictus_server::{
    domain::{legacy_user_data::*, login_policy::*},
    error::Error,
};
fn bytes(value: &Value) -> Option<Vec<u8>> {
    value.as_str().map(|text| {
        text.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect()
    })
}
fn datetime2(value: &Value) -> Option<SqlDateTime2> {
    value
        .as_str()
        .map(|text| SqlDateTime2::from_ticks(text.parse().unwrap()).unwrap())
}
fn hex_equal(actual: Option<&[u8]>, expected: &Value) -> bool {
    actual == bytes(expected).as_deref()
}
#[test]
fn recovered_framework_ip_policy_matches_two_profiles_and_refuses_clock_overflow() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-login-policy.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["ip_cases"].as_array().unwrap().len(), 694);
        let mut intentional_refusals = 0;
        for (index, case) in profile["ip_cases"].as_array().unwrap().iter().enumerate() {
            let state = IpState {
                last_login_ip: bytes(&case["last_ip_hex"])
                    .map(|b| LegacyBytes::from_bytes(b).unwrap()),
                lock_until: datetime2(&case["lock_ticks"]),
            };
            let request = IpRequest {
                connection_ip: case["current"].as_str().unwrap().parse().unwrap(),
                secured: case["secured"].as_bool().unwrap(),
                has_cheat_permission: case["cheat"].as_bool().unwrap(),
                lock_minutes_feature: case["minutes"].as_i64().unwrap() as i32,
                clock: ApplicationClockSamples {
                    set_lock_now: datetime2(&case["set_now_ticks"]).unwrap(),
                    check_lock_now: datetime2(&case["check_now_ticks"]).unwrap(),
                },
            };
            let result = check_last_login_ip(state, &request);
            if case["caught_error"].as_bool().unwrap() {
                // Legacy swallows this error and returns zero. New adapter refuses the mutation.
                assert!(matches!(result, Err(Error::LegacyClockOverflow)));
                intentional_refusals += 1;
                continue;
            }
            let actual = result.unwrap();
            assert_eq!(
                actual.lock_seconds,
                case["seconds"].as_i64().unwrap() as i32,
                "case {index}"
            );
            assert!(
                hex_equal(
                    actual.previous_ip.as_ref().map(LegacyBytes::as_bytes),
                    &case["returned_ip_hex"]
                ),
                "case {index}"
            );
            assert!(
                hex_equal(
                    actual
                        .state
                        .last_login_ip
                        .as_ref()
                        .map(LegacyBytes::as_bytes),
                    &case["output_ip_hex"]
                ),
                "case {index}"
            );
            assert_eq!(
                actual.state.lock_until.map(SqlDateTime2::ticks),
                datetime2(&case["output_lock_ticks"]).map(SqlDateTime2::ticks),
                "case {index}"
            );
            let hidden = case["returned_ip_hex"].as_str() == Some("");
            assert_eq!(
                actual.client_lock_seconds,
                if hidden { 0 } else { actual.lock_seconds }
            );
        }
        assert_eq!(intentional_refusals, 1);
    }
}
#[test]
fn framework_ban_callback_matches_without_sharing_reasons_between_users() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-login-policy.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["ban_cases"].as_array().unwrap().len(), 50);
        for case in profile["ban_cases"].as_array().unwrap() {
            let ban = case["status"].as_i64().map(|status| UserBan {
                name: "BanFixture".into(),
                status: status as i32,
                expire_time: None,
                reason: bytes(&case["reason_hex"]).map(|b| LegacyBytes::from_bytes(b).unwrap()),
                source_rowversion: None,
            });
            let source = if case["query_failed"] == true {
                Err(())
            } else {
                Ok(ban.as_ref())
            };
            match member_state(source, case["notify"].as_bool().unwrap()) {
                MemberState::Unknown => assert!(case["member_state"].is_null()),
                MemberState::Clear => assert_eq!(case["member_state"], false),
                MemberState::Banned(reason) => {
                    assert_eq!(case["member_state"], true);
                    match reason {
                        BanReason::Empty => assert_eq!(case["output_reason_hex"], ""),
                        BanReason::AccountStolen => {
                            let key: Vec<_> = "GameUI_Login_Fail_BannedReason_AccountStolen"
                                .encode_utf16()
                                .flat_map(u16::to_le_bytes)
                                .collect();
                            assert!(hex_equal(Some(&key), &case["output_reason_hex"]));
                        }
                        BanReason::Source(bytes) => {
                            assert!(hex_equal(Some(bytes), &case["output_reason_hex"]))
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn recovered_sql_ban_predicate_matches_ascii_names_nulls_status_and_expiry_boundaries() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../docs/evidence/database/ban-policy-oracle.json"
    ))
    .unwrap();
    let now =
        SqlDateTime::from_units300(fixture["now_units300"].as_str().unwrap().parse().unwrap())
            .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 96);
    for case in cases {
        let ban = UserBan {
            name: case["name"].as_str().unwrap().into(),
            status: case["status"].as_i64().unwrap() as i32,
            expire_time: case["expiry_units300"]
                .as_str()
                .map(|s| SqlDateTime::from_units300(s.parse().unwrap()).unwrap()),
            reason: None,
            source_rowversion: None,
        };
        let selected = ban.name_key().unwrap() == "banfixture" && ban.selected_at(now);
        assert_eq!(selected, case["selected"].as_bool().unwrap());
        if selected && ban.status < 0 {
            assert!(matches!(
                member_state(Ok(Some(&ban)), true),
                MemberState::Clear
            ));
        }
    }
}
#[test]
fn update_last_login_ip_only_clears_existing_expired_locks_and_ban_keys_are_bounded() {
    let now = SqlDateTime2::from_ticks(10_000_000).unwrap();
    for lock in [None, Some(9_999_999), Some(10_000_000), Some(10_000_001)] {
        let state = IpState {
            last_login_ip: None,
            lock_until: lock.map(|t| SqlDateTime2::from_ticks(t).unwrap()),
        };
        let actual = update_expired_ip_lock(state, "127.0.0.1".parse().unwrap(), now).unwrap();
        if lock.is_some_and(|t| t <= now.ticks()) {
            assert!(actual.lock_until.is_none());
            assert!(actual.last_login_ip.is_some());
        } else {
            assert!(actual.last_login_ip.is_none());
            assert_eq!(actual.lock_until.map(SqlDateTime2::ticks), lock);
        }
    }
    for name in ["é".into(), "a".repeat(51), "bad\n".into()] {
        let ban = UserBan {
            name,
            status: 1,
            expire_time: None,
            reason: None,
            source_rowversion: None,
        };
        assert!(matches!(ban.name_key(), Err(Error::LegacyBanData)));
    }
}

#[test]
fn explicit_reference_vm_clock_matches_sql_rounding_including_negative_epoch_and_overflow() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../docs/evidence/database/ban-policy-oracle.json"
    ))
    .unwrap();
    let profile = FixedLocalClock::reference_vm();
    assert_eq!(fixture["clock_cases"].as_array().unwrap().len(), 10);
    for case in fixture["clock_cases"].as_array().unwrap() {
        let utc =
            SqlDateTime2::from_ticks(case["utc_ticks"].as_str().unwrap().parse().unwrap()).unwrap();
        assert_eq!(
            profile.application_local(utc).unwrap().ticks(),
            case["local_ticks"]
                .as_str()
                .unwrap()
                .parse::<i64>()
                .unwrap()
        );
        assert_eq!(
            profile.sql_local(utc).unwrap().units300(),
            case["sql_units300"]
                .as_str()
                .unwrap()
                .parse::<i64>()
                .unwrap()
        );
    }
    assert!(matches!(
        profile.application_local(SqlDateTime2::from_ticks(SqlDateTime2::MAX).unwrap()),
        Err(Error::LegacyClockOverflow)
    ));
    assert!(matches!(
        profile.sql_local(SqlDateTime2::from_ticks(0).unwrap()),
        Err(Error::LegacyClockOverflow)
    ));
    assert!(FixedLocalClock::from_offset_minutes(841).is_err());
    assert!(FixedLocalClock::from_offset_minutes(-841).is_err());
    let neutral = FixedLocalClock::from_offset_minutes(0).unwrap();
    assert_eq!(
        neutral
            .application_local(SqlDateTime2::from_ticks(0).unwrap())
            .unwrap()
            .ticks(),
        0
    );
    let min = FixedLocalClock::from_offset_minutes(-840).unwrap();
    assert!(matches!(
        min.application_local(SqlDateTime2::from_ticks(0).unwrap()),
        Err(Error::LegacyClockOverflow)
    ));
}
