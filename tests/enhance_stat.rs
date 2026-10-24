use serde_json::Value;
use vindictus_server::domain::{
    character_stats::Arithmetic,
    enhance::{EnhanceStat, load, multiply},
    item_stats::ItemStat,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-enhance-stat.json")).unwrap()
}
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn text(v: &Value) -> Option<Vec<u16>> {
    v.as_str().map(units)
}
fn single(v: &Value) -> f32 {
    f32::from_bits(u32::try_from(v.as_u64().unwrap()).unwrap())
}
fn class_stat(stats: &Value) -> ItemStat {
    let s: Vec<i32> = stats
        .as_array()
        .unwrap()
        .iter()
        .map(|v| i32::try_from(v.as_i64().unwrap()).unwrap())
        .collect();
    ItemStat::from_raw(
        &[
            (units("ATK"), s[0]),
            (units("MATK"), s[1]),
            (units("DEF"), s[2]),
        ],
        &[],
        [s[3], s[4], 0, 0],
    )
    .unwrap()
}
fn rows(v: &Value) -> Option<Vec<EnhanceStat>> {
    v.as_array().map(|rows| {
        rows.iter()
            .map(|r| EnhanceStat {
                stat: text(&r["stat"]),
                value_type: r["value_type"].as_str().map(str::to_owned),
                value: single(&r["value_bits"]),
                base: text(&r["base"]),
            })
            .collect()
    })
}
fn snapshot(stat: &ItemStat) -> (Vec<(Vec<u16>, i32)>, [i32; 4]) {
    (
        stat.character_entries()
            .map(|e| (e.name.to_vec(), e.value))
            .collect(),
        stat.fields(),
    )
}
fn expected(v: &Value) -> (Vec<(Vec<u16>, i32)>, [i32; 4]) {
    let character = v["character"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e[0].as_array()
                    .unwrap()
                    .iter()
                    .map(|u| u16::try_from(u.as_u64().unwrap()).unwrap())
                    .collect(),
                i32::try_from(e[1].as_i64().unwrap()).unwrap(),
            )
        })
        .collect();
    let f: Vec<i32> = v["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| i32::try_from(x.as_i64().unwrap()).unwrap())
        .collect();
    (character, [f[0], f[1], f[2], f[3]])
}

#[test]
fn enhance_try_load_matches_the_recovered_entity_on_both_architectures() {
    let f = fixture();
    let five = [EnhanceStat {
        stat: Some(units("ATK")),
        value_type: Some("Add".into()),
        value: 7.0,
        base: Some(units("ATK")),
    }];
    for (name, arithmetic) in [
        ("x86", Arithmetic::FrameworkX86),
        ("x64", Arithmetic::FrameworkX64),
    ] {
        let cases = f[name]["cases"].as_array().unwrap();
        assert!(cases.len() > 500);
        for case in cases {
            let missing = case["missing"].as_bool().unwrap();
            let three = rows(&case["rows"]);
            let other = if case["part"].is_null() {
                0
            } else {
                i32::try_from(case["other"].as_i64().unwrap()).unwrap()
            };
            let info = |level: i32| -> Option<Option<&[EnhanceStat]>> {
                match (missing, level) {
                    (true, _) => None,
                    (false, 3) => Some(three.as_deref()),
                    (false, 5) => Some(Some(&five[..])),
                    _ => None,
                }
            };
            let stat = class_stat(&case["stats"]);
            let result = &case["result"];
            match load(case["level"].as_str(), other, info, &stat, arithmetic) {
                Ok((changed, loaded)) => {
                    assert!(result["error"].is_null(), "{name} {case}");
                    assert_eq!(
                        changed,
                        result["changed"].as_bool().unwrap(),
                        "{name} {case}"
                    );
                    assert_eq!(
                        i64::from(loaded.level),
                        result["level"].as_i64().unwrap(),
                        "{name} {case}"
                    );
                    assert_eq!(
                        snapshot(&loaded.stat),
                        expected(&result["stat"]),
                        "{name} {case}"
                    );
                }
                Err(_) => assert!(!result["error"].is_null(), "{name} {case}"),
            }
        }
    }
}

#[test]
fn enhance_multiply_grid_matches_each_architecture() {
    let f = fixture();
    assert!(f["grid_architecture_differences"].as_u64().unwrap() > 0);
    for (name, arithmetic) in [
        ("x86", Arithmetic::FrameworkX86),
        ("x64", Arithmetic::FrameworkX64),
    ] {
        let step = usize::try_from(f[name]["grid_step"].as_u64().unwrap()).unwrap();
        for row in f[name]["grid"].as_array().unwrap() {
            let m = single(&row["multiplier_bits"]);
            for (i, r) in row["results"].as_array().unwrap().iter().enumerate() {
                let n = i32::try_from(i * step).unwrap();
                assert_eq!(
                    i64::from(multiply(n, m, arithmetic)),
                    r.as_i64().unwrap(),
                    "{name} {n} × {m}"
                );
            }
        }
    }
}
