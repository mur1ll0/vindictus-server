use serde::Deserialize;
use vindictus_server::domain::{
    item_durability::{EquipmentDurability, initialize},
    item_stats::ItemStat,
};
#[derive(Deserialize)]
struct Fixture {
    cases: Cases,
}
#[derive(Deserialize)]
struct Cases {
    initial: Vec<Initial>,
    batches: Vec<Batch>,
    non_equipment: Snapshot,
}
#[derive(Deserialize)]
struct Initial {
    input: [i32; 5],
    output: Snapshot,
}
#[derive(Deserialize)]
struct Batch {
    input: [i32; 4],
    before: Snapshot,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Step {
    maximum: bool,
    value: i32,
    limit: i32,
    output: Snapshot,
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Snapshot {
    has: bool,
    maximum: i32,
    current: i32,
    reduced: i32,
    bonus: i32,
    updates: usize,
}
fn fixture() -> Cases {
    serde_json::from_str::<Fixture>(include_str!("fixtures/framework-item-durability.json"))
        .unwrap()
        .cases
}
fn stat(maximum: i32) -> ItemStat {
    ItemStat::from_raw(&[], &[], [maximum, 0, 0, 0]).unwrap()
}
fn snapshot(state: &EquipmentDurability, updates: usize) -> Snapshot {
    Snapshot {
        has: state.has_durability(),
        maximum: state.maximum(),
        current: state.current(),
        reduced: state.reduced(),
        bonus: state.bonus(),
        updates,
    }
}
#[test]
fn initialize_matches_source_maximum_clamps_wrapping_and_update_order() {
    let cases = fixture();
    assert_eq!(cases.initial.len(), 3125);
    for (i, case) in cases.initial.into_iter().enumerate() {
        let [source, modified, bonus, reduced, limit] = case.input;
        let (state, updates) =
            initialize(Some(&stat(source)), &stat(modified), bonus, reduced, limit).unwrap();
        assert_eq!(snapshot(&state, updates), case.output, "case {i}");
    }
}
#[test]
fn persisted_fields_and_setter_sequences_match_framework_including_getter_asymmetry() {
    let cases = fixture();
    assert_eq!(cases.batches.len(), 180);
    for (i, batch) in cases.batches.into_iter().enumerate() {
        let [source, modified, reduced, bonus] = batch.input;
        let mut state = EquipmentDurability::load(&stat(source), &stat(modified), reduced, bonus);
        let mut updates = 0;
        assert_eq!(snapshot(&state, updates), batch.before, "batch {i} load");
        for (j, step) in batch.steps.into_iter().enumerate() {
            updates += usize::from(if step.maximum {
                state.set_maximum(step.value, step.limit)
            } else {
                state.set_current(step.value)
            });
            assert_eq!(snapshot(&state, updates), step.output, "batch {i} step {j}");
        }
    }
}
#[test]
fn non_equipment_skips_initialize_without_manufacturing_an_equippable() {
    let cases = fixture();
    assert!(initialize(None, &stat(i32::MAX), i32::MAX, i32::MIN, -1).is_none());
    assert_eq!(
        cases.non_equipment,
        Snapshot {
            has: false,
            maximum: 0,
            current: 0,
            reduced: 0,
            bonus: 0,
            updates: 0
        }
    );
}
