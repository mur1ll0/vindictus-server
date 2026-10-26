//! Recovered frontend control DTOs: sealed typed bodies and selected category-zero ingress.
use super::{
    encode_integer,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ping,
    Pong,
    System,
    Ng,
    Cafe,
    /// NoticeShutDownMessage: the server stops in `Delay` seconds (FrontendServiceCore
    /// ShutDownProcessor).
    NoticeShutDown,
}
impl Kind {
    pub fn category(self) -> i32 {
        match self {
            Self::Ping => 521,
            Self::Pong => 523,
            Self::System => 602,
            Self::Ng => 16,
            Self::Cafe => 236,
            Self::NoticeShutDown => 505,
        }
    }
    pub fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::Ping => 0xf4889dff7be13380a75d16931cf0367f,
            Self::Pong => 0xe7c289c5c6c3356085222b09a473e7d9,
            Self::System => 0xe3fb23ddbce03c81b40416e93f7bea38,
            Self::Ng => 0x5ee15fa278083ab28dd2caa46efd8124,
            Self::Cafe => 0x3c438b534b8739819ef1e164d6632266,
            Self::NoticeShutDown => 0x88cb090bcdc1367c9397698aabbb3e8a,
        })
    }
}
#[derive(PartialEq, Eq)]
pub struct HeroesText<'a> {
    pub format: Option<&'a str>,
    pub parameters: Option<Vec<Option<&'a str>>>,
}
#[derive(PartialEq, Eq)]
pub enum Message<'a> {
    Cafe {
        home: i32,
        account: i32,
        cafe: i32,
    },
    Ping(i32),
    Pong(i32),
    NoticeShutDown(i32),
    System {
        category: u8,
        text: Option<HeroesText<'a>>,
    },
    Ng {
        data: Option<&'a [u8]>,
        checksum: u64,
    },
}
impl Message<'_> {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Ping(_) => Kind::Ping,
            Self::Pong(_) => Kind::Pong,
            Self::System { .. } => Kind::System,
            Self::Ng { .. } => Kind::Ng,
            Self::Cafe { .. } => Kind::Cafe,
            Self::NoticeShutDown(_) => Kind::NoticeShutDown,
        }
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ControlWireError {
    #[error("unsupported frontend control category/type")]
    Type,
    #[error("non-null System.Object is not a recovered frontend control")]
    Object,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
pub const MAX_PARAMETERS: usize = 64;
pub fn encode(
    kind: Kind,
    message: Option<&Message<'_>>,
    max_body: usize,
) -> Result<Vec<u8>, ControlWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let Some(m) = message else {
        w.append(&[0])?;
        return Ok(w.bytes);
    };
    if m.kind() != kind {
        return Err(ControlWireError::Type);
    }
    w.append(&[1])?;
    match m {
        Message::Ping(tag) | Message::Pong(tag) | Message::NoticeShutDown(tag) => {
            w.append(&tag.to_be_bytes())?
        }
        Message::Cafe {
            home,
            account,
            cafe,
        } => {
            for value in [home, account, cafe] {
                w.append(&value.to_be_bytes())?;
            }
        }
        Message::Ng { data, checksum } => {
            w.nullable(*data)?;
            w.append(&checksum.to_be_bytes())?;
        }
        Message::System { category, text } => {
            w.append(&[*category])?;
            if let Some(text) = text {
                w.append(&[1])?;
                w.nullable(text.format.map(str::as_bytes))?;
                if let Some(parameters) = &text.parameters {
                    if parameters.len() > MAX_PARAMETERS {
                        return Err(PayloadError::TooLarge.into());
                    }
                    w.append(&encode_integer((parameters.len() + 1) as i32))?;
                    for p in parameters {
                        w.nullable(p.map(str::as_bytes))?;
                    }
                } else {
                    w.append(&[0])?;
                }
            } else {
                w.append(&[0])?;
            }
        }
    }
    Ok(w.bytes)
}
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message<'_>>, ControlWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::Ping => Message::Ping(i32::from_be_bytes(r.fixed()?)),
            Kind::Pong => Message::Pong(i32::from_be_bytes(r.fixed()?)),
            Kind::NoticeShutDown => Message::NoticeShutDown(i32::from_be_bytes(r.fixed()?)),
            Kind::Cafe => Message::Cafe {
                home: i32::from_be_bytes(r.fixed()?),
                account: i32::from_be_bytes(r.fixed()?),
                cafe: i32::from_be_bytes(r.fixed()?),
            },
            Kind::Ng => Message::Ng {
                data: r.bytes()?,
                checksum: u64::from_be_bytes(r.fixed()?),
            },
            Kind::System => {
                let category = r.byte()?;
                let text = if r.byte()? == 0 {
                    None
                } else {
                    let format = r.string()?;
                    let parameters = r
                        .count()?
                        .map(|n| {
                            if n > MAX_PARAMETERS {
                                return Err(PayloadError::TooLarge);
                            }
                            (0..n).map(|_| r.string()).collect::<Result<Vec<_>, _>>()
                        })
                        .transpose()?;
                    Some(HeroesText { format, parameters })
                };
                Message::System { category, text }
            }
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message<'_>>, ControlWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    if category == kind.category() {
        return decode(kind, body, max_body);
    }
    if category != 0 {
        return Err(ControlWireError::Type);
    }
    let id = body.get(..16).ok_or(PayloadError::Truncated)?;
    let fields = &body[16..];
    if id == kind.type_id().as_bytes() {
        return decode(kind, fields, max_body - 16);
    }
    let object = Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21);
    if id == object.as_bytes() {
        let presence = fields.first().ok_or(PayloadError::Truncated)?;
        if *presence != 0 {
            return Err(ControlWireError::Object);
        }
        if fields.len() != 1 {
            return Err(PayloadError::TrailingBytes.into());
        }
        return Ok(None);
    }
    Err(ControlWireError::Type)
}
/// Recovered GameClient Check/Set failure. Category Dialog=4, empty Parameters collection.
pub fn second_password_locked(max_body: usize) -> Result<Vec<u8>, ControlWireError> {
    encode(
        Kind::System,
        Some(&Message::System {
            category: 4,
            text: Some(HeroesText {
                format: Some("GameUI_Heroes_SecondPassword_Locked"),
                parameters: Some(Vec::new()),
            }),
        }),
        max_body,
    )
}
