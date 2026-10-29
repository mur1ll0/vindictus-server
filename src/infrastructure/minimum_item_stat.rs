//! Item.Stat of an equipment item: ItemClassInfo.GetStat() plus the Stat of its
//! attribute entities, as Item.UpdateItemStat(null) builds it. ENHANCE, QUALITY and
//! PREFIX/SUFFIX carry a Stat (the enchant conditions read the owner's level, class, skill
//! ranks and titles); ANTIBIND and RESTORED (set by MoveToBuybackStorage) have none. Other
//! equipment attributes (ENHANCEOTHER feeds Gear.GetEnhanceOtherValue) are refused, so
//! the applied EnhanceOther value is always 0.
use super::{minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        character_stats::Arithmetic,
        enchant::{self, Context},
        enhance,
        item_attribute::Attributes,
        item_stats::ItemStat,
    },
    protocol::inventory::Slot,
};
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const ENHANCE: &str = "ENHANCE";
pub(super) const ANTIBIND: &str = "ANTIBIND";
pub(super) const RESTORED: &str = "RESTORED";
pub(super) const PREFIX: &str = "PREFIX";
pub(super) const SUFFIX: &str = "SUFFIX";
pub(super) const QUALITY: &str = "QUALITY";

pub(super) struct EquipmentStat {
    pub stat: ItemStat,
    /// Item.EnhanceLevel (0 without the entity).
    pub enhance_level: i32,
    /// EquippedItemInfo.Effect: Enhance.Effect when the entity exists.
    pub effect: Option<i32>,
    /// Prefix/Suffix StatBonusStatus (empty without the entity).
    pub prefix_status: Vec<(i32, bool)>,
    pub suffix_status: Vec<(i32, bool)>,
    /// PrefixEnchantLevel / SuffixEnchantLevel.
    pub prefix_level: i32,
    pub suffix_level: i32,
    /// Item.QualityLevel: Quality.Level with the attribute, 2 without it.
    pub quality_level: i32,
}

