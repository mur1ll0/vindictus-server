use serde_json::Value;
use vindictus_server::{
    domain::{character_lobby::*, feature_policy::FeaturePolicy, legacy_user_data::LegacyBytes},
    infrastructure::{character_lobby::LobbyError, lobby_export::read_snapshot},
};
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
fn source() -> Value {
    serde_json::from_str(include_str!("fixtures/synthetic-character-lobby.json")).unwrap()
}
#[test]
fn lobby_reader_preserves_nulls_signed_ids_raw_utf16_and_civil_dates() {
    let s = read_snapshot(include_bytes!("fixtures/synthetic-character-lobby.json").as_slice())
        .unwrap();
    assert_eq!(s.user_info.len(), 2);
    assert_eq!(s.user_info[0].nexon_sn, i32::MIN);
    assert_eq!(s.user_info[0].last_login_cid, Some(i64::MIN));
    assert!(s.user_info[0].prologue_played);
    assert_eq!(s.user_info[1].last_login_cid, None);
    assert_eq!(s.user_products[0].id, i64::MAX);
    assert_eq!(
        s.user_products[0].argument.as_ref().unwrap().as_bytes(),
        &[0, 216, 0, 0]
    );
    assert_eq!(
        s.user_products[0].expire_time.unwrap().ticks(),
        3155378975999999999
    );
    assert_eq!(s.user_care_info[0].kind, 255);
    assert_eq!(s.user_care_info[0].state, i16::MIN);
    assert_eq!(s.user_care_info[0].last_update.units300(), -1391644800000);
    let mut inputs = Vec::new();
    let mut v = source();
    v["user_products"][0]
        .as_object_mut()
        .unwrap()
        .remove("argument_utf16_hex");
    inputs.push(v);
    let mut v = source();
    v["user_info"][1]
        .as_object_mut()
        .unwrap()
        .remove("last_login_cid");
    inputs.push(v);
    let mut v = source();
    v["user_products"][0]["product_name_utf16_hex"] = Value::String("000".into());
    inputs.push(v);
    let mut v = source();
    v["user_products"][0]["expire_time_ticks"] = Value::String("3155378976000000000".into());
    inputs.push(v);
    let mut v = source();
    v["user_care_info"][0]["type"] = Value::from(256);
    inputs.push(v);
    let mut v = source();
    v["user_info"][1] = v["user_info"][0].clone();
    inputs.push(v);
    let mut v = source();
    v["user_products"][0]["id"] = Value::String("01".into());
    inputs.push(v);
    let mut v = source();
    v["unknown"] = Value::Bool(true);
    inputs.push(v);
    for v in inputs {
        assert!(read_snapshot(v.to_string().as_bytes()).is_err());
    }
    assert!(matches!(
        read_snapshot(vec![b' '; 2 * 1024 * 1024 + 1].as_slice()),
        Err(LobbyError::Export)
    ));
}
#[test]
fn product_classification_matches_sql_ascii_case_and_padding_and_refuses_unproved_units() {
    let f: Value = serde_json::from_str(include_str!("fixtures/sql-lobby-products.json")).unwrap();
    assert_eq!(f["cases"].as_array().unwrap().len(), 9);
    for c in f["cases"].as_array().unwrap() {
        let bytes = hex(c["product_name_utf16_hex"].as_str().unwrap());
        let n = LegacyBytes::from_bytes(bytes).unwrap();
        let actual = classify_product(&n);
        if c["case_id"] == 8 {
            assert!(matches!(actual, Err(LobbyValueError::ProductName)));
            continue;
        }
        let expected = if c["character_slot"].as_bool().unwrap() {
            ProductKind::CharacterSlot
        } else if c["premium_slot"].as_bool().unwrap() {
            ProductKind::PremiumCharacterSlot
        } else {
            ProductKind::Other
        };
        assert_eq!(actual.unwrap(), expected);
    }
    assert!(matches!(
        classify_product(&LegacyBytes::from_bytes(vec![233, 0]).unwrap()),
        Err(LobbyValueError::ProductName)
    ));
}
#[test]
fn lobby_version_pairs_match_framework_in_five_cultures_and_two_architectures() {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-lobby-features.json")).unwrap();
    for profile in f["profiles"].as_array().unwrap() {
        assert_eq!(profile["cases"].as_array().unwrap().len(), 15);
        for c in profile["cases"].as_array().unwrap() {
            let p = FeaturePolicy::new([
                (
                    String::from("CurrentVer"),
                    Some(c["current"].as_str().unwrap().into()),
                ),
                (
                    String::from("syntheticFeature"),
                    Some(c["value"].as_str().unwrap().into()),
                ),
            ])
            .unwrap();
            assert_eq!(
                p.is_enabled(Some("syntheticFeature")).unwrap(),
                c["enabled"].as_bool().unwrap()
            );
        }
    }
}
