use serde_json::json;
use vindictus_server::infrastructure::mail_reference as db;
mod fixture {
    include!("fixtures/mail_reference_support.rs");
}
#[test]
fn mail_parser_accepts_raw_utf16_signed_null_and_original_sql_profile() {
    let v = fixture::data();
    assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_ok());
    let o = fixture::oracle();
    assert_eq!(o["procedure_sha256"], db::QUERY_SHA256);
    assert_eq!(o["cases"].as_array().unwrap().len(), 12);
    for c in o["cases"].as_array().unwrap() {
        assert_eq!(
            c["empty"].as_bool().unwrap(),
            c["mail_ids"].as_array().unwrap().is_empty()
        );
    }
}
#[test]
fn mail_parser_rejects_missing_null_invalid_dates_hex_duplicate_ids_and_large_input() {
    let good = fixture::data();
    for (key, value) in [
        ("posted_binary", json!("00000000018B8200")),
        ("arrive_binary", json!("00")),
        ("expire_binary", json!("00000000018B8200")),
        ("mail_type", json!(-1)),
        ("mail_type", json!(256)),
        ("from_name_hex", json!("00")),
        ("to_name_hex", json!("00".repeat(41))),
        ("title_hex", json!("xx")),
        ("content_hex", json!("00".repeat(2050))),
        ("mail_id", json!("01")),
        ("from_cid", json!("9223372036854775808")),
        ("charged_gold", json!(2147483648_i64)),
    ] {
        let mut v = good.clone();
        v["rows"][0][key] = value;
        assert!(
            db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err(),
            "{key}"
        );
    }
    for key in ["expire_binary", "is_read"] {
        let mut v = good.clone();
        v["rows"][0].as_object_mut().unwrap().remove(key);
        assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    }
    let mut v = good.clone();
    v["rows"][1]["mail_id"] = v["rows"][0]["mail_id"].clone();
    assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    let mut v = good.clone();
    v["procedure_sha256"] = json!("0".repeat(64));
    assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    let mut v = good;
    v["unexpected"] = json!(true);
    assert!(db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    assert!(db::read_snapshot(std::io::repeat(b' ').take(16 * 1024 * 1024 + 1)).is_err());
}
use std::io::Read;
