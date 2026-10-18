use serde_json::{Value, json};
use vindictus_server::{
    domain::{
        initial_vocation::{Details, Loaded, Skill},
        legacy_user_data::LegacyBytes,
    },
    infrastructure::character_vocation,
};
fn bytes(h: &str) -> Vec<u8> {
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
        .collect()
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn compare(arch: &str) {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-vocation.json")).unwrap();
    let cases = fixture[arch].as_array().unwrap();
    assert_eq!(cases.len(), 128);
    for c in cases {
        let input = c["input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| Skill {
                row_id: s["row_id"].as_i64().unwrap(),
                id: LegacyBytes::from_bytes(bytes(s["id_utf16"].as_str().unwrap())).unwrap(),
                rank: s["rank"].as_i64().unwrap() as i32,
            })
            .collect();
        let details = if c["absent"].as_bool().unwrap() {
            None
        } else {
            Some(Details {
                class: -2,
                level: 0,
                exp: i32::MIN,
                last_transform_ticks: None,
            })
        };
        let out = Loaded::load(details, input).unwrap();
        let actual = json!({"skills":out.skills().map(|s|json!({"row_id":s.row_id,"id_utf16":hex(s.id.as_bytes()),"rank":s.rank})).collect::<Vec<_>>(),"used":out.used_points(),"changed":out.changed_skills().len(),"vocation_changed":out.vocation_changed(),"transform_level":out.transform_level()});
        assert_eq!(actual, c["expected"]);
        assert_eq!(out.duplicates() as u64, c["duplicates"].as_u64().unwrap());
        assert_eq!(out.details().is_none(), c["absent"].as_bool().unwrap());
    }
}
#[test]
fn initial_vocation_framework_x86() {
    compare("x86")
}
#[test]
fn initial_vocation_framework_x64() {
    compare("x64")
}
#[test]
fn vocation_snapshot_rejects_loss_or_orphans_and_preserves_nullable_tick_extremes() {
    let base = json!({"format_version":1,"source":"study_heroes.vocation-state","source_last_value":"100","vocations":[{"cid":"1","vocation_class":-2,"vocation_level":0,"vocation_exp":i32::MIN,"last_transform_ticks":"3155378975999999999"}],"skills":[{"row_id":"9","cid":"1","skill_utf16":"00d8","rank":i32::MAX}]});
    let parse = |v: &Value| {
        let b = serde_json::to_vec(v).unwrap();
        character_vocation::read_snapshot(b.as_slice())
    };
    assert!(parse(&base).is_ok());
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("source_last_value");
    assert!(parse(&missing).is_err());

    for val in [json!("-1"), json!("3155378976000000000")] {
        let mut v = base.clone();
        v["vocations"][0]["last_transform_ticks"] = val;
        assert!(parse(&v).is_err());
    }
    let mut v = base.clone();
    v["vocations"][0]["last_transform_ticks"] = Value::Null;
    assert!(parse(&v).is_ok());
    for (field, val) in [
        ("row_id", json!("101")),
        ("cid", json!("2")),
        ("skill_utf16", json!("0")),
        ("skill_utf16", json!("4100".repeat(51))),
    ] {
        let mut v = base.clone();
        v["skills"][0][field] = val;
        assert!(parse(&v).is_err());
    }
    let mut v = base.clone();
    let duplicate = v["skills"][0].clone();
    v["skills"].as_array_mut().unwrap().push(duplicate);
    assert!(parse(&v).is_err());
}
