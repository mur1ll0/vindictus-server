//! Recovered UTF-16 parser. Date/culture/local-clock arguments require an explicit resolver.
use crate::domain::legacy_user_data::{LegacyBytes, LegacyUserDataError};
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ItemClassExError {
    #[error("item expression is null")]
    NullInput,
    #[error("item expression contains duplicate attribute names")]
    DuplicateAttribute,
    #[error("item expression exceeds the bounded queue representation")]
    Bound,
    #[error("argument requires an explicit legacy culture/date/clock resolver")]
    ArgumentUnsupported,
    #[error(transparent)]
    Value(#[from] LegacyUserDataError),
}
pub struct ItemAttribute {
    pub name: LegacyBytes<1024, true>,
    pub value: LegacyBytes<1024, true>,
    pub arg: i32,
    pub arg2: i32,
}
pub struct ParsedItemClassEx {
    pub item_class: LegacyBytes<1024, true>,
    pub attributes: Vec<ItemAttribute>,
}
fn units(bytes: &[u8]) -> Zeroizing<Vec<u16>> {
    Zeroizing::new(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect(),
    )
}
fn opaque(value: &[u16]) -> Result<LegacyBytes<1024, true>, ItemClassExError> {
    Ok(LegacyBytes::from_bytes(
        value.iter().flat_map(|u| u.to_le_bytes()).collect(),
    )?)
}
fn whitespace(unit: u16) -> bool {
    matches!(unit, 0x9..=0xd | 0x20 | 0x85 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000)
}
fn trim(mut text: &[u16]) -> &[u16] {
    while text.first().is_some_and(|c| whitespace(*c)) {
        text = &text[1..];
    }
    while text.last().is_some_and(|c| whitespace(*c)) {
        text = &text[..text.len() - 1];
    }
    text
}
/// Only int.TryParse's Integer branch, for the five verified source cultures.
/// None means fall through to legacy DateTime/relative/invalid handling, never silently zero.
pub fn try_integer_argument(mut text: &[u16]) -> Option<i32> {
    while text.last() == Some(&0) {
        text = &text[..text.len() - 1];
    }
    let ws = |c: &u16| matches!(*c, 0x20 | 0x9..=0xd);
    while text.first().is_some_and(ws) {
        text = &text[1..];
    }
    while text.last().is_some_and(ws) {
        text = &text[..text.len() - 1];
    }
    let negative = text.first() == Some(&45);
    if matches!(text.first(), Some(43 | 45)) {
        text = &text[1..];
    }
    if text.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    for unit in text {
        if !(48..=57).contains(unit) {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(i64::from(*unit - 48))?;
        if value > 2_147_483_648 {
            return None;
        }
    }
    i32::try_from(if negative { -value } else { value }).ok()
}
pub fn integer_only_argument(text: &[u16]) -> Result<i32, ItemClassExError> {
    try_integer_argument(text).ok_or(ItemClassExError::ArgumentUnsupported)
}
fn single(
    text: &[u16],
    resolve: &mut impl FnMut(&[u16]) -> Result<i32, ItemClassExError>,
) -> Result<Option<ItemAttribute>, ItemClassExError> {
    let mut name = None;
    let mut value = None;
    let (mut arg, mut arg2, mut start, mut state) = (0, 0, 0, 0);
    for (i, unit) in text.iter().enumerate() {
        match *unit {
            58 => {
                if state != 0 {
                    return Ok(None);
                }
                name = Some(trim(&text[start..i]));
                state = 1;
                start = i + 1;
            }
            40 | 60 => {
                if state == 1 {
                    value = Some(trim(&text[start..i]));
                }
                state = if *unit == 40 { 2 } else { 3 };
                start = i + 1;
            }
            41 => {
                if state == 2 {
                    arg = resolve(&text[start..i])?;
                }
                state = -1;
            }
            62 => {
                if state == 3 {
                    arg2 = resolve(&text[start..i])?;
                }
                state = -1;
            }
            _ => {}
        }
    }
    if state == 1 {
        value = Some(trim(&text[start..]));
    }
    match (name, value) {
        (Some(name), Some(value)) => Ok(Some(ItemAttribute {
            name: opaque(name)?,
            value: opaque(value)?,
            arg,
            arg2,
        })),
        _ => Ok(None),
    }
}
pub fn parse_single_attribute(
    input: Option<&LegacyBytes<1024, true>>,
    mut resolve: impl FnMut(&[u16]) -> Result<i32, ItemClassExError>,
) -> Result<Option<ItemAttribute>, ItemClassExError> {
    let input = input.ok_or(ItemClassExError::NullInput)?;
    single(&units(input.as_bytes()), &mut resolve)
}
fn parse_parts<'a>(
    parts: impl Iterator<Item = &'a [u16]>,
    resolve: &mut impl FnMut(&[u16]) -> Result<i32, ItemClassExError>,
) -> Result<Vec<ItemAttribute>, ItemClassExError> {
    let mut result: Vec<ItemAttribute> = Vec::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if let Some(attribute) = single(part, resolve)? {
            if result
                .iter()
                .any(|a| a.name.as_bytes() == attribute.name.as_bytes())
            {
                return Err(ItemClassExError::DuplicateAttribute);
            }
            if result.len() == 256 {
                return Err(ItemClassExError::Bound);
            }
            result.push(attribute);
        }
    }
    Ok(result)
}
pub fn parse(
    input: Option<&LegacyBytes<1024, true>>,
    mut resolve: impl FnMut(&[u16]) -> Result<i32, ItemClassExError>,
) -> Result<ParsedItemClassEx, ItemClassExError> {
    let input = input.ok_or(ItemClassExError::NullInput)?;
    let text = units(input.as_bytes());
    let mut parts = text.split(|u| matches!(*u, 44 | 91 | 93));
    // split always produces its first segment, including the empty input.
    let item_class = opaque(parts.next().unwrap_or_default())?;
    Ok(ParsedItemClassEx {
        item_class,
        attributes: parse_parts(parts, &mut resolve)?,
    })
}
pub fn parse_attributes(
    input: Option<&LegacyBytes<1024, true>>,
    mut resolve: impl FnMut(&[u16]) -> Result<i32, ItemClassExError>,
) -> Result<Vec<ItemAttribute>, ItemClassExError> {
    let input = input.ok_or(ItemClassExError::NullInput)?;
    let text = units(input.as_bytes());
    parse_parts(text.split(|u| matches!(*u, 44 | 91 | 93)), &mut resolve)
}
fn append(out: &mut Zeroizing<Vec<u8>>, bytes: &[u8]) -> Result<(), ItemClassExError> {
    if out.len() + bytes.len() > 1024 {
        return Err(ItemClassExError::Bound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
fn punctuation(out: &mut Zeroizing<Vec<u8>>, text: &str) -> Result<(), ItemClassExError> {
    for unit in text.encode_utf16() {
        append(out, &unit.to_le_bytes())?;
    }
    Ok(())
}
pub fn build(
    item_class: &LegacyBytes<1024, true>,
    attributes: &[ItemAttribute],
) -> Result<LegacyBytes<1024, true>, ItemClassExError> {
    if attributes.len() > 256 {
        return Err(ItemClassExError::Bound);
    }
    let mut out = Zeroizing::new(Vec::new());
    append(&mut out, item_class.as_bytes())?;
    if !attributes.is_empty() {
        punctuation(&mut out, "[")?;
        for (i, attribute) in attributes.iter().enumerate() {
            if i != 0 {
                punctuation(&mut out, ",")?;
            }
            append(&mut out, attribute.name.as_bytes())?;
            punctuation(&mut out, ":")?;
            append(&mut out, attribute.value.as_bytes())?;
            if attribute.arg != 0 {
                punctuation(&mut out, &format!("({})", attribute.arg))?;
            }
            if attribute.arg2 != 0 {
                punctuation(&mut out, &format!("<{}>", attribute.arg2))?;
            }
        }
        punctuation(&mut out, "]")?;
    }
    Ok(LegacyBytes::from_bytes(out.to_vec())?)
}
/// Dictionary equality for parsed unique names; attribute insertion order is irrelevant.
pub fn same(left: &ParsedItemClassEx, right: &ParsedItemClassEx) -> bool {
    left.item_class.as_bytes() == right.item_class.as_bytes()
        && left.attributes.len() == right.attributes.len()
        && left.attributes.iter().all(|a| {
            right.attributes.iter().any(|b| {
                a.name.as_bytes() == b.name.as_bytes()
                    && a.value.as_bytes() == b.value.as_bytes()
                    && a.arg == b.arg
                    && a.arg2 == b.arg2
            })
        })
}
