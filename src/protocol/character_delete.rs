//! Source DeleteCharacter451 / DeleteCharacterResult407 / DeleteCharacterCancel258 /
//! DeleteCharacterCancelResult259 / SelectOrdersChange489 codecs of the character list.
//! Fields only; deletion, cancellation and slot order are decided by the storage boundary.
use super::{
    inventory::Wire,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Delete,
    DeleteResult,
    Cancel,
    CancelResult,
    SelectOrders,
}
impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::Delete => 451,
            Self::DeleteResult => 407,
            Self::Cancel => 258,
            Self::CancelResult => 259,
            Self::SelectOrders => 489,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::Delete => 0x65699026_e61b_38d9_a403_28178a907f59,
            Self::DeleteResult => 0x19af38d6_d58a_39e5_bfc7_7b5b88c50e4a,
            Self::Cancel => 0x7a123c93_9f06_3bba_8e0e_1bf3d7a1d512,
            Self::CancelResult => 0x8f6e7d11_8bdc_3e5b_9704_44e95b136338,
            Self::SelectOrders => 0xd7d63e49_9742_3287_a815_7a0483760e81,
        })
    }
}

/// ServiceCore.EndPointNetwork.DeleteCharacterResult (wire: Int32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DeleteResult {
    Unknown = 0,
    SuccessDeleted = 1,
    SuccessDeleting = 2,
    FailNoSuchCharacter = 3,
    FailNoCashShopService = 4,
    FailRemainCashItems = 5,
    FailNoGuildService = 6,
    FailInGuild = 7,
    FailNeedMoreWaiting = 8,
    FailInvalidStatus = 9,
    FailException = 10,
    FailUnknown = 11,
    FailExcuteOp = 12,
}

/// ServiceCore.EndPointNetwork.CharacterList.DeleteCharacterCancelResult (wire: Int32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum CancelResult {
    Unknown = 0,
    Success = 1,
    FailNoSuchCharacter = 2,
    FailInvalidStatus = 3,
    FailException = 4,
    FailUnknown = 5,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// DeleteCharacterMessage and DeleteCharacterCancelMessage share {Name, CharacterSN}.
    Delete {
        name: Option<String>,
        character_sn: i32,
    },
    Cancel {
        name: Option<String>,
        character_sn: i32,
    },
    /// Result keeps the raw Int32: the enum is not range-checked by the source reader.
    DeleteResult {
        character_sn: i32,
        result: i32,
        remain_time_sec: i32,
    },
    CancelResult {
        character_sn: i32,
        result: i32,
    },
    SelectOrders(Option<Vec<u8>>),
}
impl Message {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Delete { .. } => Kind::Delete,
            Self::Cancel { .. } => Kind::Cancel,
            Self::DeleteResult { .. } => Kind::DeleteResult,
            Self::CancelResult { .. } => Kind::CancelResult,
            Self::SelectOrders(_) => Kind::SelectOrders,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CharacterDeleteWireError {
    #[error("character delete message kind mismatch")]
    Kind,
    #[error("unsupported character delete category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, CharacterDeleteWireError> {
    if message.is_some_and(|m| m.kind() != kind) {
        return Err(CharacterDeleteWireError::Kind);
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
            Message::Delete { name, character_sn } | Message::Cancel { name, character_sn } => {
                name.write(&mut writer)?;
                character_sn.write(&mut writer)?;
            }
            Message::DeleteResult {
                character_sn,
                result,
                remain_time_sec,
            } => {
                character_sn.write(&mut writer)?;
                result.write(&mut writer)?;
                remain_time_sec.write(&mut writer)?;
            }
            Message::CancelResult {
                character_sn,
                result,
            } => {
                character_sn.write(&mut writer)?;
                result.write(&mut writer)?;
            }
            Message::SelectOrders(orders) => orders.write(&mut writer)?,
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
) -> Result<Option<Message>, CharacterDeleteWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut reader = Reader { remaining: body };
    let message = if reader.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::Delete => Message::Delete {
                name: Option::<String>::read(&mut reader)?,
                character_sn: i32::read(&mut reader)?,
            },
            Kind::Cancel => Message::Cancel {
                name: Option::<String>::read(&mut reader)?,
                character_sn: i32::read(&mut reader)?,
            },
            Kind::DeleteResult => Message::DeleteResult {
                character_sn: i32::read(&mut reader)?,
                result: i32::read(&mut reader)?,
                remain_time_sec: i32::read(&mut reader)?,
            },
            Kind::CancelResult => Message::CancelResult {
                character_sn: i32::read(&mut reader)?,
                result: i32::read(&mut reader)?,
            },
            Kind::SelectOrders => Message::SelectOrders(Option::<Vec<u8>>::read(&mut reader)?),
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
) -> Result<Option<Message>, CharacterDeleteWireError> {
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
    Err(CharacterDeleteWireError::Type)
}
