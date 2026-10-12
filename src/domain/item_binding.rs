//! Recovered MobileAppBuy binding decision; effective catalog loading remains separate.
use super::{
    feature_policy::FeaturePolicy,
    item_class_ex::{ItemAttribute, try_integer_argument},
};
use std::collections::BTreeMap;
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Debug, Error)]
pub enum BindingError {
    #[error("binding representation exceeds verified bounds")]
    Bound,
    #[error("binding attributes or enchant catalog contain duplicate keys")]
    Duplicate,
    #[error("item class requires unverified culture-sensitive comparison")]
    CultureUnsupported,
    #[error("enhancement value is not a Framework Int32")]
    IntegerFormat,
    #[error("enhancement value overflows Framework Int32")]
    IntegerOverflow,
    #[error("binding references an absent enchant definition")]
    MissingEnchant,
    #[error("binding accesses a gemstone slot outside the recovered enum")]
    GemstoneRange,
    #[error(transparent)]
    Feature(#[from] crate::error::Error),
}
/// Projection from an effective ItemClassInfo and its exact-key EquipItemInfo lookup.
/// Equippable is dictionary membership, not an inference from item naming.
#[derive(Clone, Copy)]
pub struct BindingInfo<'a> {
    pub item_class: &'a [u8],
    pub equippable: bool,
    pub bind: Option<&'a str>,
    pub trade_on_bind: Option<&'a str>,
}
#[derive(Clone, Copy)]
pub struct BindingElement<'a> {
    pub value: Option<&'a [u8]>,
    pub arg: i32,
}
#[derive(Clone, Copy)]
pub struct BindingAttribute<'a> {
    pub name: &'a [u8],
    /// A present dictionary key can contain a null element; LOOK still matches it.
    pub element: Option<BindingElement<'a>>,
}
pub struct BindingAttributes<'a> {
    entries: &'a [BindingAttribute<'a>],
}
fn bounded(text: &[u8]) -> Result<(), BindingError> {
    if text.len() > 1024 || !text.len().is_multiple_of(2) {
        return Err(BindingError::Bound);
    }
    Ok(())
}
fn equals_ascii(text: &[u8], ascii: &str) -> bool {
    text.len() == ascii.len() * 2
        && text
            .as_chunks::<2>()
            .0
            .iter()
            .zip(ascii.bytes())
            .all(|(u, b)| *u == [b, 0])
}
impl<'a> BindingAttributes<'a> {
    pub fn new(entries: &'a [BindingAttribute<'a>]) -> Result<Self, BindingError> {
        if entries.len() > 256 {
            return Err(BindingError::Bound);
        }
        for (i, entry) in entries.iter().enumerate() {
            bounded(entry.name)?;
            if let Some(value) = entry.element.and_then(|e| e.value) {
                bounded(value)?;
            }
            if entries[..i].iter().any(|prior| prior.name == entry.name) {
                return Err(BindingError::Duplicate);
            }
        }
        Ok(Self { entries })
    }
    fn get(&self, name: &str) -> Option<BindingElement<'a>> {
        self.entries
            .iter()
            .find(|e| equals_ascii(e.name, name))
            .and_then(|e| e.element)
    }
    fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|e| equals_ascii(e.name, name))
    }
}
/// Dependencies are evaluated lazily, in the recovered order. No fallback on error.
pub trait BindingContext {
    fn is_enabled(&mut self, expression: Option<&str>) -> Result<bool, BindingError>;
    fn enchant_level(&mut self, key: Option<&[u8]>) -> Result<i32, BindingError>;
    fn slot_count(&mut self) -> Result<i32, BindingError>;
}
/// Concrete adapter reuses the verified feature policy and exact UTF-16 enchant keys.
/// EnchantInfo uses ToDictionary: duplicate definitions are errors, not last-wins.
pub struct CatalogBindingContext<'a> {
    features: &'a FeaturePolicy,
    enchants: BTreeMap<Vec<u8>, i32>,
}
impl<'a> CatalogBindingContext<'a> {
    pub fn new(features: &'a FeaturePolicy, rows: &[(&[u8], i32)]) -> Result<Self, BindingError> {
        if rows.len() > 8192 {
            return Err(BindingError::Bound);
        }
        let mut enchants = BTreeMap::new();
        for (key, level) in rows {
            bounded(key)?;
            if enchants.insert(key.to_vec(), *level).is_some() {
                return Err(BindingError::Duplicate);
            }
        }
        Ok(Self { features, enchants })
    }
}
impl BindingContext for CatalogBindingContext<'_> {
    fn is_enabled(&mut self, expression: Option<&str>) -> Result<bool, BindingError> {
        Ok(self.features.is_enabled(expression)?)
    }
    fn enchant_level(&mut self, key: Option<&[u8]>) -> Result<i32, BindingError> {
        key.and_then(|k| self.enchants.get(k))
            .copied()
            .ok_or(BindingError::MissingEnchant)
    }
    fn slot_count(&mut self) -> Result<i32, BindingError> {
        Ok(self.features.get_integer("BraceletMaxSlotCount")?)
    }
}
fn printable_ascii(text: &[u8]) -> Result<(), BindingError> {
    bounded(text)?;
    if text
        .as_chunks::<2>()
        .0
        .iter()
        .any(|u| u[1] != 0 || !(0x20..=0x7e).contains(&u[0]))
    {
        return Err(BindingError::CultureUnsupported);
    }
    Ok(())
}
fn enhance_integer(value: Option<&[u8]>) -> Result<i32, BindingError> {
    let Some(value) = value else {
        return Ok(0);
    };
    bounded(value)?;
    let units = Zeroizing::new(
        value
            .as_chunks::<2>()
            .0
            .iter()
            .map(|u| u16::from_le_bytes([u[0], u[1]]))
            .collect::<Vec<_>>(),
    );
    if let Some(integer) = try_integer_argument(&units) {
        return Ok(integer);
    }
    let mut text: &[u16] = &units;
    while text.last() == Some(&0) {
        text = &text[..text.len() - 1];
    }
    let ws = |u: &u16| matches!(*u, 0x9..=0xd | 0x20);
    while text.first().is_some_and(ws) {
        text = &text[1..];
    }
    while text.last().is_some_and(ws) {
        text = &text[..text.len() - 1];
    }
    if matches!(text.first(), Some(43 | 45)) {
        text = &text[1..];
    }
    if text.is_empty() || text.iter().any(|u| !(48..=57).contains(u)) {
        return Err(BindingError::IntegerFormat);
    }
    Err(BindingError::IntegerOverflow)
}
/// The original null-info result is true, including when explicitly unbound.
pub fn check_character_bind(
    is_binded: bool,
    info: Option<BindingInfo<'_>>,
    attributes: &BindingAttributes<'_>,
    context: &mut impl BindingContext,
) -> Result<bool, BindingError> {
    let Some(info) = info else {
        return Ok(true);
    };
    if is_binded {
        return Ok(true);
    }
    bounded(info.item_class)?;
    if !info.equippable && !equals_ascii(info.item_class, "gold") {
        return Ok(true);
    }
    if context.is_enabled(info.bind)? || context.is_enabled(info.trade_on_bind)? {
        return Ok(true);
    }
    printable_ascii(info.item_class)?;
    if info
        .item_class
        .as_chunks::<2>()
        .0
        .iter()
        .map(|u| u[0])
        .take("enchant_scroll".len())
        .eq("enchant_scroll".bytes())
    {
        return context.is_enabled(Some("Macro_EnchantTradeRestriction"));
    }
    check_high_level_equipment(info, attributes, context)
}
/// Source precedence: wing, enhancement, LOOK, quality, prefix, suffix, gemstone.
pub fn check_high_level_equipment(
    info: BindingInfo<'_>,
    attributes: &BindingAttributes<'_>,
    context: &mut impl BindingContext,
) -> Result<bool, BindingError> {
    if !info.equippable {
        return Ok(false);
    }
    printable_ascii(info.item_class)?;
    if info.item_class.len() == "succubus_wing_upper_cash".len() * 2
        && info
            .item_class
            .as_chunks::<2>()
            .0
            .iter()
            .zip("succubus_wing_upper_cash".bytes())
            .all(|(u, b)| u[0].to_ascii_lowercase() == b)
    {
        return Ok(true);
    }
    if let Some(enhance) = attributes.get("ENHANCE")
        && enhance_integer(enhance.value)? >= 9
    {
        return Ok(true);
    }
    if attributes.contains("LOOK") {
        return Ok(true);
    }
    if attributes.get("QUALITY").is_some_and(|a| a.arg >= 5) {
        return Ok(true);
    }
    for name in ["PREFIX", "SUFFIX"] {
        if let Some(enchant) = attributes.get(name)
            && context.enchant_level(enchant.value)? >= 7
        {
            return Ok(true);
        }
    }
    let mut i = 0;
    // GetInteger is called at every condition in the source, including the final false one.
    while i < context.slot_count()? {
        let key = ["GS_0", "GS_1", "GS_2", "GS_3"]
            .get(i as usize)
            .ok_or(BindingError::GemstoneRange)?;
        if attributes.get(key).is_some() {
            return Ok(true);
        }
        i += 1;
    }
    Ok(false)
}
/// Integrates the recovered ItemClassEx attributes without copying their opaque values.
pub fn check_parsed_character_bind(
    is_binded: bool,
    info: Option<BindingInfo<'_>>,
    attributes: &[ItemAttribute],
    context: &mut impl BindingContext,
) -> Result<bool, BindingError> {
    if attributes.len() > 256 {
        return Err(BindingError::Bound);
    }
    let entries = attributes
        .iter()
        .map(|a| BindingAttribute {
            name: a.name.as_bytes(),
            element: Some(BindingElement {
                value: Some(a.value.as_bytes()),
                arg: a.arg,
            }),
        })
        .collect::<Vec<_>>();
    check_character_bind(is_binded, info, &BindingAttributes::new(&entries)?, context)
}
