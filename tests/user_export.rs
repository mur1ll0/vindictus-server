use vindictus_server::{
    error::Error,
    infrastructure::user_export::{MAX_INPUT_BYTES, MAX_ROWS, read_export},
};
const FIXTURE: &[u8] = include_bytes!("fixtures/user-export.json");
fn rejected(json: &serde_json::Value) {
    assert!(read_export(serde_json::to_vec(json).unwrap().as_slice()).is_err());
}
#[test]
fn export_preserves_exact_large_ticks_signed_ids_null_empty_and_raw_bytes() {
    let rows = read_export(FIXTURE).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].user.id, -200);
    assert_eq!(rows[0].user.name, "Import_Full ");
    assert_eq!(rows[0].user.channeling_code, Some(0));
    assert_eq!(
        rows[0].details.ip_lock_time.unwrap().ticks(),
        3_155_378_975_999_999_999
    );
    assert_eq!(
        rows[0].details.second_password.as_ref().unwrap().as_bytes(),
        &[0, 216, 0, 0]
    );
    assert!(rows[1].details.create_time.is_none());
    assert!(rows[1].details.last_login_ip.is_none());
    assert!(
        rows[1]
            .details
            .second_password
            .as_ref()
            .unwrap()
            .as_bytes()
            .is_empty()
    );
}
#[test]
fn export_requires_every_source_column_including_nullable_ones() {
    let original: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    let keys: Vec<_> = original["rows"][0]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys.len(), 15);
    for key in keys {
        let mut altered = original.clone();
        altered["rows"][0].as_object_mut().unwrap().remove(&key);
        rejected(&altered);
    }
}
#[test]
fn export_rejects_numeric_dates_noncanonical_integers_overflow_and_wrong_ranges() {
    for value in [
        serde_json::json!(3155378975999999999i64),
        serde_json::json!(1.0),
        serde_json::json!("3.155378975999999999e18"),
        serde_json::json!("+1"),
        serde_json::json!("01"),
        serde_json::json!("-1"),
        serde_json::json!("3155378976000000000"),
        serde_json::json!("9223372036854775808"),
    ] {
        let mut altered: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        altered["rows"][0]["ip_lock_time_ticks"] = value;
        rejected(&altered);
    }
}
#[test]
fn export_rejects_bad_hex_lengths_unicode_and_invalid_channels() {
    for (key, value) in [
        ("second_password_utf16_hex", serde_json::json!("00")),
        (
            "second_password_utf16_hex",
            serde_json::json!("00".repeat(514)),
        ),
        ("second_password_utf16_hex", serde_json::json!("XX")),
        ("second_password_utf16_hex", serde_json::json!("éé")),
        ("last_update_hex", serde_json::json!("")),
        ("channeling_code", serde_json::json!(256)),
        ("name", serde_json::json!("Straße")),
    ] {
        let mut altered: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        altered["rows"][0][key] = value;
        rejected(&altered);
    }
}
#[test]
fn export_is_bounded_and_rejects_unknown_duplicate_trailing_or_partial_documents() {
    assert!(matches!(
        read_export(vec![b' '; MAX_INPUT_BYTES + 1].as_slice()),
        Err(Error::UserExport)
    ));
    assert!(read_export(&FIXTURE[..FIXTURE.len() / 2]).is_err());
    assert!(read_export([FIXTURE, b"{}"].concat().as_slice()).is_err());
    assert!(read_export(br#"{"format_version":1,"format_version":1}"#.as_slice()).is_err());
    let mut altered: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    altered["rows"][0]["unexpected"] = serde_json::json!("private");
    rejected(&altered);
    let row = altered["rows"][1].clone();
    altered["rows"] = serde_json::json!(vec![row; MAX_ROWS + 1]);
    altered["exported_row_count"] = serde_json::json!(MAX_ROWS + 1);
    rejected(&altered);
}
#[test]
fn export_validates_origin_version_and_count_without_echoing_values() {
    for (key, value) in [
        ("source", serde_json::json!("private-secret")),
        ("format_version", serde_json::json!(2)),
        ("exported_row_count", serde_json::json!(29)),
        ("rows", serde_json::json!([])),
    ] {
        let mut altered: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        altered[key] = value;
        let error = read_export(serde_json::to_vec(&altered).unwrap().as_slice())
            .err()
            .unwrap();
        assert!(matches!(error, Error::UserExport));
        assert!(!error.to_string().contains("private-secret"));
    }
}
