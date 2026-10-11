use serde::Deserialize;
use vindictus_server::protocol::bootstrap::encode_initial_packets;
use vindictus_server::protocol::converter::decode_converter;
use vindictus_server::protocol::features::{
    FeatureEntry, FeatureMatrix, decode_feature_matrix, encode_feature_matrix,
};
use vindictus_server::protocol::login::PayloadError;
use vindictus_server::protocol::server_profile::SYNC_FEATURE_MATRIX_MESSAGE_CATEGORY;
use vindictus_server::protocol::transport::{CipherProfile, LegacyReceiver};
use vindictus_server::protocol::{Frame, encode_integer};
#[derive(Deserialize)]
struct Fixture {
    features: Vec<Vector>,
    bootstrap: Bootstrap,
}
#[derive(Deserialize)]
struct Vector {
    message_is_null: bool,
    entries: Option<Vec<Entry>>,
    category: i32,
    body: Vec<u8>,
}
#[derive(Deserialize)]
struct Entry {
    key: String,
    value: Option<String>,
}
#[derive(Deserialize)]
struct Bootstrap {
    salt: u32,
    entries: Vec<Entry>,
    wire: Vec<u8>,
    followup_wire: Vec<u8>,
}
fn fixtures() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-converter.json")).unwrap()
}
fn entries(values: &[Entry]) -> Vec<FeatureEntry<'_>> {
    values
        .iter()
        .map(|e| FeatureEntry {
            key: &e.key,
            value: e.value.as_deref(),
        })
        .collect()
}
#[test]
fn features_match_framework_serializer_null_message_null_dictionary_empty_utf8_and_count_boundaries()
 {
    for vector in fixtures().features {
        assert_eq!(vector.category, SYNC_FEATURE_MATRIX_MESSAGE_CATEGORY);
        let matrix = FeatureMatrix {
            features: vector.entries.as_deref().map(entries),
        };
        let expected = if vector.message_is_null {
            None
        } else {
            Some(matrix)
        };
        assert!(decode_feature_matrix(&vector.body, 102400, 1024).unwrap() == expected);
        assert_eq!(
            encode_feature_matrix(expected.as_ref(), 102400, 1024).unwrap(),
            vector.body
        );
    }
}
#[test]
fn every_incomplete_features_payload_is_rejected() {
    for vector in fixtures().features {
        for end in 0..vector.body.len() {
            assert!(
                decode_feature_matrix(&vector.body[..end], 102400, 1024).is_err(),
                "end={end}"
            );
        }
    }
}
#[test]
fn untrusted_feature_counts_are_bounded_before_entry_allocation() {
    let mut body = vec![1];
    body.extend(encode_integer(1026));
    assert!(matches!(
        decode_feature_matrix(&body, 102400, 1024),
        Err(PayloadError::TooLarge)
    ));
    assert!(matches!(
        decode_feature_matrix(&body, 102400, 2048),
        Err(PayloadError::Truncated)
    ));
    let body = fixtures().features.remove(4).body;
    assert!(matches!(
        decode_feature_matrix(&body, body.len() - 1, 1024),
        Err(PayloadError::TooLarge)
    ));
    assert!(matches!(
        decode_feature_matrix(&body, 102400, 127),
        Err(PayloadError::TooLarge)
    ));
}
#[test]
fn duplicate_and_null_dictionary_keys_are_rejected_like_framework_dictionary() {
    assert!(matches!(
        decode_feature_matrix(&[1, 2, 0, 0], 1024, 16),
        Err(PayloadError::NullKey)
    ));
    // Presence, two entries, empty string key + null value, repeated empty key + null.
    assert!(matches!(
        decode_feature_matrix(&[1, 3, 1, 0, 1, 0], 1024, 16),
        Err(PayloadError::DuplicateKey)
    ));
    let value = FeatureMatrix {
        features: Some(vec![
            FeatureEntry {
                key: "",
                value: None,
            },
            FeatureEntry {
                key: "",
                value: None,
            },
        ]),
    };
    assert_eq!(
        encode_feature_matrix(Some(&value), 1024, 16),
        Err(PayloadError::DuplicateKey)
    );
}
#[test]
fn whole_bootstrap_matches_framework_registration_features_and_heroes_cipher() {
    let fixture = fixtures().bootstrap;
    let features = FeatureMatrix {
        features: Some(entries(&fixture.entries)),
    };
    let (mut sender, packets) =
        encode_initial_packets(fixture.salt, Some(&features), 102400, 1024).unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets.concat(), fixture.wire);
    let mut receiver = LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, 102400);
    let mut received = Vec::new();
    for chunk in fixture.wire.chunks(7) {
        received.extend(receiver.push(chunk).unwrap());
    }
    assert_eq!(received.len(), 2);
    assert_eq!(received[0].category_id, 0);
    assert_eq!(
        decode_converter(&received[0].body, 102400, 1024)
            .unwrap()
            .entries()
            .count(),
        649
    );
    assert_eq!(
        received[1].category_id,
        SYNC_FEATURE_MATRIX_MESSAGE_CATEGORY
    );
    assert!(decode_feature_matrix(&received[1].body, 102400, 1024).unwrap() == Some(features));
    let body = received[1].body.clone();
    assert_eq!(
        sender
            .encode(&Frame {
                instance_id: 123456789,
                category_id: SYNC_FEATURE_MATRIX_MESSAGE_CATEGORY,
                body: &body
            })
            .unwrap(),
        fixture.followup_wire
    );
}
#[test]
fn bootstrap_omits_empty_feature_sync_as_the_recovered_server_does() {
    for matrix in [
        None,
        Some(FeatureMatrix { features: None }),
        Some(FeatureMatrix {
            features: Some(vec![]),
        }),
    ] {
        let (_, packets) = encode_initial_packets(169, matrix.as_ref(), 102400, 1024).unwrap();
        assert_eq!(packets.len(), 1);
    }
}
