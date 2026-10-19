use serde_json::json;
use vindictus_server::infrastructure::newbie_attendance as db;
fn row() -> serde_json::Value {
    json!({"uid":-7,"event_version":-1,"start_ticks":"0","end_ticks":"3155378975999999999"})
}
fn input(r: serde_json::Value) -> serde_json::Value {
    json!({"format_version":1,"source":"study_heroes.newbie-attendance","rows":[r]})
}
#[test]
fn newbie_reference_preserves_full_datetime2_range_and_rejects_bad_shapes() {
    assert!(db::read_snapshot(serde_json::to_vec(&input(row())).unwrap().as_slice()).is_ok());
    for (key, value) in [
        ("start_ticks", json!("-1")),
        ("end_ticks", json!("3155378976000000000")),
        ("start_ticks", json!(0)),
        ("event_version", json!(2147483648i64)),
        ("uid", json!(null)),
    ] {
        let mut r = row();
        r[key] = value;
        assert!(db::read_snapshot(serde_json::to_vec(&input(r)).unwrap().as_slice()).is_err());
    }
    for key in ["uid", "event_version", "start_ticks", "end_ticks"] {
        let mut r = row();
        r.as_object_mut().unwrap().remove(key);
        assert!(db::read_snapshot(serde_json::to_vec(&input(r)).unwrap().as_slice()).is_err());
    }
    let mut v = input(row());
    v["rows"] = json!([row(), row()]);
    assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    v = input(row());
    v["extra"] = json!(true);
    assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
}
#[test]
fn newbie_reference_is_bounded_and_keeps_source_nonchronological_ranges() {
    let mut r = row();
    r["start_ticks"] = json!("17");
    r["end_ticks"] = json!("13");
    assert!(db::read_snapshot(serde_json::to_vec(&input(r)).unwrap().as_slice()).is_ok());
    let mut bytes = serde_json::to_vec(&input(row())).unwrap();
    bytes.resize(8 * 1024 * 1024 + 1, b' ');
    assert!(db::read_snapshot(bytes.as_slice()).is_err());
}
