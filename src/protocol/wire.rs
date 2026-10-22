//! Bounded recovered primitive payload fields shared by concrete message codecs.
use super::{ProtocolError, decode_integer, encode_integer};
use thiserror::Error;
/// Bounded payload decoding or encoding error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PayloadError {
    /// Advertised or accumulated data exceeds the supplied bound.
    #[error("payload exceeds configured limit")]
    TooLarge,
    /// A required field is incomplete.
    #[error("truncated payload")]
    Truncated,
    /// Collection/string count cannot be negative.
    #[error("negative field length")]
    NegativeLength,
    /// Malformed strings are rejected rather than silently replaced.
    #[error("invalid UTF-8")]
    InvalidUtf8,
    /// The message contains unconsumed trailing bytes.
    #[error("trailing payload bytes")]
    TrailingBytes,
    /// A string dictionary cannot contain a null key.
    #[error("null dictionary key")]
    NullKey,
    /// String dictionary keys must be unique.
    #[error("duplicate dictionary key")]
    DuplicateKey,
    /// A recovered nullable field whose non-null form is not migrated.
    #[error("unsupported non-null field")]
    Unsupported,
    /// Invalid variable integer.
    #[error(transparent)]
    Integer(#[from] ProtocolError),
}

pub(crate) struct Reader<'a> {
    pub(crate) remaining: &'a [u8],
}
impl<'a> Reader<'a> {
    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8], PayloadError> {
        let value = self.remaining.get(..count).ok_or(PayloadError::Truncated)?;
        self.remaining = &self.remaining[count..];
        Ok(value)
    }
    pub(crate) fn byte(&mut self) -> Result<u8, PayloadError> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn count(&mut self) -> Result<Option<usize>, PayloadError> {
        let (count, used) = decode_integer(self.remaining)?.ok_or(PayloadError::Truncated)?;
        self.take(used)?;
        let count = usize::try_from(count).map_err(|_| PayloadError::NegativeLength)?;
        Ok(count.checked_sub(1))
    }
    pub(crate) fn bytes(&mut self) -> Result<Option<&'a [u8]>, PayloadError> {
        let (count, used) = decode_integer(self.remaining)?.ok_or(PayloadError::Truncated)?;
        self.take(used)?;
        let count = usize::try_from(count).map_err(|_| PayloadError::NegativeLength)?;
        if count == 0 {
            return Ok(None);
        }
        Ok(Some(self.take(count - 1)?))
    }
    pub(crate) fn string(&mut self) -> Result<Option<&'a str>, PayloadError> {
        self.bytes()?
            .map(|value| std::str::from_utf8(value).map_err(|_| PayloadError::InvalidUtf8))
            .transpose()
    }
    pub(crate) fn fixed<const N: usize>(&mut self) -> Result<[u8; N], PayloadError> {
        let mut result = [0; N];
        result.copy_from_slice(self.take(N)?);
        Ok(result)
    }
}

pub(crate) struct Writer {
    pub(crate) bytes: Vec<u8>,
    pub(crate) limit: usize,
}
impl Writer {
    pub(crate) fn append(&mut self, value: &[u8]) -> Result<(), PayloadError> {
        let length = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or(PayloadError::TooLarge)?;
        if length > self.limit {
            return Err(PayloadError::TooLarge);
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    pub(crate) fn nullable(&mut self, value: Option<&[u8]>) -> Result<(), PayloadError> {
        let Some(value) = value else {
            return self.append(&[0]);
        };
        let count = value
            .len()
            .checked_add(1)
            .and_then(|v| i32::try_from(v).ok())
            .ok_or(PayloadError::TooLarge)?;
        self.append(&encode_integer(count))?;
        self.append(value)
    }
}
