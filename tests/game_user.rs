use serde::Deserialize;
use vindictus_server::domain::{
    game_user::{GameUserNameError, LegacyNameKey, NewGameUser},
    legacy_user::LegacyUserError,
};

#[derive(Deserialize)]
struct CollationFixture {
    cases: Vec<CollationCase>,
}

#[derive(Deserialize)]
struct CollationCase {
    left_value: String,
    right_value: String,
    equal: bool,
    printable_ascii_contract: bool,
}

#[test]
fn ascii_name_keys_match_sql_server_equality_cases_and_reject_unproven_inputs() {
    let fixture: CollationFixture = serde_json::from_str(include_str!(
        "../docs/evidence/database/collation-oracle.json"
    ))
    .unwrap();
    let mut compared = 0;
    for case in fixture.cases {
        let left = LegacyNameKey::parse(&case.left_value);
        let right = LegacyNameKey::parse(&case.right_value);
        if case.printable_ascii_contract {
            assert_eq!(left.unwrap() == right.unwrap(), case.equal);
            compared += 1;
        } else {
            assert!(left.is_err() || right.is_err());
        }
    }
    assert_eq!(compared, 11);
}

#[test]
fn persistence_name_policy_preserves_punctuation_leading_spaces_and_display_case() {
    let original = " MiXeD_' !  ";
    let user = NewGameUser::new(i32::MIN, original, -1, 255).unwrap();
    assert_eq!(user.name(), original);
    assert_eq!(user.name_key(), " mixed_' !");
    assert_eq!(user.id(), i32::MIN);
    assert_eq!(user.next_character_sn(), 0);
    assert_eq!(user.channeling_code(), Some(255));
    assert!(LegacyNameKey::parse(" a").unwrap() != LegacyNameKey::parse("a").unwrap());
}

#[test]
fn name_policy_rejects_unicode_controls_and_overlong_names_without_truncating() {
    for name in ["É", "Ａ", "a\t", "a\n", "a\0", "\u{7f}"] {
        assert!(matches!(
            LegacyNameKey::parse(name),
            Err(GameUserNameError::UnsupportedCharacters)
        ));
    }
    assert!(matches!(
        LegacyNameKey::parse(&"a".repeat(25)),
        Err(GameUserNameError::Bound(LegacyUserError::NameTooLong))
    ));
    assert!(LegacyNameKey::parse(&"a".repeat(24)).is_ok());
    // Schema equality is separate from authentication's empty-passport rejection.
    assert!(LegacyNameKey::parse("").unwrap() == LegacyNameKey::parse(" ").unwrap());
}

#[derive(Deserialize)]
struct UserFixture {
    cases: Vec<UserCase>,
}

#[derive(Deserialize)]
struct UserCase {
    id: i32,
    name: String,
    service_id: i32,
    channel_code: u8,
    next_character_sn: i32,
    channeling_code: Option<u8>,
}

#[test]
fn validated_new_user_keeps_source_dto_id_and_initial_values_for_all_oracle_cases() {
    let fixture: UserFixture =
        serde_json::from_str(include_str!("fixtures/framework-user.json")).unwrap();
    for case in fixture.cases {
        let user =
            NewGameUser::new(case.id, &case.name, case.service_id, case.channel_code).unwrap();
        assert_eq!(user.id(), case.id);
        assert_eq!(user.name(), case.name);
        assert_eq!(user.service_id(), case.service_id);
        assert_eq!(user.next_character_sn(), case.next_character_sn);
        assert_eq!(user.channeling_code(), case.channeling_code);
    }
}
