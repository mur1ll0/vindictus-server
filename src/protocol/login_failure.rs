//! Recovered LoginFailMessage fields; a failure response never authenticates an account.
use super::wire::{PayloadError, Reader, Writer};

/// Original PassportCheck uses this reason when login permissions are insufficient.
pub const PERMISSION_FAILED: i32 = 13;

/// Signed reason and optional explanation from the recovered message.
#[derive(PartialEq, Eq)]
pub struct LoginFailMessage<'a> {
    /// FailReason numeric value from this server version.
    pub reason: i32,
    /// Nullable localized explanation. Debug is omitted to prevent incidental data logging.
    pub banned_reason: Option<&'a str>,
}

/// Serialize this sealed message body (without a type GUID).
pub fn encode_login_failure(
    value: Option<&LoginFailMessage<'_>>,
    max_body: usize,
) -> Result<Vec<u8>, PayloadError> {
    let mut writer = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    match value {
        Some(value) => {
            writer.append(&[1])?;
            writer.append(&value.reason.to_be_bytes())?;
            writer.nullable(value.banned_reason.map(str::as_bytes))?;
        }
        None => writer.append(&[0])?,
    }
    Ok(writer.bytes)
}

/// Decode the original presence, signed int32 and nullable UTF8 string.
pub fn decode_login_failure(
    body: &[u8],
    max_body: usize,
) -> Result<Option<LoginFailMessage<'_>>, PayloadError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge);
    }
    let mut reader = Reader { remaining: body };
    let value = if reader.byte()? == 0 {
        None
    } else {
        Some(LoginFailMessage {
            reason: i32::from_be_bytes(reader.fixed()?),
            banned_reason: reader.string()?,
        })
    };
    if !reader.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes);
    }
    Ok(value)
}
