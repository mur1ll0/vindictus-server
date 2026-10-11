use serde::Deserialize;
use vindictus_server::domain::legacy_user::{LegacyUserError, prepare_legacy_user};
#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    id: i32,
    name: String,
    service_id: i32,
    channel_code: u8,
    next_character_sn: i32,
    channeling_code: Option<u8>,
    last_login_ip: Option<String>,
    ip_lock_time: Option<String>,
    second_password: Option<String>,
    second_password_fail_count: Option<i32>,
    second_password_last_fail: Option<String>,
    facebook_token: Option<String>,
}
#[test]
fn user_insert_defaults_match_unchanged_recovered_clr_dto_and_original_initializer() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-user.json")).unwrap();
    for case in fixture.cases {
        let user =
            prepare_legacy_user(case.id, &case.name, case.service_id, case.channel_code).unwrap();
        assert_eq!(user.id, case.id);
        assert_eq!(user.name, case.name);
        assert_eq!(user.service_id, case.service_id);
        assert_eq!(user.next_character_sn, case.next_character_sn);
        assert_eq!(user.channeling_code, case.channeling_code);
        assert_eq!(user.last_login_ip, case.last_login_ip.as_deref());
        assert_eq!(user.second_password, case.second_password.as_deref());
        assert_eq!(
            user.second_password_fail_count,
            case.second_password_fail_count
        );
        assert_eq!(user.facebook_token, case.facebook_token.as_deref());
        assert!(case.ip_lock_time.is_none());
        assert!(case.second_password_last_fail.is_none());
    }
}
#[test]
fn database_name_bound_uses_utf16_units_and_does_not_truncate_or_normalize() {
    assert!(prepare_legacy_user(-1, &"😀".repeat(12), 42, 0).is_ok());
    assert!(matches!(
        prepare_legacy_user(-1, &"😀".repeat(13), 42, 0),
        Err(LegacyUserError::NameTooLong)
    ));
    assert!(prepare_legacy_user(0, &"É".repeat(24), 42, 0).is_ok());
    assert!(matches!(
        prepare_legacy_user(0, &"a".repeat(25), 42, 0),
        Err(LegacyUserError::NameTooLong)
    ));
    let user = prepare_legacy_user(-1, "MiXeD ", -1, 255).unwrap();
    assert_eq!(user.name, "MiXeD ");
    assert_eq!(user.channeling_code, Some(255));
}
