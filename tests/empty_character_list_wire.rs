use serde_json::Value;
use vindictus_server::protocol::{
    Frame,
    empty_character_list::*,
    encode_integer,
    login::PayloadError,
    server_profile::REGISTERED_CLASSES,
    transport::{CipherProfile, LegacyReceiver, LegacySender},
};
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/framework-empty-character-list-wire.json"
    ))
    .unwrap()
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_u64().unwrap() as u8)
        .collect()
}
fn fields(c: &Value) -> Option<Message> {
    if c["is_null"].as_bool().unwrap() {
        return None;
    }
    let v = &c["fields"];
    assert!(v["characters_count"] == 0 || v["characters_count"] == -1);
    Some(Message {
        characters_present: v["characters_present"].as_bool().unwrap(),
        max_free: v["free"].as_i64().unwrap() as i32,
        max_purchased: v["purchased"].as_i64().unwrap() as i32,
        max_premium: v["premium"].as_i64().unwrap() as i32,
        prologue_played: v["prologue"].as_u64().unwrap() as u8,
        preset_used: v["preset"].as_i64().unwrap() as i32,
        party_states: v["party"]
            .as_array()
            .map(|p| p.iter().map(|n| n.as_i64().unwrap() as i32).collect()),
    })
}
#[test]
fn empty_character_list_fields_and_bytes_match_original_dto_and_retained_cipher() {
    let f = fixture();
    for profile in f["profiles"].as_array().unwrap() {
        assert_eq!(profile["cases"].as_array().unwrap().len(), 183);
        let mut sender =
            LegacySender::with_profile(CipherProfile::VindictusFrontend, 123457, 4096).unwrap();
        let mut receiver = LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, 4096);
        for c in profile["cases"].as_array().unwrap() {
            let b = bytes(&c["body"]);
            let typed = bytes(&c["typed_body"]);
            let expected = fields(c);
            let category = c["category"].as_i64().unwrap() as i32;
            assert!(decode_ingress(category, &b, 4096).unwrap() == expected);
            assert!(decode(&typed, 4096).unwrap() == expected);
            assert_eq!(encode(expected.as_ref(), 4096).unwrap(), typed);
            let packet = sender
                .encode(&Frame {
                    instance_id: 0,
                    category_id: category,
                    body: &b,
                })
                .unwrap();
            let mut frames = Vec::new();
            for chunk in packet.chunks(3) {
                frames.extend(receiver.push(chunk).unwrap());
            }
            assert_eq!(frames.len(), 1);
            assert!(
                decode_ingress(frames[0].category_id, &frames[0].body, 4096).unwrap() == expected
            );
            assert_eq!(receiver.buffered_len(), 0);
        }
    }
}
#[test]
fn empty_character_list_original_category_and_guid_are_registered() {
    assert!(
        REGISTERED_CLASSES
            .iter()
            .any(|c| c.category_id == CATEGORY && c.type_id == TYPE_ID)
    );
}
#[test]
fn empty_character_list_rejects_truncation_trailing_and_output_limits() {
    let f = fixture();
    for c in f["profiles"][0]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["mode"] == "registered-typed")
    {
        let body = bytes(&c["body"]);
        for n in 0..body.len() {
            assert!(decode(&body[..n], 4096).is_err());
        }
        assert!(matches!(
            decode(&body, body.len() - 1),
            Err(CharacterListWireError::Payload(PayloadError::TooLarge))
        ));
        let mut extra = body.clone();
        extra.push(0);
        assert!(matches!(
            decode(&extra, 4096),
            Err(CharacterListWireError::Payload(PayloadError::TrailingBytes))
        ));
        assert!(encode(fields(c).as_ref(), body.len() - 1).is_err());
    }
}
#[test]
fn empty_character_list_refuses_nonempty_summaries_unknown_envelopes_and_excess_party_states() {
    assert!(matches!(
        decode(&[1, 2], 100),
        Err(CharacterListWireError::Characters)
    ));
    let mut negative = vec![1];
    negative.extend(encode_integer(-1));
    assert!(matches!(
        decode(&negative, 100),
        Err(CharacterListWireError::Payload(
            PayloadError::NegativeLength
        ))
    ));
    assert!(matches!(
        decode_ingress(480, &[0], 100),
        Err(CharacterListWireError::Type)
    ));
    let mut object = uuid::Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21)
        .as_bytes()
        .to_vec();
    object.push(1);
    assert!(matches!(
        decode_ingress(0, &object, 100),
        Err(CharacterListWireError::Object)
    ));
    object[16] = 0;
    assert!(decode_ingress(0, &object, 100).unwrap().is_none());
    let m = Message {
        characters_present: true,
        max_free: 20,
        max_purchased: 0,
        max_premium: 0,
        prologue_played: 1,
        preset_used: 0,
        party_states: Some(vec![0; 1001]),
    };
    assert!(matches!(
        encode(Some(&m), 100000),
        Err(CharacterListWireError::Payload(PayloadError::TooLarge))
    ));
    let mut body = encode(
        Some(&Message {
            party_states: None,
            ..m
        }),
        100,
    )
    .unwrap();
    body.pop();
    body.extend(encode_integer(1002));
    assert!(matches!(
        decode(&body, 100),
        Err(CharacterListWireError::Payload(PayloadError::TooLarge))
    ));
}
