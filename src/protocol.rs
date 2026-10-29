//! Recovered Devcat packet framing, before transport encryption.
//!
//! Source: server Core.dll SHA256 8942a0c26a9abf4c5f0d1f7ffff41452a3714d099e3b76012cebdee97293c2f0,
//! Devcat.Core.Net.Message.Packet. This implements framing only, not message
//! serialization, message negotiation, or ProudNet. The transport submodule implements ISAAC framing.
pub mod bootstrap;
pub mod converter;
pub mod features;
pub mod identity;
pub mod isaac;
pub mod login;
pub mod login_failure;
pub mod login_ingress;
pub mod second_password;
pub mod server_profile;
pub mod transport;

use thiserror::Error;

/// Default defensive body limit; the legacy analyzer warns at this size.
pub const DEFAULT_MAX_BODY: usize = 102_400;

/// Invalid data discovered while parsing a recovered packet.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    /// A variable integer starts with an unsupported prefix.
    #[error("invalid integer prefix")]
    InvalidPrefix,
    /// A variable integer contains a malformed continuation byte.
    #[error("invalid integer continuation")]
    InvalidContinuation,
    /// An integer uses more bytes than its value requires.
    #[error("noncanonical integer")]
    NoncanonicalInteger,
    /// A frame length cannot be negative.
    #[error("negative body length")]
    NegativeLength,
    /// The body exceeds the caller's limit or the legacy signed length range.
    #[error("body exceeds configured limit")]
    BodyTooLarge,
}

fn integer_size(value: u32) -> usize {
    match value {
        0..=127 => 1,
        128..=2047 => 2,
        2048..=65535 => 3,
        65536..=2097151 => 4,
        2097152..=67108863 => 5,
        _ => 6,
    }
}

/// Encode an i32 using the legacy UTF-8-shaped integer encoding, including negatives.
/// This is not LEB128 and is not restricted to Unicode scalar values.
pub fn encode_integer(value: i32) -> Vec<u8> {
    let value = value as u32;
    let count = integer_size(value);
    if count == 1 {
        return vec![value as u8];
    }
    let prefix = [0, 0, 0xc0, 0xe0, 0xf0, 0xf8, 0xfc][count];
    let mut result = Vec::with_capacity(count);
    result.push(prefix | (value >> (6 * (count - 1))) as u8);
    for index in (0..count - 1).rev() {
        result.push(0x80 | ((value >> (6 * index)) & 0x3f) as u8);
    }
    result
}

/// Decode one canonical legacy integer. None means more stream bytes are needed.
pub fn decode_integer(bytes: &[u8]) -> Result<Option<(i32, usize)>, ProtocolError> {
    let Some(&first) = bytes.first() else {
        return Ok(None);
    };
    let (count, mask) = match first {
        0x00..=0x7f => (1, 0x7f),
        0xc0..=0xdf => (2, 0x1f),
        0xe0..=0xef => (3, 0x0f),
        0xf0..=0xf7 => (4, 0x07),
        0xf8..=0xfb => (5, 0x03),
        0xfc..=0xff => (6, 0x03),
        _ => return Err(ProtocolError::InvalidPrefix),
    };
    if bytes.len() < count {
        return Ok(None);
    }
    let mut value = u32::from(first & mask);
    for &byte in &bytes[1..count] {
        if byte & 0xc0 != 0x80 {
            return Err(ProtocolError::InvalidContinuation);
        }
        value = (value << 6) | u32::from(byte & 0x3f);
    }
    if integer_size(value) != count {
        return Err(ProtocolError::NoncanonicalInteger);
    }
    Ok(Some((value as i32, count)))
}

/// Borrowed plaintext packet; instance id is a signed big-endian 64-bit field.
#[derive(Debug, PartialEq, Eq)]
pub struct Frame<'a> {
    /// Legacy entity routing id; may be replaced by a salt by the encryption layer.
    pub instance_id: i64,
    /// Legacy message class category, serialized as a signed variable integer.
    pub category_id: i32,
    /// Serialized payload, without a copied allocation.
    pub body: &'a [u8],
}

impl Frame<'_> {
    /// Encode a complete plaintext frame with a caller-controlled allocation limit.
    pub fn encode(&self, max_body: usize) -> Result<Vec<u8>, ProtocolError> {
        if self.body.len() > max_body {
            return Err(ProtocolError::BodyTooLarge);
        }
        let length = i32::try_from(self.body.len()).map_err(|_| ProtocolError::BodyTooLarge)?;
        let category = encode_integer(self.category_id);
        let length = encode_integer(length);
        let mut result = Vec::with_capacity(8 + category.len() + length.len() + self.body.len());
        result.extend_from_slice(&self.instance_id.to_be_bytes());
        result.extend_from_slice(&category);
        result.extend_from_slice(&length);
        result.extend_from_slice(self.body);
        Ok(result)
    }
}

/// Parse one plaintext frame from a TCP stream, leaving subsequent frames untouched.
/// No allocation is made from an untrusted advertised length.
pub fn decode_frame(
    bytes: &[u8],
    max_body: usize,
) -> Result<Option<(Frame<'_>, usize)>, ProtocolError> {
    let Some(instance) = bytes.get(..8) else {
        return Ok(None);
    };
    let mut instance_bytes = [0; 8];
    instance_bytes.copy_from_slice(instance);
    let Some((category_id, category_bytes)) = decode_integer(&bytes[8..])? else {
        return Ok(None);
    };
    let length_start = 8 + category_bytes;
    let Some((length, length_bytes)) = decode_integer(&bytes[length_start..])? else {
        return Ok(None);
    };
    let length = usize::try_from(length).map_err(|_| ProtocolError::NegativeLength)?;
    if length > max_body {
        return Err(ProtocolError::BodyTooLarge);
    }
    let body_start = length_start + length_bytes;
    let end = body_start
        .checked_add(length)
        .ok_or(ProtocolError::BodyTooLarge)?;
    let Some(body) = bytes.get(body_start..end) else {
        return Ok(None);
    };
    Ok(Some((
        Frame {
            instance_id: i64::from_be_bytes(instance_bytes),
            category_id,
            body,
        },
        end,
    )))
}

mod wire;

pub mod empty_character_list;

pub mod frontend_control;

pub mod character_setup;

pub mod character_creation;

pub mod character_list;

pub mod minimum_map;
pub mod mission;
pub mod proudnet;

pub mod inventory;

pub mod equipment;

pub mod character_delete;

pub mod dye;
