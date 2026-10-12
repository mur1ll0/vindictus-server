//! Source ToArgInt decision chain with explicit culture, clock and local-zone ports.
//! This module does not implement the Framework's general DateTime.TryParse grammar.
use super::item_class_ex::try_integer_argument;
use thiserror::Error;

const MAX_TICKS: i64 = 3_155_378_975_999_999_999;
const EPOCH_TICKS: i64 = 621_355_968_000_000_000;
const TICKS_PER_SECOND: i64 = 10_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateTimeKind {
    Unspecified,
    Utc,
    Local,
}

/// Valid CLR DateTime ticks; Kind affects only the source's local-to-UTC branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgumentDateTime {
    ticks: i64,
    kind: DateTimeKind,
}
impl ArgumentDateTime {
    pub fn new(ticks: i64, kind: DateTimeKind) -> Result<Self, ArgumentError> {
        if !(0..=MAX_TICKS).contains(&ticks) {
            return Err(ArgumentError::DateRange);
        }
        Ok(Self { ticks, kind })
    }
    pub fn ticks(self) -> i64 {
        self.ticks
    }
    pub fn kind(self) -> DateTimeKind {
        self.kind
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArgumentError {
    #[error("item argument exceeds the queue UTF16 bound")]
    Bound,
    #[error("item date is outside the CLR range")]
    DateRange,
    #[error("explicit legacy temporal profile is unavailable")]
    ProfileUnavailable,
    #[error("temporal profile returned an inconsistent result")]
    ProfileContract,
}

/// All calls are lazy and source-ordered. None from parse_date means proven invalid;
/// unsupported grammar/culture must return an error, never None.
/// Providers must pin culture (including integer signs), date defaults and zone rules.
/// ends_with_seconds must reproduce culture-sensitive String.EndsWith("sec").
/// Local conversion includes source boundary clamping and DST behavior; no inferred offset.
/// now_local is sampled once per relative argument, not once per parsed expression.
pub trait ArgumentContext {
    fn parse_date(&mut self, text: &[u16]) -> Result<Option<ArgumentDateTime>, ArgumentError>;
    fn ends_with_seconds(&mut self, text: &[u16]) -> Result<bool, ArgumentError>;
    fn now_local(&mut self) -> Result<ArgumentDateTime, ArgumentError>;
    fn local_to_utc(&mut self, local: ArgumentDateTime) -> Result<ArgumentDateTime, ArgumentError>;
}

/// Source Utility.Extention.ToInt32: truncate signed seconds toward zero, then
/// retain the low 32 bits (unchecked CLR cast), even beyond the Unix 2038 range.
pub fn date_to_argument(
    date: ArgumentDateTime,
    context: &mut impl ArgumentContext,
) -> Result<i32, ArgumentError> {
    let date = if date.kind == DateTimeKind::Local {
        let utc = context.local_to_utc(date)?;
        if utc.kind != DateTimeKind::Utc {
            return Err(ArgumentError::ProfileContract);
        }
        utc
    } else {
        date
    };
    let seconds = (date.ticks - EPOCH_TICKS) / TICKS_PER_SECOND;
    let bytes = seconds.to_le_bytes();
    Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// Recovered null -> integer -> date -> relative seconds -> invalid zero chain.
/// Typed profile errors deliberately differ from the source's invalid-input fallback.
/// Logging the source argument is deliberately omitted.
pub fn resolve(
    text: Option<&[u16]>,
    context: &mut impl ArgumentContext,
) -> Result<i32, ArgumentError> {
    let Some(text) = text else {
        return Ok(0);
    };
    if text.len() > 512 {
        return Err(ArgumentError::Bound);
    }
    if let Some(integer) = try_integer_argument(text) {
        return Ok(integer);
    }
    if let Some(date) = context.parse_date(text)? {
        return date_to_argument(date, context);
    }
    if context.ends_with_seconds(text)? {
        let end = text
            .len()
            .checked_sub(3)
            .ok_or(ArgumentError::ProfileContract)?;
        if let Some(seconds) = try_integer_argument(&text[..end]) {
            let now = context.now_local()?;
            if now.kind != DateTimeKind::Local {
                return Err(ArgumentError::ProfileContract);
            }
            // Framework AddSeconds rounds to milliseconds; an i32 number of seconds
            // is exact. Preserve all original clock ticks, including submillisecond ticks.
            let date = ArgumentDateTime::new(
                now.ticks + i64::from(seconds) * TICKS_PER_SECOND,
                DateTimeKind::Local,
            )?;
            return date_to_argument(date, context);
        }
    }
    Ok(0)
}
