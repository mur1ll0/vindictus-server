//! Panel edit of an equipment item's ENHANCE level and PREFIX/SUFFIX enchants
//! (docs/future/painel-personagem-e-busca.md, section 2): the values the game itself could
//! produce. ENHANCE: a level of the class's EnhanceType (EnhanceInfo rows, 1..=GetEnhanceMaxLevel).
//! Enchant: an EnchantInfo of the right slot (IsPrefix), EnchantInfo.IsAvailableTo the class
//! and EquipItemInfo.Enchantable, as EnchantItemProcessor checks the scroll, with the Arg
//! DoEnchant draws (MinArgValue..=MaxArgValue). Timed enchants (Duration) are left out.
use super::{
    enchant::{self, EnchantInfo},
    item_attribute::Attributes,
};

pub const ENHANCE: &str = "ENHANCE";
pub const PREFIX: &str = "PREFIX";
pub const SUFFIX: &str = "SUFFIX";
/// Enhance.Success binds at this level (EnhanceItemByDataProcessor).
pub const ENHANCE_BIND_LEVEL: i32 = 9;
/// DoEnchant binds at this EnchantLevel.
pub const ENCHANT_BIND_LEVEL: i32 = 7;

/// What the content says about one item class.
#[derive(Clone, Debug)]
pub struct Facts<'a> {
    pub class: &'a str,
    /// The equipment category IsAvailableTo reads (WEAPON, TUNIC...).
    pub category: &'a str,
    pub equip_class: Option<&'a str>,
    /// EquipItemInfo.Enchantable != 0.
    pub enchantable: bool,
    /// GetEnhanceMaxLevel: the largest EnhanceInfo level of the class's type, -1 without one.
    pub max_enhance: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the item has no EnhanceType")]
    NoEnhance,
    #[error("enhance level outside the item's EnhanceInfo levels")]
    EnhanceLevel,
    #[error("the item is not enchantable")]
    NotEnchantable,
    #[error("unknown enchant")]
    UnknownEnchant,
    #[error("enchant of the other slot")]
    WrongSlot,
    #[error("enchant not available to the item")]
    NotAvailable,
    #[error("timed enchant")]
    Timed,
    #[error("enchant argument outside its range")]
    Argument,
    #[error("enhance value is not a level")]
    EnhanceValue,
}

impl Refusal {
    /// Fixed label for the log and the panel.
    pub fn name(self) -> &'static str {
        match self {
            Self::NoEnhance => "no-enhance",
            Self::EnhanceLevel => "enhance-level",
            Self::NotEnchantable => "not-enchantable",
            Self::UnknownEnchant => "unknown-enchant",
            Self::WrongSlot => "wrong-slot",
            Self::NotAvailable => "not-available",
            Self::Timed => "timed-enchant",
            Self::Argument => "enchant-argument",
            Self::EnhanceValue => "enhance-value",
        }
    }
}

/// Enhance level 0 removes the attribute; otherwise 1..=max of the item's type.
pub fn check_enhance(facts: &Facts<'_>, level: i32) -> Result<(), Refusal> {
    if level == 0 {
        return Ok(());
    }
    if facts.max_enhance < 1 {
        return Err(Refusal::NoEnhance);
    }
    if !(1..=facts.max_enhance).contains(&level) {
        return Err(Refusal::EnhanceLevel);
    }
    Ok(())
}

/// One enchant for the prefix (`prefix` true) or suffix slot with its Arg.
pub fn check_enchant(
    facts: &Facts<'_>,
    info: Option<&EnchantInfo>,
    prefix: bool,
    arg: i32,
) -> Result<(), Refusal> {
    let info = info.ok_or(Refusal::UnknownEnchant)?;
    if !facts.enchantable {
        return Err(Refusal::NotEnchantable);
    }
    if info.is_prefix != prefix {
        return Err(Refusal::WrongSlot);
    }
    if info.duration > 0 {
        return Err(Refusal::Timed);
    }
    if !enchant::is_available_to(info, facts.class, facts.category, facts.equip_class) {
        return Err(Refusal::NotAvailable);
    }
    if !(info.min_arg..=info.max_arg).contains(&arg) {
        return Err(Refusal::Argument);
    }
    Ok(())
}

/// The enchants of a slot the item accepts, by EnchantLevel then class.
pub fn options<'a>(
    facts: &Facts<'_>,
    enchants: impl IntoIterator<Item = &'a EnchantInfo>,
    prefix: bool,
) -> Vec<&'a EnchantInfo> {
    let mut found: Vec<&EnchantInfo> = enchants
        .into_iter()
        .filter(|i| check_enchant(facts, Some(i), prefix, i.max_arg).is_ok())
        .collect();
    found.sort_by(|a, b| (a.level, &a.class).cmp(&(b.level, &b.class)));
    found
}

