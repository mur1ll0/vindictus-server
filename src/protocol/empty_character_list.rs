//! Recovered CharacterListMessage subset with a NULL/empty character collection only.
//! Nonempty summaries are a concrete unsupported dependency, never silently discarded.
use super::{
    encode_integer,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
pub const CATEGORY: i32 = 479;
pub const TYPE_ID: Uuid = Uuid::from_u128(0xf2750037d9ab3b349697e1626e224040);
pub const MAX_PARTY_STATES: usize = 1000;
#[derive(PartialEq, Eq)]
pub struct Message {
    pub characters_present: bool,
    pub max_free: i32,
    pub max_purchased: i32,
    pub max_premium: i32,
    pub prologue_played: u8,
    pub preset_used: i32,
    pub party_states: Option<Vec<i32>>,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CharacterListWireError {
    #[error("nonempty character list requires recovered CharacterSummary serialization")]
    Characters,
    #[error("unsupported character-list category/type")]
    Type,
    #[error("non-null System.Object is not a character list")]
    Object,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
pub fn encode(
    message: Option<&Message>,
    max_body: usize,
) -> Result<Vec<u8>, CharacterListWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let Some(m) = message else {
        w.append(&[0])?;
        return Ok(w.bytes);
    };
    if m.party_states
        .as_ref()
        .is_some_and(|p| p.len() > MAX_PARTY_STATES)
    {
        return Err(PayloadError::TooLarge.into());
    }
    w.append(&[1, u8::from(m.characters_present)])?;
    for value in [m.max_free, m.max_purchased, m.max_premium] {
        w.append(&value.to_be_bytes())?;
    }
    w.append(&[m.prologue_played])?;
    w.append(&m.preset_used.to_be_bytes())?;
    match &m.party_states {
        None => w.append(&[0])?,
        Some(p) => {
            w.append(&encode_integer((p.len() + 1) as i32))?;
            for value in p {
                w.append(&value.to_be_bytes())?;
            }
        }
    }
    Ok(w.bytes)
}
pub fn decode(body: &[u8], max_body: usize) -> Result<Option<Message>, CharacterListWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let message = if r.byte()? == 0 {
        None
    } else {
        let characters_present = match r.count()? {
            None => false,
            Some(0) => true,
            Some(_) => return Err(CharacterListWireError::Characters),
        };
        let max_free = i32::from_be_bytes(r.fixed()?);
        let max_purchased = i32::from_be_bytes(r.fixed()?);
        let max_premium = i32::from_be_bytes(r.fixed()?);
        let prologue_played = r.byte()?;
        let preset_used = i32::from_be_bytes(r.fixed()?);
        let party_states = match r.count()? {
            None => None,
            Some(n) => {
                if n > MAX_PARTY_STATES {
                    return Err(PayloadError::TooLarge.into());
                }
                if n > r.remaining.len() / 4 {
                    return Err(PayloadError::Truncated.into());
                }
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(i32::from_be_bytes(r.fixed()?));
                }
                Some(values)
            }
        };
        Some(Message {
            characters_present,
            max_free,
            max_purchased,
            max_premium,
            prologue_played,
            preset_used,
            party_states,
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(message)
}
pub fn decode_ingress(
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, CharacterListWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    if category == CATEGORY {
        return decode(body, max_body);
    }
    if category != 0 {
        return Err(CharacterListWireError::Type);
    }
    let guid = body.get(..16).ok_or(PayloadError::Truncated)?;
    let fields = &body[16..];
    if guid == TYPE_ID.as_bytes() {
        return decode(fields, max_body - 16);
    }
    if guid == Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21).as_bytes() {
        let presence = fields.first().ok_or(PayloadError::Truncated)?;
        if *presence != 0 {
            return Err(CharacterListWireError::Object);
        }
        if fields.len() != 1 {
            return Err(PayloadError::TrailingBytes.into());
        }
        return Ok(None);
    }
    Err(CharacterListWireError::Type)
}
