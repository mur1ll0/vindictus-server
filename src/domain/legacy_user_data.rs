//! Lossless SQL Server user values. Dates have no inferred timezone.
//! These are preservation values, not primary credentials or authentication proof.
use crate::domain::game_user::{GameUser, LegacyNameKey};
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LegacyUserDataError {
    #[error("legacy datetime value is outside the SQL Server range")]
    DateTimeRange,
    #[error("legacy datetime2 value is outside the SQL Server range")]
    DateTime2Range,
    #[error("legacy byte value exceeds its column bound or has invalid UTF-16 byte length")]
    ByteBound,
    #[error("legacy rowversion must contain exactly eight bytes")]
    RowVersionLength,
    #[error("legacy user channel is outside tinyint range")]
    ChannelRange,
}

/// Exact 1/300-second units since 1900-01-01; preserves SQL datetime quantization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SqlDateTime(i64);
impl SqlDateTime {
    pub const MIN: i64 = -1_391_644_800_000;
    pub const MAX: i64 = 76_683_386_879_999;
    const PER_DAY: i64 = 25_920_000;

    pub fn from_units300(value: i64) -> Result<Self, LegacyUserDataError> {
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(LegacyUserDataError::DateTimeRange);
        }
        Ok(Self(value))
    }
    pub fn from_sql_binary(bytes: [u8; 8]) -> Result<Self, LegacyUserDataError> {
        let days = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let time = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if i64::from(time) >= Self::PER_DAY {
            return Err(LegacyUserDataError::DateTimeRange);
        }
        Self::from_units300(i64::from(days) * Self::PER_DAY + i64::from(time))
    }
    pub fn units300(self) -> i64 {
        self.0
    }
    pub fn to_sql_binary(self) -> [u8; 8] {
        // Constructors bound days to -53690..2958463 and time to one day.
        let days = (self.0.div_euclid(Self::PER_DAY) as i32).to_be_bytes();
        let time = (self.0.rem_euclid(Self::PER_DAY) as u32).to_be_bytes();
        [
            days[0], days[1], days[2], days[3], time[0], time[1], time[2], time[3],
        ]
    }
}

/// Civil time in 100ns ticks since 0001-01-01, with SQL datetime2(7) bounds.
/// Unlike TIMESTAMPTZ this preserves the value without assigning an offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SqlDateTime2(i64);
impl SqlDateTime2 {
    pub const MAX: i64 = 3_155_378_975_999_999_999;
    pub fn from_ticks(value: i64) -> Result<Self, LegacyUserDataError> {
        if !(0..=Self::MAX).contains(&value) {
            return Err(LegacyUserDataError::DateTime2Range);
        }
        Ok(Self(value))
    }
    pub fn ticks(self) -> i64 {
        self.0
    }
}

/// Opaque source bytes, zeroized on drop. UTF16=true uses SQL little-endian code
/// units, including NUL and unpaired surrogates; it does not require Rust UTF8.
pub struct LegacyBytes<const LIMIT: usize, const UTF16: bool>(Zeroizing<Vec<u8>>);
impl<const LIMIT: usize, const UTF16: bool> LegacyBytes<LIMIT, UTF16> {
    pub fn from_bytes(value: Vec<u8>) -> Result<Self, LegacyUserDataError> {
        let value = Zeroizing::new(value);
        let bound = if UTF16 {
            LIMIT.checked_mul(2)
        } else {
            Some(LIMIT)
        }
        .ok_or(LegacyUserDataError::ByteBound)?;
        if value.len() > bound || (UTF16 && !value.len().is_multiple_of(2)) {
            return Err(LegacyUserDataError::ByteBound);
        }
        Ok(Self(value))
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
impl<const LIMIT: usize, const UTF16: bool> PartialEq for LegacyBytes<LIMIT, UTF16> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl<const LIMIT: usize, const UTF16: bool> Eq for LegacyBytes<LIMIT, UTF16> {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SourceRowVersion([u8; 8]);
impl SourceRowVersion {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LegacyUserDataError> {
        Ok(Self(
            bytes
                .try_into()
                .map_err(|_| LegacyUserDataError::RowVersionLength)?,
        ))
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// All ten fields absent from the initial game-user schema. No Debug/Serialize.
#[derive(Default, PartialEq, Eq)]
pub struct LegacyUserDetails {
    pub last_login_ip: Option<LegacyBytes<50, true>>,
    pub ip_lock_time: Option<SqlDateTime2>,
    pub facebook_token: Option<LegacyBytes<255, false>>,
    pub is_dormant_user_event: Option<SqlDateTime>,
    pub second_password: Option<LegacyBytes<256, true>>,
    pub second_password_fail_count: Option<i32>,
    pub second_password_last_fail: Option<SqlDateTime2>,
    pub last_update: Option<SourceRowVersion>,
    pub is_dormant_user: Option<SqlDateTime>,
    pub create_time: Option<SqlDateTime>,
}

/// Complete fifteen-column source snapshot. Not a user authenticated by the game.
/// Source timestamps/tokens are preserved separately from operational metadata.
#[derive(PartialEq, Eq)]
pub struct LegacyUserSnapshot {
    pub user: GameUser,
    pub details: LegacyUserDetails,
}
impl LegacyUserSnapshot {
    pub fn validate(&self) -> Result<LegacyNameKey, crate::error::Error> {
        if self
            .user
            .channeling_code
            .is_some_and(|value| !(0..=255).contains(&value))
        {
            return Err(LegacyUserDataError::ChannelRange.into());
        }
        Ok(LegacyNameKey::parse(&self.user.name)?)
    }
}
