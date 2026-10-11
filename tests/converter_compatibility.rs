use serde::Deserialize;
use uuid::Uuid;
use vindictus_server::protocol::converter::{
    ClassRegistration, ConverterError, decode_converter, encode_converter,
};
use vindictus_server::protocol::encode_integer;
use vindictus_server::protocol::server_profile::{
    REGISTERED_CLASSES, SOURCE_SHA256, USER_LOGIN_MESSAGE_CATEGORY,
};
#[derive(Deserialize)]
struct Fixture {
    source_sha256: String,
    converters: Vec<Vector>,
}
#[derive(Deserialize)]
struct Vector {
    entries: Option<Vec<Binding>>,
    category: i32,
    body: Vec<u8>,
}
#[derive(Deserialize)]
struct Binding {
    category: i32,
    guid: Uuid,
}
fn fixtures() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-converter.json")).unwrap()
}
fn expected(vector: &Vector) -> Option<Vec<ClassRegistration>> {
    vector.entries.as_ref().map(|entries| {
        entries
            .iter()
            .map(|e| ClassRegistration {
                category_id: e.category,
                type_id: e.guid,
            })
            .collect()
    })
}
#[test]
fn converter_encoding_matches_framework_serializer_null_empty_count_boundaries_and_full_table() {
    for vector in fixtures().converters {
        assert_eq!(vector.category, 0);
        let entries = expected(&vector);
        assert_eq!(
            encode_converter(entries.as_deref(), 102400).unwrap(),
            vector.body
        );
    }
}
#[test]
fn converter_decoding_matches_framework_guid_field_order_and_dictionary_counts() {
    for vector in fixtures().converters {
        let parsed = decode_converter(&vector.body, 102400, 1024).unwrap();
        assert_eq!(parsed.is_null(), vector.entries.is_none());
        assert_eq!(
            parsed.entries().collect::<Vec<_>>(),
            expected(&vector).unwrap_or_default()
        );
    }
}
#[test]
fn generated_original_server_registry_matches_entire_framework_converter_byte_for_byte() {
    let fixture = fixtures();
    assert_eq!(fixture.source_sha256, SOURCE_SHA256);
    let full = fixture.converters.last().unwrap();
    assert_eq!(full.entries.as_ref().unwrap().len(), 649);
    assert_eq!(REGISTERED_CLASSES.len(), 649);
    assert_eq!(
        encode_converter(Some(REGISTERED_CLASSES), 102400).unwrap(),
        full.body
    );
    assert_eq!(USER_LOGIN_MESSAGE_CATEGORY, 478);
    assert!(REGISTERED_CLASSES.iter().any(|e| e.category_id == 478
        && e.type_id == Uuid::parse_str("47f6f00c-0bd3-3059-afad-84a872ed7399").unwrap()));
}
#[test]
fn every_truncated_converter_prefix_is_rejected_without_remote_count_allocation() {
    for vector in fixtures().converters {
        for end in 0..vector.body.len() {
            assert!(
                decode_converter(&vector.body[..end], 102400, 1024).is_err(),
                "end={end}"
            );
        }
    }
}
#[test]
fn excessive_converter_count_is_rejected_at_header_before_waiting_for_body() {
    let mut body = vec![0xff; 16];
    body.extend(encode_integer(650));
    assert!(matches!(
        decode_converter(&body, 102400, 648),
        Err(ConverterError::TooLarge)
    ));
    assert!(matches!(
        decode_converter(&body, 102400, 649),
        Err(ConverterError::Truncated)
    ));
    let bytes = encode_converter(Some(REGISTERED_CLASSES), 102400).unwrap();
    assert_eq!(
        encode_converter(Some(REGISTERED_CLASSES), bytes.len() - 1),
        Err(ConverterError::TooLarge)
    );
    assert!(matches!(
        decode_converter(&bytes, bytes.len() - 1, 1024),
        Err(ConverterError::TooLarge)
    ));
}
#[test]
fn unsupported_types_negative_counts_and_trailing_data_are_rejected() {
    let mut body = vec![0xff; 16];
    body.extend(encode_integer(-1));
    assert!(matches!(
        decode_converter(&body, 102400, 1024),
        Err(ConverterError::NegativeCount)
    ));
    body[0] = 0;
    assert!(matches!(
        decode_converter(&body, 102400, 1024),
        Err(ConverterError::UnsupportedType)
    ));
    let mut body = encode_converter(None, 102400).unwrap();
    body.push(0);
    assert!(matches!(
        decode_converter(&body, 102400, 1024),
        Err(ConverterError::TrailingBytes)
    ));
}
