use serde_json::Value;
use vindictus_server::infrastructure::character_export::read_export;
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/character-bundle.json")).unwrap()
}
fn parse(
    v: &Value,
) -> Result<vindictus_server::domain::character_data::CharacterBundle, vindictus_server::error::Error>
{
    read_export(serde_json::to_vec(v).unwrap().as_slice())
}
#[test]
fn complete_bundle_preserves_null_empty_opaque_bytes_signed_ids_and_precision() {
    let b = parse(&fixture()).unwrap();
    assert_eq!(b.characters.len(), 4);
    assert!(b.character_info[1].name.is_none());
    assert_eq!(b.character_info[3].name.as_ref().unwrap().as_bytes(), b"");
    assert_eq!(
        b.character_info[2].name.as_ref().unwrap().as_bytes(),
        [0, 216, 0, 0]
    );
    assert_eq!(b.character_info[0].exp, i64::MIN);
    assert_eq!(b.character_info[0].create_time.ticks(), 1);
    assert_eq!(b.character_info[0].last_ap_update.units300(), -1);
    assert_eq!(b.queued_items[1].item_id, Some(i64::MIN));
    assert!(b.queued_items[0].mail_title.is_none());
    assert_eq!(
        b.queued_items[1].mail_title.as_ref().unwrap().as_bytes(),
        b""
    );
}
#[test]
fn nullable_columns_are_required_and_unknown_duplicate_fields_refused() {
    for (t, c) in [
        ("character_info", "name_utf16"),
        ("connections", "last_logout_units300"),
        ("queued_items", "mail_title_utf16"),
    ] {
        let mut v = fixture();
        v[t][0].as_object_mut().unwrap().remove(c);
        assert!(parse(&v).is_err());
    }
    let mut v = fixture();
    v["character_info"][0]["unexpected"] = 1.into();
    assert!(parse(&v).is_err());
    let json = serde_json::to_string(&fixture()).unwrap().replacen(
        "\"format_version\":1",
        "\"format_version\":1,\"format_version\":1",
        1,
    );
    assert!(read_export(json.as_bytes()).is_err());
}
#[test]
fn malformed_hex_bounds_dates_and_noncanonical_integer_strings_are_refused() {
    for (c, v) in [
        ("name_utf16", Value::from("0")),
        ("name_utf16", Value::from("gg")),
        ("name_utf16", Value::from("00".repeat(102))),
        ("legacy_last_update", Value::from("00")),
        ("create_time_ticks", Value::from("-1")),
        ("create_time_ticks", Value::from("3155378976000000000")),
        ("last_ap_update_units300", Value::from("76683386880000")),
        ("id", Value::from("01")),
        ("id", Value::from("+1")),
        ("id", Value::from("9223372036854775808")),
        ("class", Value::from(256)),
        ("order_num", Value::from(-1)),
    ] {
        let mut f = fixture();
        f["character_info"][0][c] = v;
        assert!(parse(&f).is_err(), "{c}");
    }
    let mut v = fixture();
    v["queued_items"][0]["item_class_ex_utf16"] = "00".repeat(2050).into();
    assert!(parse(&v).is_err());
}
#[test]
fn declared_counts_duplicate_keys_and_total_input_limits_are_enforced() {
    let mut v = fixture();
    v["source_counts"]["connections"] = 4.into();
    assert!(parse(&v).is_err());
    let mut v = fixture();
    v["characters"][0]["id"] = v["characters"][1]["id"].clone();
    assert!(parse(&v).is_err());
    let mut v = fixture();
    v["queued_items"][0]["row_id"] = v["queued_items"][1]["row_id"].clone();
    assert!(parse(&v).is_err());
    assert!(read_export(vec![b' '; 6 * 1024 * 1024 + 1].as_slice()).is_err());
    let mut v = fixture();
    v["source"] = "untrusted".into();
    assert!(parse(&v).is_err());
}
