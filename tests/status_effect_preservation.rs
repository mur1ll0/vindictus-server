use serde_json::{Value, json};
use vindictus_server::infrastructure::character_status_effects::{StatusError, read_snapshot};
fn input() -> Value {
    json!({"format_version":1,"source":"study_heroes.status-effects","source_last_value":"107","effects":[{"row_id":"2","cid":"1","type_utf16":"00d8","level":i32::MIN,"remain_time":null,"expire_ticks":null,"remain_micro":null},{"row_id":"9","cid":"1","type_utf16":"00d8","level":i32::MAX,"remain_time":i32::MIN,"expire_ticks":"3155378975999999999","remain_micro":255}]})
}
fn read(v: &Value) -> Result<(), StatusError> {
    read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).map(|_| ())
}
#[test]
fn status_snapshot_accepts_raw_utf16_nullable_fields_and_empty_source() {
    assert!(read(&input()).is_ok());
    let mut v = input();
    v["effects"][0]["type_utf16"] = json!("");
    assert!(read(&v).is_ok());
    v["effects"] = json!([]);
    v["source_last_value"] = Value::Null;
    assert!(read(&v).is_ok());
}
#[test]
fn status_snapshot_refuses_invalid_fields_duplicates_ranges_and_missing_nullables() {
    for phase in 0..17 {
        let mut v = input();
        match phase {
            0 => v["effects"][1]["row_id"] = json!("2"),
            1 => v["effects"][0]["row_id"] = json!("108"),
            2 => v["effects"][0]["row_id"] = json!("0"),
            3 => v["source_last_value"] = Value::Null,
            4 => v["source_last_value"] = json!(i64::MAX.to_string()),
            5 => v["effects"][0]["expire_ticks"] = json!("3155378976000000000"),
            6 => v["effects"][0]["expire_ticks"] = json!("-1"),
            7 => v["effects"][0]["remain_micro"] = json!(256),
            8 => v["effects"][0]["remain_micro"] = json!(-1),
            9 => v["effects"][0]["remain_time"] = json!(2147483648i64),
            10 => v["effects"][0]["type_utf16"] = json!("a"),
            11 => v["effects"][0]["type_utf16"] = json!("4100".repeat(51)),
            12..=14 => {
                let field = ["remain_time", "expire_ticks", "remain_micro"][phase - 12];
                v["effects"][0].as_object_mut().unwrap().remove(field);
            }
            15 => {
                v.as_object_mut().unwrap().remove("source_last_value");
            }
            _ => v["unexpected"] = json!(true),
        }
        assert!(
            matches!(read(&v), Err(StatusError::Export)),
            "phase={phase}"
        );
    }
    assert!(matches!(
        read_snapshot(vec![b' '; 8 * 1024 * 1024 + 1].as_slice()),
        Err(StatusError::Export)
    ));
}
