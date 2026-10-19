use serde_json::{Value, json};
use vindictus_server::infrastructure::user_captcha::read_snapshot;
fn input() -> Value {
    json!({"format_version":1,"source":"study_heroes.user-captcha","rows":[{"uid":i32::MIN,"punished_count":null,"failure_count":255,"last_failure_units300":"-1391644800000","last_succeed_units300":"76683386879999","condition":255}]})
}
fn accepts(v: &Value) -> bool {
    read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).is_ok()
}
#[test]
fn captcha_preserves_tinyint_nullable_and_exact_sql_datetime_boundaries() {
    assert!(accepts(&input()));
    let mut v = input();
    for (key, value) in [
        ("uid", json!(i32::MAX)),
        ("last_failure_units300", json!("-1")),
        ("last_succeed_units300", json!("1")),
        ("condition", Value::Null),
    ] {
        v["rows"][0][key] = value;
        assert!(accepts(&v));
    }
    v["rows"] = json!([]);
    assert!(accepts(&v));
}
#[test]
fn captcha_rejects_duplicates_missing_nullable_fields_and_noncanonical_values() {
    let v = input();
    for (key, value) in [
        ("uid", json!(2147483648_i64)),
        ("punished_count", json!(-1)),
        ("failure_count", json!(256)),
        ("condition", json!(true)),
        ("last_failure_units300", json!("-1391644800001")),
        ("last_succeed_units300", json!("76683386880000")),
        ("last_failure_units300", json!("+1")),
        ("last_failure_units300", json!(1)),
        ("last_failure_units300", json!("01")),
    ] {
        let mut bad = v.clone();
        bad["rows"][0][key] = value;
        assert!(!accepts(&bad), "{key}");
    }
    for key in [
        "punished_count",
        "failure_count",
        "last_failure_units300",
        "last_succeed_units300",
        "condition",
    ] {
        let mut bad = v.clone();
        bad["rows"][0].as_object_mut().unwrap().remove(key);
        assert!(!accepts(&bad));
    }
    let mut bad = v.clone();
    bad["rows"]
        .as_array_mut()
        .unwrap()
        .push(v["rows"][0].clone());
    assert!(!accepts(&bad));
    let mut bad = v;
    bad["rows"][0]["extra"] = json!(0);
    assert!(!accepts(&bad));
}
#[test]
fn captcha_rejects_oversized_wrong_format_and_source_without_echoing_private_data() {
    for bytes in [
        vec![b' '; 8 * 1024 * 1024 + 1],
        b"private-invalid-json".to_vec(),
    ] {
        let err = read_snapshot(bytes.as_slice()).err().unwrap().to_string();
        assert!(!err.contains("private-invalid"));
    }
    let mut v = input();
    v["format_version"] = json!(2);
    assert!(!accepts(&v));
    v["format_version"] = json!(1);
    v["source"] = json!("other");
    assert!(!accepts(&v));
}
