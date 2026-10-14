use serde_json::{Value, json};
use vindictus_server::infrastructure::character_collections::{CollectionError, read_snapshot};
fn data() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/synthetic-character-collections.json"
    ))
    .unwrap()
}
fn read(v: &Value) -> bool {
    read_snapshot(v.to_string().as_bytes()).is_ok()
}
#[test]
fn creation_export_retains_binary_settings_duplicates_and_signed_owner_permissions() {
    let mut v = data();
    assert!(read(&v));
    v["settings"][0]["value_hex"] = json!("FF".repeat(255));
    assert!(read(&v));
    v["settings"][0]["value_hex"] = json!("FF".repeat(256));
    assert!(!read(&v));
    v["settings"][0]["value_hex"] = json!("Z0");
    assert!(!read(&v));
    v["settings"][0]["value_hex"] = json!("0");
    assert!(!read(&v));
    v = data();
    v["item_collections"][0]["owner_id"] = json!("-9223372036854775808");
    assert!(read(&v));
    v["item_collections"][0]["owner_id"] = json!("9223372036854775808");
    assert!(!read(&v));
}
#[test]
fn creation_export_rejects_missing_fields_duplicate_owners_and_unbounded_input() {
    for key in ["item_collections", "quest_collections", "mailboxes"] {
        let mut v = data();
        let duplicate = v[key][0].clone();
        v[key].as_array_mut().unwrap().push(duplicate);
        assert!(!read(&v));
    }
    let mut v = data();
    v["story_entries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"cid":"1","permission":0,"hint_category":1}));
    assert!(!read(&v));
    for key in [
        "item_collections",
        "quest_collections",
        "mailboxes",
        "story_entries",
        "settings",
    ] {
        let mut v = data();
        v.as_object_mut().unwrap().remove(key);
        assert!(!read(&v));
    }
    let mut v = data();
    v["story_entries"][0]["hint_category"] = json!(256);
    assert!(!read(&v));
    v = data();
    v["settings"] = json!(vec![json!({"code_hex":"","value_hex":""}); 1001]);
    assert!(!read(&v));
    v = data();
    v["private_extra"] = json!(true);
    assert!(!read(&v));
    assert!(matches!(
        read_snapshot(vec![b' '; 2 * 1024 * 1024 + 1].as_slice()),
        Err(CollectionError::Export)
    ));
    assert!(matches!(
        read_snapshot(b"{\"format_version\":1}".as_slice()),
        Err(CollectionError::Export)
    ));
}
