//! Bounded en-US/Gregorian date grammar for the offline reference package.
//! Explicit UTC clock and fixed UTC+8; no host locale, timezone or date defaults.
use super::item_argument::{ArgumentContext, ArgumentDateTime, ArgumentError, DateTimeKind};

const OFFSET_TICKS: i64 = 480 * 60 * 10_000_000;
const MAX_TICKS: i64 = 3_155_378_975_999_999_999;

/// Explicitly selected offline profile, with a caller-supplied UTC clock.
/// The context supports complete numeric dates and ASCII relative seconds only.
/// It is not automatically connected to the live queue consumer.
pub struct ReferenceVmArgumentContext<C> {
    utc_clock: C,
}
impl<C: FnMut() -> Result<ArgumentDateTime, ArgumentError>> ReferenceVmArgumentContext<C> {
    pub fn new(utc_clock: C) -> Self {
        Self { utc_clock }
    }
}
fn ws(c: u16) -> bool {
    matches!(c, 0x9..=0xd | 0x20)
}
fn trim(mut text: &[u16]) -> &[u16] {
    while text.first().is_some_and(|&c| ws(c)) {
        text = &text[1..];
    }
    while text.last().is_some_and(|&c| ws(c)) {
        text = &text[..text.len() - 1];
    }
    text
}
fn number(text: &[u16], min: usize, max: usize) -> Result<u32, ArgumentError> {
    if !(min..=max).contains(&text.len()) {
        return Err(ArgumentError::ProfileUnavailable);
    }
    text.iter().try_fold(0_u32, |n, &c| {
        if !(0x30..=0x39).contains(&c) {
            return Err(ArgumentError::ProfileUnavailable);
        }
        Ok(n * 10 + u32::from(c - 0x30))
    })
}
fn integer_shape(mut text: &[u16]) -> bool {
    text = trim(text);
    if matches!(text.first(), Some(0x2b | 0x2d)) {
        text = &text[1..];
    }
    !text.is_empty() && text.iter().all(|c| (0x30..=0x39).contains(c))
}
fn date_fields(text: &[u16]) -> Result<(u32, u32, u32), ArgumentError> {
    let slash = text.contains(&0x2f);
    let mut parts = text.split(|&c| c == if slash { 0x2f } else { 0x2d });
    let first = parts.next().ok_or(ArgumentError::ProfileUnavailable)?;
    let second = parts.next().ok_or(ArgumentError::ProfileUnavailable)?;
    let third = parts.next().ok_or(ArgumentError::ProfileUnavailable)?;
    if parts.next().is_some() {
        return Err(ArgumentError::ProfileUnavailable);
    }
    if slash {
        // en-US M/d/yyyy, with no two-digit-year calendar cutoff.
        Ok((
            number(third, 4, 4)?,
            number(first, 1, 2)?,
            number(second, 1, 2)?,
        ))
    } else {
        Ok((
            number(first, 4, 4)?,
            number(second, 2, 2)?,
            number(third, 2, 2)?,
        ))
    }
}
fn time_fields(mut text: &[u16], iso: bool) -> Result<(u32, u32, u32, u32), ArgumentError> {
    let mut meridiem = None;
    if text.len() >= 3 {
        let end = &text[text.len() - 3..];
        if end[0] == 0x20
            && matches!(end[1], 0x41 | 0x61 | 0x50 | 0x70)
            && matches!(end[2], 0x4d | 0x6d)
        {
            if iso {
                return Err(ArgumentError::ProfileUnavailable);
            }
            meridiem = Some(matches!(end[1], 0x50 | 0x70));
            text = &text[..text.len() - 3];
        }
    }
    let mut fractional = text.split(|&c| c == 0x2e);
    let base = fractional.next().ok_or(ArgumentError::ProfileUnavailable)?;
    let fraction = if let Some(units) = fractional.next() {
        number(units, 1, 7)? * 10_u32.pow(7 - units.len() as u32)
    } else {
        0
    };
    if fractional.next().is_some() {
        return Err(ArgumentError::ProfileUnavailable);
    }
    let mut parts = base.split(|&c| c == 0x3a);
    let mut hour = number(
        parts.next().ok_or(ArgumentError::ProfileUnavailable)?,
        if iso { 2 } else { 1 },
        2,
    )?;
    let minute = number(parts.next().ok_or(ArgumentError::ProfileUnavailable)?, 2, 2)?;
    let second = number(parts.next().ok_or(ArgumentError::ProfileUnavailable)?, 2, 2)?;
    if parts.next().is_some() {
        return Err(ArgumentError::ProfileUnavailable);
    }
    if let Some(pm) = meridiem {
        if !(1..=12).contains(&hour) {
            return Err(ArgumentError::ProfileUnavailable);
        }
        hour = hour % 12 + if pm { 12 } else { 0 };
    }
    Ok((hour, minute, second, fraction))
}
fn ticks(year: u32, month: u32, day: u32, time: (u32, u32, u32, u32)) -> Option<i64> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let lengths = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=lengths[(month - 1) as usize]).contains(&day) {
        return None;
    }
    let (hour, minute, second, fraction) = time;
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let prior = i64::from(year - 1);
    let days = prior * 365 + prior / 4 - prior / 100
        + prior / 400
        + lengths[..(month - 1) as usize]
            .iter()
            .map(|&n| i64::from(n))
            .sum::<i64>()
        + i64::from(day - 1);
    Some(
        (days * 86400 + i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second))
            * 10_000_000
            + i64::from(fraction),
    )
}
/// Supported canonical forms only. Unknown formats are errors, not invalid zero.
pub fn parse_reference_date(text: &[u16]) -> Result<Option<ArgumentDateTime>, ArgumentError> {
    if text.len() > 512 {
        return Err(ArgumentError::Bound);
    }
    if text.iter().any(|&c| c > 0x7f) {
        return Err(ArgumentError::ProfileUnavailable);
    }
    let text = trim(text);
    if text.is_empty() || integer_shape(text) {
        return Ok(None);
    }
    if text.ends_with(&[0x73, 0x65, 0x63]) {
        let prefix = trim(&text[..text.len() - 3]);
        if prefix.is_empty() || integer_shape(prefix) {
            return Ok(None);
        }
    }
    let split = text.iter().position(|&c| matches!(c, 0x20 | 0x54));
    let (date, time) = if let Some(index) = split {
        (
            &text[..index],
            time_fields(&text[index + 1..], text[index] == 0x54)?,
        )
    } else {
        (text, (0, 0, 0, 0))
    };
    let (year, month, day) = date_fields(date)?;
    if split.is_some_and(|index| text[index] == 0x54) && date.contains(&0x2f) {
        // Framework's ISO T branch rejects en-US month/day/year date order.
        // This recognized combination is invalid, rather than unknown grammar.
        return Ok(None);
    }
    ticks(year, month, day, time)
        .map(|ticks| ArgumentDateTime::new(ticks, DateTimeKind::Unspecified))
        .transpose()
}
impl<C: FnMut() -> Result<ArgumentDateTime, ArgumentError>> ArgumentContext
    for ReferenceVmArgumentContext<C>
{
    fn parse_date(&mut self, text: &[u16]) -> Result<Option<ArgumentDateTime>, ArgumentError> {
        parse_reference_date(text)
    }
    fn ends_with_seconds(&mut self, text: &[u16]) -> Result<bool, ArgumentError> {
        if text.len() > 512 {
            return Err(ArgumentError::Bound);
        }
        if text.iter().any(|&c| c > 0x7f || c == 0) {
            return Err(ArgumentError::ProfileUnavailable);
        }
        Ok(text.ends_with(&[0x73, 0x65, 0x63]))
    }
    fn now_local(&mut self) -> Result<ArgumentDateTime, ArgumentError> {
        let utc = (self.utc_clock)()?;
        if utc.kind() != DateTimeKind::Utc {
            return Err(ArgumentError::ProfileContract);
        }
        ArgumentDateTime::new(
            (utc.ticks() + OFFSET_TICKS).min(MAX_TICKS),
            DateTimeKind::Local,
        )
    }
    fn local_to_utc(&mut self, local: ArgumentDateTime) -> Result<ArgumentDateTime, ArgumentError> {
        if local.kind() != DateTimeKind::Local {
            return Err(ArgumentError::ProfileContract);
        }
        ArgumentDateTime::new((local.ticks() - OFFSET_TICKS).max(0), DateTimeKind::Utc)
    }
}
