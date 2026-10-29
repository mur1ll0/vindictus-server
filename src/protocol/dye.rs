//! Source DyeAmpleUsed270 / DyeAmpleRequest269 / DyeAmpleComplete268 codecs of the
//! empty dye ampoule (DyeAmpleSession). Fields only; the session and the colored
//! ampoule are decided by the storage boundary.
use super::{
    inventory::Wire,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Used,
    Request,
    Complete,
}
impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::Used => 270,
            Self::Request => 269,
            Self::Complete => 268,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::Used => 0x25fbb3e8_bab4_3548_a0a1_224aa97b4f25,
            Self::Request => 0xc55d23ca_6885_3155_9375_5035631f3ff5,
            Self::Complete => 0xc9765023_10d1_3a0a_bf97_6b45e7312258,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// DyeAmpleUsedMessage: ColorTable and Seed1..Seed4 (the client's color picker).
    Used {
        color_table: Option<String>,
        seeds: [i32; 4],
    },
    /// DyeAmpleRequestMessage: the picked coordinate and the client's cursor colors.
    Request {
        x: i32,
        y: i32,
        is_avatar: bool,
        color_value: Option<Vec<i32>>,
    },
    /// DyeAmpleCompleteMessage: the color of the new ampoule.
    Complete { color: i32 },
}
impl Message {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Used { .. } => Kind::Used,
            Self::Request { .. } => Kind::Request,
            Self::Complete { .. } => Kind::Complete,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DyeWireError {
    #[error("dye ampoule message kind mismatch")]
    Kind,
    #[error("unsupported dye ampoule category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, DyeWireError> {
    if message.is_some_and(|m| m.kind() != kind) {
        return Err(DyeWireError::Kind);
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
            Message::Used { color_table, seeds } => {
                color_table.write(&mut writer)?;
                for seed in seeds {
                    seed.write(&mut writer)?;
                }
            }
            Message::Request {
                x,
                y,
                is_avatar,
                color_value,
            } => {
                x.write(&mut writer)?;
                y.write(&mut writer)?;
                is_avatar.write(&mut writer)?;
                color_value.write(&mut writer)?;
            }
            Message::Complete { color } => color.write(&mut writer)?,
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

pub fn decode(kind: Kind, body: &[u8], max_body: usize) -> Result<Option<Message>, DyeWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut reader = Reader { remaining: body };
    let message = if reader.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::Used => Message::Used {
                color_table: Option::<String>::read(&mut reader)?,
                seeds: [
                    i32::read(&mut reader)?,
                    i32::read(&mut reader)?,
                    i32::read(&mut reader)?,
                    i32::read(&mut reader)?,
                ],
            },
            Kind::Request => Message::Request {
                x: i32::read(&mut reader)?,
                y: i32::read(&mut reader)?,
                is_avatar: bool::read(&mut reader)?,
                color_value: Option::<Vec<i32>>::read(&mut reader)?,
            },
            Kind::Complete => Message::Complete {
                color: i32::read(&mut reader)?,
            },
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
) -> Result<Option<Message>, DyeWireError> {
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
    Err(DyeWireError::Type)
}
