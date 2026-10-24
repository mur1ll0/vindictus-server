//! Item.Attribute: the ItemAttribute rows of an item as ItemAttributeElement values keyed
//! by AttributeName (Dictionary, ordinal), in load order. Covers the storage text bounds
//! (varchar(50) columns), the ItemClassEx request form and the attribute part of
//! ItemTraitExtension.IsStackableWith (same count, same keys, equal Value and Arg; Arg2
//! is not compared).
use super::{
    item_class_ex::{self, ItemAttribute, ItemClassExError},
    legacy_user_data::LegacyBytes,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AttributeError {
    /// Dictionary.Add of an existing AttributeName throws.
    #[error("duplicate item attribute name")]
    Duplicate,
    /// Name/Value outside the varchar(50) printable ASCII profile, or too many attributes.
    #[error("item attribute outside the stored profile")]
    Bound,
    #[error(transparent)]
    ClassEx(#[from] ItemClassExError),
}

/// The varchar(50) ItemAttribute columns hold printable ASCII in this profile.
const TEXT_LIMIT: usize = 50;
pub const MAX_ATTRIBUTES: usize = 256;
/// Item.MinUpTime source; SlotInfo MinUpTimeDateTimeDiff needs ToFormatableDateTime.
pub const MIN_UPTIME: &str = "MIN_UPTIME";
/// ItemRequestProperty.Build_Attribute_Expire sets ExpireDateTime from it.
pub const EXPIRE: &str = "EXPIRE";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
    pub arg: i32,
    pub arg2: i32,
}
impl Attribute {
    /// A stored row: both columns as varchar bytes.
    pub fn from_stored(
        name: &[u8],
        value: &[u8],
        arg: i32,
        arg2: i32,
    ) -> Result<Self, AttributeError> {
        Ok(Self {
            name: stored_text(name, false)?,
            value: stored_text(value, true)?,
            arg,
            arg2,
        })
    }
}

fn stored_text(bytes: &[u8], empty: bool) -> Result<String, AttributeError> {
    if bytes.len() > TEXT_LIMIT
        || (!empty && bytes.is_empty())
        || !bytes.iter().all(|b| (0x20..=0x7e).contains(b))
    {
        return Err(AttributeError::Bound);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| AttributeError::Bound)
}

/// Item.Attribute in insertion (load) order with unique ordinal names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attributes(Vec<Attribute>);
/// The empty dictionary of a plain request.
pub static EMPTY: Attributes = Attributes(Vec::new());
impl Attributes {
    pub fn new(list: Vec<Attribute>) -> Result<Self, AttributeError> {
        if list.len() > MAX_ATTRIBUTES {
            return Err(AttributeError::Bound);
        }
        for (i, a) in list.iter().enumerate() {
            stored_text(a.name.as_bytes(), false)?;
            stored_text(a.value.as_bytes(), true)?;
            if list[..i].iter().any(|b| b.name == a.name) {
                return Err(AttributeError::Duplicate);
            }
        }
        Ok(Self(list))
    }
    pub fn as_slice(&self) -> &[Attribute] {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, name: &str) -> Option<&Attribute> {
        self.0.iter().find(|a| a.name == name)
    }
    /// The IsStackableWith attribute comparison as a value: equal keys ⇔ stackable.
    pub fn stack_key(&self) -> StackKey {
        let mut key: Vec<_> = self
            .0
            .iter()
            .map(|a| (a.name.clone(), a.value.clone(), a.arg))
            .collect();
        key.sort();
        StackKey(key)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackKey(Vec<(String, String, i32)>);

/// Interns stack keys as small integers for the storage planner; 0 is the empty
/// dictionary.
#[derive(Default)]
pub struct StackKeys(Vec<StackKey>);
impl StackKeys {
    pub fn intern(&mut self, attributes: &Attributes) -> u32 {
        let key = attributes.stack_key();
        if key.0.is_empty() {
            return 0;
        }
        let position = match self.0.iter().position(|k| *k == key) {
            Some(p) => p,
            None => {
                self.0.push(key);
                self.0.len() - 1
            }
        };
        u32::try_from(position + 1).unwrap_or(u32::MAX)
    }
}

/// IsStackableWith over two attribute views (name, value, arg).
pub fn stackable<'a>(
    left: impl IntoIterator<Item = (&'a str, &'a str, i32)>,
    right: impl IntoIterator<Item = (&'a str, &'a str, i32)>,
) -> bool {
    let left: Vec<_> = left.into_iter().collect();
    let right: Vec<_> = right.into_iter().collect();
    left.len() == right.len()
        && left.iter().all(|(name, value, arg)| {
            right
                .iter()
                .find(|r| r.0 == *name)
                .is_some_and(|r| r.1 == *value && r.2 == *arg)
        })
}

fn utf16(text: &str) -> Result<LegacyBytes<1024, true>, AttributeError> {
    Ok(
        LegacyBytes::from_bytes(text.encode_utf16().flat_map(u16::to_le_bytes).collect())
            .map_err(ItemClassExError::from)?,
    )
}
fn text(bytes: &LegacyBytes<1024, true>) -> Result<String, AttributeError> {
    let units: Vec<u16> = bytes
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes(*b))
        .collect();
    String::from_utf16(&units).map_err(|_| AttributeError::Bound)
}

/// ItemClassExBuilder.Parse for a request: the class and its attributes. Arguments
/// accept only the Integer branch of ToArgInt; dates/relative values are refused.
pub fn parse_class_ex(class_ex: &str) -> Result<(String, Attributes), AttributeError> {
    let parsed = item_class_ex::parse(Some(&utf16(class_ex)?), |a| {
        item_class_ex::integer_only_argument(a)
    })?;
    let attributes = parsed
        .attributes
        .iter()
        .map(|a| {
            Ok(Attribute {
                name: text(&a.name)?,
                value: text(&a.value)?,
                arg: a.arg,
                arg2: a.arg2,
            })
        })
        .collect::<Result<Vec<_>, AttributeError>>()?;
    Ok((text(&parsed.item_class)?, Attributes::new(attributes)?))
}

/// ItemClassExBuilder.Build(ItemClass, Attribute), as BuybackInfo and ToItemClassEx use.
pub fn build_class_ex(class: &str, attributes: &Attributes) -> Result<String, AttributeError> {
    let list = attributes
        .0
        .iter()
        .map(|a| {
            Ok(ItemAttribute {
                name: utf16(&a.name)?,
                value: utf16(&a.value)?,
                arg: a.arg,
                arg2: a.arg2,
            })
        })
        .collect::<Result<Vec<_>, AttributeError>>()?;
    text(&item_class_ex::build(&utf16(class)?, &list)?)
}
