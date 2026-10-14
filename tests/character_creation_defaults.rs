use serde::Deserialize;
use vindictus_server::domain::legacy_user_data::SqlDateTime;
#[derive(Deserialize)]
struct Oracle {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    unit: i64,
    framework_millisecond: i64,
    datetime2_subsecond_ticks: i64,
}
#[test]
fn creation_getutcdate_defaults_match_all_sql_datetime_subsecond_phases() {
    let data: Oracle =
        serde_json::from_str(include_str!("fixtures/sql-creation-clock-defaults.json")).unwrap();
    assert_eq!(data.cases.len(), 300);
    let base = SqlDateTime::from_units300(946_684_800i64 * 300 + 2_208_988_800i64 * 300).unwrap();
    let base_ticks = base.framework_date_time().unwrap().ticks();
    for c in data.cases {
        let ticks = SqlDateTime::from_units300(base.units300() + c.unit)
            .unwrap()
            .framework_date_time()
            .unwrap()
            .ticks();
        assert_eq!(ticks - base_ticks, c.datetime2_subsecond_ticks);
        assert_eq!(
            c.datetime2_subsecond_ticks,
            c.framework_millisecond * 10_000
        );
    }
}
