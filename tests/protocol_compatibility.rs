//! Differential checks against the unchanged C# Packet source recovered by ILSpy.
use serde::Deserialize;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY, Frame, ProtocolError, decode_frame, decode_integer, encode_integer,
};

#[derive(Deserialize)]
struct Oracle {
    integers: Vec<IntegerVector>,
    frames: Vec<FrameVector>,
}
#[derive(Deserialize)]
struct IntegerVector {
    value: i32,
    bytes: Vec<u8>,
}
#[derive(Deserialize)]
struct FrameVector {
    instance_id: i64,
    category_id: i32,
    body: Vec<u8>,
    bytes: Vec<u8>,
}
fn oracle() -> Oracle {
    serde_json::from_str(include_str!("fixtures/legacy-packet.json"))
        .expect("C# oracle fixtures must parse")
}

#[test]
fn encoded_integer_bytes_match_recovered_csharp_at_boundaries_and_seeded_values() {
    for vector in oracle().integers {
        assert_eq!(
            encode_integer(vector.value),
            vector.bytes,
            "value {}",
            vector.value
        );
    }
}
#[test]
fn decoded_integer_values_match_recovered_csharp() {
    for vector in oracle().integers {
        assert_eq!(
            decode_integer(&vector.bytes),
            Ok(Some((vector.value, vector.bytes.len())))
        );
    }
}
#[test]
fn encoded_frames_match_csharp_instance_category_and_length_layout() {
    for vector in oracle().frames {
        let frame = Frame {
            instance_id: vector.instance_id,
            category_id: vector.category_id,
            body: &vector.body,
        };
        assert_eq!(frame.encode(DEFAULT_MAX_BODY).unwrap(), vector.bytes);
    }
}
#[test]
fn decoded_frames_match_csharp_payloads() {
    for vector in oracle().frames {
        let expected = Frame {
            instance_id: vector.instance_id,
            category_id: vector.category_id,
            body: &vector.body,
        };
        assert_eq!(
            decode_frame(&vector.bytes, DEFAULT_MAX_BODY),
            Ok(Some((expected, vector.bytes.len())))
        );
    }
}
#[test]
fn every_partial_tcp_frame_waits_for_more_bytes() {
    for vector in oracle().frames {
        for prefix_length in 0..vector.bytes.len() {
            assert_eq!(
                decode_frame(&vector.bytes[..prefix_length], DEFAULT_MAX_BODY),
                Ok(None)
            );
        }
    }
}
#[test]
fn concatenated_tcp_frames_return_only_the_first_frame() {
    let fixtures = oracle().frames;
    let mut bytes = fixtures[1].bytes.clone();
    bytes.extend_from_slice(&fixtures[2].bytes);
    assert_eq!(
        decode_frame(&bytes, DEFAULT_MAX_BODY).unwrap().unwrap().1,
        fixtures[1].bytes.len()
    );
}
#[test]
fn overlong_integer_is_rejected() {
    assert_eq!(
        decode_integer(&[0xc0, 0x80]),
        Err(ProtocolError::NoncanonicalInteger)
    );
}
#[test]
fn malformed_continuation_is_rejected() {
    assert_eq!(
        decode_integer(&[0xc2, 0x00]),
        Err(ProtocolError::InvalidContinuation)
    );
}
#[test]
fn unsupported_integer_prefix_is_rejected() {
    assert_eq!(decode_integer(&[0x80]), Err(ProtocolError::InvalidPrefix));
}
#[test]
fn negative_body_length_is_rejected_before_allocation() {
    let mut bytes = vec![0; 8];
    bytes.push(1);
    bytes.extend(encode_integer(-1));
    assert_eq!(
        decode_frame(&bytes, DEFAULT_MAX_BODY),
        Err(ProtocolError::NegativeLength)
    );
}
#[test]
fn oversized_advertised_body_is_rejected_without_waiting_for_body() {
    let mut bytes = vec![0; 8];
    bytes.push(1);
    bytes.extend(encode_integer(i32::MAX));
    assert_eq!(
        decode_frame(&bytes, DEFAULT_MAX_BODY),
        Err(ProtocolError::BodyTooLarge)
    );
}
