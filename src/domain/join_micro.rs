//! Initial MicroPlay daily window for the recovered fixed UTC+8 VM profile.
use super::{legacy_user_data::SqlDateTime2, login_policy::FixedLocalClock};
use crate::error::Error;
const DAY: i64 = 864_000_000_000;
#[derive(Clone, Copy)]
pub struct Window {
    last: SqlDateTime2,
    next: SqlDateTime2,
}
impl Window {
    pub fn reference_vm(now: SqlDateTime2) -> Result<Self, Error> {
        let local = FixedLocalClock::reference_vm().application_local(now)?;
        let boundary = local.ticks() / DAY * DAY + 9 * 36_000_000_000 - 480 * 600_000_000;
        let (last, next) = if boundary < now.ticks() {
            (boundary, boundary + DAY)
        } else {
            (boundary - DAY, boundary)
        };
        Ok(Self {
            last: SqlDateTime2::from_ticks(last).map_err(|_| Error::LegacyClockOverflow)?,
            next: SqlDateTime2::from_ticks(next).map_err(|_| Error::LegacyClockOverflow)?,
        })
    }
    pub fn last(&self) -> SqlDateTime2 {
        self.last
    }
    pub fn next(&self) -> SqlDateTime2 {
        self.next
    }
    pub fn needs_reset(&self, started: SqlDateTime2) -> bool {
        started.ticks() < self.last.ticks()
    }
}
