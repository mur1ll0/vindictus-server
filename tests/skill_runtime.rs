use serde::Deserialize;
use vindictus_server::domain::skill_runtime::{Argument, Cache, CacheError};
fn text(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[derive(Deserialize)]
struct Row {
    key: Option<String>,
    value: Option<i32>,
}
#[derive(Deserialize)]
struct Case {
    initial_rank: Option<i32>,
    current_rank: Option<i32>,
    rank_from: i32,
    rank_to: i32,
    known_final: bool,
    arguments: Vec<Row>,
    error: bool,
    rank: i32,
    rank_sum: i32,
    arg_a: i32,
    arg_missing: i32,
}
#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}
#[test]
fn rank_and_argument_mutations_match_original_framework_including_partial_failure_and_overflow() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-skill-cache.json")).unwrap();
    assert_eq!(fixture.cases.len(), 120);
    for (i, c) in fixture.cases.iter().enumerate() {
        let skill = text("synthetic");
        let a = text("A");
        let missing = text("missing");
        let warm = [Argument::new(Some(a.clone()), Some(19)).unwrap()];
        let mut cache = Cache::default();
        if let Some(rank) = c.initial_rank {
            cache
                .rank_changed(&skill, Some(rank), 0, 0, Some(&warm))
                .unwrap();
        }
        cache.update_arguments(&skill, Some(&warm)).unwrap();
        let rows = c
            .arguments
            .iter()
            .map(|r| Argument::new(r.key.as_deref().map(text), r.value).unwrap())
            .collect::<Vec<_>>();
        let result = cache.rank_changed(
            &skill,
            c.current_rank,
            c.rank_from,
            c.rank_to,
            if c.known_final { Some(&rows) } else { None },
        );
        assert_eq!(result.is_err(), c.error, "case {i}");
        assert_eq!(
            (
                cache.base_rank(&skill),
                cache.rank_sum(),
                cache.argument(&skill, &a, -123),
                cache.argument(&skill, &missing, -9)
            ),
            (c.rank, c.rank_sum, c.arg_a, c.arg_missing),
            "case {i}"
        );
    }
}
#[test]
fn argument_dictionary_distinguishes_absent_empty_case_sensitive_and_invalid_provider_values() {
    let skill = text("synthetic");
    let a = text("A");
    let small = text("a");
    let mut cache = Cache::default();
    assert_eq!(cache.argument(&skill, &a, -1), -1);
    cache.update_arguments(&skill, Some(&[])).unwrap();
    assert_eq!(cache.argument(&skill, &a, -1), 0);
    cache
        .update_arguments(
            &skill,
            Some(&[
                Argument::new(Some(a.clone()), Some(7)).unwrap(),
                Argument::new(Some(small.clone()), Some(8)).unwrap(),
            ]),
        )
        .unwrap();
    assert_eq!(
        (
            cache.argument(&skill, &a, 0),
            cache.argument(&skill, &small, 0)
        ),
        (7, 8)
    );
    for rows in [
        vec![Argument::new(None, Some(1)).unwrap()],
        vec![Argument::new(Some(a.clone()), None).unwrap()],
    ] {
        assert_eq!(
            cache.update_arguments(&skill, Some(&rows)),
            Err(CacheError::Null)
        );
        assert_eq!(cache.argument(&skill, &a, 0), 7);
    }
    cache.update_arguments(&skill, None).unwrap();
    assert_eq!(cache.argument(&skill, &a, -1), -1);
    assert!(Argument::new(Some(vec![0; 1026]), Some(0)).is_err());
    assert!(Argument::new(Some(vec![0]), Some(0)).is_err());
    assert!(cache.update_arguments(&[0; 102], Some(&[])).is_err());
    let row = Argument::new(Some(a), Some(0)).unwrap();
    assert!(
        cache
            .update_arguments(&skill, Some(&vec![row; 4097]))
            .is_err()
    );
}
#[test]
fn normal_grant_updates_required_ap_before_rank_and_retains_cache_effects_after_argument_failure() {
    let skill = text("synthetic");
    let a = text("A");
    let mut cache = Cache::default();
    cache
        .after_grant(
            &skill,
            2,
            9,
            &[Argument::new(Some(a.clone()), Some(7)).unwrap()],
        )
        .unwrap();
    assert_eq!(
        (
            cache.base_rank(&skill),
            cache.rank_sum(),
            cache.required_ap(&skill)
        ),
        (2, 4, Some(9))
    );
    let result = cache.after_grant(
        &skill,
        3,
        0,
        &[
            Argument::new(Some(a.clone()), Some(1)).unwrap(),
            Argument::new(Some(a.clone()), Some(2)).unwrap(),
        ],
    );
    assert_eq!(result, Err(CacheError::Duplicate));
    assert_eq!(
        (
            cache.base_rank(&skill),
            cache.rank_sum(),
            cache.required_ap(&skill),
            cache.argument(&skill, &a, -1)
        ),
        (3, 8, Some(-1), 7)
    );
}
