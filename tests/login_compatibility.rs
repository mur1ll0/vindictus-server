use serde::Deserialize;
use vindictus_server::protocol::login::{
    PayloadError, UserLoginMessage, decode_user_login, encode_user_login,
};
#[derive(Deserialize)]
struct Fixture {
    login: Vec<Vector>,
}
#[derive(Deserialize)]
struct Vector {
    value: Option<Value>,
    body: Vec<u8>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Value {
    passport: Option<String>,
    local_address: u32,
    #[serde(rename = "hwID")]
    hw_id: Option<String>,
    #[serde(rename = "MachineID")]
    machine_id: Option<String>,
    game_room_client: i32,
    is_character_select_skipped: bool,
    #[serde(rename = "NexonID")]
    nexon_id: Option<String>,
    up_to_date_info: Option<String>,
    check_sum: i64,
}
fn fixtures() -> Vec<Vector> {
    serde_json::from_str::<Fixture>(include_str!("fixtures/legacy-packet.json"))
        .expect("C# oracle")
        .login
}
#[test]
fn login_payload_matches_recovered_serializer_null_empty_utf8_and_numeric_fields() {
    for vector in fixtures() {
        let decoded = decode_user_login(&vector.body, 4096).unwrap();
        if let Some(value) = &vector.value {
            // JsonSerializer emits byte arrays as base64; compare deterministic machine bytes separately.
            let machine: Option<Vec<u8>> = value.machine_id.as_ref().map(|_| (0..16).collect());
            let expected = UserLoginMessage {
                passport: value.passport.as_deref(),
                local_address: value.local_address,
                hw_id: value.hw_id.as_deref(),
                machine_id: machine.as_deref(),
                game_room_client: value.game_room_client,
                is_character_select_skipped: value.is_character_select_skipped,
                nexon_id: value.nexon_id.as_deref(),
                up_to_date_info: value.up_to_date_info.as_deref(),
                check_sum: value.check_sum,
            };
            assert!(
                decoded.as_ref() == Some(&expected),
                "field mismatch (passport redacted)"
            );
            assert_eq!(
                encode_user_login(Some(&expected), 4096).unwrap(),
                vector.body
            );
        } else {
            assert!(decoded.is_none());
            assert_eq!(encode_user_login(None, 4096).unwrap(), vector.body);
        }
    }
}
#[test]
fn every_truncated_login_prefix_is_rejected_without_allocation_or_panic() {
    for vector in fixtures() {
        for end in 0..vector.body.len() {
            assert!(
                decode_user_login(&vector.body[..end], 4096).is_err(),
                "end={end}"
            );
        }
    }
}
#[test]
fn trailing_bytes_limits_and_untrusted_field_lengths_are_rejected() {
    let mut body = fixtures().remove(1).body;
    assert!(matches!(
        decode_user_login(&body, body.len() - 1),
        Err(PayloadError::TooLarge)
    ));
    let decoded = decode_user_login(&body, 4096).unwrap();
    assert_eq!(
        encode_user_login(decoded.as_ref(), body.len() - 1),
        Err(PayloadError::TooLarge)
    );
    body.push(0);
    assert!(matches!(
        decode_user_login(&body, 4096),
        Err(PayloadError::TrailingBytes)
    ));
    assert!(matches!(
        decode_user_login(&[1, 0xff, 0xbf, 0xbf, 0xbf, 0xbf, 0xbf], 4096),
        Err(PayloadError::NegativeLength)
    ));
    assert!(matches!(
        decode_user_login(&[1, 0xfc, 0xbf, 0xbf, 0xbf, 0xbf, 0xbf], 4096),
        Err(PayloadError::Truncated)
    ));
}
#[test]
fn invalid_utf8_is_rejected_and_legacy_nonzero_boolean_is_accepted() {
    assert!(matches!(
        decode_user_login(&[1, 2, 0xff], 4096),
        Err(PayloadError::InvalidUtf8)
    ));
    let mut body = fixtures().remove(1).body;
    body[0] = 255;
    let parsed = decode_user_login(&body, 4096).unwrap();
    assert!(
        parsed.is_some(),
        "any nonzero object-presence byte is true in C#"
    );
    // The default-valued message has presence + null passport + IPv4 + null hwID
    // + null MachineID + i32. Its next byte is the boolean field.
    let mut default_body = fixtures().remove(0).body;
    default_body[12] = 255;
    assert!(
        decode_user_login(&default_body, 4096)
            .unwrap()
            .unwrap()
            .is_character_select_skipped
    );
    assert!(decode_user_login(&[0], 1).unwrap().is_none());
}
