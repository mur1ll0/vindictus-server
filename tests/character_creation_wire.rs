use serde_json::{Value, json};
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    character_creation::{self as codec, Kind, Message, Template},
};
fn view_template(t: &Template<'_>) -> Value {
    json!({"character_class":t.character_class,"skin_color":t.skin_color,"shineness":t.shineness,"eye_color":t.eye_color,"height":t.height,"bust":t.bust,"painting_x":t.painting_x,"painting_y":t.painting_y,"painting_rotation":t.painting_rotation,"painting_size":t.painting_size,"items":t.items.as_ref().map(|items|items.iter().map(|i|i.as_ref().map(|i|json!({"item_class":i.item_class,"category":i.category,"c1":i.colors[0],"c2":i.colors[1],"c3":i.colors[2],"duration":i.duration,"price":i.price,"coupon":i.coupon.to_string()}))).collect::<Vec<_>>()),"body_shape":t.body_shape.as_ref().map(|shapes|shapes.iter().map(|(key,bits)|json!({"key":key,"bits":format!("{bits:08x}")})).collect::<Vec<_>>())})
}
fn bytes(value: &Value) -> Vec<u8> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_u64().unwrap() as u8)
        .collect()
}
#[test]
fn creation_codecs_match_original_dto_fields_bytes_and_all_three_envelopes_in_both_architectures() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/framework-character-creation-wire.json"
    ))
    .unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["cases"].as_array().unwrap().len(), 255);
        for case in profile["cases"].as_array().unwrap() {
            let kind = if case["kind"] == "create" {
                Kind::Create
            } else {
                Kind::Fail
            };
            let body = bytes(&case["body"]);
            let typed = bytes(&case["typed_body"]);
            let message = codec::decode_ingress(
                kind,
                case["category"].as_i64().unwrap() as i32,
                &body,
                DEFAULT_MAX_BODY,
            )
            .unwrap();
            assert_eq!(message.is_none(), case["is_null"].as_bool().unwrap());
            let fields = match &message {
                None => Value::Null,
                Some(Message::Fail(error)) => json!({"error":error}),
                Some(Message::Create {
                    name,
                    template,
                    preset,
                    premium,
                }) => {
                    json!({"name":name,"template":template.as_ref().map(view_template),"preset":preset,"premium":premium})
                }
            };
            assert_eq!(fields, case["fields"]);
            assert_eq!(
                codec::encode(kind, message.as_ref(), DEFAULT_MAX_BODY)
                    .unwrap()
                    .as_slice(),
                typed
            );
            for cut in 0..typed.len() {
                assert!(codec::decode(kind, &typed[..cut], DEFAULT_MAX_BODY).is_err());
            }
            let mut extra = typed.clone();
            extra.push(0);
            assert!(codec::decode(kind, &extra, DEFAULT_MAX_BODY).is_err());
            if !body.is_empty() {
                assert!(
                    codec::decode_ingress(
                        kind,
                        case["category"].as_i64().unwrap() as i32,
                        &body,
                        body.len() - 1
                    )
                    .is_err()
                );
            }
        }
    }
}
fn empty_template() -> Template<'static> {
    Template {
        character_class: 1,
        items: None,
        skin_color: 0,
        shineness: 0,
        eye_color: 0,
        height: 0,
        bust: 0,
        painting_x: 0,
        painting_y: 0,
        painting_rotation: 0,
        painting_size: 0,
        body_shape: None,
    }
}
#[test]
fn creation_codec_rejects_unbounded_collections_duplicate_shape_keys_and_wrong_envelopes() {
    use codec::CreationWireError;
    let mut template = empty_template();
    template.items = Some((0..129).map(|_| None).collect());
    let message = Message::Create {
        name: None,
        template: Some(template),
        preset: false,
        premium: false,
    };
    assert!(codec::encode(Kind::Create, Some(&message), DEFAULT_MAX_BODY).is_err());
    let mut oversized_items = vec![1, 0, 1, 1];
    oversized_items.extend(vindictus_server::protocol::encode_integer(130));
    assert!(codec::decode(Kind::Create, &oversized_items, DEFAULT_MAX_BODY).is_err());
    let mut template = empty_template();
    template.body_shape = Some((0..129).map(|i| (i, 0)).collect());
    let message = Message::Create {
        name: None,
        template: Some(template),
        preset: false,
        premium: false,
    };
    assert!(codec::encode(Kind::Create, Some(&message), DEFAULT_MAX_BODY).is_err());
    let mut template = empty_template();
    template.body_shape = Some(vec![(1, 0), (1, 0x80000000)]);
    let message = Message::Create {
        name: None,
        template: Some(template),
        preset: false,
        premium: false,
    };
    assert!(matches!(
        codec::encode(Kind::Create, Some(&message), DEFAULT_MAX_BODY),
        Err(CreationWireError::DuplicateKey)
    ));
    let mut template = empty_template();
    template.body_shape = Some(vec![(1, 0), (2, 0x80000000)]);
    let message = Message::Create {
        name: None,
        template: Some(template),
        preset: false,
        premium: false,
    };
    let mut body = codec::encode(Kind::Create, Some(&message), DEFAULT_MAX_BODY).unwrap();
    let second_key = body.len() - 10;
    body[second_key..second_key + 4].copy_from_slice(&1_i32.to_be_bytes());
    assert!(matches!(
        codec::decode(Kind::Create, &body, DEFAULT_MAX_BODY),
        Err(CreationWireError::DuplicateKey)
    ));
    assert!(codec::decode_ingress(Kind::Create, 123, &[0], DEFAULT_MAX_BODY).is_err());
    assert!(codec::decode_ingress(Kind::Create, 0, &[0; 17], DEFAULT_MAX_BODY).is_err());
    assert!(codec::encode(Kind::Create, Some(&Message::Fail(0)), DEFAULT_MAX_BODY).is_err());
}
