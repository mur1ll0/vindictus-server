use serde_json::Value;
use vindictus_server::domain::{
    login_effects::EffectFlags, user_login_composition::*, user_login_return::UserLoginReturn,
};
fn units(v: &Value) -> Option<Vec<u16>> {
    v.as_array()
        .map(|a| a.iter().map(|v| v.as_u64().unwrap() as u16).collect())
}
#[test]
fn final_feedback_matches_recovered_framework_projection() {
    let f: Value = serde_json::from_str(include_str!(
        "fixtures/framework-user-login-composition.json"
    ))
    .unwrap();
    for profile in f["profiles"].as_array().unwrap() {
        let mut success = 0;
        for case in profile["cases"].as_array().unwrap() {
            let i = &case["input"];
            if i["member"] != false {
                continue;
            }
            let ip = units(&i["ip"])
                .map(|v| v.into_iter().flat_map(u16::to_le_bytes).collect::<Vec<_>>());
            let token = i["token"].as_array().map(|v| {
                v.iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect::<Vec<_>>()
            });
            let value = compose_user_login(
                ip.as_deref(),
                i["seconds"].as_i64().unwrap() as i32,
                i["limited"].as_bool().unwrap(),
                EffectFlags {
                    is_dormant: i["dormant"].as_bool().unwrap(),
                    is_return: i["returning"].as_bool().unwrap(),
                },
                token.as_deref(),
                TokenProfile::SqlLatin1GeneralCp1,
            )
            .unwrap();
            let UserLoginReturn::Success(s) = value.feedback() else {
                panic!("success expected")
            };
            let o = &case["output"];
            assert_eq!(s.package_count, o[0]["number"].as_i64().unwrap() as i32);
            assert_eq!(s.last_login_ip, units(&o[1]["units"]).as_deref());
            assert_eq!(s.lock_last_seconds, o[2]["number"].as_i64().unwrap() as i32);
            assert_eq!(s.limited, o[3]["number"].as_i64().unwrap() as i32);
            assert_eq!(s.is_dormant, o[4]["flag"].as_bool().unwrap());
            assert_eq!(s.facebook_token, units(&o[5]["units"]).as_deref());
            assert_eq!(s.is_return, o[6]["flag"].as_bool().unwrap());
            success += 1;
        }
        assert_eq!(success, 160);
    }
}
#[test]
fn cp1252_matches_all_sql_and_framework_byte_values() {
    let f: Value = serde_json::from_str(include_str!(
        "fixtures/framework-user-login-composition.json"
    ))
    .unwrap();
    let sql: Value = serde_json::from_str(include_str!(
        "../docs/evidence/database/token-codepage.json"
    ))
    .unwrap();
    assert_eq!(sql["code_page"], 1252);
    for profile in f["profiles"].as_array().unwrap() {
        for (n, c) in profile["decoding"].as_array().unwrap().iter().enumerate() {
            let actual = decode_token(Some(&[n as u8]), TokenProfile::SqlLatin1GeneralCp1).unwrap();
            assert_eq!(&*actual, units(&c["units"]).as_ref().unwrap());
            assert_eq!(sql["cases"][n]["unit"].as_u64().unwrap() as u16, actual[0]);
        }
    }
}
#[test]
fn source_bounds_and_empty_null_projection_are_preserved() {
    let flags = || EffectFlags {
        is_dormant: false,
        is_return: false,
    };
    for ip in [vec![0; 101], vec![0; 102]] {
        assert!(matches!(
            compose_user_login(
                Some(&ip),
                0,
                false,
                flags(),
                None,
                TokenProfile::SqlLatin1GeneralCp1
            ),
            Err(CompositionError::TextBound)
        ));
    }
    assert!(matches!(
        compose_user_login(
            Some(&[0]),
            0,
            false,
            flags(),
            None,
            TokenProfile::SqlLatin1GeneralCp1
        ),
        Err(CompositionError::Utf16Length)
    ));
    assert!(matches!(
        decode_token(Some(&[0; 256]), TokenProfile::SqlLatin1GeneralCp1),
        Err(CompositionError::TextBound)
    ));
    assert_eq!(
        decode_token(Some(&[0; 255]), TokenProfile::SqlLatin1GeneralCp1)
            .unwrap()
            .len(),
        255
    );
    assert!(
        decode_token(None, TokenProfile::SqlLatin1GeneralCp1)
            .unwrap()
            .is_empty()
    );
    let v = compose_user_login(
        None,
        42,
        false,
        flags(),
        None,
        TokenProfile::SqlLatin1GeneralCp1,
    )
    .unwrap();
    let UserLoginReturn::Success(s) = v.feedback() else {
        panic!()
    };
    assert_eq!(s.lock_last_seconds, 42);
    assert_eq!(s.last_login_ip, Some([].as_slice()));
}

#[test]
fn member_failure_feedback_matches_existing_recovered_consumer() {
    use vindictus_server::domain::user_login_return::{
        LoginFeedback as F, decode_user_login_return,
    };
    let f: Value = serde_json::from_str(include_str!(
        "fixtures/framework-user-login-composition.json"
    ))
    .unwrap();
    for profile in f["profiles"].as_array().unwrap() {
        let mut compared = 0;
        for case in profile["cases"].as_array().unwrap() {
            let i = &case["input"];
            if i["member"] == false {
                continue;
            }
            let reason = if i["member"] == true { 4 } else { 3 };
            let text = units(&i["banned"]);
            let fields = if reason == 4 {
                vec![F::FailureReason(reason), F::Text(text.as_deref())]
            } else {
                vec![F::FailureReason(reason)]
            };
            let UserLoginReturn::Failure {
                reason: r,
                banned_reason,
            } = decode_user_login_return(&fields, 256).unwrap()
            else {
                panic!()
            };
            assert_eq!(r, case["output"][0]["number"].as_i64().unwrap() as i32);
            if reason == 4 {
                assert_eq!(banned_reason, units(&case["output"][1]["units"]).as_deref());
            }
            compared += 1;
        }
        assert_eq!(compared, 320);
    }
}
