use serde::Deserialize;
use vindictus_server::domain::legacy_user_data::{
    LegacyBytes, LegacyUserDataError, SourceRowVersion, SqlDateTime, SqlDateTime2,
};

#[derive(Deserialize)]
struct Fixture {
    datetime_cases: Vec<DateCase>,
    datetime2_cases: Vec<Date2Case>,
    nvarchar_cases: Vec<ByteCase>,
    varchar_cases: Vec<ByteCase>,
}
#[derive(Deserialize)]
struct DateCase {
    binary_hex: String,
    units300: i64,
    binary_roundtrip_equal: bool,
}
#[derive(Deserialize)]
struct Date2Case {
    ticks: i64,
}
#[derive(Deserialize)]
struct ByteCase {
    binary_hex: String,
    byte_length: usize,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!(
        "../docs/evidence/database/user-types-oracle.json"
    ))
    .unwrap()
}
fn hex(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn datetime_binary_and_negative_epoch_match_sql_server_oracle_exactly() {
    for case in fixture().datetime_cases {
        let bytes: [u8; 8] = hex(&case.binary_hex).try_into().unwrap();
        let value = SqlDateTime::from_sql_binary(bytes).unwrap();
        assert_eq!(value.units300(), case.units300);
        assert_eq!(value.to_sql_binary(), bytes);
        assert!(case.binary_roundtrip_equal);
        assert_eq!(SqlDateTime::from_units300(case.units300).unwrap(), value);
    }
}
#[test]
fn datetime2_keeps_seventh_fractional_digit_and_full_range() {
    let cases = fixture().datetime2_cases;
    for case in &cases {
        assert_eq!(
            SqlDateTime2::from_ticks(case.ticks).unwrap().ticks(),
            case.ticks
        );
    }
    assert_eq!(cases[1].ticks - cases[0].ticks, 1);
    assert_eq!(cases[4].ticks - cases[3].ticks, 1);
    assert!(matches!(
        SqlDateTime2::from_ticks(-1),
        Err(LegacyUserDataError::DateTime2Range)
    ));
    assert!(SqlDateTime2::from_ticks(SqlDateTime2::MAX + 1).is_err());
}
#[test]
fn timestamp_bounds_and_invalid_binary_time_are_rejected_before_persistence() {
    assert!(SqlDateTime::from_units300(SqlDateTime::MIN - 1).is_err());
    assert!(SqlDateTime::from_units300(SqlDateTime::MAX + 1).is_err());
    assert!(SqlDateTime::from_sql_binary([0, 0, 0, 0, 1, 139, 130, 0]).is_err());
    assert!(SourceRowVersion::from_bytes(&[0; 7]).is_err());
    assert!(SourceRowVersion::from_bytes(&[0; 9]).is_err());
    assert_eq!(
        SourceRowVersion::from_bytes(&[255; 8]).unwrap().as_bytes(),
        &[255; 8]
    );
}
#[test]
fn source_string_bytes_preserve_empty_nul_and_unpaired_utf16_without_utf8_conversion() {
    let oracle = fixture();
    for case in oracle.nvarchar_cases {
        let bytes = hex(&case.binary_hex);
        assert_eq!(bytes.len(), case.byte_length);
        assert_eq!(
            LegacyBytes::<50, true>::from_bytes(bytes.clone())
                .unwrap()
                .as_bytes(),
            bytes
        );
    }
    for case in oracle.varchar_cases {
        let bytes = hex(&case.binary_hex);
        assert_eq!(bytes.len(), case.byte_length);
        assert_eq!(
            LegacyBytes::<255, false>::from_bytes(bytes.clone())
                .unwrap()
                .as_bytes(),
            bytes
        );
    }
    assert!(LegacyBytes::<50, true>::from_bytes(vec![0; 101]).is_err());
    assert!(LegacyBytes::<50, true>::from_bytes(vec![0; 1]).is_err());
    assert!(LegacyBytes::<256, true>::from_bytes(vec![0; 512]).is_ok());
    assert!(LegacyBytes::<256, true>::from_bytes(vec![0; 514]).is_err());
    assert!(LegacyBytes::<255, false>::from_bytes(vec![0; 256]).is_err());
}
