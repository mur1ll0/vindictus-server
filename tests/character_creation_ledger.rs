#[test]
fn creation_ledger_partition_matches_readonly_sql_dates_and_midnight_boundaries() {
    use vindictus_server::domain::{
        character_ledger::{LedgerPartition, partition},
        legacy_user_data::SqlDateTime,
    };
    let v: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/sql-creation-ledger-routing.json")).unwrap();
    let rows = v["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 13);
    for row in rows {
        let h = row["sql_datetime_hex"].as_str().unwrap();
        let raw = (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let dt = SqlDateTime::from_sql_binary(raw.try_into().unwrap()).unwrap();
        let expected = if row["partition"] == 1 {
            LedgerPartition::One
        } else {
            LedgerPartition::Two
        };
        assert_eq!(partition(dt), expected);
    }
}
