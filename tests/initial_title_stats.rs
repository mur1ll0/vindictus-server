use serde_json::{Value, json};
mod fixture {
    include!("fixtures/initializer_profile_support.rs");
}
fn compare(arch: &str) {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-titles.json")).unwrap();
    assert_eq!(oracle[arch]["counterfactual_hits"], json!([1, 1]));
    for c in oracle[arch]["cases"].as_array().unwrap() {
        let mut v = fixture::synthetic();
        let row = v["features"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|v| v["key"] == "AccountTitleStatRatio")
            .unwrap();
        row["value"] = c["raw"].clone();
        let p = fixture::read(&v).unwrap();
        let result = p.account_title_stat_ratio();
        assert_eq!(c["reads"], 1);
        assert_eq!(c["provider_calls"], 0);
        if c["error"] == "overflow" {
            assert!(result.is_err());
            assert!(c["stats"].is_null());
        } else {
            assert_eq!(result.unwrap() as i64, c["integer"].as_i64().unwrap());
            assert!(c["stats"].as_array().unwrap().iter().all(|v| v == 0));
            assert_eq!(*p.base_stats([0; 7]).values(), [1, 2, 3, 4, 100, 5, 6]);
        }
    }
}
#[test]
fn initial_titles_framework_x86() {
    compare("x86");
}
#[test]
fn initial_titles_framework_x64() {
    compare("x64");
}
#[test]
fn initial_titles_require_projection_but_read_ratio_lazily() {
    let mut v = fixture::synthetic();
    v["features"]
        .as_array_mut()
        .unwrap()
        .retain(|x| x["key"] != "AccountTitleStatRatio");
    let p = fixture::read(&v).unwrap();
    assert!(p.account_title_stat_ratio().is_err());
    v["features"]
        .as_array_mut()
        .unwrap()
        .push(json!({"key":"AccountTitleStatRatio","value":null}));
    assert_eq!(
        fixture::read(&v)
            .unwrap()
            .account_title_stat_ratio()
            .unwrap(),
        0
    );
}
