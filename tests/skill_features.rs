use serde::Deserialize;
use vindictus_server::domain::feature_policy::FeaturePolicy;
#[derive(Deserialize)]
struct Fixture {
    pairs: Vec<Pair>,
}
#[derive(Deserialize)]
struct Pair {
    current: String,
    value: String,
    enabled: bool,
}
#[test]
fn skill_rank_version_pairs_match_original_framework_comparisons() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-skill-features.json")).unwrap();
    assert_eq!(fixture.pairs.len(), 1);
    for p in fixture.pairs {
        let policy = FeaturePolicy::new([
            ("CurrentVer".to_owned(), Some(p.current)),
            ("candidate".to_owned(), Some(p.value)),
        ])
        .unwrap();
        assert_eq!(policy.is_enabled(Some("candidate")).unwrap(), p.enabled);
    }
}
