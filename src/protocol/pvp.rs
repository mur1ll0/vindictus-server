//! PvP messages recovered from the Framework serializer: ServiceCore.EndPointNetwork
//! FreeMatchReportMessage (276), the MicroPlay host's report of a free duel's winner and
//! loser by player tag.
use super::{
    inventory::{Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    FreeMatchReport,
}
impl Kind {
    /// Messages the client sends (InGame).
    pub const INGRESS: [Kind; 1] = [Kind::FreeMatchReport];
    pub const fn category(self) -> i32 {
        match self {
            Self::FreeMatchReport => 276,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::FreeMatchReport => 0x8673db49_f849_349b_acea_3794efe7ece5,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PvpWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
    #[error("unsupported PvP category/type")]
    Type,
}

/// FreeMatchReportMessage: WinnerTag, LoserTag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreeMatchReport {
    pub winner_tag: i32,
    pub loser_tag: i32,
}
object!(FreeMatchReport {
    winner_tag,
    loser_tag
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    FreeMatchReport(FreeMatchReport),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::FreeMatchReport(_) => Kind::FreeMatchReport,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, PvpWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), PvpWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(PvpWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::FreeMatchReport(v) => v.write(&mut w)?,
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(Zeroizing::new(w.bytes)),
        Err(e) => {
            w.bytes.zeroize();
            Err(e)
        }
    }
}

/// Strict decode: the whole body must be consumed (encoder round trips and fixtures).
pub fn decode(kind: Kind, body: &[u8], max_body: usize) -> Result<Option<Message>, PvpWireError> {
    decode_body(kind, body, max_body, true)
}

fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, PvpWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::FreeMatchReport => Message::FreeMatchReport(Wire::read(r)?),
        })
    };
    if strict && !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}

/// The generic null envelope of the Framework serializer (an unregistered null object).
const NULL_OBJECT: Uuid = Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21);

/// Registered category, the unregistered GUID form, or the generic null envelope;
/// trailing bytes are ignored as SerializeReader.FromBinary does.
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, PvpWireError> {
    if category == kind.category() {
        return decode_body(kind, body, max_body, false);
    }
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode_body(kind, &body[16..], max_body.saturating_sub(16), false);
    }
    if category == 0
        && body.len() <= max_body
        && body.get(..16) == Some(NULL_OBJECT.as_bytes())
        && body.get(16..) == Some(&[0])
    {
        return Ok(None);
    }
    Err(PvpWireError::Type)
}
