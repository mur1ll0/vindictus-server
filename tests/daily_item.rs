use vindictus_server::domain::daily_item::{Procedure, ProcessKind, SQL_MIN_TICKS};

const MINUTE: i64 = 600_000_000;
const DAY: i64 = 1440 * MINUTE;
// 2010-02-02 00:00 and 3000-12-31 00:00:00.001 (DailyItemProcessInfo 110).
const FROM: i64 = 634_006_656_000_000_000;
const UNTIL: i64 = 946_707_264_000_010_000;

fn daily(period: i32) -> Procedure {
    Procedure::parse(
        110,
        FROM,
        UNTIL,
        period,
        0,
        3,
        None,
        None,
        "giveitemuntil cash_revive_solo_homevip_kr 3",
        Some("GameUI_Heroes_DailyItemProcess_GiveItemMessage".to_owned()),
    )
    .unwrap()
}

#[test]
fn parse_reads_the_process_type_ignoring_case_and_the_arguments() {
    let p = daily(1440);
    assert_eq!(p.kind, ProcessKind::GiveItemUntil);
    assert_eq!(p.args, ["cash_revive_solo_homevip_kr", "3"]);
    assert_eq!(
        (p.count(), p.level_min, p.level_max),
        (Some(3), 0, i32::MAX)
    );
    assert_eq!(p.period, Some(DAY));
    let fatigue = Procedure::parse(
        1,
        FROM,
        UNTIL,
        60,
        0,
        3,
        None,
        None,
        "GiveFatigueItem",
        None,
    )
    .unwrap();
    assert!(matches!(fatigue.kind, ProcessKind::NotMigrated(_)));
    assert!(Procedure::parse(1, FROM, UNTIL, 60, 0, 3, None, None, "unknown x", None).is_none());
}

#[test]
fn ranges_follow_time_range_get_current_period() {
    let p = daily(1440);
    let now = FROM + 10 * DAY + 5 * MINUTE;
    assert_eq!(p.current_range(now), (FROM + 10 * DAY, FROM + 11 * DAY));
    assert_eq!(p.next_range_start(now), Some(FROM + 11 * DAY));
    // Without a period the range is the whole DateFrom..DateUntil, never rescheduled.
    let once = daily(0);
    assert_eq!(once.current_range(now), (FROM, UNTIL));
    assert_eq!(once.next_range_start(now), None);
}

#[test]
fn a_procedure_runs_once_per_range_inside_its_dates() {
    let p = daily(60);
    let now = FROM + 100 * 60 * MINUTE + 30 * MINUTE;
    // Never processed (SQLDateTime.MinValue) or processed in an earlier range: due.
    assert!(p.is_due(None, now));
    assert!(p.is_due(Some(SQL_MIN_TICKS), now));
    assert!(p.is_due(Some(now - 60 * MINUTE), now));
    // Processed in this range: not due until the next one.
    assert!(!p.is_due(Some(now - 10 * MINUTE), now));
    assert!(p.is_due(Some(now - 10 * MINUTE), now + 31 * MINUTE));
    // Outside DateFrom..DateUntil.
    assert!(!p.is_due(None, FROM));
    assert!(!p.is_due(None, UNTIL));
    assert!(p.fits_level(1) && p.fits_level(i32::MAX));
}
