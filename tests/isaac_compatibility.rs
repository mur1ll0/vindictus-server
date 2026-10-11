use serde::Deserialize;
use vindictus_server::protocol::isaac::LegacyIsaac;

#[derive(Deserialize)]
struct Fixture {
    isaac: Vec<Vector>,
}
#[derive(Deserialize)]
struct Vector {
    seed: i32,
    words: Vec<i32>,
}

fn fixtures() -> Fixture {
    serde_json::from_str(include_str!("fixtures/legacy-packet.json")).expect("C# oracle fixture")
}

#[test]
fn isaac_matches_recovered_csharp_across_three_output_blocks_and_signed_seeds() {
    let fixture = fixtures();
    assert_eq!(fixture.isaac.len(), 6, "six signed seed cases required");
    for vector in fixture.isaac {
        assert_eq!(
            vector.words.len(),
            600,
            "fixture must cross two refill boundaries"
        );
        let mut generator = LegacyIsaac::seeded(vector.seed);
        for (index, expected) in vector.words.into_iter().enumerate() {
            assert_eq!(
                generator.next_word(),
                expected,
                "seed={} word={index}",
                vector.seed
            );
        }
    }
}

#[test]
fn byte_xor_uses_one_word_per_byte_and_matches_the_recovered_reference() {
    for vector in fixtures().isaac {
        let mut input: Vec<u8> = (0..600).map(|i| (i % 256) as u8).collect();
        let expected: Vec<u8> = input
            .iter()
            .zip(&vector.words)
            .map(|(byte, word)| byte ^ *word as u8)
            .collect();
        LegacyIsaac::seeded(vector.seed).xor_bytes(&mut input);
        assert_eq!(input, expected, "seed={}", vector.seed);
    }
}

#[test]
fn stream_state_survives_arbitrary_chunks_and_empty_calls() {
    let mut whole: Vec<u8> = (0..600).map(|i| (i % 256) as u8).collect();
    let mut chunked = whole.clone();
    let mut generator = LegacyIsaac::seeded(-1);
    LegacyIsaac::seeded(-1).xor_bytes(&mut whole);
    generator.xor_bytes(&mut []);
    for chunk in chunked.chunks_mut(37) {
        generator.xor_bytes(chunk);
    }
    assert_eq!(chunked, whole);
    LegacyIsaac::seeded(-1).xor_bytes(&mut chunked);
    assert_eq!(
        chunked,
        (0..600).map(|i| (i % 256) as u8).collect::<Vec<_>>()
    );
}

#[derive(Deserialize)]
struct HeroesFixture {
    heroes_isaac: Vec<HeroesVector>,
}
#[derive(Deserialize)]
struct HeroesVector {
    seed: u32,
    words: Vec<u32>,
}
#[test]
fn heroes_isaac_matches_frontend_csharp_for_seed_slots_overflow_and_three_blocks() {
    let fixture: HeroesFixture =
        serde_json::from_str(include_str!("fixtures/legacy-packet.json")).unwrap();
    assert_eq!(fixture.heroes_isaac.len(), 8);
    for vector in fixture.heroes_isaac {
        assert_eq!(vector.words.len(), 600);
        let mut generator = LegacyIsaac::heroes_seeded(vector.seed);
        for (index, expected) in vector.words.into_iter().enumerate() {
            assert_eq!(
                generator.next_word() as u32,
                expected,
                "Heroes salt={} word={index}",
                vector.seed
            );
        }
    }
}
#[test]
fn heroes_seed_initialization_is_distinct_from_core_for_nonzero_salt() {
    let mut core = LegacyIsaac::seeded(169);
    let mut heroes = LegacyIsaac::heroes_seeded(169);
    let core_words: Vec<_> = (0..600).map(|_| core.next_word()).collect();
    let heroes_words: Vec<_> = (0..600).map(|_| heroes.next_word()).collect();
    assert_ne!(heroes_words, core_words);
}
