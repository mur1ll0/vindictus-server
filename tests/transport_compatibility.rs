use serde::Deserialize;
use vindictus_server::protocol::transport::{
    LegacyReceiver, LegacySender, OwnedFrame, TransportError,
};
use vindictus_server::protocol::{Frame, ProtocolError};

#[derive(Deserialize)]
struct Fixture {
    transport: Vec<Vector>,
}
#[derive(Deserialize)]
struct Vector {
    salt: u32,
    frames: Vec<ExpectedFrame>,
    wire: Vec<u8>,
    decoded: Vec<DecodedFrame>,
}
#[derive(Deserialize)]
struct ExpectedFrame {
    instance_id: i64,
    original_instance_id: i64,
    category_id: i32,
    body: Vec<u8>,
}
#[derive(Deserialize)]
struct DecodedFrame {
    instance_id: i64,
    category_id: i32,
    body: Vec<u8>,
}
fn fixtures() -> Vec<Vector> {
    serde_json::from_str::<Fixture>(include_str!("fixtures/legacy-packet.json"))
        .expect("C# oracle")
        .transport
}
fn expected(vector: &Vector) -> Vec<OwnedFrame> {
    vector
        .frames
        .iter()
        .map(|f| OwnedFrame {
            instance_id: f.instance_id,
            category_id: f.category_id,
            body: f.body.clone(),
        })
        .collect()
}

#[test]
fn encrypted_packet_chain_matches_unchanged_recovered_csharp_sender() {
    for vector in fixtures() {
        let mut sender = LegacySender::new(vector.salt, 4096).unwrap();
        let mut wire = Vec::new();
        for f in &vector.frames {
            wire.extend(
                sender
                    .encode(&Frame {
                        instance_id: f.original_instance_id,
                        category_id: f.category_id,
                        body: &f.body,
                    })
                    .unwrap(),
            );
        }
        assert_eq!(wire, vector.wire, "salt={}", vector.salt);
    }
}

#[test]
fn receiver_matches_csharp_analyzer_for_fragmentation_and_concatenation() {
    for vector in fixtures() {
        let csharp: Vec<_> = vector
            .decoded
            .iter()
            .map(|f| OwnedFrame {
                instance_id: f.instance_id,
                category_id: f.category_id,
                body: f.body.clone(),
            })
            .collect();
        assert_eq!(
            csharp,
            expected(&vector),
            "recovered sender/analyzer disagree"
        );
        for chunk in [1, 2, 7, 8, 9, 19, 20, 255, 256, 257, vector.wire.len()] {
            let mut receiver = LegacyReceiver::new(4096);
            let mut frames = Vec::new();
            for data in vector.wire.chunks(chunk) {
                frames.extend(receiver.push(data).unwrap());
            }
            assert_eq!(frames, csharp, "salt={} chunk={chunk}", vector.salt);
            assert_eq!(receiver.buffered_len(), 0);
            assert!(receiver.push(&[]).unwrap().is_empty());
        }
    }
}

#[test]
fn receiver_handles_every_first_header_split_and_zeros_first_instance() {
    let vector = &fixtures()[0];
    for split in 0..=20 {
        let mut receiver = LegacyReceiver::new(4096);
        let mut frames = receiver.push(&vector.wire[..split]).unwrap();
        frames.extend(receiver.push(&vector.wire[split..]).unwrap());
        assert_eq!(frames, expected(vector), "split={split}");
        assert_eq!(frames[0].instance_id, 0);
        assert_ne!(frames[1].instance_id, 0);
    }
}

#[test]
fn rejected_outgoing_body_does_not_consume_cipher_or_first_packet_salt() {
    let frame = Frame {
        instance_id: 7,
        category_id: 1,
        body: &[42],
    };
    let mut sender = LegacySender::new(169, 1).unwrap();
    assert_eq!(
        sender.encode(&Frame {
            body: &[1, 2],
            ..frame
        }),
        Err(TransportError::Packet(ProtocolError::BodyTooLarge))
    );
    let fresh = LegacySender::new(169, 1).unwrap().encode(&frame).unwrap();
    assert_eq!(sender.encode(&frame).unwrap(), fresh);
}

#[test]
fn oversized_encrypted_body_closes_receiver_before_payload_allocation() {
    let wire = LegacySender::new(169, 4096)
        .unwrap()
        .encode(&Frame {
            instance_id: 0,
            category_id: 0,
            body: &[0; 2048],
        })
        .unwrap();
    let mut receiver = LegacyReceiver::new(1024);
    assert_eq!(
        receiver.push(&wire[..12]),
        Err(TransportError::Packet(ProtocolError::BodyTooLarge))
    );
    assert_eq!(receiver.buffered_len(), 0);
    assert_eq!(receiver.push(&[]), Err(TransportError::Closed));
}

#[test]
fn salt_range_matches_legacy_random_next_contract() {
    assert!(LegacySender::new(i32::MAX as u32, 1).is_ok());
    assert!(matches!(
        LegacySender::new(u32::MAX, 1),
        Err(TransportError::InvalidSalt)
    ));
}

#[derive(Deserialize)]
struct HeroesFixture {
    heroes_transport: Vec<Vector>,
}
fn heroes_fixtures() -> Vec<Vector> {
    serde_json::from_str::<HeroesFixture>(include_str!("fixtures/legacy-packet.json"))
        .unwrap()
        .heroes_transport
}
#[test]
fn heroes_sender_matches_actual_frontend_cipher_csharp_packets() {
    use vindictus_server::protocol::transport::CipherProfile;
    for vector in heroes_fixtures() {
        let mut sender =
            LegacySender::with_profile(CipherProfile::VindictusFrontend, vector.salt, 4096)
                .unwrap();
        let mut wire = Vec::new();
        for f in &vector.frames {
            wire.extend(
                sender
                    .encode(&Frame {
                        instance_id: f.original_instance_id,
                        category_id: f.category_id,
                        body: &f.body,
                    })
                    .unwrap(),
            );
        }
        assert_eq!(wire, vector.wire, "Heroes salt={}", vector.salt);
    }
}
#[test]
fn heroes_receiver_matches_frontend_analyzer_with_fragmented_and_concatenated_streams() {
    use vindictus_server::protocol::transport::CipherProfile;
    for vector in heroes_fixtures() {
        for chunk in [1, 7, 8, 9, 20, 256, vector.wire.len()] {
            let mut receiver = LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, 4096);
            let mut frames = Vec::new();
            for data in vector.wire.chunks(chunk) {
                frames.extend(receiver.push(data).unwrap());
            }
            assert_eq!(
                frames,
                expected(&vector),
                "Heroes salt={} chunk={chunk}",
                vector.salt
            );
            assert_eq!(receiver.buffered_len(), 0);
        }
    }
}
#[test]
fn generic_core_cipher_cannot_replace_the_frontend_cipher() {
    let vector = heroes_fixtures().remove(0);
    let mut sender = LegacySender::new(vector.salt, 4096).unwrap();
    let mut wire = Vec::new();
    for f in &vector.frames {
        wire.extend(
            sender
                .encode(&Frame {
                    instance_id: f.original_instance_id,
                    category_id: f.category_id,
                    body: &f.body,
                })
                .unwrap(),
        );
    }
    assert_ne!(wire, vector.wire);
}
