//! Original character setup DTOs, validated with recovered serializers and metadata.
//! These codecs do not validate a name or create/select a character.
use super::{
    encode_integer,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Check,
    Result,
    Query,
    Reserved,
    Play,
}
impl Kind {
    pub fn category(self) -> i32 {
        match self {
            Self::Check => 261,
            Self::Result => 150,
            Self::Query => 439,
            Self::Reserved => 443,
            Self::Play => 644,
        }
    }
    pub fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::Check => 0xc63a6f7537c935f68cffd7ae87ac2967,
            Self::Result => 0x6de1bf749845344890af230ce1779c2b,
            Self::Query => 0x0e5fd723eaf13c00aff49360c9367364,
            Self::Reserved => 0xea3d352343e1370aaad51f0c477c4d5b,
            Self::Play => 0x62e666da21033f50b021aa8f0f0aa617,
        })
    }
}
/// Private names are borrowed and intentionally have no Debug/Serialize.
#[derive(PartialEq, Eq)]
pub enum Message<'a> {
    Check {
        name: Option<&'a str>,
        name_change: bool,
    },
    Result {
        valid: bool,
        name: Option<&'a str>,
        error: Option<&'a str>,
    },
    Query,
    Reserved {
        names: Option<Vec<Option<&'a str>>>,
        titles: Option<Vec<i32>>,
    },
    Play(i32),
}
impl Message<'_> {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Check { .. } => Kind::Check,
            Self::Result { .. } => Kind::Result,
            Self::Query => Kind::Query,
            Self::Reserved { .. } => Kind::Reserved,
            Self::Play(_) => Kind::Play,
        }
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SetupWireError {
    #[error("character setup message kind mismatch")]
    Kind,
    #[error("unsupported character setup category/type")]
    Type,
    #[error("non-null object is not a recovered character setup message")]
    Object,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
pub const MAX_COLLECTION: usize = 1024;
fn collection_count(w: &mut Writer, count: Option<usize>) -> Result<(), SetupWireError> {
    let n = match count {
        None => 0,
        Some(n) if n <= MAX_COLLECTION => n + 1,
        Some(_) => return Err(PayloadError::TooLarge.into()),
    };
    w.append(&encode_integer(n as i32))?;
    Ok(())
}
fn read_count(r: &mut Reader<'_>) -> Result<Option<usize>, SetupWireError> {
    let n = r.count()?;
    if n.is_some_and(|v| v > MAX_COLLECTION) {
        return Err(PayloadError::TooLarge.into());
    }
    Ok(n)
}
pub fn encode(
    kind: Kind,
    message: Option<&Message<'_>>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, SetupWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), SetupWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if m.kind() != kind {
            return Err(SetupWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::Check { name, name_change } => {
                w.nullable(name.map(str::as_bytes))?;
                w.append(&[u8::from(*name_change)])?;
            }
            Message::Result { valid, name, error } => {
                w.append(&[u8::from(*valid)])?;
                w.nullable(name.map(str::as_bytes))?;
                w.nullable(error.map(str::as_bytes))?;
            }
            Message::Query => {}
            Message::Play(state) => w.append(&state.to_be_bytes())?,
            Message::Reserved { names, titles } => {
                collection_count(&mut w, names.as_ref().map(Vec::len))?;
                if let Some(names) = names {
                    for name in names {
                        w.nullable(name.map(str::as_bytes))?;
                    }
                }
                collection_count(&mut w, titles.as_ref().map(Vec::len))?;
                if let Some(titles) = titles {
                    for title in titles {
                        w.append(&title.to_be_bytes())?;
                    }
                }
            }
        }
        Ok(())
    })();
    let bytes = Zeroizing::new(w.bytes);
    result?;
    Ok(bytes)
}
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message<'_>>, SetupWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let value = if r.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::Check => Message::Check {
                name: r.string()?,
                name_change: r.byte()? != 0,
            },
            Kind::Result => Message::Result {
                valid: r.byte()? != 0,
                name: r.string()?,
                error: r.string()?,
            },
            Kind::Query => Message::Query,
            Kind::Play => Message::Play(i32::from_be_bytes(r.fixed()?)),
            Kind::Reserved => {
                let names = if let Some(n) = read_count(&mut r)? {
                    if n > r.remaining.len() {
                        return Err(PayloadError::Truncated.into());
                    }
                    let mut names = Vec::with_capacity(n);
                    for _ in 0..n {
                        names.push(r.string()?)
                    }
                    Some(names)
                } else {
                    None
                };
                let titles = if let Some(n) = read_count(&mut r)? {
                    if n > r.remaining.len() / 4 {
                        return Err(PayloadError::Truncated.into());
                    }
                    let mut titles = Vec::with_capacity(n);
                    for _ in 0..n {
                        titles.push(i32::from_be_bytes(r.fixed()?))
                    }
                    Some(titles)
                } else {
                    None
                };
                Message::Reserved { names, titles }
            }
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(value)
}
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message<'_>>, SetupWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    if category == kind.category() {
        return decode(kind, body, max_body);
    }
    if category != 0 {
        return Err(SetupWireError::Type);
    }
    let guid = body.get(..16).ok_or(PayloadError::Truncated)?;
    let fields = &body[16..];
    if guid == kind.type_id().as_bytes() {
        return decode(kind, fields, max_body - 16);
    }
    let object = Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21);
    if guid == object.as_bytes() {
        let presence = fields.first().ok_or(PayloadError::Truncated)?;
        if *presence != 0 {
            return Err(SetupWireError::Object);
        }
        if fields.len() != 1 {
            return Err(PayloadError::TrailingBytes.into());
        }
        return Ok(None);
    }
    Err(SetupWireError::Type)
}
