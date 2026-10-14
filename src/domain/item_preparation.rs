//! Recovered ItemFactory preparation. No item IDs, locations, stats or writes are produced.
use super::item_catalog::EffectiveItem;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("item preparation exceeds verified representation or allocation bounds")]
pub struct PreparationBound;

/// Dictionary key and element fields are separate in the original. Arg2 is not compared.
pub struct StackAttribute<'a> {
    pub key: &'a [u8],
    pub value: Option<&'a [u8]>,
    pub arg: i32,
    pub arg2: i32,
}
/// Borrow the fully constructed property; this module does not replace its initialization.
pub struct RequestProperty<'a> {
    pub item: Option<EffectiveItem<'a>>,
    pub character_bound: bool,
    /// Framework DateTime ticks, with Kind deliberately excluded from equality.
    pub expire_ticks: Option<i64>,
    pub attributes: &'a [StackAttribute<'a>],
    pub colors: [i32; 3],
    pub reduced_durability: i32,
    pub max_durability_bonus: i32,
}
pub struct GiveRequest<'a> {
    pub property: &'a RequestProperty<'a>,
    pub count: i32,
    pub before_item_id: Option<i64>,
}
/// A borrowed virtual item, retaining the first matching request's complete property.
pub struct PreparedItem<'a> {
    pub property: &'a RequestProperty<'a>,
    pub count: i32,
    pub before_item_id: Option<i64>,
}
const MAX_REQUESTS: usize = 4096;
const MAX_ATTRIBUTES: usize = 128;
const MAX_ITEMS: usize = 4096;
const MAX_COMPARISON_WORK: usize = 1_000_000;
fn text(value: &[u8]) -> bool {
    value.len() <= 1024 && value.len().is_multiple_of(2)
}
fn validate(property: &RequestProperty<'_>) -> Result<(), PreparationBound> {
    if property.item.is_some_and(|v| !text(v.binding.item_class))
        || property
            .expire_ticks
            .is_some_and(|v| !(0..=3_155_378_975_999_999_999).contains(&v))
        || property.attributes.len() > MAX_ATTRIBUTES
    {
        return Err(PreparationBound);
    }
    for (index, a) in property.attributes.iter().enumerate() {
        if !text(a.key)
            || a.value.is_some_and(|v| !text(v))
            || property.attributes[..index].iter().any(|b| b.key == a.key)
        {
            return Err(PreparationBound);
        }
    }
    Ok(())
}
fn stackable(lhs: &RequestProperty<'_>, rhs: &RequestProperty<'_>) -> bool {
    let (Some(l), Some(r)) = (lhs.item, rhs.item) else {
        return false;
    };
    !l.binding.equippable
        && !r.binding.equippable
        && l.binding.item_class == r.binding.item_class
        && (l.storage == 3 || r.storage == 3 || lhs.character_bound == rhs.character_bound)
        && lhs.expire_ticks == rhs.expire_ticks
        && lhs.attributes.len() == rhs.attributes.len()
        && lhs.attributes.iter().all(|a| {
            rhs.attributes
                .iter()
                .any(|b| a.key == b.key && a.value == b.value && a.arg == b.arg)
        })
}
/// CLR ordinal UTF-16 equality; missing class returns false at this bounded boundary.
pub fn is_stackable(
    lhs: &RequestProperty<'_>,
    rhs: &RequestProperty<'_>,
) -> Result<bool, PreparationBound> {
    validate(lhs)?;
    validate(rhs)?;
    Ok(stackable(lhs, rhs))
}
/// None is the original factory's rejection (absent class, nonpositive MaxStack/count).
/// Defensive bounds are errors, never an empty or partially delivered success.
/// Int32 merge addition wraps as in the recovered unchecked Framework method.
pub fn prepare<'a>(
    requests: &[GiveRequest<'a>],
) -> Result<Option<Vec<PreparedItem<'a>>>, PreparationBound> {
    if requests.len() > MAX_REQUESTS {
        return Err(PreparationBound);
    }
    for request in requests {
        validate(request.property)?;
    }
    if requests
        .iter()
        .any(|r| r.count <= 0 || r.property.item.is_none_or(|i| i.max_stack <= 0))
    {
        return Ok(None);
    }
    let mut merged: Vec<PreparedItem<'a>> = Vec::new();
    let mut work_left = MAX_COMPARISON_WORK;
    for request in requests {
        let mut match_index = None;
        for (index, item) in merged.iter().enumerate() {
            // Conservative budget also covers the nested attribute dictionary search.
            let cost = 1 + item.property.attributes.len() * request.property.attributes.len();
            work_left = work_left.checked_sub(cost).ok_or(PreparationBound)?;
            if stackable(item.property, request.property) {
                match_index = Some(index);
                break;
            }
        }
        if let Some(index) = match_index {
            merged[index].count = merged[index].count.wrapping_add(request.count);
        } else {
            merged.push(PreparedItem {
                property: request.property,
                count: request.count,
                before_item_id: request.before_item_id,
            });
        }
    }
    let mut result = Vec::new();
    for item in merged {
        let Some(class) = item.property.item else {
            return Ok(None);
        };
        let mut remaining = item.count;
        while remaining > 0 {
            if result.len() == MAX_ITEMS {
                return Err(PreparationBound);
            }
            let count = remaining.min(class.max_stack);
            result.push(PreparedItem {
                property: item.property,
                count,
                before_item_id: item.before_item_id,
            });
            remaining -= count;
        }
    }
    Ok(Some(result))
}
