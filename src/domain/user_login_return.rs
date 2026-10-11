//! Typed internal cooperation feedback, not a native client packet or authentication proof.
use thiserror::Error;

/// .NET strings as exact UTF16 code units, including unpaired surrogates and embedded NUL.
pub enum LoginFeedback<'a> {
    Integer(i32),
    Text(Option<&'a [u16]>),
    Boolean(bool),
    FailureReason(i32),
}
pub struct UserLoginSuccess<'a> {
    pub package_count: i32,
    pub last_login_ip: Option<&'a [u16]>,
    pub lock_last_seconds: i32,
    pub limited: i32,
    pub is_dormant: bool,
    pub facebook_token: Option<&'a [u16]>,
    pub is_return: bool,
}
pub enum UserLoginReturn<'a> {
    Success(UserLoginSuccess<'a>),
    Failure {
        reason: i32,
        banned_reason: Option<&'a [u16]>,
    },
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LoginReturnError {
    #[error("internal UserLogin feedback is incomplete, has extra fields or wrong types")]
    Shape,
    #[error("internal UserLogin text exceeds configured UTF16 bound")]
    TextBound,
}
fn text<'a>(
    field: &LoginFeedback<'a>,
    limit: usize,
) -> Result<Option<&'a [u16]>, LoginReturnError> {
    match field {
        LoginFeedback::Text(value) if value.is_none_or(|v| v.len() <= limit) => Ok(*value),
        LoginFeedback::Text(_) => Err(LoginReturnError::TextBound),
        _ => Err(LoginReturnError::Shape),
    }
}
fn integer(field: &LoginFeedback<'_>) -> Result<i32, LoginReturnError> {
    if let LoginFeedback::Integer(v) = field {
        Ok(*v)
    } else {
        Err(LoginReturnError::Shape)
    }
}
fn boolean(field: &LoginFeedback<'_>) -> Result<bool, LoginReturnError> {
    if let LoginFeedback::Boolean(v) = field {
        Ok(*v)
    } else {
        Err(LoginReturnError::Shape)
    }
}
/// The first integer is a success/package count, even when numerically equal to Banned.
/// FailureReason is a distinct runtime type, preserving unknown signed enum values.
pub fn decode_user_login_return<'a>(
    feedback: &[LoginFeedback<'a>],
    max_text_units: usize,
) -> Result<UserLoginReturn<'a>, LoginReturnError> {
    match feedback {
        [
            LoginFeedback::Integer(package),
            ip,
            seconds,
            limited,
            dormant,
            token,
            returning,
        ] => Ok(UserLoginReturn::Success(UserLoginSuccess {
            package_count: *package,
            last_login_ip: text(ip, max_text_units)?,
            lock_last_seconds: integer(seconds)?,
            limited: integer(limited)?,
            is_dormant: boolean(dormant)?,
            facebook_token: text(token, max_text_units)?,
            is_return: boolean(returning)?,
        })),
        [LoginFeedback::FailureReason(reason)] if *reason != 4 => Ok(UserLoginReturn::Failure {
            reason: *reason,
            banned_reason: Some(&[]),
        }),
        [LoginFeedback::FailureReason(4), reason] => Ok(UserLoginReturn::Failure {
            reason: 4,
            banned_reason: text(reason, max_text_units)?,
        }),
        _ => Err(LoginReturnError::Shape),
    }
}
