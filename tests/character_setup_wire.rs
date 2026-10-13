use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY, Frame,
    character_setup::*,
    encode_integer,
    server_profile::REGISTERED_CLASSES,
    transport::{CipherProfile, LegacyReceiver, LegacySender},
};
fn kind(s: &str) -> Kind {
    match s {
        "check" => Kind::Check,
        "result" => Kind::Result,
        "query" => Kind::Query,
        "reserved" => Kind::Reserved,
        "play" => Kind::Play,
        _ => panic!("synthetic kind"),
    }
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_u64().unwrap() as u8)
        .collect()
}
fn message(c: &Value) -> Option<Message<'_>> {
    if c["is_null"].as_bool().unwrap() {
        return None;
    }
    let f = &c["fields"];
    Some(match kind(c["kind"].as_str().unwrap()) {
        Kind::Check => Message::Check {
            name: f["name"].as_str(),
            name_change: f["name_change"].as_bool().unwrap(),
        },
        Kind::Result => Message::Result {
            valid: f["valid"].as_bool().unwrap(),
            name: f["name"].as_str(),
            error: f["error"].as_str(),
        },
        Kind::Query => Message::Query,
        Kind::Play => Message::Play(f["state"].as_i64().unwrap() as i32),
        Kind::Reserved => Message::Reserved {
            names: f["names"]
                .as_array()
                .map(|v| v.iter().map(Value::as_str).collect()),
            titles: f["titles"]
                .as_array()
                .map(|v| v.iter().map(|n| n.as_i64().unwrap() as i32).collect()),
        },
    })
}
#[test]
fn setup_dtos_match_original_nullable_fields_envelopes_and_continuous_cipher() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-character-setup-wire.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["cases"].as_array().unwrap().len(), 252);
        let mut tx =
            LegacySender::with_profile(CipherProfile::VindictusFrontend, 123456, DEFAULT_MAX_BODY)
                .unwrap();
        let mut rx =
            LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
        for case in profile["cases"].as_array().unwrap() {
            let k = kind(case["kind"].as_str().unwrap());
            let body = bytes(&case["body"]);
            let typed = bytes(&case["typed_body"]);
            let expected = message(case);
            let category = case["category"].as_i64().unwrap() as i32;
            let decoded = decode_ingress(k, category, &body, DEFAULT_MAX_BODY).unwrap();
            assert!(decoded == expected);
            assert_eq!(
                encode(k, decoded.as_ref(), DEFAULT_MAX_BODY)
                    .unwrap()
                    .as_slice(),
                typed
            );
            assert!(decode(k, &typed, DEFAULT_MAX_BODY).unwrap() == expected);
            let packet = tx
                .encode(&Frame {
                    instance_id: 0,
                    category_id: category,
                    body: &body,
                })
                .unwrap();
            let mut frames = Vec::new();
            for chunk in packet.chunks(3) {
                frames.extend(rx.push(chunk).unwrap())
            }
            assert_eq!(frames.len(), 1);
            assert!(
                decode_ingress(k, category, &frames[0].body, DEFAULT_MAX_BODY).unwrap() == expected
            );
        }
    }
    for k in [
        Kind::Check,
        Kind::Result,
        Kind::Query,
        Kind::Reserved,
        Kind::Play,
    ] {
        assert!(
            REGISTERED_CLASSES
                .iter()
                .any(|m| m.category_id == k.category() && m.type_id == k.type_id())
        );
    }
}
#[test]
fn setup_decoder_refuses_wrong_types_truncation_trailing_lengths_and_limits() {
    for (k, m) in [
        (
            Kind::Check,
            Message::Check {
                name: Some("LabFiona"),
                name_change: false,
            },
        ),
        (Kind::Play, Message::Play(i32::MAX)),
        (
            Kind::Reserved,
            Message::Reserved {
                names: Some(vec![Some("Lab")]),
                titles: Some(vec![1]),
            },
        ),
    ] {
        let body = encode(k, Some(&m), DEFAULT_MAX_BODY).unwrap();
        for n in 0..body.len() {
            assert!(decode(k, &body[..n], DEFAULT_MAX_BODY).is_err())
        }
        let mut extra = body.to_vec();
        extra.push(0);
        assert!(decode(k, &extra, DEFAULT_MAX_BODY).is_err());
        assert!(decode(k, &body, body.len() - 1).is_err());
        assert!(encode(k, Some(&m), body.len() - 1).is_err());
        assert!(decode_ingress(k, 999, &body, DEFAULT_MAX_BODY).is_err());
        let mut wrong = Kind::Query.type_id().as_bytes().to_vec();
        wrong.extend_from_slice(&body);
        assert!(decode_ingress(k, 0, &wrong, DEFAULT_MAX_BODY).is_err());
    }
    let mut count = vec![1];
    count.extend(encode_integer((MAX_COLLECTION + 2) as i32));
    assert!(decode(Kind::Reserved, &count, DEFAULT_MAX_BODY).is_err());
    let mut negative = vec![1];
    negative.extend(encode_integer(-1));
    assert!(decode(Kind::Reserved, &negative, DEFAULT_MAX_BODY).is_err());
    assert!(decode(Kind::Check, &[1, 2, 255, 0], DEFAULT_MAX_BODY).is_err());
    assert!(encode(Kind::Play, Some(&Message::Query), DEFAULT_MAX_BODY).is_err());
}
