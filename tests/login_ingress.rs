use serde::Deserialize;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    login::{PayloadError, encode_user_login},
    login_ingress::{LoginIngressError, decode_login_ingress},
    server_profile::{USER_LOGIN_MESSAGE_CATEGORY, USER_LOGIN_MESSAGE_TYPE_ID},
    transport::{CipherProfile, LegacyReceiver},
};

#[derive(Deserialize)]
struct Fixture {
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    architecture: String,
    login_guid: uuid::Uuid,
    object_guid: uuid::Uuid,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    mode: String,
    category: i32,
    body: Vec<u8>,
    typed_body: Vec<u8>,
    wire: Vec<u8>,
    message_is_null: bool,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-login-envelope.json")).unwrap()
}

#[test]
fn recovered_framework_envelopes_preserve_every_typed_login_field() {
    let fixture = fixture();
    assert_eq!(fixture.profiles.len(), 2);
    for profile in fixture.profiles {
        assert!(matches!(profile.architecture.as_str(), "x86" | "x64"));
        assert_eq!(profile.login_guid, USER_LOGIN_MESSAGE_TYPE_ID);
        assert_eq!(profile.cases.len(), 39);
        for case in profile.cases {
            let login = decode_login_ingress(case.category, &case.body, DEFAULT_MAX_BODY).unwrap();
            assert_eq!(login.is_none(), case.message_is_null);
            assert_eq!(
                encode_user_login(login.as_ref(), DEFAULT_MAX_BODY).unwrap(),
                case.typed_body,
                "{} case {}",
                case.mode,
                case.id
            );
            if case.category == 0 {
                let guid = if case.message_is_null {
                    profile.object_guid
                } else {
                    profile.login_guid
                };
                assert_eq!(&case.body[..16], guid.as_bytes());
            } else {
                assert_eq!(case.category, USER_LOGIN_MESSAGE_CATEGORY);
            }
        }
    }
}

#[test]
fn recovered_ciphertext_decodes_when_every_envelope_byte_arrives_separately() {
    for profile in fixture().profiles {
        for case in profile.cases {
            let mut receiver =
                LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
            let mut frames = Vec::new();
            for byte in case.wire {
                frames.extend(receiver.push(&[byte]).unwrap());
            }
            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0].category_id, case.category);
            assert_eq!(frames[0].body, case.body);
            assert_eq!(receiver.buffered_len(), 0);
            let login =
                decode_login_ingress(frames[0].category_id, &frames[0].body, DEFAULT_MAX_BODY)
                    .unwrap();
            assert_eq!(
                encode_user_login(login.as_ref(), DEFAULT_MAX_BODY).unwrap(),
                case.typed_body
            );
        }
    }
}

#[test]
fn every_truncated_envelope_and_excess_byte_or_total_size_is_rejected() {
    for case in fixture().profiles.remove(0).cases {
        for end in 0..case.body.len() {
            assert!(
                decode_login_ingress(case.category, &case.body[..end], DEFAULT_MAX_BODY).is_err(),
                "{} case {} prefix {end}",
                case.mode,
                case.id
            );
        }
        assert!(matches!(
            decode_login_ingress(case.category, &case.body, case.body.len() - 1),
            Err(LoginIngressError::Payload(PayloadError::TooLarge))
        ));
        assert!(decode_login_ingress(case.category, &case.body, case.body.len()).is_ok());
        let mut extra = case.body.clone();
        extra.push(0);
        assert!(matches!(
            decode_login_ingress(case.category, &extra, DEFAULT_MAX_BODY),
            Err(LoginIngressError::Payload(PayloadError::TrailingBytes))
        ));
    }
    for limit in 0..16 {
        assert!(decode_login_ingress(0, &vec![0; limit], limit).is_err());
    }
}

#[test]
fn unknown_types_converter_guid_and_wrong_category_never_activate_a_decoder() {
    for guid in [[0; 16], [0xff; 16], [0x42; 16]] {
        let mut body = guid.to_vec();
        body.extend_from_slice(&[1, 0, 0]);
        assert!(matches!(
            decode_login_ingress(0, &body, DEFAULT_MAX_BODY),
            Err(LoginIngressError::UnsupportedType)
        ));
    }
    let body = &fixture().profiles[0].cases[11].typed_body;
    for category in [-1, 1, 479, 482, i32::MAX] {
        assert!(matches!(
            decode_login_ingress(category, body, DEFAULT_MAX_BODY),
            Err(LoginIngressError::UnsupportedCategory)
        ));
    }
    assert!(matches!(
        decode_login_ingress(
            USER_LOGIN_MESSAGE_CATEGORY,
            USER_LOGIN_MESSAGE_TYPE_ID.as_bytes(),
            DEFAULT_MAX_BODY
        ),
        Err(LoginIngressError::Payload(_))
    ));
}

#[test]
fn non_null_system_object_is_not_login_and_null_does_not_hide_trailing_fields() {
    let profile = fixture().profiles.remove(0);
    let mut body = profile.object_guid.as_bytes().to_vec();
    assert!(matches!(
        decode_login_ingress(0, &body, DEFAULT_MAX_BODY),
        Err(LoginIngressError::Payload(PayloadError::Truncated))
    ));
    body.push(1);
    assert!(matches!(
        decode_login_ingress(0, &body, DEFAULT_MAX_BODY),
        Err(LoginIngressError::NonLoginObject)
    ));
    body[16] = 0;
    assert!(
        decode_login_ingress(0, &body, DEFAULT_MAX_BODY)
            .unwrap()
            .is_none()
    );
    body.push(0);
    assert!(matches!(
        decode_login_ingress(0, &body, DEFAULT_MAX_BODY),
        Err(LoginIngressError::Payload(PayloadError::TrailingBytes))
    ));
}
