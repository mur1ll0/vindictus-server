use serde::Deserialize;
use vindictus_server::protocol::login::PayloadError;
use vindictus_server::protocol::login_failure::{
    LoginFailMessage, PERMISSION_FAILED, decode_login_failure, encode_login_failure,
};
use vindictus_server::protocol::server_profile::LOGIN_FAIL_MESSAGE_CATEGORY;

#[derive(Deserialize)]
struct Fixture {
    failures: Vec<Vector>,
}
#[derive(Deserialize)]
struct Vector {
    reason: i32,
    banned_reason: Option<String>,
    category: i32,
    body: Vec<u8>,
}
fn vectors() -> Vec<Vector> {
    serde_json::from_str::<Fixture>(include_str!("fixtures/framework-converter.json"))
        .unwrap()
        .failures
}
#[test]
fn failures_match_framework_serializer_signed_reasons_null_empty_and_unicode_explanations() {
    for vector in vectors() {
        let expected = LoginFailMessage {
            reason: vector.reason,
            banned_reason: vector.banned_reason.as_deref(),
        };
        assert_eq!(vector.category, LOGIN_FAIL_MESSAGE_CATEGORY);
        assert!(decode_login_failure(&vector.body, 102400).unwrap() == Some(expected));
        assert_eq!(
            encode_login_failure(
                Some(&LoginFailMessage {
                    reason: vector.reason,
                    banned_reason: vector.banned_reason.as_deref(),
                }),
                102400
            )
            .unwrap(),
            vector.body
        );
    }
    assert_eq!(PERMISSION_FAILED, 13);
}
#[test]
fn every_incomplete_failure_payload_is_rejected_and_null_is_distinct() {
    for vector in vectors() {
        for end in 0..vector.body.len() {
            assert!(decode_login_failure(&vector.body[..end], 102400).is_err());
        }
    }
    assert!(decode_login_failure(&[0], 102400).unwrap().is_none());
    assert_eq!(encode_login_failure(None, 102400).unwrap(), [0]);
}
#[test]
fn failure_codec_rejects_trailing_bytes_and_bounds() {
    let mut body = vectors().remove(0).body;
    assert!(matches!(
        decode_login_failure(&body, body.len() - 1),
        Err(PayloadError::TooLarge)
    ));
    body.push(0);
    assert!(matches!(
        decode_login_failure(&body, 102400),
        Err(PayloadError::TrailingBytes)
    ));
    assert!(matches!(
        encode_login_failure(
            Some(&LoginFailMessage {
                reason: PERMISSION_FAILED,
                banned_reason: Some("too long"),
            }),
            5
        ),
        Err(PayloadError::TooLarge)
    ));
}
