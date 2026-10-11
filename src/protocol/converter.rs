//! Recovered category-zero TypeConverter for plain (non-array, non-generic) message types.
//! GUID layout is confirmed with the recovered serializer on .NET Framework BCL.
use super::{ProtocolError, decode_integer, encode_integer};
use thiserror::Error;
use uuid::Uuid;

/// One category binding in the recovered negotiation table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassRegistration {
    /// Signed category id, encoded as a fixed big-endian int32 within the dictionary.
    pub category_id: i32,
    /// CLR type GUID encoded in network field order, not Guid.ToByteArray order.
    pub type_id: Uuid,
}
/// Converter payload error before allocation from any remote length.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConverterError {
    /// Body or count exceeds caller bounds.
    #[error("converter exceeds configured bounds")]
    TooLarge,
    /// Incomplete converter fields.
    #[error("truncated converter")]
    Truncated,
    /// Only the recovered TypeConverter GUID is accepted here.
    #[error("unsupported converter type")]
    UnsupportedType,
    /// Count cannot be negative.
    #[error("negative converter count")]
    NegativeCount,
    /// Complete converter cannot contain trailing bytes.
    #[error("trailing converter bytes")]
    TrailingBytes,
    /// Malformed variable count.
    #[error(transparent)]
    Integer(#[from] ProtocolError),
}
/// Borrowed converter dictionary. Null and empty dictionaries remain distinct.
/// This is byte decoding only; callers must validate trusted GUIDs and category collisions.
pub struct TypeConverter<'a> {
    entries: Option<&'a [u8]>,
}
impl TypeConverter<'_> {
    /// True for a serialized null dictionary (which the legacy factory rejects).
    pub fn is_null(&self) -> bool {
        self.entries.is_none()
    }
    /// Iterate complete plain-type entries without allocating from advertised counts.
    pub fn entries(&self) -> impl Iterator<Item = ClassRegistration> + '_ {
        self.entries
            .unwrap_or_default()
            .as_chunks::<20>()
            .0
            .iter()
            .map(|bytes| {
                let mut category = [0; 4];
                category.copy_from_slice(&bytes[..4]);
                let mut guid = [0; 16];
                guid.copy_from_slice(&bytes[4..]);
                ClassRegistration {
                    category_id: i32::from_be_bytes(category),
                    type_id: Uuid::from_bytes(guid),
                }
            })
    }
}
/// Parse the exact plain-type converter payload with bounded count and no heap allocation.
/// Non-generic sealed types in the recovered server profile use this shape.
pub fn decode_converter(
    body: &[u8],
    max_body: usize,
    max_entries: usize,
) -> Result<TypeConverter<'_>, ConverterError> {
    if body.len() > max_body {
        return Err(ConverterError::TooLarge);
    }
    let type_id = body.get(..16).ok_or(ConverterError::Truncated)?;
    if type_id != [0xff; 16] {
        return Err(ConverterError::UnsupportedType);
    }
    let (count, used) = decode_integer(&body[16..])?.ok_or(ConverterError::Truncated)?;
    let count = usize::try_from(count).map_err(|_| ConverterError::NegativeCount)?;
    let start = 16 + used;
    let entries = if count == 0 {
        None
    } else {
        let count = count - 1;
        if count > max_entries {
            return Err(ConverterError::TooLarge);
        }
        let length = count.checked_mul(20).ok_or(ConverterError::TooLarge)?;
        Some(
            body.get(start..start.checked_add(length).ok_or(ConverterError::TooLarge)?)
                .ok_or(ConverterError::Truncated)?,
        )
    };
    let consumed = start + entries.map_or(0, <[u8]>::len);
    if consumed != body.len() {
        return Err(ConverterError::TrailingBytes);
    }
    Ok(TypeConverter { entries })
}
/// Serialize a plain-type converter, checking count and total size before allocation.
/// Null dictionaries are supported as wire data, not accepted as a valid negotiation.
pub fn encode_converter(
    entries: Option<&[ClassRegistration]>,
    max_body: usize,
) -> Result<Vec<u8>, ConverterError> {
    let count = entries.map_or(0, |v| v.len().saturating_add(1));
    let count = i32::try_from(count).map_err(|_| ConverterError::TooLarge)?;
    let count_bytes = encode_integer(count);
    let entry_bytes = entries
        .map_or(0, <[ClassRegistration]>::len)
        .checked_mul(20)
        .ok_or(ConverterError::TooLarge)?;
    let length = 16usize
        .checked_add(count_bytes.len())
        .and_then(|v| v.checked_add(entry_bytes))
        .ok_or(ConverterError::TooLarge)?;
    if length > max_body {
        return Err(ConverterError::TooLarge);
    }
    let mut result = Vec::with_capacity(length);
    result.extend_from_slice(&[0xff; 16]);
    result.extend_from_slice(&count_bytes);
    for entry in entries.unwrap_or_default() {
        result.extend_from_slice(&entry.category_id.to_be_bytes());
        result.extend_from_slice(entry.type_id.as_bytes());
    }
    Ok(result)
}
