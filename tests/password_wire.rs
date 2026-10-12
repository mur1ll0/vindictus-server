use serde_json::Value;
use vindictus_server::protocol::{
    Frame,
    transport::{CipherProfile, LegacyReceiver, LegacySender},
};
use vindictus_server::protocol::{
    login::PayloadError, second_password::*, server_profile::REGISTERED_CLASSES,
};
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-password-wire.json")).unwrap()
}
fn kind(s: &str) -> Kind {
    match s {
        "ask" => Kind::Ask,
        "check" => Kind::Check,
        "set" => Kind::Set,
        "has" => Kind::Has,
        "result" => Kind::Result,
        _ => panic!("fixture kind"),
    }
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_u64().unwrap() as u8)
        .collect()
}
fn fields(v: &Value, k: Kind) -> Option<Message<'_>> {
    if v["is_null"].as_bool().unwrap() {
        return None;
    }
    let f = &v["fields"];
    Some(match k {
        Kind::Ask => Message::Ask,
        Kind::Check => Message::Check(f["password"].as_str()),
        Kind::Set => Message::Set(f["password"].as_str()),
        Kind::Has => Message::Has(HasPassword {
            first: f["first"].as_bool().unwrap(),
            has_password: f["has"].as_bool().unwrap(),
            passed: f["passed"].as_bool().unwrap(),
            fail_count: f["fail"].as_i64().unwrap() as i32,
            retry_seconds: f["retry"].as_i64().unwrap() as i32,
        }),
        Kind::Result => Message::Result(PasswordResult {
            operation: f["operation"].as_i64().unwrap() as i32,
            passed: f["passed"].as_bool().unwrap(),
            fail_count: f["fail"].as_i64().unwrap() as i32,
            retry_seconds: f["retry"].as_i64().unwrap() as i32,
        }),
    })
}
#[test]
fn five_password_messages_typed_and_guid_envelopes_match_framework() {
    let f = fixture();
    for p in f["profiles"].as_array().unwrap() {
        let mut sender =
            LegacySender::with_profile(CipherProfile::VindictusFrontend, 123456, 2048).unwrap();
        let mut receiver = LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, 2048);
        assert_eq!(p["cases"].as_array().unwrap().len(), 426);
        for c in p["cases"].as_array().unwrap() {
            let k = kind(c["kind"].as_str().unwrap());
            let b = bytes(&c["body"]);
            let typed = bytes(&c["typed_body"]);
            let expected = fields(c, k);
            let decoded =
                decode_ingress(k, c["category"].as_i64().unwrap() as i32, &b, 2048).unwrap();
            assert!(decoded == expected);
            assert_eq!(*encode(k, decoded.as_ref(), 2048).unwrap(), typed);
            assert!(decode(k, &typed, 2048).unwrap() == expected);
            let category = c["category"].as_i64().unwrap() as i32;
            let packet = sender
                .encode(&Frame {
                    instance_id: 0,
                    category_id: category,
                    body: &b,
                })
                .unwrap();
            let mut frames = Vec::new();
            for fragment in packet.chunks(3) {
                frames.extend(receiver.push(fragment).unwrap());
            }
            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0].category_id, category);
            assert!(decode_ingress(k, category, &frames[0].body, 2048).unwrap() == expected);
            assert_eq!(receiver.buffered_len(), 0);
        }
    }
}
#[test]
fn password_type_ids_and_categories_match_original_registration_table() {
    for k in [Kind::Ask, Kind::Check, Kind::Set, Kind::Has, Kind::Result] {
        assert!(
            REGISTERED_CLASSES
                .iter()
                .any(|v| v.category_id == k.category() && v.type_id == k.type_id())
        );
    }
}
#[test]
fn password_decoders_reject_truncation_trailing_and_limits() {
    let f = fixture();
    for c in f["profiles"][0]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["mode"] == "registered-typed")
    {
        let k = kind(c["kind"].as_str().unwrap());
        let b = bytes(&c["body"]);
        for n in 0..b.len() {
            assert!(decode(k, &b[..n], 2048).is_err());
        }
        assert!(matches!(
            decode(k, &b, b.len() - 1),
            Err(PasswordWireError::Payload(PayloadError::TooLarge))
        ));
        let mut extra = b.clone();
        extra.push(0);
        assert!(matches!(
            decode(k, &extra, 2048),
            Err(PasswordWireError::Payload(PayloadError::TrailingBytes))
        ));
        assert!(encode(k, fields(c, k).as_ref(), b.len() - 1).is_err());
    }
    assert!(matches!(
        decode(Kind::Check, &[1, 2, 255], 100),
        Err(PasswordWireError::Payload(PayloadError::InvalidUtf8))
    ));
    assert!(matches!(
        encode(Kind::Set, Some(&Message::Ask), 100),
        Err(PasswordWireError::Kind)
    ));
}
#[test]
fn password_ingress_refuses_cross_kind_peer_types_and_nonnull_object() {
    let body = [1];
    assert!(matches!(
        decode_ingress(Kind::Check, Kind::Set.category(), &body, 100),
        Err(PasswordWireError::Category)
    ));
    let mut envelope = Kind::Set.type_id().as_bytes().to_vec();
    envelope.push(0);
    assert!(matches!(
        decode_ingress(Kind::Check, 0, &envelope, 100),
        Err(PasswordWireError::Type)
    ));
    assert!(matches!(
        decode_ingress(Kind::Set, 0, &envelope, 16),
        Err(PasswordWireError::Payload(PayloadError::TooLarge))
    ));
    let mut object = uuid::Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21)
        .as_bytes()
        .to_vec();
    object.push(1);
    assert!(matches!(
        decode_ingress(Kind::Check, 0, &object, 100),
        Err(PasswordWireError::Object)
    ));
    object[16] = 0;
    assert!(
        decode_ingress(Kind::Check, 0, &object, 100)
            .unwrap()
            .is_none()
    );
    object.push(0);
    assert!(matches!(
        decode_ingress(Kind::Check, 0, &object, 100),
        Err(PasswordWireError::Payload(PayloadError::TrailingBytes))
    ));
}
