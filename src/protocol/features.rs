//! Recovered SyncFeatureMatrixMessage string dictionary codec.
//! This handles wire shape only; region/overrides and content import are separate.
use super::encode_integer;
use super::wire::{PayloadError, Reader, Writer};
use std::collections::HashSet;

/// One feature key and its nullable serialized value.
#[derive(PartialEq, Eq)]
pub struct FeatureEntry<'a> {
    /// Non-null dictionary key.
    pub key: &'a str,
    /// Nullable string; null is distinct from empty or disabled.
    pub value: Option<&'a str>,
}
/// Borrowed strings in a count-bounded dictionary, distinct from a null object.
#[derive(PartialEq, Eq)]
pub struct FeatureMatrix<'a> {
    /// None is a null dictionary; Some(empty) is an empty one.
    pub features: Option<Vec<FeatureEntry<'a>>>,
}
/// Decode a complete SyncFeatureMatrix payload with field strings borrowed from input.
/// Bounds and minimum byte counts are checked before allocating entry/index capacity.
pub fn decode_feature_matrix(
    body: &[u8],
    max_body: usize,
    max_entries: usize,
) -> Result<Option<FeatureMatrix<'_>>, PayloadError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge);
    }
    let mut reader = Reader { remaining: body };
    let message = if reader.byte()? == 0 {
        None
    } else {
        let features = if let Some(count) = reader.count()? {
            if count > max_entries {
                return Err(PayloadError::TooLarge);
            }
            if count > reader.remaining.len() / 2 {
                return Err(PayloadError::Truncated);
            }
            let mut entries = Vec::with_capacity(count);
            let mut keys = HashSet::with_capacity(count);
            for _ in 0..count {
                let key = reader.string()?.ok_or(PayloadError::NullKey)?;
                if !keys.insert(key) {
                    return Err(PayloadError::DuplicateKey);
                }
                entries.push(FeatureEntry {
                    key,
                    value: reader.string()?,
                });
            }
            Some(entries)
        } else {
            None
        };
        Some(FeatureMatrix { features })
    };
    if !reader.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes);
    }
    Ok(message)
}
/// Encode the recovered sealed-message presence marker and string dictionary.
pub fn encode_feature_matrix(
    value: Option<&FeatureMatrix<'_>>,
    max_body: usize,
    max_entries: usize,
) -> Result<Vec<u8>, PayloadError> {
    let mut writer = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    if let Some(value) = value {
        writer.append(&[1])?;
        if let Some(features) = &value.features {
            if features.len() > max_entries || features.len() > max_body / 2 {
                return Err(PayloadError::TooLarge);
            }
            let count = features
                .len()
                .checked_add(1)
                .and_then(|n| i32::try_from(n).ok())
                .ok_or(PayloadError::TooLarge)?;
            writer.append(&encode_integer(count))?;
            let mut keys = HashSet::with_capacity(features.len());
            for feature in features {
                if !keys.insert(feature.key) {
                    return Err(PayloadError::DuplicateKey);
                }
                writer.nullable(Some(feature.key.as_bytes()))?;
                writer.nullable(feature.value.map(str::as_bytes))?;
            }
        } else {
            writer.append(&[0])?;
        }
    } else {
        writer.append(&[0])?;
    }
    Ok(writer.bytes)
}
