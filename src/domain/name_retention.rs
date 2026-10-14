//! Recovered temporal decisions AFTER the SQL name filter, not name equality.
use super::legacy_user_data::{LegacyUserDataError, SqlDateTime, SqlDateTime2};

#[derive(Clone, Copy)]
pub struct MatchedCharacter {
    pub status: i32,
    /// SqlClient materialized DateTime, not the raw datetime 1/300-second counter.
    pub deleted_at: Option<SqlDateTime2>,
}
/// Preserve iteration and per-deleted-row UTC sampling from CharacterMaker.
/// Active rows short-circuit, including in the jaJP branch. No user/status filter is added.
pub fn duplicate_name_available(
    japanese: bool,
    matched: impl IntoIterator<Item = MatchedCharacter>,
    mut utc_now: impl FnMut() -> SqlDateTime2,
) -> bool {
    let mut recent = false;
    for row in matched {
        if row.status != 1 {
            return false;
        }
        if let Some(deleted) = row.deleted_at {
            // TimeSpan.Days truncates toward zero, including future deletion times.
            if (utc_now().ticks() - deleted.ticks()) / 864_000_000_000 < 30 {
                recent = true;
            }
        }
    }
    japanese || !recent
}
/// DATEADD(day,-period,GETUTCDATE()) retains SQL datetime quantization and range.
/// GETUTCDATE is UTC and is already datetime, not an unquantized DateTime.Now sample.
pub fn history_threshold(
    utc_now: SqlDateTime,
    period: i32,
) -> Result<SqlDateTime, LegacyUserDataError> {
    let days = period
        .checked_neg()
        .ok_or(LegacyUserDataError::DateTimeRange)?;
    SqlDateTime::from_units300(utc_now.units300() + i64::from(days) * 25_920_000)
}
/// ChangedNameCheck's strict greater-than boundary; caller must supply old-name matches.
pub fn history_name_blocked(
    matched_log_dates: impl IntoIterator<Item = SqlDateTime>,
    threshold: SqlDateTime,
) -> bool {
    matched_log_dates
        .into_iter()
        .any(|date| date.units300() > threshold.units300())
}
