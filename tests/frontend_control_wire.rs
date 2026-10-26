use serde_json::Value;
use vindictus_server::protocol::login::PayloadError;
use vindictus_server::protocol::{
    Frame,
    frontend_control::*,
    server_profile::REGISTERED_CLASSES,
    transport::{CipherProfile, LegacyReceiver, LegacySender},
};
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-lobby-control-wire.json")).unwrap()
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn kind(v: &str) -> Kind {
    match v {
        "ping" => Kind::Ping,
        "pong" => Kind::Pong,
        "system" => Kind::System,
        "ng" => Kind::Ng,
        "cafe" => Kind::Cafe,
        "notice_shutdown" => Kind::NoticeShutDown,
        _ => panic!("fixture kind"),
    }
}
fn message<'a>(c: &'a Value, data: &'a [u8]) -> Option<Message<'a>> {
    if c["is_null"].as_bool().unwrap() {
        return None;
    }
    let f = &c["fields"];
    Some(match kind(c["kind"].as_str().unwrap()) {
        Kind::Ping => Message::Ping(f["tag"].as_i64().unwrap() as i32),
        Kind::Pong => Message::Pong(f["tag"].as_i64().unwrap() as i32),
        Kind::NoticeShutDown => Message::NoticeShutDown(f["delay"].as_i64().unwrap() as i32),
        Kind::Cafe => Message::Cafe {
            home: f["home"].as_i64().unwrap() as i32,
            account: f["account"].as_i64().unwrap() as i32,
            cafe: f["cafe"].as_i64().unwrap() as i32,
        },
        Kind::Ng => Message::Ng {
            data: if f["data"].is_null() {
                None
            } else {
                Some(data)
            },
            checksum: f["checksum"].as_u64().unwrap(),
        },
        Kind::System => Message::System {
            category: f["category"].as_u64().unwrap() as u8,
            text: if f["text_present"].as_bool().unwrap() {
                Some(HeroesText {
                    format: f["format"].as_str(),
                    parameters: f["parameters"]
                        .as_array()
                        .map(|p| p.iter().map(Value::as_str).collect()),
                })
            } else {
                None
            },
        },
    })
}
#[test]
fn five_control_messages_match_original_fields_bytes_and_stateful_cipher_in_all_envelopes() {
    let f = fixture();
    for p in f["profiles"].as_array().unwrap() {
        assert_eq!(p["cases"].as_array().unwrap().len(), 459);
        let mut sender =
            LegacySender::with_profile(CipherProfile::VindictusFrontend, 123456, 16384).unwrap();
        let mut receiver = LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, 16384);
        for c in p["cases"].as_array().unwrap() {
            let k = kind(c["kind"].as_str().unwrap());
            let b = bytes(&c["body"]);
            let typed = bytes(&c["typed_body"]);
            let data = if c["fields"]["data"].is_array() {
                bytes(&c["fields"]["data"])
            } else {
                Vec::new()
            };
            let expected = message(c, &data);
            let category = c["category"].as_i64().unwrap() as i32;
            let decoded = decode_ingress(k, category, &b, 16384).unwrap();
            assert!(decoded == expected);
            assert_eq!(encode(k, decoded.as_ref(), 16384).unwrap(), typed);
            assert!(decode(k, &typed, 16384).unwrap() == expected);
            let packet = sender
                .encode(&Frame {
                    instance_id: 0,
                    category_id: category,
                    body: &b,
                })
                .unwrap();
            let mut frames = Vec::new();
            for fragment in packet.chunks(3) {
                frames.extend(receiver.push(fragment).unwrap())
            }
            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0].category_id, category);
            assert!(decode_ingress(k, category, &frames[0].body, 16384).unwrap() == expected);
            assert_eq!(receiver.buffered_len(), 0);
        }
    }
}
#[test]
fn control_categories_guids_and_source_failure_literal_match_original_metadata() {
    for k in [
        Kind::Ping,
        Kind::Pong,
        Kind::System,
        Kind::Ng,
        Kind::Cafe,
        Kind::NoticeShutDown,
    ] {
        assert!(
            REGISTERED_CLASSES
                .iter()
                .any(|x| x.category_id == k.category() && x.type_id == k.type_id())
        );
    }
    let b = second_password_locked(1000).unwrap();
    let decoded = decode(Kind::System, &b, 1000).unwrap().unwrap();
    assert!(
        decoded
            == Message::System {
                category: 4,
                text: Some(HeroesText {
                    format: Some("GameUI_Heroes_SecondPassword_Locked"),
                    parameters: Some(Vec::new())
                })
            }
    );
}
#[test]
fn control_decoders_refuse_truncation_trailing_cross_kind_and_unsupported_types() {
    let f = fixture();
    for c in f["profiles"][0]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["mode"] == "registered-typed")
    {
        let k = kind(c["kind"].as_str().unwrap());
        let b = bytes(&c["body"]);
        for n in 0..b.len() {
            assert!(decode(k, &b[..n], 16384).is_err())
        }
        assert!(matches!(
            decode(k, &b, b.len() - 1),
            Err(ControlWireError::Payload(PayloadError::TooLarge))
        ));
        let mut extra = b.clone();
        extra.push(0);
        assert!(matches!(
            decode(k, &extra, 16384),
            Err(ControlWireError::Payload(PayloadError::TrailingBytes))
        ));
        if b[0] != 0 {
            let mut unknown = vec![0; 16];
            unknown.extend_from_slice(&b);
            assert!(matches!(
                decode_ingress(k, 0, &unknown, 16384),
                Err(ControlWireError::Type)
            ));
            assert!(matches!(
                decode_ingress(k, 999, &b, 16384),
                Err(ControlWireError::Type)
            ));
        }
    }
    assert!(matches!(
        encode(Kind::Ping, Some(&Message::Pong(0)), 20),
        Err(ControlWireError::Type)
    ));
    let mut object = uuid::Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21)
        .as_bytes()
        .to_vec();
    object.push(1);
    assert!(matches!(
        decode_ingress(Kind::Ping, 0, &object, 20),
        Err(ControlWireError::Object)
    ));
}
#[test]
fn heroes_string_collection_bounds_negative_lengths_and_invalid_utf8_are_refused() {
    let message = Message::System {
        category: 4,
        text: Some(HeroesText {
            format: Some("lab"),
            parameters: Some(vec![None; MAX_PARAMETERS]),
        }),
    };
    let b = encode(Kind::System, Some(&message), 1000).unwrap();
    assert!(decode(Kind::System, &b, 1000).unwrap() == Some(message));
    assert!(matches!(
        encode(
            Kind::System,
            Some(&Message::System {
                category: 4,
                text: Some(HeroesText {
                    format: None,
                    parameters: Some(vec![None; MAX_PARAMETERS + 1])
                })
            }),
            1000
        ),
        Err(ControlWireError::Payload(PayloadError::TooLarge))
    ));
    assert!(matches!(
        decode(Kind::System, &[1, 4, 1, 2, 255, 0], 1000),
        Err(ControlWireError::Payload(PayloadError::InvalidUtf8))
    ));
    let mut b = vec![1, 4, 1, 0];
    b.extend(vindictus_server::protocol::encode_integer(-1));
    assert!(matches!(
        decode(Kind::System, &b, 1000),
        Err(ControlWireError::Payload(PayloadError::NegativeLength))
    ));
}
