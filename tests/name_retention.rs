use serde_json::Value;
use vindictus_server::domain::{
    legacy_user_data::{SqlDateTime, SqlDateTime2},
    name_retention::{
        MatchedCharacter, duplicate_name_available, history_name_blocked, history_threshold,
    },
};
fn ticks(value: &Value) -> SqlDateTime2 {
    SqlDateTime2::from_ticks(value.as_str().unwrap().parse().unwrap()).unwrap()
}
#[test]
fn duplicate_retention_matches_extracted_framework_for_status_dates_and_japanese_branch() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-duplicate-name.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["cases"].as_array().unwrap().len(), 480);
        let now = ticks(&profile["clock_ticks"]);
        for case in profile["cases"].as_array().unwrap() {
            let mut rows = Vec::new();
            if case["match"].as_bool().unwrap() {
                rows.push(MatchedCharacter {
                    status: case["status"].as_i64().unwrap() as i32,
                    deleted_at: (!case["deleted_ticks"].is_null())
                        .then(|| ticks(&case["deleted_ticks"])),
                });
            }
            if case["other_active"].as_bool().unwrap() {
                rows.push(MatchedCharacter {
                    status: 0,
                    deleted_at: None,
                });
            }
            assert_eq!(
                duplicate_name_available(case["jp"].as_bool().unwrap(), rows, || now),
                case["result"].as_bool().unwrap()
            );
        }
    }
}
#[test]
fn duplicate_retention_preserves_clock_sampling_and_active_row_short_circuit() {
    let now = SqlDateTime2::from_ticks(639_011_808_000_000_000).unwrap();
    let boundary = SqlDateTime2::from_ticks(now.ticks() - 30 * 864_000_000_000).unwrap();
    let rows = [
        MatchedCharacter {
            status: 1,
            deleted_at: Some(boundary),
        },
        MatchedCharacter {
            status: 1,
            deleted_at: None,
        },
        MatchedCharacter {
            status: 1,
            deleted_at: Some(boundary),
        },
        MatchedCharacter {
            status: -1,
            deleted_at: Some(boundary),
        },
        MatchedCharacter {
            status: 1,
            deleted_at: Some(boundary),
        },
    ];
    let mut calls = 0;
    assert!(!duplicate_name_available(true, rows, || {
        calls += 1;
        now
    }));
    assert_eq!(calls, 2);
    calls = 0;
    assert!(!duplicate_name_available(false, [rows[0], rows[2]], || {
        calls += 1;
        SqlDateTime2::from_ticks(now.ticks() - i64::from(calls == 2)).unwrap()
    }));
    assert_eq!(calls, 2);
}
fn sql_date(value: &Value) -> SqlDateTime {
    let hex = value.as_str().unwrap();
    let bytes = (0..16)
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    SqlDateTime::from_sql_binary(bytes.try_into().unwrap()).unwrap()
}
#[test]
fn history_retention_matches_sql_dateadd_and_strict_quantized_boundary() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/sql-name-history.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 25);
    for case in fixture["cases"].as_array().unwrap() {
        let threshold = history_threshold(
            sql_date(&case["now_binary"]),
            case["period"].as_i64().unwrap() as i32,
        )
        .unwrap();
        assert_eq!(threshold, sql_date(&case["threshold_binary"]));
        assert_eq!(
            history_name_blocked([sql_date(&case["log_binary"])], threshold),
            case["blocked"].as_bool().unwrap()
        );
        assert!(!history_name_blocked([], threshold));
    }
}
#[test]
fn history_retention_refuses_unary_and_sql_datetime_range_overflow() {
    let min = SqlDateTime::from_units300(SqlDateTime::MIN).unwrap();
    let max = SqlDateTime::from_units300(SqlDateTime::MAX).unwrap();
    assert!(history_threshold(min, 1).is_err());
    assert!(history_threshold(max, -1).is_err());
    assert!(history_threshold(min, i32::MIN).is_err());
    assert_eq!(history_threshold(min, 0).unwrap(), min);
    assert_eq!(history_threshold(max, 0).unwrap(), max);
}
