//! Recovered character creation wire; fields only, no creation or ownership effect.
use super::{
    encode_integer,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Create,
    Fail,
}
impl Kind {
    pub fn category(self) -> i32 {
        match self {
            Self::Create => 482,
            Self::Fail => 485,
        }
    }
    pub fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::Create => 0x677216687ebf37919ccd53bb01933e99,
            Self::Fail => 0x8676de0575e83b6ab6a481065f352b9b,
        })
    }
}
#[derive(PartialEq, Eq)]
pub struct CustomizeItem<'a> {
    pub item_class: Option<&'a str>,
    pub category: Option<&'a str>,
    pub colors: [i32; 3],
    pub duration: i32,
    pub price: Option<&'a str>,
    pub coupon: i64,
}
#[derive(PartialEq, Eq)]
pub struct Template<'a> {
    pub character_class: u8,
    pub items: Option<Vec<Option<CustomizeItem<'a>>>>,
    pub skin_color: i32,
    pub shineness: i32,
    pub eye_color: i32,
    pub height: i32,
    pub bust: i32,
    pub painting_x: i32,
    pub painting_y: i32,
    pub painting_rotation: i32,
    pub painting_size: i32,
    /// Raw IEEE754 bits, including signed zero/NaN payload. Ordering is retained.
    pub body_shape: Option<Vec<(i32, u32)>>,
}
#[derive(PartialEq, Eq)]
pub enum Message<'a> {
    Create {
        name: Option<&'a str>,
        template: Option<Template<'a>>,
        preset: bool,
        premium: bool,
    },
    Fail(i32),
}
impl Message<'_> {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Create { .. } => Kind::Create,
            Self::Fail(_) => Kind::Fail,
        }
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CreationWireError {
    #[error("creation message kind mismatch")]
    Kind,
    #[error("unsupported creation category/type")]
    Type,
    #[error("object envelope is not a recovered creation message")]
    Object,
    #[error("duplicate body-shape dictionary key")]
    DuplicateKey,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
const MAX_ITEMS: usize = 128;
const MAX_SHAPES: usize = 128;
fn count(w: &mut Writer, n: Option<usize>, limit: usize) -> Result<(), CreationWireError> {
    let value = match n {
        None => 0,
        Some(n) if n <= limit => n + 1,
        Some(_) => return Err(PayloadError::TooLarge.into()),
    };
    w.append(&encode_integer(value as i32))?;
    Ok(())
}
fn read_count(r: &mut Reader<'_>, limit: usize) -> Result<Option<usize>, CreationWireError> {
    let n = r.count()?;
    if n.is_some_and(|n| n > limit) {
        return Err(PayloadError::TooLarge.into());
    }
    Ok(n)
}
fn write_template(w: &mut Writer, t: Option<&Template<'_>>) -> Result<(), CreationWireError> {
    let Some(t) = t else {
        w.append(&[0])?;
        return Ok(());
    };
    w.append(&[1, t.character_class])?;
    count(w, t.items.as_ref().map(Vec::len), MAX_ITEMS)?;
    if let Some(items) = &t.items {
        for item in items {
            let Some(i) = item else {
                w.append(&[0])?;
                continue;
            };
            w.append(&[1])?;
            w.nullable(i.item_class.map(str::as_bytes))?;
            w.nullable(i.category.map(str::as_bytes))?;
            for c in i.colors {
                w.append(&c.to_be_bytes())?
            }
            w.append(&i.duration.to_be_bytes())?;
            w.nullable(i.price.map(str::as_bytes))?;
            w.append(&i.coupon.to_be_bytes())?
        }
    }
    for value in [
        t.skin_color,
        t.shineness,
        t.eye_color,
        t.height,
        t.bust,
        t.painting_x,
        t.painting_y,
        t.painting_rotation,
        t.painting_size,
    ] {
        w.append(&value.to_be_bytes())?
    }
    count(w, t.body_shape.as_ref().map(Vec::len), MAX_SHAPES)?;
    if let Some(shapes) = &t.body_shape {
        for (index, (key, bits)) in shapes.iter().enumerate() {
            if shapes[..index].iter().any(|(k, _)| k == key) {
                return Err(CreationWireError::DuplicateKey);
            }
            w.append(&key.to_be_bytes())?;
            w.append(&bits.to_be_bytes())?
        }
    }
    Ok(())
}
fn read_template<'a>(r: &mut Reader<'a>) -> Result<Option<Template<'a>>, CreationWireError> {
    if r.byte()? == 0 {
        return Ok(None);
    }
    let character_class = r.byte()?;
    let items = if let Some(n) = read_count(r, MAX_ITEMS)? {
        let mut items = Vec::with_capacity(n);
        for _ in 0..n {
            if r.byte()? == 0 {
                items.push(None);
                continue;
            }
            items.push(Some(CustomizeItem {
                item_class: r.string()?,
                category: r.string()?,
                colors: [
                    i32::from_be_bytes(r.fixed()?),
                    i32::from_be_bytes(r.fixed()?),
                    i32::from_be_bytes(r.fixed()?),
                ],
                duration: i32::from_be_bytes(r.fixed()?),
                price: r.string()?,
                coupon: i64::from_be_bytes(r.fixed()?),
            }))
        }
        Some(items)
    } else {
        None
    };
    let skin_color = i32::from_be_bytes(r.fixed()?);
    let shineness = i32::from_be_bytes(r.fixed()?);
    let eye_color = i32::from_be_bytes(r.fixed()?);
    let height = i32::from_be_bytes(r.fixed()?);
    let bust = i32::from_be_bytes(r.fixed()?);
    let painting_x = i32::from_be_bytes(r.fixed()?);
    let painting_y = i32::from_be_bytes(r.fixed()?);
    let painting_rotation = i32::from_be_bytes(r.fixed()?);
    let painting_size = i32::from_be_bytes(r.fixed()?);
    let body_shape = if let Some(n) = read_count(r, MAX_SHAPES)? {
        let mut shapes = Vec::with_capacity(n);
        for _ in 0..n {
            let key = i32::from_be_bytes(r.fixed()?);
            let bits = u32::from_be_bytes(r.fixed()?);
            if shapes.iter().any(|(k, _)| *k == key) {
                return Err(CreationWireError::DuplicateKey);
            }
            shapes.push((key, bits))
        }
        Some(shapes)
    } else {
        None
    };
    Ok(Some(Template {
        character_class,
        items,
        skin_color,
        shineness,
        eye_color,
        height,
        bust,
        painting_x,
        painting_y,
        painting_rotation,
        painting_size,
        body_shape,
    }))
}
pub fn encode(
    kind: Kind,
    message: Option<&Message<'_>>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, CreationWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), CreationWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if m.kind() != kind {
            return Err(CreationWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::Create {
                name,
                template,
                preset,
                premium,
            } => {
                w.nullable(name.map(str::as_bytes))?;
                write_template(&mut w, template.as_ref())?;
                w.append(&[u8::from(*preset), u8::from(*premium)])?
            }
            Message::Fail(code) => w.append(&code.to_be_bytes())?,
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
) -> Result<Option<Message<'_>>, CreationWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::Create => Message::Create {
                name: r.string()?,
                template: read_template(&mut r)?,
                preset: r.byte()? != 0,
                premium: r.byte()? != 0,
            },
            Kind::Fail => Message::Fail(i32::from_be_bytes(r.fixed()?)),
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
) -> Result<Option<Message<'_>>, CreationWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    if category == kind.category() {
        return decode(kind, body, max_body);
    }
    if category != 0 {
        return Err(CreationWireError::Type);
    }
    let guid = body.get(..16).ok_or(PayloadError::Truncated)?;
    let fields = &body[16..];
    if guid == kind.type_id().as_bytes() {
        return decode(kind, fields, max_body - 16);
    }
    let object = Uuid::from_u128(0x81c5fe01027c3e1c98d5da9c9862aa21);
    if guid == object.as_bytes() {
        if *fields.first().ok_or(PayloadError::Truncated)? != 0 {
            return Err(CreationWireError::Object);
        }
        if fields.len() != 1 {
            return Err(PayloadError::TrailingBytes.into());
        }
        return Ok(None);
    }
    Err(CreationWireError::Type)
}
