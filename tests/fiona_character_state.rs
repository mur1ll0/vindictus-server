mod fixture {
    include!("fixtures/character_state_support.rs");
}
use serde_json::{Value, json};
use vindictus_server::infrastructure::fiona_character_state::{StateError, read_snapshot};
fn read(v: &Value) -> Result<[usize; 6], StateError> {
    read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).map(|s| s.counts())
}
#[test]
fn character_state_reader_preserves_source_edge_types_and_only_source_primary_keys() {
    let source = fixture::export();
    assert_eq!(read(&source).unwrap(), [2; 6]);
    assert_eq!(read(&fixture::empty()).unwrap(), [0; 6]);
    for table in [
        "titles",
        "goals",
        "presets",
        "micro",
        "missions",
        "assignments",
    ] {
        let mut v = source.clone();
        let row = v[table][0].clone();
        v[table].as_array_mut().unwrap().push(row);
        assert!(read(&v).is_err(), "duplicate {table} PK");
    }
    let mut v = source.clone();
    v["titles"][0]["acquired_quest_utf16"] = json!("00d8".repeat(50));
    assert!(read(&v).is_ok());
    v["titles"][0]["acquired_quest_utf16"] = json!("00d8".repeat(51));
    assert!(read(&v).is_err());
    let mut v = source.clone();
    v["titles"][0]["acquired_time"] = json!("8000000000000000");
    assert!(read(&v).is_err());
    let mut v = source.clone();
    v["missions"][0]["last_clear_ticks"] = json!("-1");
    assert!(read(&v).is_err());
    let mut v = source.clone();
    v["titles"][0]["expire_ticks"] = json!("3155378976000000000");
    assert!(read(&v).is_err());
    let mut v = source;
    v["micro"][0]["micro_play_time"] = json!("9223372036854775808");
    assert!(read(&v).is_err());
}
#[test]
fn character_state_reader_requires_nullable_fields_rejects_unknown_and_bounds_input_without_private_errors()
 {
    let source = fixture::export();
    for key in source.as_object().unwrap().keys() {
        let mut v = source.clone();
        v.as_object_mut().unwrap().remove(key);
        assert!(read(&v).is_err(), "missing {key}");
    }
    for table in [
        "titles",
        "goals",
        "presets",
        "micro",
        "missions",
        "assignments",
    ] {
        for key in source[table][0].as_object().unwrap().keys() {
            let mut v = source.clone();
            v[table][0].as_object_mut().unwrap().remove(key);
            assert!(read(&v).is_err(), "missing {table}.{key}");
        }
    }
    let mut v = source;
    v["private_extra"] = json!("never_print_this_private_value");
    let error = read(&v).unwrap_err();
    assert!(!error.to_string().contains("never_print_this_private_value"));
    assert!(read_snapshot(vec![b' '; 64 * 1024 * 1024 + 1].as_slice()).is_err());
    assert!(read_snapshot(b"{}{}".as_slice()).is_err());
}
