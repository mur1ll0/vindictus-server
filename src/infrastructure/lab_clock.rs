//! Explicit laboratory UTC/civil clocks, independent of the host's timezone.
use crate::domain::legacy_user_data::SqlDateTime2;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClockError {
    #[error("laboratory clock is outside the verified DateTime/offset bounds")]
    Range,
}
pub fn unix_ticks(seconds: i64, nanoseconds: u32) -> Result<SqlDateTime2, ClockError> {
    if nanoseconds >= 1_000_000_000 {
        return Err(ClockError::Range);
    }
    let ticks = seconds
        .checked_mul(10_000_000)
        .and_then(|v| v.checked_add(621_355_968_000_000_000))
        .and_then(|v| v.checked_add(i64::from(nanoseconds / 100)))
        .ok_or(ClockError::Range)?;
    SqlDateTime2::from_ticks(ticks).map_err(|_| ClockError::Range)
}
pub fn civil_from_utc(utc: SqlDateTime2, offset_seconds: i32) -> Result<SqlDateTime2, ClockError> {
    if !(-50_400..=50_400).contains(&offset_seconds) {
        return Err(ClockError::Range);
    }
    let ticks = utc
        .ticks()
        .checked_add(i64::from(offset_seconds) * 10_000_000)
        .ok_or(ClockError::Range)?;
    SqlDateTime2::from_ticks(ticks).map_err(|_| ClockError::Range)
}
pub fn utc_now() -> Result<SqlDateTime2, ClockError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ClockError::Range)?;
    unix_ticks(
        i64::try_from(duration.as_secs()).map_err(|_| ClockError::Range)?,
        duration.subsec_nanos(),
    )
}
pub fn civil_now(offset_seconds: i32) -> Result<SqlDateTime2, ClockError> {
    civil_from_utc(utc_now()?, offset_seconds)
}
