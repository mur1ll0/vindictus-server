use serde::Deserialize;
use vindictus_server::domain::item_colors::*;
#[derive(Deserialize)]
struct Fixture {
    cases: Cases,
}
#[derive(Deserialize)]
struct Cases {
    raw: Vec<Raw>,
    conversions: Vec<Conversion>,
    rng: Vec<Rng>,
    coords: Vec<Coord>,
    cases: Vec<Set>,
}
#[derive(Deserialize)]
struct Raw {
    data: String,
    colors: Vec<i32>,
}
#[derive(Deserialize)]
struct Conversion {
    input: i32,
    rgb: [u8; 3],
    roundtrip: i32,
}
#[derive(Deserialize)]
struct Rng {
    seed: i32,
    maxima: Vec<i32>,
    output: Vec<Option<i32>>,
}
#[derive(Deserialize)]
struct Coord {
    name: Option<String>,
    x: i32,
    y: i32,
    value: Option<i32>,
    error: Option<String>,
}
#[derive(Deserialize)]
struct Set {
    requests: [Option<i32>; 3],
    equippable: bool,
    look: i32,
    names: [Option<String>; 3],
    seed: i32,
    colors: [i32; 3],
    updates: usize,
    error: Option<String>,
    next: i32,
}
fn fixture() -> Cases {
    serde_json::from_str::<Fixture>(include_str!("fixtures/framework-item-colors.json"))
        .unwrap()
        .cases
}
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn bytes(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn materials() -> Materials {
    let mut p = Materials::default();
    for (name, data) in [
        (
            "full",
            Some(vec![0, 1, 2, 253, 254, 255, 17, 34, 51, 1, 3, 5]),
        ),
        ("empty", Some(vec![])),
        ("short", Some(vec![9, 8])),
        ("tail", Some(vec![0, 255, 17, 99])),
        ("missing", None),
    ] {
        p.insert(&units(name), data.as_deref()).unwrap();
    }
    p
}
#[test]
fn raw_rgb_triples_trailing_bytes_and_signed_conversions_match_framework() {
    let f = fixture();
    assert_eq!(f.raw.len(), 21);
    for row in f.raw {
        assert_eq!(
            Material::from_raw(&bytes(&row.data)).unwrap().colors(),
            row.colors
        );
    }
    for row in f.conversions {
        assert_eq!(int_to_rgb(row.input), row.rgb);
        assert_eq!(rgb_to_int(row.rgb), row.roundtrip);
    }
}
#[test]
fn shared_framework_generator_matches_extreme_seeds_and_zero_negative_maximum_consumption() {
    let rows = fixture().rng;
    assert_eq!(rows.len(), 9);
    for row in rows {
        let mut rng = ColorRandom::from_seed(row.seed);
        for (i, output) in row.output.into_iter().enumerate() {
            let result = rng.next(row.maxima[i % row.maxima.len()]);
            match output {
                Some(expected) => {
                    assert_eq!(result.unwrap(), expected, "seed {} step {i}", row.seed)
                }
                None => assert_eq!(result, Err(ColorError::RandomMaximum)),
            }
        }
    }
}
#[test]
fn coordinate_wrapping_signed_remainder_null_missing_and_empty_match_framework() {
    let rows = fixture().coords;
    assert_eq!(rows.len(), 384);
    let pool = materials();
    for row in rows {
        let name = row.name.as_deref().map(units);
        let actual = pool.pick_at(name.as_deref(), row.x, row.y);
        match row.error.as_deref() {
            None => assert_eq!(actual.unwrap(), row.value.unwrap()),
            Some("DivideByZeroException") => assert_eq!(actual, Err(ColorError::EmptyCoordinates)),
            Some("ArgumentOutOfRangeException") => {
                assert_eq!(actual, Err(ColorError::CoordinateIndex))
            }
            other => panic!("unhandled synthetic source error {other:?}"),
        }
    }
}
#[test]
fn set_color_order_partial_failure_null_asymmetry_and_shared_draws_match_framework() {
    let rows = fixture().cases;
    assert_eq!(rows.len(), 3000);
    let pool = materials();
    for (i, row) in rows.into_iter().enumerate() {
        let names = row.names.each_ref().map(|n| n.as_deref().map(units));
        let look = match row.look {
            0 => Look::Absent,
            1 => Look::WithoutEquipment,
            2 => Look::Equipment(names.each_ref().map(|n| n.as_deref())),
            _ => panic!("synthetic look"),
        };
        let mut colors = [7, 8, 9];
        let mut rng = ColorRandom::from_seed(row.seed);
        let result = set_colors(
            &mut colors,
            row.requests,
            row.equippable,
            look,
            &pool,
            &mut rng,
        );
        match row.error.as_deref() {
            None => assert_eq!(usize::from(result.unwrap()), row.updates, "case {i}"),
            Some("NullReferenceException") => {
                assert_eq!(result, Err(ColorError::MissingLookEquipment))
            }
            other => panic!("source error {other:?}"),
        }
        assert_eq!(colors, row.colors, "case {i}");
        assert_eq!(rng.next(1000000).unwrap(), row.next, "case {i} draws");
    }
}
#[test]
fn bounds_and_unbound_profiles_fail_explicitly_without_synthesizing_white() {
    assert_eq!(
        Material::from_raw(&vec![0; 196610]).unwrap().colors().len(),
        65536
    );
    assert!(matches!(
        Material::from_raw(&vec![0; 196611]),
        Err(ColorError::Bound)
    ));
    let mut pool = materials();
    assert_eq!(pool.insert(&units("full"), None), Err(ColorError::Bound));
    assert_eq!(
        pool.pick(Some(&units("unlisted")), &mut ColorRandom::from_seed(1)),
        Err(ColorError::UnboundMaterial)
    );
}
