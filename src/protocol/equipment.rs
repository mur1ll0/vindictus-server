//! Source EquipItem457 / EquipmentInfo458 codecs. This module grants no item
//! authority and does not register a runtime mutation handler.
use super::{
    inventory::{Dict, Wire},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    EquipItem,
    EquipmentInfo,
}
impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::EquipItem => 457,
            Self::EquipmentInfo => 458,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::EquipItem => 0xa6551816_3ca1_38a2_8ec0_7ecba2ac7f36,
            Self::EquipmentInfo => 0x9df997b4_ce70_3558_86a1_7ee1ea91325c,
        })
    }
}
#[derive(Clone, PartialEq, Eq)]
pub enum Message {
    /// ItemID=-1 is the recovered unequip request; the wire preserves every other
    /// signed value for validation by a future authoritative storage boundary.
    EquipItem {
        item_id: i64,
        part: i32,
    },
    EquipmentInfo(Dict<i64>),
}
impl Message {
    pub fn kind(&self) -> Kind {
        match self {
            Self::EquipItem { .. } => Kind::EquipItem,
            Self::EquipmentInfo(_) => Kind::EquipmentInfo,
        }
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EquipmentWireError {
    #[error("equipment message kind mismatch")]
    Kind,
    #[error("unsupported equipment category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, EquipmentWireError> {
    if message.is_some_and(|m| m.kind() != kind) {
        return Err(EquipmentWireError::Kind);
    }
    let mut writer = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), PayloadError> {
        let Some(message) = message else {
            return writer.append(&[0]);
        };
        writer.append(&[1])?;
        match message {
            Message::EquipItem { item_id, part } => {
                item_id.write(&mut writer)?;
                part.write(&mut writer)?;
            }
            Message::EquipmentInfo(equipment) => equipment.write(&mut writer)?,
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(Zeroizing::new(writer.bytes)),
        Err(error) => {
            writer.bytes.zeroize();
            Err(error.into())
        }
    }
}
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, EquipmentWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut reader = Reader { remaining: body };
    let message = if reader.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::EquipItem => Message::EquipItem {
                item_id: i64::read(&mut reader)?,
                part: i32::read(&mut reader)?,
            },
            Kind::EquipmentInfo => Message::EquipmentInfo(Dict::<i64>::read(&mut reader)?),
        })
    };
    if !reader.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(message)
}
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, EquipmentWireError> {
    if category == kind.category() {
        return decode(kind, body, max_body);
    }
    if category == 0 && body.len() <= max_body {
        if body.get(..16) == Some(kind.type_id().as_bytes()) {
            return decode(kind, &body[16..], max_body.saturating_sub(16));
        }
        if body.get(..16)
            == Some(Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21).as_bytes())
            && body.get(16..) == Some(&[0])
        {
            return Ok(None);
        }
    }
    Err(EquipmentWireError::Type)
}
