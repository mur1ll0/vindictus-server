//! Final internal UserLogin feedback, recovered from UserLoginProcessor.
//! No wire success or passport authentication follows from constructing this value.
use super::{
    login_effects::EffectFlags,
    user_login_return::{UserLoginReturn, UserLoginSuccess},
};
use thiserror::Error;
use zeroize::Zeroizing;

/// Explicit SQL varchar decoder. CP1 has code page 1252 in the preserved reference.
#[derive(Clone, Copy)]
pub enum TokenProfile {
    SqlLatin1GeneralCp1,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompositionError {
    #[error("login text exceeds the recovered source column bound")]
    TextBound,
    #[error("login UTF16 source has odd byte length")]
    Utf16Length,
}
/// Sensitive text cannot be Debug/Serialize'd and is zeroized on drop.
pub struct LoginSuccess {
    ip: Zeroizing<Vec<u16>>,
    token: Zeroizing<Vec<u16>>,
    seconds: i32,
    limited: bool,
    flags: EffectFlags,
}
impl LoginSuccess {
    pub fn feedback(&self) -> UserLoginReturn<'_> {
        UserLoginReturn::Success(UserLoginSuccess {
            package_count: 1, // UserLoginProcessor's literal, not the number of queued rewards.
            last_login_ip: Some(&self.ip),
            lock_last_seconds: self.seconds,
            limited: i32::from(self.limited),
            is_dormant: self.flags.is_dormant,
            facebook_token: Some(&self.token),
            is_return: self.flags.is_return,
        })
    }
}
pub fn decode_token(
    bytes: Option<&[u8]>,
    profile: TokenProfile,
) -> Result<Zeroizing<Vec<u16>>, CompositionError> {
    let bytes = bytes.unwrap_or_default();
    if bytes.len() > 255 {
        return Err(CompositionError::TextBound);
    }
    match profile {
        TokenProfile::SqlLatin1GeneralCp1 => {}
    }
    // Framework CP1252 preserves the five undefined positions as control code units.
    const HIGH: [u16; 32] = [
        0x20ac, 0x81, 0x201a, 0x192, 0x201e, 0x2026, 0x2020, 0x2021, 0x2c6, 0x2030, 0x160, 0x2039,
        0x152, 0x8d, 0x17d, 0x8f, 0x90, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014,
        0x2dc, 0x2122, 0x161, 0x203a, 0x153, 0x9d, 0x17e, 0x178,
    ];
    Ok(Zeroizing::new(
        bytes
            .iter()
            .map(|&b| {
                if (0x80..=0x9f).contains(&b) {
                    HIGH[usize::from(b - 0x80)]
                } else {
                    u16::from(b)
                }
            })
            .collect(),
    ))
}
pub fn compose_user_login(
    previous_ip_utf16: Option<&[u8]>,
    raw_lock_seconds: i32,
    limited: bool,
    flags: EffectFlags,
    token_bytes: Option<&[u8]>,
    profile: TokenProfile,
) -> Result<LoginSuccess, CompositionError> {
    let ip = previous_ip_utf16.unwrap_or_default();
    if ip.len() > 100 {
        return Err(CompositionError::TextBound);
    }
    if !ip.len().is_multiple_of(2) {
        return Err(CompositionError::Utf16Length);
    }
    let seconds = if previous_ip_utf16 == Some(&[]) {
        0
    } else {
        raw_lock_seconds
    };
    Ok(LoginSuccess {
        ip: Zeroizing::new(
            ip.as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect(),
        ),
        token: decode_token(token_bytes, profile)?,
        seconds,
        limited,
        flags,
    })
}
