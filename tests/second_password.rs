use serde_json::Value;
use vindictus_server::domain::{legacy_user_data::SqlDateTime2, second_password::*};
use vindictus_server::protocol::identity::{AsciiCulture, FrameworkArchitecture, IdentityProfile};
fn identity(p: &Value) -> IdentityProfile {
    IdentityProfile {
        architecture: if p["architecture"] == "x86" {
            FrameworkArchitecture::X86
        } else {
            FrameworkArchitecture::X64
        },
        culture: AsciiCulture::Invariant,
    }
}
fn profile(mode: i32, p: &Value) -> HashProfile {
    if mode == 0 {
        HashProfile::Legacy(identity(p))
    } else {
        HashProfile::try_from(mode).unwrap()
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-second-password.json")).unwrap()
}
fn units(v: &Value) -> Vec<u16> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u16)
        .collect()
}
fn bytes(v: &Value) -> Option<Vec<u8>> {
    v.as_array()
        .map(|_| units(v).into_iter().flat_map(u16::to_le_bytes).collect())
}
fn ticks(v: &Value) -> Option<SqlDateTime2> {
    v.as_i64().map(|v| SqlDateTime2::from_ticks(v).unwrap())
}
fn clock(now: i64) -> ClockSamples {
    let now = SqlDateTime2::from_ticks(now).unwrap();
    ClockSamples {
        operation_now: now,
        retry_compare_now: now,
        retry_remaining_now: now,
    }
}
#[test]
fn sha_profiles_ascii_fallback_and_version_lengths_match_framework() {
    let f = fixture();
    for p in f["profiles"].as_array().unwrap() {
        for h in p["hashes"].as_array().unwrap() {
            let encoded = hash(
                h["name"].as_str().unwrap(),
                &units(&h["password"]),
                profile(h["mode"].as_i64().unwrap() as i32, p),
            )
            .unwrap();
            assert_eq!(*encoded, bytes(&h["encoded"]).unwrap());
        }
        for v in p["versions"].as_array().unwrap() {
            assert_eq!(
                stored_version(v["length"].as_u64().unwrap() as usize),
                v["version"].as_i64().unwrap() as i32
            );
        }
    }
}
#[test]
fn first_query_and_retry_projection_match_recovered_user_entity() {
    let f = fixture();
    for p in f["profiles"].as_array().unwrap() {
        let c = clock(p["now"].as_i64().unwrap());
        for q in p["queries"].as_array().unwrap() {
            let stored = bytes(&q["stored"]);
            let result = query(
                stored.as_deref(),
                q["count"].as_i64().map(|v| v as i32),
                ticks(&q["last"]),
                c,
            )
            .unwrap();
            assert_eq!(
                result,
                QueryResult {
                    restricted_until: ticks(&q["until"]),
                    first: q["first"].as_bool().unwrap(),
                    has_password: q["has"].as_bool().unwrap(),
                    fail_count: q["fail"].as_i64().unwrap() as i32,
                    retry_seconds: q["retry"].as_i64().unwrap() as i32
                }
            );
        }
    }
}
#[test]
fn check_matching_rehash_and_counter_plan_match_framework() {
    let f = fixture();
    for p in f["profiles"].as_array().unwrap() {
        for q in p["checks"].as_array().unwrap() {
            let pw: Vec<u16> = if q["correct"].as_bool().unwrap() {
                "123456"
            } else {
                "654321"
            }
            .encode_utf16()
            .collect();
            let stored = bytes(&q["stored"]).unwrap();
            let matched = matches("MiXeDLab", &pw, &stored, identity(p)).unwrap();
            assert_eq!(matched, q["passed"].as_bool().unwrap());
            let count = checked_count(q["count"].as_i64().map(|v| v as i32), matched).unwrap();
            assert_eq!(count, q["fail"].as_i64().unwrap() as i32);
            if matched {
                let encoded = hash(
                    "MiXeDLab",
                    &pw,
                    profile(q["mode"].as_i64().unwrap() as i32, p),
                )
                .unwrap();
                assert_eq!(*encoded, bytes(&q["after"]["password"]).unwrap());
            }
        }
    }
}
#[test]
fn retry_boundary_state_gates_and_unsupported_inputs_fail_closed() {
    let c = clock(638000000000000000);
    let until = retry_until(5, c.operation_now).unwrap().unwrap();
    let mut at = clock(until.ticks());
    assert_eq!(retry_seconds(Some(until), at).unwrap(), -1);
    at.retry_compare_now = SqlDateTime2::from_ticks(until.ticks() - 1).unwrap();
    assert_eq!(retry_seconds(Some(until), at).unwrap(), 0);
    at.retry_remaining_now = SqlDateTime2::from_ticks(until.ticks() + 15000000).unwrap();
    assert_eq!(retry_seconds(Some(until), at).unwrap(), -1);
    assert!(matches!(
        checked_count(Some(-1), false),
        Err(SecondPasswordError::Counter)
    ));
    assert!(matches!(
        checked_count(Some(i32::MAX), false),
        Err(SecondPasswordError::Counter)
    ));
    assert!(matches!(
        HashProfile::try_from(0),
        Err(SecondPasswordError::HashProfile)
    ));
    assert!(matches!(
        matches(
            "lab",
            &[],
            &[0, 0, 0, 0],
            identity(&serde_json::json!({"architecture":"x86"}))
        ),
        Err(SecondPasswordError::Input)
    ));
    assert!(matches!(
        hash("lab", &[1; 257], HashProfile::Sha512),
        Err(SecondPasswordError::Input)
    ));
    assert!(hash("λ", &[], HashProfile::Sha512).is_err());
    for state in [
        ClientState::Unknown,
        ClientState::NoPassword,
        ClientState::Locked,
        ClientState::Passed,
    ] {
        assert!(state.passed(false));
        assert_eq!(
            state.essential_passed(true, true),
            state == ClientState::Passed
        );
        assert_eq!(
            state.passed(true),
            matches!(state, ClientState::NoPassword | ClientState::Passed)
        );
    }
}

