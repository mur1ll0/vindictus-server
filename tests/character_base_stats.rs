use serde::Deserialize;
use vindictus_server::domain::character_base_stats::BaseStats;
#[derive(Deserialize)]
struct Case {
    basis: [i32; 6],
    titles: [i32; 7],
    expected: [i32; 7],
}
#[derive(Deserialize)]
struct Fixture {
    profiles: Vec<String>,
    cases: Vec<Case>,
}
#[test]
fn base_stats_match_original_framework_constructor_get_stats_and_title_addition() {
    let f: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-character-base-stats.json")).unwrap();
    assert_eq!(f.profiles, ["Framework-x86", "Framework-x64"]);
    assert_eq!(f.cases.len(), 256);
    for c in f.cases {
        assert_eq!(*BaseStats::compose(c.basis, c.titles).values(), c.expected);
    }
}