/// The ENHANCE, PREFIX and SUFFIX attributes of an ItemClassEx request, by the same rules
/// (the panel's "give item"). Other attributes are not judged here.
pub fn check_attributes<'a>(
    facts: &Facts<'_>,
    attributes: &Attributes,
    enchant: impl Fn(&str) -> Option<&'a EnchantInfo>,
) -> Result<(), Refusal> {
    if let Some(a) = attributes.get(ENHANCE) {
        let level = a.value.parse::<i32>().map_err(|_| Refusal::EnhanceValue)?;
        check_enhance(facts, level)?;
    }
    for (name, prefix) in [(PREFIX, true), (SUFFIX, false)] {
        if let Some(a) = attributes.get(name) {
            if a.arg2 != 0 {
                return Err(Refusal::Timed);
            }
            check_enchant(facts, enchant(&a.value), prefix, a.arg)?;
        }
    }
    Ok(())
}

/// Whether the game would have bound the item (tradable = false) for these values.
pub fn binds(enhance: i32, enchant_levels: [i32; 2]) -> bool {
    enhance >= ENHANCE_BIND_LEVEL || enchant_levels.iter().any(|l| *l >= ENCHANT_BIND_LEVEL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::item_attribute::parse_class_ex;

    fn info(class: &str, prefix: bool, level: i32, constraint: &str) -> EnchantInfo {
        EnchantInfo {
            class: class.into(),
            is_prefix: prefix,
            level,
            min_arg: 0,
            max_arg: 0,
            constraint: Some(constraint.into()),
            min_ratio: 100,
            max_ratio: 100,
            destruction: 0,
            mini_game: 0,
            duration: 0,
            ignore_max_armor: false,
        }
    }

    fn sword() -> Facts<'static> {
        Facts {
            class: "longsword_swordwind",
            category: "WEAPON",
            equip_class: Some("LONGSWORD"),
            enchantable: true,
            max_enhance: 15,
        }
    }

    fn enchants() -> Vec<EnchantInfo> {
        let mut timed = info("timed", true, 1, "WEAPON");
        timed.duration = 3600;
        let mut ranged = info("ranged", false, 6, "LONGSWORD");
        ranged.min_arg = 2;
        ranged.max_arg = 5;
        vec![
            info("sharp", true, 2, "WEAPON"),
            info("protected", true, 2, "TUNIC"),
            info("simple", true, 1, "CLOTH,LIGHT_ARMOR,WEAPON"),
            info(
                "plate_helm",
                false,
                3,
                "(HEAVY_ARMOR,PLATE_ARMOR)&(HELM,PANTS)",
            ),
            ranged,
            timed,
        ]
    }

    #[test]
    fn enhance_levels_follow_the_type() {
        assert_eq!(check_enhance(&sword(), 0), Ok(()));
        assert_eq!(check_enhance(&sword(), 15), Ok(()));
        assert_eq!(check_enhance(&sword(), 16), Err(Refusal::EnhanceLevel));
        assert_eq!(check_enhance(&sword(), -1), Err(Refusal::EnhanceLevel));
        let plain = Facts {
            max_enhance: -1,
            ..sword()
        };
        assert_eq!(check_enhance(&plain, 1), Err(Refusal::NoEnhance));
    }

    #[test]
    fn options_are_the_enchants_the_scroll_would_accept() {
        let all = enchants();
        let classes = |prefix| -> Vec<&str> {
            options(&sword(), &all, prefix)
                .iter()
                .map(|i| i.class.as_str())
                .collect()
        };
        assert_eq!(classes(true), ["simple", "sharp"]);
        assert_eq!(classes(false), ["ranged"]);
        let helm = Facts {
            class: "plate_helm_x",
            category: "PLATE_ARMOR",
            equip_class: Some("HELM"),
            enchantable: true,
            max_enhance: 10,
        };
        assert_eq!(options(&helm, &all, false)[0].class, "plate_helm");
        let closed = Facts {
            enchantable: false,
            ..sword()
        };
        assert!(options(&closed, &all, true).is_empty());
    }

    #[test]
    fn requests_are_judged_like_the_game() {
        let all = enchants();
        let find = |c: &str| all.iter().find(|i| i.class == c);
        let check = |text: &str| {
            let (_, attributes) = parse_class_ex(text).expect("class ex");
            check_attributes(&sword(), &attributes, find)
        };
        assert_eq!(
            check("longsword_swordwind[ENHANCE:10,PREFIX:sharp]"),
            Ok(())
        );
        assert_eq!(
            check("longsword_swordwind[ENHANCE:20]"),
            Err(Refusal::EnhanceLevel)
        );
        assert_eq!(
            check("longsword_swordwind[PREFIX:protected]"),
            Err(Refusal::NotAvailable)
        );
        assert_eq!(
            check("longsword_swordwind[SUFFIX:sharp]"),
            Err(Refusal::WrongSlot)
        );
        assert_eq!(
            check("longsword_swordwind[PREFIX:timed]"),
            Err(Refusal::Timed)
        );
        assert_eq!(
            check("longsword_swordwind[PREFIX:nothing]"),
            Err(Refusal::UnknownEnchant)
        );
        assert_eq!(
            check("longsword_swordwind[SUFFIX:ranged]"),
            Err(Refusal::Argument)
        );
        assert!(binds(9, [0, 0]) && binds(0, [0, 7]) && !binds(8, [6, 6]));
    }
}
