//! Temporal branches of ItemRequestProperty/Item.Initialize for the explicit en-US profile.
//! These transformations do not construct attributes, initialize stats or deliver an item.
use super::{
    item_argument_profile::ticks, item_class_ex::try_integer_argument,
    legacy_user_data::SqlDateTime2,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ItemTimeError {
    #[error("item time attribute exceeds its verified UTF16 bound")]
    Bound,
    #[error("EXPIRE has a null value in the recovered property constructor")]
    NullExpire,
    #[error("item expiry is outside the Framework DateTime range")]
    DateRange,
    #[error("item UTC clock is unavailable")]
    Clock,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpiryUpdate {
    pub expiry: Option<SqlDateTime2>,
    pub remove_expire_attribute: bool,
}
fn unchanged(expiry: Option<SqlDateTime2>) -> ExpiryUpdate {
    ExpiryUpdate {
        expiry,
        remove_expire_attribute: false,
    }
}
/// EXPIRE absence, present NULL and present text are distinct. Gregorian construction
/// is independent of the current calendar; integer syntax is the verified en-US profile.
/// Malformed dates are caught by the original, but NULL Split fails outside its catch.
pub fn property_expiry(
    expiry: Option<SqlDateTime2>,
    attribute: Option<Option<&[u16]>>,
) -> Result<ExpiryUpdate, ItemTimeError> {
    let Some(attribute) = attribute else {
        return Ok(unchanged(expiry));
    };
    let text = attribute.ok_or(ItemTimeError::NullExpire)?;
    if text.len() > 1024 {
        return Err(ItemTimeError::Bound);
    }
    let mut fields = text.split(|&c| c == 47);
    let mut values = [0_u32; 6];
    for value in &mut values {
        let Some(part) = fields.next() else {
            return Ok(unchanged(expiry));
        };
        let Some(number) = try_integer_argument(part).and_then(|n| u32::try_from(n).ok()) else {
            return Ok(unchanged(expiry));
        };
        *value = number;
    }
    if fields.next().is_some() {
        return Ok(unchanged(expiry));
    }
    let Some(date) = ticks(
        values[0],
        values[1],
        values[2],
        (values[3], values[4], values[5], 0),
    ) else {
        return Ok(unchanged(expiry));
    };
    let date = SqlDateTime2::from_ticks(date).map_err(|_| ItemTimeError::DateRange)?;
    if expiry.is_some_and(|v| v.ticks() < date.ticks()) {
        Ok(ExpiryUpdate {
            expiry,
            remove_expire_attribute: true,
        })
    } else {
        Ok(ExpiryUpdate {
            expiry: Some(date),
            remove_expire_attribute: false,
        })
    }
}
/// Only a present NULL/empty MIN_UPTIME samples UTC; its value is decimal ticks,
/// as Utility.Extention.ToFormatableString specifies, with no timezone conversion.
/// None means leave the attribute unchanged, including when it is absent.
pub fn minimum_uptime(
    attribute: Option<Option<&[u16]>>,
    utc_now: &mut impl FnMut() -> Result<SqlDateTime2, ItemTimeError>,
) -> Result<Option<Vec<u16>>, ItemTimeError> {
    let Some(value) = attribute else {
        return Ok(None);
    };
    if value.is_some_and(|v| v.len() > 1024) {
        return Err(ItemTimeError::Bound);
    }
    if value.is_some_and(|v| !v.is_empty()) {
        return Ok(None);
    }
    Ok(Some(
        utc_now()?.ticks().to_string().encode_utf16().collect(),
    ))
}
/// The source samples UTC whenever ExpireIn is present, even if it cannot replace
/// an explicit expiry. An earlier class expiry removes EXPIRE only when present.
/// Integer seconds preserve submillisecond UTC ticks under Framework AddSeconds.
pub fn initialize_expiry(
    expiry: Option<SqlDateTime2>,
    has_expire_attribute: bool,
    expire_in: Option<i32>,
    utc_now: &mut impl FnMut() -> Result<SqlDateTime2, ItemTimeError>,
) -> Result<ExpiryUpdate, ItemTimeError> {
    let Some(seconds) = expire_in else {
        return Ok(unchanged(expiry));
    };
    let candidate = utc_now()?.ticks() + i64::from(seconds) * 10_000_000;
    let candidate = SqlDateTime2::from_ticks(candidate).map_err(|_| ItemTimeError::DateRange)?;
    if expiry.is_none()
        || (has_expire_attribute && expiry.is_some_and(|v| candidate.ticks() < v.ticks()))
    {
        Ok(ExpiryUpdate {
            expiry: Some(candidate),
            remove_expire_attribute: expiry.is_some() && has_expire_attribute,
        })
    } else {
        Ok(unchanged(expiry))
    }
}
