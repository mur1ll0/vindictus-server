//! Item.Stat of an equipment item: ItemClassInfo.GetStat() plus the Stat of its
//! attribute entities, as Item.UpdateItemStat(null) builds it. ENHANCE carries a Stat;
//! ANTIBIND and RESTORED (set by MoveToBuybackStorage) have none. Other equipment
//! attributes (ENHANCEOTHER feeds Gear.GetEnhanceOtherValue) are refused, so the applied
//! EnhanceOther value is always 0.
use super::{minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        character_stats::Arithmetic, enhance, item_attribute::Attributes, item_stats::ItemStat,
    },
    protocol::inventory::Slot,
};

pub(super) const ENHANCE: &str = "ENHANCE";
pub(super) const ANTIBIND: &str = "ANTIBIND";
pub(super) const RESTORED: &str = "RESTORED";

pub(super) struct EquipmentStat {
    pub stat: ItemStat,
    /// Item.EnhanceLevel (0 without the entity).
    pub enhance_level: i32,
    /// EquippedItemInfo.Effect: Enhance.Effect when the entity exists.
    pub effect: Option<i32>,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// Item.Stat for the equipment class and attributes. Without a configured arithmetic
/// both measured profiles are evaluated and must agree.
pub(super) fn equipment(
    assets: &Assets,
    class: &[u8],
    attributes: &Attributes,
) -> Result<EquipmentStat, MinimumError> {
    let base = assets.stats_index.get(Some(class)).ok_or_else(fail)?;
    if attributes.is_empty() {
        return Ok(EquipmentStat {
            stat: base.clone(),
            enhance_level: 0,
            effect: None,
        });
    }
    if attributes
        .as_slice()
        .iter()
        .any(|a| ![ENHANCE, ANTIBIND, RESTORED].contains(&a.name.as_str()))
    {
        return Err(fail());
    }
    let Some(value) = attributes.get(ENHANCE).map(|a| a.value.as_str()) else {
        return Ok(EquipmentStat {
            stat: base.clone(),
            enhance_level: 0,
            effect: None,
        });
    };
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let run = |arithmetic| {
        enhance::load(
            Some(value),
            0,
            |level| contents.stat_rows(class, level),
            base,
            arithmetic,
        )
        .map_err(|_| fail())
    };
    let (_, loaded) = match assets.item_arithmetic {
        Some(arithmetic) => run(arithmetic)?,
        None => {
            let x86 = run(Arithmetic::FrameworkX86)?;
            if run(Arithmetic::FrameworkX64)? != x86 {
                return Err(fail());
            }
            x86
        }
    };
    let mut stat = base.clone();
    stat.add(Some(&loaded.stat)).map_err(|_| fail())?;
    let effect = enhance::current_level(loaded.level, 0, |l| contents.info(class, l).is_some())
        .and_then(|l| contents.info(class, l))
        .map(|i| i.effect);
    Ok(EquipmentStat {
        stat,
        enhance_level: loaded.level,
        effect,
    })
}

/// Item.EnhanceLevel of a projected slot (its ENHANCE attribute, 0 without it).
pub(super) fn slot_enhance_level(slot: &Slot) -> i32 {
    slot.attributes
        .iter()
        .flatten()
        .flatten()
        .find(|a| a.name.as_deref() == Some(ENHANCE))
        .map_or(0, |a| enhance::level(a.value.as_deref()))
}
