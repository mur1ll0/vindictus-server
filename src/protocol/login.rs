//! Recovered UserLoginMessage payload, without a hardcoded category/opcode.
//! Field order comes from the original managed metadata and SerializeWriterHelper.
//! A valid payload is not authentication; passport validation and identity mapping are pending.
pub use super::wire::PayloadError;
use super::wire::{Reader, Writer};

/// Borrowed login fields. Debug is intentionally absent to avoid printing passports.
#[derive(PartialEq, Eq)]
pub struct UserLoginMessage<'a> {
    /// Legacy passport; not a plaintext password field.
    pub passport: Option<&'a str>,
    /// Reported client IPv4 value.
    pub local_address: u32,
    /// Reported hardware identifier.
    pub hw_id: Option<&'a str>,
    /// Reported machine identifier, nullable in the original serializer.
    pub machine_id: Option<&'a [u8]>,
    /// Legacy game room indicator.
    pub game_room_client: i32,
    /// Whether the client requests skipping character selection.
    pub is_character_select_skipped: bool,
    /// Reported Nexon account name.
    pub nexon_id: Option<&'a str>,
    /// Client version information.
    pub up_to_date_info: Option<&'a str>,
    /// Reported signed checksum.
    pub check_sum: i64,
}

/// Decode a complete payload with borrowed fields and no allocation from remote lengths.
/// The serialized null object (one zero byte) returns None.
pub fn decode_user_login(
    body: &[u8],
    max_body: usize,
) -> Result<Option<UserLoginMessage<'_>>, PayloadError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge);
    }
    let mut reader = Reader { remaining: body };
    let value = if reader.byte()? == 0 {
        None
    } else {
        Some(UserLoginMessage {
            passport: reader.string()?,
            local_address: u32::from_be_bytes(reader.fixed()?),
            hw_id: reader.string()?,
            machine_id: reader.bytes()?,
            game_room_client: i32::from_be_bytes(reader.fixed()?),
            is_character_select_skipped: reader.byte()? != 0,
            nexon_id: reader.string()?,
            up_to_date_info: reader.string()?,
            check_sum: i64::from_be_bytes(reader.fixed()?),
        })
    };
    if !reader.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes);
    }
    Ok(value)
}

/// Encode recovered fields in original metadata order; caller assigns a verified category later.
pub fn encode_user_login(
    value: Option<&UserLoginMessage<'_>>,
    max_body: usize,
) -> Result<Vec<u8>, PayloadError> {
    let mut writer = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    if let Some(value) = value {
        writer.append(&[1])?;
        writer.nullable(value.passport.map(str::as_bytes))?;
        writer.append(&value.local_address.to_be_bytes())?;
        writer.nullable(value.hw_id.map(str::as_bytes))?;
        writer.nullable(value.machine_id)?;
        writer.append(&value.game_room_client.to_be_bytes())?;
        writer.append(&[u8::from(value.is_character_select_skipped)])?;
        writer.nullable(value.nexon_id.map(str::as_bytes))?;
        writer.nullable(value.up_to_date_info.map(str::as_bytes))?;
        writer.append(&value.check_sum.to_be_bytes())?;
    } else {
        writer.append(&[0])?;
    }
    Ok(writer.bytes)
}
