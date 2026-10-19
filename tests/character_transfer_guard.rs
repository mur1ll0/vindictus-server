use serde_json::{Value, json};
use vindictus_server::infrastructure::character_transfer_guard as db;
fn input() -> Value {
    json!({"format_version":1,"source":"study_heroes.character-transfer-empty","rows":0,"columns":8,"schema_sha256":"a".repeat(64),"procedure_sha256":"6f47c1f5f6926d7d5bd474fa91547c2e31343d3afc0f8853e3a0ffee0370baaa"})
}
#[test]
fn transfer_guard_admits_only_recovered_empty_reference_shape() {
    assert!(db::read_snapshot(serde_json::to_vec(&input()).unwrap().as_slice()).is_ok());
    for (key, value) in [
        ("format_version", json!(2)),
        ("source", json!("other")),
        ("rows", json!(1)),
        ("columns", json!(7)),
        ("schema_sha256", json!("A".repeat(64))),
        ("procedure_sha256", json!("b".repeat(64))),
    ] {
        let mut v = input();
        v[key] = value;
        assert!(matches!(
            db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()),
            Err(db::TransferError::Input)
        ));
    }
}
#[test]
fn transfer_guard_rejects_missing_null_unknown_and_wrong_types() {
    for key in [
        "format_version",
        "source",
        "rows",
        "columns",
        "schema_sha256",
        "procedure_sha256",
    ] {
        let mut v = input();
        v.as_object_mut().unwrap().remove(key);
        assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
        let mut v = input();
        v[key] = Value::Null;
        assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    }
    let mut v = input();
    v["extra"] = json!(true);
    assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    let mut v = input();
    v["rows"] = json!("0");
    assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
}
#[test]
fn transfer_guard_bounds_input_and_rejects_read_failure() {
    let mut bytes = serde_json::to_vec(&input()).unwrap();
    bytes.resize(8193, b' ');
    assert!(db::read_snapshot(bytes.as_slice()).is_err());
    struct Failed;
    impl std::io::Read for Failed {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("synthetic read failure"))
        }
    }
    assert!(matches!(
        db::read_snapshot(Failed),
        Err(db::TransferError::Input)
    ));
}
