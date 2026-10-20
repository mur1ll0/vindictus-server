//! EntityCommunicator.IsConstraintSatisfied for an ItemClassInfo-backed item.
//! The source returns "satisfied" while the character snapshot is not loaded; the
//! caller must supply loaded state instead of relying on that bypass.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub cafe_only: i32,
    pub required_level: i32,
    /// ItemClassInfo.LevelUpperBound; zero means Int32.MaxValue (UseLevelLimit).
    pub level_upper_bound: i32,
    pub class_restriction: i32,
    /// Nonzero only for guild_campfire_kit_lvl1 in the recovered trait.
    pub required_guild_level: i32,
    pub required_skill: Option<(String, i32)>,
    pub required_enhance: i32,
    pub required_enchant: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    NeedCafeAuth,
    NeedVvipAuth,
    HighLevel,
    LowLevel,
    InvalidClass,
    LowSkillRank,
    EnhanceLevel,
    EnchantLevel,
}

impl Failure {
    /// ItemOperationResult member name used by SendOperationFailedDialog.
    pub const fn name(self) -> &'static str {
        match self {
            Self::NeedCafeAuth => "ConstraintFail_NeedCafeAuth",
            Self::NeedVvipAuth => "ConstraintFail_NeedVVIPAuth",
            Self::HighLevel => "ConstraintFail_HighLevel",
            Self::LowLevel => "ConstraintFail_LowLevel",
            Self::InvalidClass => "ConstraintFail_InvalidClass",
            Self::LowSkillRank => "ConstraintFail_LowSkillRank",
            Self::EnhanceLevel => "ConstraintFail_EnhanceLevel",
            Self::EnchantLevel => "ConstraintFail_EnchantLevel",
        }
    }
}

/// Enhance/prefix/suffix levels of each non-null equipped item.
#[derive(Clone, Copy, Debug)]
pub struct Equipped {
    pub enhance: i32,
    pub prefix: i32,
    pub suffix: i32,
}

pub struct Character<'a> {
    pub level: i32,
    pub class: i32,
    pub cafe_type: i32,
    /// Result of the source cafe predicate (CafeType == 1, or CafeLevel > 0 when
    /// HomeCafeBonus_Rule_zhCN is enabled); the caller owns feature selection.
    pub cafe: bool,
    /// None when Guild.GuildInfoLoaded is false.
    pub guild_level: Option<i32>,
    pub skills: &'a BTreeMap<String, i32>,
    /// None when Gear.EquippedItems is null.
    pub equipped: Option<&'a [Equipped]>,
}

pub fn check(c: &Character<'_>, k: &Constraint) -> Option<Failure> {
    if k.cafe_only == 1 && !c.cafe {
        return Some(Failure::NeedCafeAuth);
    }
    if k.cafe_only >= 2 {
        if k.cafe_only == 2 {
            if c.cafe_type < 2 {
                return Some(Failure::NeedCafeAuth);
            }
        } else if c.cafe_type <= 2 {
            return Some(Failure::NeedVvipAuth);
        }
    }
    let upper = if k.level_upper_bound != 0 {
        k.level_upper_bound
    } else {
        i32::MAX
    };
    if c.level > upper {
        return Some(Failure::HighLevel);
    }
    if c.level < k.required_level {
        return Some(Failure::LowLevel);
    }
    // C# masks an Int32 shift count to its low five bits.
    if 1_i32.wrapping_shl(c.class as u32 & 31) & k.class_restriction == 0 {
        return Some(Failure::InvalidClass);
    }
    if k.required_guild_level > 0 && c.guild_level.is_some_and(|g| g < k.required_guild_level) {
        return Some(Failure::LowLevel);
    }
    if let Some((skill, rank)) = &k.required_skill
        && c.skills.get(skill).copied().unwrap_or(-1) < *rank
    {
        return Some(Failure::LowSkillRank);
    }
    if let Some(equipped) = c.equipped
        && k.required_enhance > 0
        && k.required_enchant > 0
    {
        let (mut enhance, mut enchant) = (false, false);
        for e in equipped {
            enhance |= k.required_enhance <= e.enhance;
            enchant |= k.required_enchant <= e.prefix || k.required_enchant <= e.suffix;
            if enhance && enchant {
                return None;
            }
        }
        if !enhance {
            return Some(Failure::EnhanceLevel);
        }
        if !enchant {
            return Some(Failure::EnchantLevel);
        }
    }
    None
}
