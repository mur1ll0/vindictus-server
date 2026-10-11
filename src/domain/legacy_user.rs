//! The source-identified insert candidate constructed by UserLoginProcessor.
//! This does not establish authentication, perform name lookup or persist any data.
use thiserror::Error;

/// Fields set or left at CLR defaults before InsertOnSubmit.
#[derive(PartialEq, Eq)]
pub struct LegacyUserInsert<'a> {
    /// Signed NexonSN supplied by the authenticated frontend entity; not an identity sequence.
    pub id: i32,
    /// Database name is limited to 24 UTF-16 code units, preserving original case.
    pub name: &'a str,
    /// The recovered CLR field defaults to zero (there is no SQL default).
    pub next_character_sn: i32,
    /// LoginService instance id, explicitly supplied by the caller.
    pub service_id: i32,
    /// Zero channel becomes NULL; nonzero tinyint is stored as supplied.
    pub channeling_code: Option<u8>,
    /// These mapped properties are initially CLR null.
    pub last_login_ip: Option<&'a str>,
    /// Separate secondary credential; it is never interpreted as a primary password here.
    pub second_password: Option<&'a str>,
    /// Initially null, distinct from a zero failure count.
    pub second_password_fail_count: Option<i32>,
    /// Initially null; not a credential or proof of authentication.
    pub facebook_token: Option<&'a str>,
}

/// Database-name bound before constructing an insert candidate.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LegacyUserError {
    /// SQL nvarchar(24) measures UTF-16 units, not UTF8 bytes or Unicode scalar values.
    #[error("legacy user name exceeds 24 UTF-16 units")]
    NameTooLong,
}

/// Reproduce the new-user object initializer, with the database's name bound enforced early.
/// Identity/collation/collision checks belong to a future authenticated persistence operation.
pub fn prepare_legacy_user(
    id: i32,
    name: &str,
    service_id: i32,
    channel_code: u8,
) -> Result<LegacyUserInsert<'_>, LegacyUserError> {
    if name.encode_utf16().take(25).count() > 24 {
        return Err(LegacyUserError::NameTooLong);
    }
    Ok(LegacyUserInsert {
        id,
        name,
        next_character_sn: 0,
        service_id,
        channeling_code: (channel_code != 0).then_some(channel_code),
        last_login_ip: None,
        second_password: None,
        second_password_fail_count: None,
        facebook_token: None,
    })
}
