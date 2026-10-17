use serde::Deserialize;
use vindictus_server::domain::{
    character_load::{BaseLoad, EmptyGear, Snapshot, State},
    character_stats::{Arithmetic, Field, Stats},
    skill_runtime::{Argument, Cache},
};
#[derive(Deserialize)]
struct Case {
    base_bits: String,
    present: i32,
    bonus: i32,
    old_gear: bool,
    new_gear: bool,
    new_set: bool,
    set_value: i32,
    old_item_present: bool,
    base_fail: bool,
    item_fail: bool,
    error: bool,
    stored_set: i32,
    gear_known: bool,
    item_value: Option<i32>,
    weight_bits: String,
    medium_bits: String,
    heavy_bits: String,
    penalty: i32,
    hp_present: bool,
    def_present: bool,
    item_calls: i32,
    base_calls: i32,
}
#[derive(Deserialize)]
struct Fixture {
    x86: Vec<Case>,
    x64: Vec<Case>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-character-load.json")).unwrap()
}
fn bits(v: &str) -> u64 {
    u64::from_str_radix(v, 16).unwrap()
}
fn stat(v: i32) -> Stats {
    let mut s = Stats::default();
    s.set(Field::Str, v);
    s
}
fn text(v: &str) -> Vec<u8> {
    v.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn verify(cases: Vec<Case>, arithmetic: Arithmetic) {
    assert_eq!(cases.len(), 640);
    for (i, c) in cases.into_iter().enumerate() {
        assert!(!c.item_fail, "unsupported item helper injection");
        let mut cache = Cache::default();
        cache
            .update_arguments(
                &text("skill_weight_mastery"),
                Some(&[
                    Argument::new(Some(text("max_weight_increament_bonus")), Some(c.bonus))
                        .unwrap(),
                ]),
            )
            .unwrap();
        let mut state = State::restore(Snapshot {
            set_bonus: stat(-7),
            equipped_empty: c.old_gear,
            item_stats: c.old_item_present.then(|| stat(-99)),
            weight: -2.0,
            medium: -3.0,
            heavy: -4.0,
            penalty: -5,
            armor_hp: None,
            armor_def: None,
        });
        let mut calls = 0;
        let result = state.recalculate(
            c.new_set.then(|| stat(c.set_value)),
            c.new_gear.then_some(EmptyGear),
            &cache,
            arithmetic,
            || {
                calls += 1;
                if c.base_fail {
                    return Err(());
                }
                Ok(BaseLoad::from_bits(if c.present == 2 {
                    bits(&c.base_bits)
                } else {
                    (-1.0_f64).to_bits()
                })
                .unwrap())
            },
        );
        let s = state.snapshot();
        assert_eq!(result.is_err(), c.error, "failure {i}");
        assert_eq!(calls, c.base_calls, "base query calls {i}");
        assert_eq!(
            s.set_bonus.get(Field::Str),
            c.stored_set,
            "set replacement {i}"
        );
        assert_eq!(s.equipped_empty, c.gear_known, "gear retained {i}");
        assert_eq!(
            s.item_stats.as_ref().map(|s| s.get(Field::Str)),
            c.item_value,
            "item stat {i}"
        );
        assert_eq!(s.weight.to_bits(), bits(&c.weight_bits), "weight {i}");
        assert_eq!(s.medium.to_bits(), bits(&c.medium_bits), "medium {i}");
        assert_eq!(s.heavy.to_bits(), bits(&c.heavy_bits), "heavy {i}");
        assert_eq!(s.penalty, c.penalty, "penalty {i}");
        assert_eq!(s.armor_hp.is_some(), c.hp_present, "hp map {i}");
        assert_eq!(s.armor_def.is_some(), c.def_present, "def map {i}");
        assert_eq!(
            c.item_calls,
            i32::from(s.equipped_empty),
            "empty item evaluation {i}"
        );
    }
}
#[test]
fn framework_x86_empty_item_load_transitions() {
    verify(fixture().x86, Arithmetic::FrameworkX86);
}
#[test]
fn framework_x64_empty_item_load_transitions() {
    verify(fixture().x64, Arithmetic::FrameworkX64);
}
#[test]
fn zero_threshold_keeps_clr_nan_conversion_instead_of_inventing_zero() {
    for arithmetic in [Arithmetic::FrameworkX86, Arithmetic::FrameworkX64] {
        for v in [0.0_f64, -0.0] {
            let mut state = State::new(Stats::default());
            state
                .recalculate(None, Some(EmptyGear), &Cache::default(), arithmetic, || {
                    Ok::<_, ()>(BaseLoad::from_bits(v.to_bits()).unwrap())
                })
                .unwrap();
            assert_eq!(state.snapshot().penalty, i32::MIN);
            // Missing argument dictionary contributes zero, including signed zero rules.
            assert_eq!(state.snapshot().medium.to_bits(), 0.0_f64.to_bits());
        }
    }
}
#[test]
fn finite_source_double_admission_preserves_bits_and_refuses_nonfinite_inputs() {
    for v in [-1.0_f64, -0.0, 0.1, f64::MIN_POSITIVE, f64::MAX, -f64::MAX] {
        assert_eq!(
            BaseLoad::from_bits(v.to_bits()).unwrap().value().to_bits(),
            v.to_bits()
        );
    }
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(BaseLoad::from_bits(v.to_bits()).is_err());
    }
}
