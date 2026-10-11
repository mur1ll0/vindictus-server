//! Bounded, private SQL Server export reader. Never serialize or debug source credentials.
use crate::{
    domain::{game_user::GameUser, legacy_user_data::*},
    error::Error,
};
use serde::{Deserialize, Deserializer};
use std::io::Read;
use zeroize::{Zeroize, Zeroizing};

pub const MAX_ROWS: usize = 1_000;
pub const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(transparent)]
struct PrivateText(String);
impl Drop for PrivateText {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
// deserialize_with makes a nullable column required: missing is not silently NULL.
fn nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    exported_row_count: usize,
    rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: i32,
    name: PrivateText,
    next_character_sn: i32,
    service_id: i32,
    #[serde(deserialize_with = "nullable")]
    channeling_code: Option<i16>,
    #[serde(deserialize_with = "nullable")]
    last_login_ip_utf16_hex: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    ip_lock_time_ticks: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    facebook_token_hex: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    is_dormant_user_event_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    second_password_utf16_hex: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    second_password_fail_count: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    second_password_last_fail_ticks: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    last_update_hex: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    is_dormant_user_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    create_time_units300: Option<PrivateText>,
}
fn integer(text: PrivateText) -> Result<i64, Error> {
    if text.0.len() > 20 {
        return Err(Error::UserExport);
    }
    let number = text.0.parse::<i64>().map_err(|_| Error::UserExport)?;
    if number.to_string() != text.0 {
        return Err(Error::UserExport);
    }
    Ok(number)
}
fn hex(text: PrivateText, bound: usize) -> Result<Vec<u8>, Error> {
    let bytes = text.0.as_bytes();
    if bytes.len() > bound * 2 || !bytes.len().is_multiple_of(2) {
        return Err(Error::UserExport);
    }
    let mut decoded = Zeroizing::new(Vec::with_capacity(bytes.len() / 2));
    for pair in bytes.as_chunks::<2>().0 {
        fn digit(b: u8) -> Result<u8, Error> {
            match b {
                b'0'..=b'9' => Ok(b - b'0'),
                b'a'..=b'f' => Ok(b - b'a' + 10),
                b'A'..=b'F' => Ok(b - b'A' + 10),
                _ => Err(Error::UserExport),
            }
        }
        decoded.push(digit(pair[0])? * 16 + digit(pair[1])?);
    }
    Ok(std::mem::take(&mut *decoded))
}
impl Row {
    fn into_snapshot(mut self) -> Result<LegacyUserSnapshot, Error> {
        let snapshot = LegacyUserSnapshot {
            user: GameUser {
                id: self.id,
                name: std::mem::take(&mut self.name.0),
                next_character_sn: self.next_character_sn,
                service_id: self.service_id,
                channeling_code: self.channeling_code,
            },
            details: LegacyUserDetails {
                last_login_ip: self
                    .last_login_ip_utf16_hex
                    .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 100)?)?))
                    .transpose()?,
                ip_lock_time: self
                    .ip_lock_time_ticks
                    .map(|v| Ok::<_, Error>(SqlDateTime2::from_ticks(integer(v)?)?))
                    .transpose()?,
                facebook_token: self
                    .facebook_token_hex
                    .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 255)?)?))
                    .transpose()?,
                is_dormant_user_event: self
                    .is_dormant_user_event_units300
                    .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                    .transpose()?,
                second_password: self
                    .second_password_utf16_hex
                    .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 512)?)?))
                    .transpose()?,
                second_password_fail_count: self.second_password_fail_count,
                second_password_last_fail: self
                    .second_password_last_fail_ticks
                    .map(|v| Ok::<_, Error>(SqlDateTime2::from_ticks(integer(v)?)?))
                    .transpose()?,
                last_update: self
                    .last_update_hex
                    .map(|v| Ok::<_, Error>(SourceRowVersion::from_bytes(&hex(v, 8)?)?))
                    .transpose()?,
                is_dormant_user: self
                    .is_dormant_user_units300
                    .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                    .transpose()?,
                create_time: self
                    .create_time_units300
                    .map(|v| Ok::<_, Error>(SqlDateTime::from_units300(integer(v)?)?))
                    .transpose()?,
            },
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}
pub fn read_export(reader: impl Read) -> Result<Vec<LegacyUserSnapshot>, Error> {
    let mut input = Zeroizing::new(Vec::new());
    reader
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .map_err(|_| Error::UserExport)?;
    if input.len() > MAX_INPUT_BYTES {
        return Err(Error::UserExport);
    }
    // Discard serde's detailed errors: they may echo credential values.
    let export: Export = serde_json::from_slice(&input).map_err(|_| Error::UserExport)?;
    if export.format_version != 1
        || export.source != "study_heroes.dbo.User"
        || export.rows.len() > MAX_ROWS
        || export.exported_row_count != export.rows.len()
        || export.rows.is_empty()
    {
        return Err(Error::UserExport);
    }
    export.rows.into_iter().map(Row::into_snapshot).collect()
}
