use serde::Deserialize;
use vindictus_server::domain::item_stats::{ItemStat, StatError};
#[derive(Deserialize)]
struct Entry {
    name: Vec<u16>,
    value: i32,
}
#[derive(Deserialize)]
struct State {
    character: Vec<Entry>,
    effects: Vec<Entry>,
    fields: [i32; 4],
}
#[derive(Deserialize)]
struct Snapshot {
    character: Vec<Entry>,
    effects: Vec<Entry>,
    fields: [i32; 4],
    dictionary: Option<Vec<Entry>>,
    dictionary_error: bool,
    lookups: [i32; 5],
}
#[derive(Deserialize)]
struct Op {
    kind: String,
    name: Option<Vec<u16>>,
    value: i32,
    rhs: Option<State>,
}
#[derive(Deserialize)]
struct Step {
    operation: Op,
    error: Option<String>,
    result: Snapshot,
}
#[derive(Deserialize)]
struct Batch {
    initial: State,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Weight {
    bits: String,
    values: [i32; 18],
    max_durability: i32,
    armor_hp: i32,
    result: Snapshot,
}
#[derive(Deserialize)]
struct Cases {
    weights: Vec<Weight>,
    batches: Vec<Batch>,
}
#[derive(Deserialize)]
struct Fixture {
    cases: Cases,
}
fn raw(rows: &[Entry]) -> Vec<(Vec<u16>, i32)> {
    rows.iter().map(|r| (r.name.clone(), r.value)).collect()
}
fn build(s: &State) -> ItemStat {
    ItemStat::from_raw(&raw(&s.character), &raw(&s.effects), s.fields).unwrap()
}
fn snapshot(actual: &ItemStat, expected: &Snapshot) {
    assert_eq!(actual.fields(), expected.fields);
    assert!(
        actual
            .character_entries()
            .map(|e| (e.name, e.value))
            .eq(expected
                .character
                .iter()
                .map(|e| (e.name.as_slice(), e.value))),
        "character values/order differ"
    );
    assert!(
        actual
            .status_entries()
            .map(|e| (e.name, e.value))
            .eq(expected
                .effects
                .iter()
                .map(|e| (e.name.as_slice(), e.value))),
        "effect values/order differ"
    );
    let dict = actual.dictionary();
    assert_eq!(dict.is_err(), expected.dictionary_error);
    if let Some(rows) = &expected.dictionary {
        assert!(
            dict.unwrap()
                .iter()
                .map(|e| (e.name, e.value))
                .eq(rows.iter().map(|e| (e.name.as_slice(), e.value))),
            "dictionary values/order differ"
        );
    }
    let keys = [
        None,
        Some("STR"),
        Some("Weight"),
        Some("A"),
        Some("missing"),
    ];
    for (key, value) in keys.into_iter().zip(expected.lookups) {
        let k = key.map(|v| v.encode_utf16().collect::<Vec<_>>());
        assert_eq!(actual.get(k.as_deref()), value);
    }
}
#[test]
fn equipment_projection_and_unchecked_arithmetic_match_framework_profiles() {
    let f: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-item-stats.json")).unwrap();
    assert_eq!(f.cases.weights.len(), 258);
    assert_eq!(f.cases.batches.len(), 80);
    for w in f.cases.weights {
        let value = f64::from_bits(u64::from_str_radix(&w.bits, 16).unwrap());
        snapshot(
            &ItemStat::from_equipment(w.values, w.max_durability, value, w.armor_hp).unwrap(),
            &w.result,
        );
    }
    for batch in f.cases.batches {
        let mut stat = build(&batch.initial);
        assert_eq!(batch.steps.len(), 18);
        for step in batch.steps {
            let op = step.operation;
            let rhs = op.rhs.as_ref().map(build);
            let result = match op.kind.as_str() {
                "set" => stat.set(op.name.as_deref(), op.value),
                "status" => stat.add_status(op.name.as_deref(), op.value),
                "add" => stat.add(rhs.as_ref()),
                "replace" => stat.replace_stat_to_zero(rhs.as_ref()),
                "copy" => {
                    stat.copy_from(rhs.as_ref().unwrap());
                    Ok(())
                }
                "clear" => {
                    stat.clear();
                    Ok(())
                }
                _ => panic!("unknown fixture operation"),
            };
            let error = result.err().map(|e| match e {
                StatError::NullKey => "ArgumentNullException",
                _ => panic!("unexpected fixture error"),
            });
            assert_eq!(error, step.error.as_deref());
            snapshot(&stat, &step.result);
        }
    }
}
#[test]
fn defensive_union_bounds_are_atomic_and_copy_does_not_alias_source() {
    let rows = (0..256)
        .map(|i| (format!("s{i}").encode_utf16().collect(), i))
        .collect::<Vec<_>>();
    let mut full = ItemStat::from_raw(&rows, &rows, [1, 2, 3, 4]).unwrap();
    let before = full.clone();
    let key = vec![88];
    let rhs = ItemStat::from_raw(&[(key.clone(), 1)], &[], [1; 4]).unwrap();
    assert_eq!(full.add(Some(&rhs)), Err(StatError::Bound));
    assert!(full == before);
    assert_eq!(full.replace_stat_to_zero(Some(&rhs)), Err(StatError::Bound));
    assert!(full == before);
    let mut source = rhs.clone();
    let mut copy = ItemStat::default();
    copy.copy_from(&source);
    source.set(Some(&key), 99).unwrap();
    assert_eq!(copy.get(Some(&key)), 1);
    assert!(matches!(
        ItemStat::from_raw(&[(key.clone(), 1), (key.clone(), 2)], &[], [0; 4]),
        Err(StatError::Duplicate)
    ));
    assert_eq!(copy.set(Some(&vec![0; 257]), 1), Err(StatError::Bound));
    let effects = ItemStat::from_raw(&[], &[(key, 9)], [0; 4]).unwrap();
    assert_eq!(full.add(Some(&effects)), Err(StatError::Bound));
    assert!(full == before);
}
