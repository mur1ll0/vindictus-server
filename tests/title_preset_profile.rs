use serde_json::{Value, json};
use vindictus_server::{
    domain::feature_policy::FeaturePolicy,
    infrastructure::{
        fiona_initializer_profile::Profile,
        fiona_title_preset::GoalProfile,
        item_catalog::{CatalogManifest, Package},
    },
};
mod fixture {
    include!("fixtures/initializer_profile_support.rs");
}
fn setup() -> (Profile, Package, Value) {
    let p = fixture::read(&fixture::synthetic()).unwrap();
    let catalog = Package {
        manifest: CatalogManifest {
            database_sha256: "a".repeat(64),
            config_sha256: "b".repeat(64),
            region: "synthetic".into(),
            override_region: None,
            equip_rows: 0,
            class_rows: 0,
            extra_rows: 0,
        },
        policy: FeaturePolicy::new([("CurrentVer".into(), Some("synthetic-current".into()))])
            .unwrap(),
        equips: vec![],
        classes: vec![],
        extras: vec![],
    };
    let code: Value =
        serde_json::from_str(include_str!("../src/domain/fiona_initializer_sources.json")).unwrap();
    let hash = p
        .fingerprint()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let raw = json!({"format_version":1,"source":"heroesContents.initial-title-goals","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"initializer_sha256":hash,"title_manager_sha256":code["titles"]["sha256"],"contents_sha256":code["contents"]["sha256"],"titles":[{"title_id":17,"goal_ids":[34,35]}]});
    (p, catalog, raw)
}
#[test]
fn title_goal_sidecar_binds_all_sources_and_exact_preset_scope() {
    let (p, c, v) = setup();
    let out = GoalProfile::read(serde_json::to_vec(&v).unwrap().as_slice(), &p, &c).unwrap();
    assert_eq!((out.title_count(), out.goal_count()), (1, 2));
    for (key, value) in [
        ("initializer_sha256", json!("c".repeat(64))),
        ("database_sha256", json!("c".repeat(64))),
        ("config_sha256", json!("c".repeat(64))),
        ("title_manager_sha256", json!("c".repeat(64))),
        ("contents_sha256", json!("c".repeat(64))),
        ("titles", json!([])),
    ] {
        let mut bad = v.clone();
        bad[key] = value;
        assert!(GoalProfile::read(serde_json::to_vec(&bad).unwrap().as_slice(), &p, &c).is_err());
    }
}
#[test]
fn title_goal_sidecar_rejects_ambiguous_ids_missing_and_oversize_input() {
    let (p, c, v) = setup();
    for titles in [
        json!([{"title_id":17,"goal_ids":[1,1]}]),
        json!([{"title_id":17,"goal_ids":[]},{"title_id":17,"goal_ids":[]}]),
        json!([{"title_id":18,"goal_ids":[]}]),
        json!([{"title_id":17}]),
    ] {
        let mut bad = v.clone();
        bad["titles"] = titles;
        assert!(GoalProfile::read(serde_json::to_vec(&bad).unwrap().as_slice(), &p, &c).is_err());
    }
    let mut bytes = serde_json::to_vec(&v).unwrap();
    bytes.resize(2 * 1024 * 1024 + 1, b' ');
    assert!(GoalProfile::read(bytes.as_slice(), &p, &c).is_err());
}
