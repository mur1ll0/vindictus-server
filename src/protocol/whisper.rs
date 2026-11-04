//! Whisper and whisper filter messages recovered from the Framework serializer:
//! WhisperFilterList (636, IDictionary<string, int> Filter), UpdateWhisperFilter (637,
//! OperationType, TargetID, Level), RequestWhisper (638, Receiver, Contents), Whisper (639,
//! Sender, Contents) and WhisperFail (640, ToName, Reason). The client sends
//! UpdateWhisperFilter and RequestWhisper.
use super::{
    inventory::{Wire, object},
    mission::StrDict,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    FilterList,
    UpdateFilter,
    Request,
    Whisper,
    Fail,
}
impl Kind {
    /// Messages the client sends to the frontend (GameClient, InGame).
    pub const INGRESS: [Kind; 2] = [Kind::UpdateFilter, Kind::Request];
    pub const fn category(self) -> i32 {
        match self {
            Self::FilterList => 636,
            Self::UpdateFilter => 637,
            Self::Request => 638,
            Self::Whisper => 639,
            Self::Fail => 640,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::FilterList => 0x9d9c801a_4700_3cd3_889e_99b210174432,
            Self::UpdateFilter => 0xa7953e9f_47e2_3ec3_9aac_c9839f26427d,
            Self::Request => 0x5e3ee58f_1ed5_3582_b197_aa2273d6b334,
            Self::Whisper => 0xac053a13_a83c_322a_9903_e0f1f68d1287,
            Self::Fail => 0x905bd8c4_246c_302c_9f46_c9a7b8927a87,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WhisperWireError {
    #[error(transparent)]
    Payload(#[from] PayloadError),
    #[error("message does not match its kind")]
    Kind,
    #[error("unsupported whisper category/type")]
    Type,
}

/// UpdateWhisperFilterMessage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateFilter {
    pub operation_type: i32,
    pub target_id: Option<String>,
    pub level: i32,
}
object!(UpdateFilter {
    operation_type,
    target_id,
    level
});

/// RequestWhisperMessage (client → server).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub receiver: Option<String>,
    pub contents: Option<String>,
}
object!(Request { receiver, contents });

/// WhisperMessage (server → receiver).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Whisper {
    pub sender: Option<String>,
    pub contents: Option<String>,
}
object!(Whisper { sender, contents });

/// WhisperFailMessage (server → sender): the receiver and a UI string key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fail {
    pub to_name: Option<String>,
    pub reason: Option<String>,
}
object!(Fail { to_name, reason });

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// WhisperFilterListMessage.Filter: blocked name → level.
    FilterList(StrDict<i32>),
    UpdateFilter(UpdateFilter),
    Request(Request),
    Whisper(Whisper),
    Fail(Fail),
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::FilterList(_) => Kind::FilterList,
            Self::UpdateFilter(_) => Kind::UpdateFilter,
            Self::Request(_) => Kind::Request,
            Self::Whisper(_) => Kind::Whisper,
            Self::Fail(_) => Kind::Fail,
        }
    }
}

pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, WhisperWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), WhisperWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(WhisperWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::FilterList(v) => v.write(&mut w)?,
            Message::UpdateFilter(v) => v.write(&mut w)?,
            Message::Request(v) => v.write(&mut w)?,
            Message::Whisper(v) => v.write(&mut w)?,
            Message::Fail(v) => v.write(&mut w)?,
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
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, WhisperWireError> {
    decode_body(kind, body, max_body, true)
}

fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, WhisperWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        let r = &mut r;
        Some(match kind {
            Kind::FilterList => Message::FilterList(Wire::read(r)?),
            Kind::UpdateFilter => Message::UpdateFilter(Wire::read(r)?),
            Kind::Request => Message::Request(Wire::read(r)?),
            Kind::Whisper => Message::Whisper(Wire::read(r)?),
            Kind::Fail => Message::Fail(Wire::read(r)?),
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
) -> Result<Option<Message>, WhisperWireError> {
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
    Err(WhisperWireError::Type)
}
