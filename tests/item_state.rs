use serde_json::{Value, json};
use vindictus_server::infrastructure::item_state::read_snapshot;
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/item-state-synthetic.json")).unwrap()
}
fn valid(v: &Value) -> bool {
    read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).is_ok()
}
#[test]
fn item_state_reader_requires_every_source_field_and_rejects_unknown_or_lossy_values() {
    let v = fixture();
    assert!(valid(&v));
    for table in ["items", "attributes", "equipment", "storage"] {
        for name in v[table][0].as_object().unwrap().keys() {
            let mut bad = v.clone();
            bad[table][0].as_object_mut().unwrap().remove(name);
            assert!(!valid(&bad), "missing {table}.{name}");
        }
    }
    for name in ["attribute_last_value", "storage_last_value"] {
        let mut bad = v.clone();
        bad.as_object_mut().unwrap().remove(name);
        assert!(!valid(&bad));
    }
    let bads = [
        ("items", "item_class_utf16", json!("00")),
        ("items", "item_class_utf16", json!("00".repeat(102))),
        ("items", "expire_ticks", json!("3155378976000000000")),
        ("items", "id", json!("01")),
        ("items", "collection", json!(256)),
        ("attributes", "attribute_bytes", json!("ff")),
        ("attributes", "value_bytes", json!("00".repeat(51))),
        ("storage", "expire_units300", json!("-1391644800001")),
        ("storage", "id", json!("21")),
        ("equipment", "part", json!(-1)),
    ];
    for (table, field, value) in bads {
        let mut bad = v.clone();
        bad[table][0][field] = value;
        assert!(!valid(&bad), "invalid {table}.{field}");
    }
    let mut bad = v.clone();
    bad["items"][0]["unknown"] = json!(0);
    assert!(!valid(&bad));
    let mut bad = v;
    bad["attributes"][1]["item_id"] = json!("1");
    bad["attributes"][1]["attribute_bytes"] = json!("647572");
    assert!(!valid(&bad), "source CI/padding unique key");
}
#[test]
fn item_state_reader_bounds_input_and_identity_without_guessing_a_counter() {
    let empty = json!({"format_version":1,"source":"study_heroes.item-state","attribute_last_value":null,"storage_last_value":null,"items":[],"attributes":[],"equipment":[],"storage":[]});
    assert!(valid(&empty));
    let mut bad = fixture();
    bad["attribute_last_value"] = Value::Null;
    assert!(!valid(&bad));
    bad["attribute_last_value"] = json!("9223372036854775807");
    assert!(!valid(&bad));
    let data = vec![b' '; 16 * 1024 * 1024 + 1];
    assert!(read_snapshot(data.as_slice()).is_err());
    let mut bad = fixture();
    let duplicate = bad["items"][0].clone();
    bad["items"].as_array_mut().unwrap().push(duplicate);
    assert!(!valid(&bad));
}