#[test]
fn registration_encoding_and_reset_plan_match_framework() {
    let f = fixture();
    for p in f["profiles"].as_array().unwrap() {
        for r in p["registers"].as_array().unwrap() {
            let password = units(&r["password"]);
            let encoded = if password.is_empty() {
                Vec::new()
            } else {
                hash(
                    "MiXeDLab",
                    &password,
                    profile(r["mode"].as_i64().unwrap() as i32, p),
                )
                .unwrap()
                .to_vec()
            };
            assert_eq!(encoded, bytes(&r["after"]["password"]).unwrap());
            assert_eq!(r["after"]["count"], 0);
            assert!(r["after"]["last"].is_null());
            assert_eq!(r["passed"], true);
            assert_eq!(r["retry"], -1);
        }
    }
}
#[test]
fn nullable_checks_and_caught_failures_match_original_user_entity_on_both_framework_profiles() {
    let f = fixture();
    for p in f["profiles"].as_array().unwrap() {
        assert_eq!(p["nullableChecks"].as_array().unwrap().len(), 18);
        for r in p["nullableChecks"].as_array().unwrap() {
            let stored = bytes(&r["stored"]).unwrap();
            let decision = compare_nullable(
                "MiXeDLab",
                None,
                &stored,
                profile(r["mode"].as_i64().unwrap() as i32, p),
                identity(p),
            );
            let (passed, count, encoded) = match decision {
                Ok(CheckComparison {
                    matched,
                    replacement: new,
                }) => (
                    matched,
                    checked_count(Some(4), matched).unwrap(),
                    new.map(|v| v.to_vec()).unwrap_or(stored),
                ),
                Err(_) => (false, -1, stored),
            };
            assert_eq!(passed, r["passed"].as_bool().unwrap());
            assert_eq!(count, r["fail"].as_i64().unwrap() as i32);
            assert_eq!(encoded, bytes(&r["after"]["password"]).unwrap());
            let until = retry_until(
                count,
                clock(f["profiles"][0]["now"].as_i64().unwrap()).operation_now,
            )
            .unwrap();
            assert_eq!(until, ticks(&r["until"]));
            assert_eq!(
                retry_seconds(until, clock(p["now"].as_i64().unwrap())).unwrap(),
                r["retry"].as_i64().unwrap() as i32
            );
        }
        for r in p["nullable"].as_array().unwrap() {
            let operation = r["operation"].as_str().unwrap();
            if operation == "null-set" || operation == "read-check" || operation == "read-set" {
                assert!(!r["passed"].as_bool().unwrap());
                assert_eq!(r["fail"], -1);
                assert_eq!(r["retry"], 600);
                assert!(r["unchanged"].as_bool().unwrap());
            }
            if operation == "read-query" {
                assert!(!r["first"].as_bool().unwrap());
                assert!(r["has"].as_bool().unwrap());
                assert_eq!(r["fail"], 0);
                assert_eq!(r["retry"], -1);
                assert!(r["unchanged"].as_bool().unwrap());
            }
        }
    }
}