/// ItemOwner.Comm values the enchant conditions read.
#[derive(Default)]
pub(super) struct ItemContext {
    level: i32,
    class: i32,
    selected_title: Option<i32>,
    skills: BTreeMap<String, i32>,
    titles: BTreeSet<i32>,
}
impl Context for ItemContext {
    fn level(&self) -> i32 {
        self.level
    }
    fn class(&self) -> i32 {
        self.class
    }
    fn skill_rank(&self, skill: Option<&str>) -> i32 {
        skill.and_then(|s| self.skills.get(s)).copied().unwrap_or(0)
    }
    fn title_acquired(&self, title: i32) -> bool {
        self.titles.contains(&title)
    }
    fn selected_title(&self) -> Option<i32> {
        self.selected_title
    }
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// The owner's level, class, selected title, skill ranks and acquired titles.
pub(super) async fn context(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<ItemContext, MinimumError> {
    let (level, class, selected_title): (i32, i32, Option<i32>) = sqlx::query_as(
        "SELECT level,class::integer,selected_title FROM game_character_info WHERE id=$1",
    )
    .bind(cid)
    .fetch_one(&mut **tx)
    .await?;
    let skills: Vec<(Vec<u8>, i32)> =
        sqlx::query_as("SELECT skill_utf16,rank FROM game_skills WHERE cid=$1 LIMIT 4097")
            .bind(cid)
            .fetch_all(&mut **tx)
            .await?;
    let titles: Vec<i32> =
        sqlx::query_scalar("SELECT title_id FROM game_titles WHERE cid=$1 AND acquired LIMIT 4097")
            .bind(cid)
            .fetch_all(&mut **tx)
            .await?;
    let mut map = BTreeMap::new();
    for (skill, rank) in skills {
        let units: Vec<u16> = skill
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .collect();
        map.insert(String::from_utf16(&units).map_err(|_| fail())?, rank);
    }
    Ok(ItemContext {
        level,
        class,
        selected_title,
        skills: map,
        titles: titles.into_iter().collect(),
    })
}

/// Item.Stat for the equipment class and attributes. Without a configured arithmetic
/// both measured profiles are evaluated and must agree.
pub(super) fn equipment_with(
    assets: &Assets,
    class: &[u8],
    attributes: &Attributes,
    owner: Option<&ItemContext>,
) -> Result<EquipmentStat, MinimumError> {
    let base = assets.stats_index.get(Some(class)).ok_or_else(fail)?;
    let mut out = EquipmentStat {
        stat: base.clone(),
        enhance_level: 0,
        effect: None,
        prefix_status: Vec::new(),
        suffix_status: Vec::new(),
        prefix_level: 0,
        suffix_level: 0,
        quality_level: 2,
    };
    if attributes.is_empty() {
        return Ok(out);
    }
    if attributes
        .as_slice()
        .iter()
        // LOOK (Item.Look) and COSTUMELOC change the appearance only; VALUE is a
        // GeneralValue, whose Stat is null (the colored dye ampoule's color).
        .any(|a| {
            ![
                ENHANCE,
                ANTIBIND,
                RESTORED,
                PREFIX,
                SUFFIX,
                QUALITY,
                "LOOK",
                "COSTUMELOC",
                "VALUE",
            ]
            .contains(&a.name.as_str())
        })
    {
        return Err(fail());
    }
    // Only stat-free attributes: the enhance contents are not consulted.
    if attributes
        .as_slice()
        .iter()
        .all(|a| ["LOOK", "COSTUMELOC", "VALUE"].contains(&a.name.as_str()))
    {
        return Ok(out);
    }
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    if let Some(value) = attributes.get(ENHANCE).map(|a| a.value.as_str()) {
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
        out.stat.add(Some(&loaded.stat)).map_err(|_| fail())?;
        out.enhance_level = loaded.level;
        out.effect = enhance::current_level(loaded.level, 0, |l| contents.info(class, l).is_some())
            .and_then(|l| contents.info(class, l))
            .map(|i| i.effect);
    }
    if let Some(attribute) = attributes.get(QUALITY) {
        // Quality.TryLoad on a fresh entity: an Arg of 0 keeps Level 0 and no Stat;
        // otherwise UpdateBonusStat(null) over GetQualityStatInfo (an empty list without
        // rows), the same Add/Multiply rule as Enhance on a non-combined item.
        out.quality_level = attribute.arg;
        if attribute.arg != 0 {
            let rows = contents.quality_stat_rows(class, attribute.arg);
            let run =
                |arithmetic| enhance::bonus_stat(base, Some(rows), arithmetic).map_err(|_| fail());
            let stat = match assets.item_arithmetic {
                Some(arithmetic) => run(arithmetic)?,
                None => {
                    let x86 = run(Arithmetic::FrameworkX86)?;
                    if run(Arithmetic::FrameworkX64)? != x86 {
                        return Err(fail());
                    }
                    x86
                }
            };
            out.stat.add(Some(&stat)).map_err(|_| fail())?;
        }
    }
    for name in [PREFIX, SUFFIX] {
        let Some(attribute) = attributes.get(name) else {
            continue;
        };
        let owner = owner.ok_or_else(fail)?;
        let now = super::lab_clock::utc_now().map_err(|_| fail())?.ticks();
        // (UtcNow - epoch) in seconds for Arg2.ToDateTime().
        let now_unix = (now - 621_355_968_000_000_000) / 10_000_000;
        let equip_class = assets.equip_class.get(class).cloned().flatten();
        let loaded = enchant::load(
            contents.enchant(&attribute.value),
            contents.enchant_stats(&attribute.value),
            attribute.arg,
            attribute.arg2,
            now_unix,
            equip_class.as_deref(),
            owner,
        )
        .map_err(|_| fail())?;
        // LoadAttributeEntity removes an expired enchant; none is created by this port.
        if loaded.expired {
            return Err(fail());
        }
        out.stat.add(loaded.stat.as_ref()).map_err(|_| fail())?;
        if name == PREFIX {
            out.prefix_status = loaded.status;
            out.prefix_level = loaded.level;
        } else {
            out.suffix_status = loaded.status;
            out.suffix_level = loaded.level;
        }
    }
    Ok(out)
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

/// Item.QualityLevel of a projected slot (2 without a QUALITY attribute).
pub(super) fn slot_quality_level(slot: &Slot) -> i32 {
    slot.attributes
        .iter()
        .flatten()
        .flatten()
        .find(|a| a.name.as_deref() == Some(QUALITY))
        .map_or(2, |a| a.arg)
}

/// Prefix/SuffixEnchantLevel of a projected slot (unknown classes are 0).
pub(super) fn slot_enchant_levels(assets: &Assets, slot: &Slot) -> (i32, i32) {
    let level = |name: &str| {
        slot.attributes
            .iter()
            .flatten()
            .flatten()
            .find(|a| a.name.as_deref() == Some(name))
            .and_then(|a| a.value.as_deref())
            .and_then(|v| assets.enhance.as_ref()?.enchant(v))
            .map_or(0, |i| i.level)
    };
    (level(PREFIX), level(SUFFIX))
}
