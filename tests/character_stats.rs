use serde::Deserialize;
use std::collections::BTreeMap;
use vindictus_server::domain::{
    character_base_stats::BaseStats,
    character_stats::{
        self as stats, Arithmetic, Callbacks, Field, FinalResult, FinalState, Stats,
    },
    skill_runtime::{Argument, Cache},
};
#[derive(Deserialize)]
struct Case {
    input: [i32; 25],
    seed: i32,
    character: i32,
    rank_sum: i32,
    defs: BTreeMap<i32, i32>,
    output: [i32; 25],
    final_defs: BTreeMap<i32, i32>,
    destroyed: i32,
}
#[derive(Deserialize)]
struct Item {
    input: [i32; 25],
    output: [i32; 25],
    weight: f64,
    hp_count: usize,
    def_count: usize,
}
#[derive(Deserialize)]
struct Callback {
    case_index: usize,
    item_present: bool,
    previous_present: bool,
    override_present: bool,
    fail_at: usize,
    error: bool,
    trace: Vec<String>,
    output: Option<[i32; 25]>,
}
#[derive(Deserialize)]
struct Architecture {
    cases: Vec<Case>,
    items: Vec<Item>,
    callbacks: Vec<Callback>,
}
#[derive(Deserialize)]
struct Fixture {
    x86: Architecture,
    x64: Architecture,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-character-stats.json")).unwrap()
}
fn text(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn cache(c: &Case) -> Cache {
    let mut cache = Cache::default();
    for (n, (s, keys)) in [
        ("skill_str_mastery", vec!["str_max_bonus"]),
        ("skill_dex_mastery", vec!["dex_max_bonus"]),
        ("skill_int_mastery", vec!["int_max_bonus"]),
        ("skill_will_mastery", vec!["will_max_bonus"]),
        ("skill_stamina_mastery", vec!["stamina_max_bonus"]),
        ("skill_evy_magic_mastery", vec!["magic_atk_bonus"]),
        ("skill_defence_mastery", vec!["def_bonus"]),
        ("skill_hp_mastery", vec!["hp_max_bonus"]),
        ("skill_critical_hit", vec!["frequency_bonus", "dmg_bonus"]),
        ("skill_stone_skin", vec!["def_bonus"]),
    ]
    .into_iter()
    .enumerate()
    {
        let rows: Vec<_> = keys
            .into_iter()
            .map(|k| {
                let v = if c.seed == 0 {
                    0
                } else {
                    c.seed.wrapping_add(n as i32 * 7).wrapping_mul(match k {
                        "frequency_bonus" => 3,
                        "dmg_bonus" => 5,
                        _ => 1,
                    })
                };
                Argument::new(Some(text(k)), Some(v)).unwrap()
            })
            .collect();
        cache.update_arguments(&text(s), Some(&rows)).unwrap();
    }
    cache
        .rank_changed(&text("synthetic_rank"), Some(c.rank_sum), 0, 0, Some(&[]))
        .unwrap();
    cache
}
fn formula(a: Architecture, arithmetic: Arithmetic) {
    assert_eq!(a.cases.len(), 800);
    for (i, c) in a.cases.iter().enumerate() {
        let r = stats::apply_skills(
            &Stats::from_values(c.input),
            &c.defs,
            &cache(c),
            c.character != 0,
            arithmetic,
        )
        .unwrap();
        assert_eq!(r.stats.values(), &c.output, "formula case {i}");
        assert_eq!(r.armor_def, c.final_defs, "armor case {i}");
        assert_eq!(r.destroyed_def, c.destroyed, "stone skin case {i}");
    }
}
#[test]
fn framework_x86_formulas() {
    formula(fixture().x86, Arithmetic::FrameworkX86);
}
#[test]
fn framework_x64_formulas() {
    formula(fixture().x64, Arithmetic::FrameworkX64);
}
#[test]
fn empty_equipped_inventory_preserves_set_bonus() {
    let f = fixture();
    for a in [f.x86, f.x64] {
        assert_eq!(a.items.len(), 8);
        for c in a.items {
            let r = stats::empty_gear(&Stats::from_values(c.input));
            assert_eq!(r.stats.values(), &c.output);
            assert_eq!(r.weight, c.weight);
            assert_eq!(r.armor_hp.len(), c.hp_count);
            assert_eq!(r.armor_def.len(), c.def_count);
        }
    }
}
struct Observer {
    fail: usize,
    trace: Vec<String>,
    observed_int: Option<i32>,
}
impl Observer {
    fn event(&mut self, s: &str) -> Result<(), ()> {
        self.trace.push(s.into());
        if self.trace.len() == self.fail {
            Err(())
        } else {
            Ok(())
        }
    }
}
impl Callbacks for Observer {
    type Error = ();
    fn int_changed(&mut self, s: &Stats) -> Result<(), ()> {
        self.observed_int = Some(s.get(Field::Int));
        self.event("int")
    }
    fn stat_updated(&mut self) -> Result<(), ()> {
        self.event("stat")
    }
    fn microplay_updated(&mut self) -> Result<(), ()> {
        self.event("micro")
    }
}
#[test]
fn final_stat_store_override_ap_and_observer_failure_order() {
    let f = fixture();
    for (a, arithmetic) in [
        (f.x86, Arithmetic::FrameworkX86),
        (f.x64, Arithmetic::FrameworkX64),
    ] {
        assert_eq!(a.callbacks.len(), 32);
        for c in a.callbacks {
            let input = &a.cases[c.case_index];
            let old = c.previous_present.then(|| FinalResult {
                stats: Stats::from_values(input.input),
                armor_def: BTreeMap::new(),
                destroyed_def: 0,
            });
            let mut state = FinalState::restore(old);
            let mut observer = Observer {
                fail: c.fail_at,
                trace: vec![],
                observed_int: None,
            };
            let overrides = if c.override_present {
                vec![(Field::Int, 77), (Field::TownSpeed, -5)]
            } else {
                vec![]
            };
            let error = if c.item_present {
                let next = stats::apply_skills(
                    &Stats::from_values(input.input),
                    &BTreeMap::new(),
                    &cache(input),
                    input.character != 0,
                    arithmetic,
                )
                .unwrap();
                state.publish(next, &overrides, &mut observer).is_err()
            } else {
                false
            };
            assert_eq!(error, c.error);
            assert_eq!(observer.trace, c.trace);
            assert_eq!(state.result().map(|r| *r.stats.values()), c.output);
            if observer.observed_int.is_some() {
                assert_eq!(observer.observed_int, c.output.map(|v| v[5]));
            }
        }
    }
}
#[test]
fn base_and_equipment_status_addition_keeps_unchecked_int32() {
    let base = Stats::from_base(&BaseStats::compose([i32::MAX, 2, 3, 4, 5, 6], [0; 7]));
    assert_eq!(base.get(Field::Luck), 100);
    let mut extra = Stats::default();
    extra.set(Field::Str, 1);
    extra.set(Field::TownSpeed, -2);
    let total = base.add(&extra).add(&extra);
    assert_eq!(total.get(Field::Str), i32::MIN + 1);
    assert_eq!(total.get(Field::TownSpeed), -4);
}
#[test]
fn oversized_armor_profile_is_refused() {
    let armor = (0..65).map(|p| (p, 0)).collect();
    assert!(
        stats::apply_skills(
            &Stats::default(),
            &armor,
            &Cache::default(),
            false,
            Arithmetic::FrameworkX86
        )
        .is_err()
    );
}
