use serde::Deserialize;
use vindictus_server::domain::user_login_return::{
    LoginFeedback as F, LoginReturnError, UserLoginReturn as R, decode_user_login_return,
};
#[derive(Deserialize)]
struct Fixture {
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    cases: Vec<Case>,
    single: Vec<Single>,
}
#[derive(Deserialize)]
struct Single {
    count: usize,
    limited: bool,
    error: Option<String>,
}
#[derive(Deserialize)]
struct Field {
    kind: String,
    number: Option<i32>,
    flag: Option<bool>,
    units: Option<Vec<u16>>,
}
#[derive(Deserialize)]
struct Case {
    label: String,
    input: Vec<Field>,
    incomplete: bool,
    error: Option<String>,
    result: bool,
    consumed: usize,
    package: i32,
    ip: Option<Vec<u16>>,
    seconds: i32,
    limited: i32,
    dormant: bool,
    token: Option<Vec<u16>>,
    returning: bool,
    reason: i32,
    banned: Option<Vec<u16>>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-user-login-return.json")).unwrap()
}
fn fields(case: &Case) -> Vec<F<'_>> {
    case.input
        .iter()
        .map(|field| match field.kind.as_str() {
            "int" => F::Integer(field.number.unwrap()),
            "reason" => F::FailureReason(field.number.unwrap()),
            "bool" => F::Boolean(field.flag.unwrap()),
            "text" => F::Text(field.units.as_deref()),
            _ => panic!("fixture kind"),
        })
        .collect()
}
#[test]
fn complete_valid_callbacks_match_original_class_in_both_framework_profiles() {
    for profile in fixture().profiles {
        assert_eq!(profile.cases.len(), 44);
        for case in profile.cases {
            if case.error.is_some()
                || case.incomplete
                || case.label == "extra"
                || case.label == "false-ambiguous"
            {
                continue;
            }
            match decode_user_login_return(&fields(&case), 512).unwrap() {
                R::Success(value) => {
                    assert!(case.result);
                    assert_eq!(value.package_count, case.package);
                    assert_eq!(value.last_login_ip, case.ip.as_deref());
                    assert_eq!(value.lock_last_seconds, case.seconds);
                    assert_eq!(value.limited, case.limited);
                    assert_eq!(value.is_dormant, case.dormant);
                    assert_eq!(value.facebook_token, case.token.as_deref());
                    assert_eq!(value.is_return, case.returning);
                }
                R::Failure {
                    reason,
                    banned_reason,
                } => {
                    assert!(!case.result);
                    assert_eq!(reason, case.reason);
                    assert_eq!(banned_reason, case.banned.as_deref());
                }
            }
        }
    }
}
#[test]
fn wrong_types_truncated_extra_and_ambiguous_bool_fail_explicitly() {
    for profile in fixture().profiles {
        for case in profile.cases {
            if case.error.is_some()
                || case.incomplete
                || case.label == "extra"
                || case.label == "false-ambiguous"
            {
                assert!(matches!(
                    decode_user_login_return(&fields(&case), 512),
                    Err(LoginReturnError::Shape)
                ));
                if case.label == "extra" {
                    assert_eq!(case.consumed, 7);
                    assert!(case.result);
                } // stricter than source ignores-tail.
                if case.label == "false-ambiguous" {
                    assert!(!case.result);
                    assert_eq!(case.reason, 0);
                }
            }
        }
    }
}
#[test]
fn text_bounds_apply_to_raw_utf16_without_collapsing_null_empty_or_surrogates() {
    let raw = [0u16, 0xd800, 0xdfff];
    let input = [
        F::Integer(4),
        F::Text(Some(&raw)),
        F::Integer(-1),
        F::Integer(i32::MIN),
        F::Boolean(false),
        F::Text(None),
        F::Boolean(true),
    ];
    assert!(matches!(
        decode_user_login_return(&input, 2),
        Err(LoginReturnError::TextBound)
    ));
    match decode_user_login_return(&input, 3).unwrap() {
        R::Success(value) => {
            assert_eq!(value.last_login_ip, Some(raw.as_slice()));
            assert_eq!(value.facebook_token, None);
            assert_eq!(value.package_count, 4);
        }
        R::Failure { .. } => panic!("integer was mistaken for FailReason"),
    }
    assert!(matches!(
        decode_user_login_return(&[F::FailureReason(4), F::Text(Some(&raw))], 2),
        Err(LoginReturnError::TextBound)
    ));
    assert!(matches!(
        decode_user_login_return(&[], 512),
        Err(LoginReturnError::Shape)
    ));
}
#[test]
fn framework_single_or_default_confirms_duplicate_rows_are_an_error() {
    for profile in fixture().profiles {
        assert_eq!(profile.single.len(), 4);
        for case in profile.single {
            match case.count {
                0 => {
                    assert!(!case.limited);
                    assert!(case.error.is_none());
                }
                1 => {
                    assert!(case.limited);
                    assert!(case.error.is_none());
                }
                _ => assert_eq!(case.error.as_deref(), Some("InvalidOperationException")),
            }
        }
    }
}
