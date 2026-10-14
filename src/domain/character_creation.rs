//! Recovered result values and UTC parameter for the normal creation caller.
use super::legacy_user_data::{LegacyUserDataError, SqlDateTime, SqlDateTime2};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum CreateResult {
    Unknown = 0,
    Success = 1,
    InvalidName = 2,
    DuplicatedName = 3,
    ReservedName = 4,
    ForbiddenName = 5,
    ChangedCharacterName = 6,
    CharacterLimit = 7,
    WebConnectionError = 8,
    CreateCharacterStopped = 9,
    MakeEntityFailed = 10,
    CustomizeOperationFailed = 11,
    CostumeSaveFailed = 12,
    InitCharacterFailed = 13,
    OverLimitUsingPreset = 14,
    PremiumSlotLimit = 15,
}
pub fn dormant_parameter(utc: SqlDateTime2) -> Result<SqlDateTime, LegacyUserDataError> {
    SqlDateTime2::from_ticks(
        utc.ticks()
            .checked_sub(7 * 864_000_000_000)
            .ok_or(LegacyUserDataError::DateTime2Range)?,
    )?
    .sql_parameter()
}
