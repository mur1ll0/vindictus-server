//! Five recovered sealed second-password messages; no network authentication is implied.
//! Categories/GUIDs from original ServiceCore metadata, checked by Framework x86/x64 oracle.
use super::wire::{PayloadError, Reader, Writer};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ask,
    Check,
    Set,
    Has,
    Result,
}
impl Kind {
    pub fn category(self) -> i32 {
        match self {
            Self::Ask => 231,
            Self::Check => 405,
            Self::Set => 572,
            Self::Has => 413,
            Self::Result => 562,
        }
    }
    pub fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::Ask => 0xe9e7d6a121313b59826d88c968ae0867,
            Self::Check => 0x1f3f118d77753b45b608a383ba381ed6,
            Self::Set => 0x0c9611cc7fe03aea8e2b35b3d8904786,
            Self::Has => 0x95739ac926a7325e9f4bb63c40dc590c,
            Self::Result => 0xb2b1f8d64c60325e854f2f73550d8e9b,
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HasPassword {
    pub first: bool,
    pub has_password: bool,
    pub passed: bool,
    pub fail_count: i32,
    pub retry_seconds: i32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PasswordResult {
    /// Original enum representation; unknown signed values remain opaque.
    pub operation: i32,
    pub passed: bool,
    pub fail_count: i32,
    pub retry_seconds: i32,
}
/// Borrowed password fields have no Debug/Serialize and are not copied by the decoder.
#[derive(PartialEq, Eq)]
pub enum Message<'a> {
    Ask,
    Check(Option<&'a str>),
    Set(Option<&'a str>),
    Has(HasPassword),
    Result(PasswordResult),
}
impl Message<'_> {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Ask => Kind::Ask,
            Self::Check(_) => Kind::Check,
            Self::Set(_) => Kind::Set,
            Self::Has(_) => Kind::Has,
            Self::Result(_) => Kind::Result,
        }
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PasswordWireError {
    #[error("second-password message kind mismatch")]
    Kind,
    #[error("unsupported second-password category")]
    Category,
    #[error("unsupported second-password envelope type")]
    Type,
    #[error("non-null object is not a second-password message")]
    Object,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
/// Typed body only. Output and partial output on failure are zeroized when dropped.
pub fn encode(
    kind: Kind,
    message: Option<&Message<'_>>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, PasswordWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result: Result<(), PasswordWireError> = (|| {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if m.kind() != kind {
            return Err(PasswordWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::Ask => {}
            Message::Check(p) | Message::Set(p) => w.nullable(p.map(str::as_bytes))?,
            Message::Has(v) => {
                w.append(&[
                    u8::from(v.first),
                    u8::from(v.has_password),
                    u8::from(v.passed),
                ])?;
                w.append(&v.fail_count.to_be_bytes())?;
                w.append(&v.retry_seconds.to_be_bytes())?;
            }
            Message::Result(v) => {
                w.append(&v.operation.to_be_bytes())?;
                w.append(&[u8::from(v.passed)])?;
                w.append(&v.fail_count.to_be_bytes())?;
                w.append(&v.retry_seconds.to_be_bytes())?;
            }
        }
        Ok(())
    })();
    let bytes = Zeroizing::new(w.bytes);
    result?;
    Ok(bytes)
}
/// Sealed presence, fixed big-endian int32 and recovered nullable UTF8 string encoding.
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message<'_>>, PasswordWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let value = if r.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::Ask => Message::Ask,
            Kind::Check => Message::Check(r.string()?),
            Kind::Set => Message::Set(r.string()?),
            Kind::Has => Message::Has(HasPassword {
                first: r.byte()? != 0,
                has_password: r.byte()? != 0,
                passed: r.byte()? != 0,
                fail_count: i32::from_be_bytes(r.fixed()?),
                retry_seconds: i32::from_be_bytes(r.fixed()?),
            }),
            Kind::Result => Message::Result(PasswordResult {
                operation: i32::from_be_bytes(r.fixed()?),
                passed: r.byte()? != 0,
                fail_count: i32::from_be_bytes(r.fixed()?),
                retry_seconds: i32::from_be_bytes(r.fixed()?),
            }),
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(value)
}
/// Dispatch only the selected original type or its recovered category-zero envelope.
/// Does not register peer types or load arbitrary remote classes. Limit includes GUID.
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message<'_>>, PasswordWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    if category == kind.category() {
        return decode(kind, body, max_body);
    }
    if category != 0 {
        return Err(PasswordWireError::Category);
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
            return Err(PasswordWireError::Object);
        }
        if fields.len() != 1 {
            return Err(PayloadError::TrailingBytes.into());
        }
        return Ok(None);
    }
    Err(PasswordWireError::Type)
}
