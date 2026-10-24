//! The ENHANCE attribute entity of a non-combined equipment item: Enhance.TryLoad and
//! Enhance.UpdateBonusStat over the class ItemStat, with the EnhanceInfo/EnhanceStatInfo
//! rows of ItemContents.GetEnhanceInfo(item, level).
use super::{
    character_stats::Arithmetic,
    item_class_ex::try_integer_argument,
    item_stats::{ItemStat, StatError},
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnhanceError {
    /// GetEnhanceStatInfo returned null: the source foreach throws NullReferenceException.
    #[error("enhance info without stat rows")]
    MissingStats,
    #[error(transparent)]
    Stat(#[from] StatError),
}

/// EnhanceInfo columns used by the item service.
#[derive(Clone, Debug, PartialEq)]
pub struct EnhanceInfo {
    pub level: i32,
    pub material1: Option<String>,
    pub num1: i32,
    pub material2: Option<String>,
    pub num2: i32,
    pub gold: i32,
    pub success_ratio: f32,
    pub on_fail: Option<String>,
    pub on_fail_recipe_ratio: f32,
    pub on_fail_material_num1: i32,
    pub effect: i32,
}

/// One EnhanceStatInfo row; names as UTF-16 like the ItemStat keys.
#[derive(Clone, Debug, PartialEq)]
pub struct EnhanceStat {
    pub stat: Option<Vec<u16>>,
    pub value_type: Option<String>,
    pub value: f32,
    pub base: Option<Vec<u16>>,
}

/// The rows of one EnhanceInfo: `None` when GetEnhanceStatInfo is null.
pub type StatRows<'a> = Option<&'a [EnhanceStat]>;

/// Enhance after TryLoad from a fresh entity (Level 0, empty Stat).
#[derive(Clone, PartialEq, Eq)]
pub struct Loaded {
    pub level: i32,
    pub stat: ItemStat,
}

/// (int) of a Single/Double on Framework: truncation; out of range is 0x80000000.
fn clr_i32(value: f64) -> i32 {
    let n = value.trunc();
    if n.is_finite() && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&n) {
        n as i32
    } else {
        i32::MIN
    }
}

/// (int)((float)(num + value) * multiplier), as each measured architecture evaluates it.
pub fn multiply(num: i32, multiplier: f32, arithmetic: Arithmetic) -> i32 {
    let base = num as f32;
    match arithmetic {
        Arithmetic::FrameworkX64 => clr_i32(f64::from(base * multiplier)),
        Arithmetic::FrameworkX86 => clr_i32(f64::from(base) * f64::from(multiplier)),
    }
}

/// UpdateBonusStat(null) for the selected EnhanceInfo rows (no combined parts).
pub fn bonus_stat(
    class_stat: &ItemStat,
    rows: StatRows<'_>,
    arithmetic: Arithmetic,
) -> Result<ItemStat, EnhanceError> {
    let rows = rows.ok_or(EnhanceError::MissingStats)?;
    let mut stat = ItemStat::default();
    for row in rows {
        let num = class_stat.get(row.base.as_deref());
        if num <= 0 {
            continue;
        }
        match row.value_type.as_deref() {
            Some("Add") => stat.set(row.stat.as_deref(), clr_i32(f64::from(row.value)))?,
            Some("Multiply") => {
                let current = stat.get(row.stat.as_deref());
                stat.set(
                    row.stat.as_deref(),
                    current.wrapping_add(multiply(num, row.value, arithmetic)),
                )?;
            }
            _ => {}
        }
    }
    Ok(stat)
}

/// Enhance.Level after TryLoad(value) on a new entity (null/invalid/zero leave 0).
pub fn level(value: Option<&str>) -> i32 {
    value
        .and_then(|v| try_integer_argument(&v.encode_utf16().collect::<Vec<_>>()))
        .unwrap_or(0)
}

/// CurrentEnhanceInfo: the EnhanceOther level when above the item's and present,
/// otherwise the item's level; `None` when GetEnhanceInfo finds neither.
pub fn current_level(level: i32, applied_other: i32, exists: impl Fn(i32) -> bool) -> Option<i32> {
    if level >= applied_other || !exists(applied_other) {
        exists(level).then_some(level)
    } else {
        Some(applied_other)
    }
}

/// Enhance.TryLoad(value) on a new entity. `info(level)` is GetEnhanceInfo: `None` when
/// absent, otherwise its stat rows. `applied_other` is Gear.GetEnhanceOtherValue of the
/// equipped part (0 in the bag). Returns whether TryLoad reported a change.
pub fn load<'a>(
    value: Option<&str>,
    applied_other: i32,
    info: impl Fn(i32) -> Option<StatRows<'a>>,
    class_stat: &ItemStat,
    arithmetic: Arithmetic,
) -> Result<(bool, Loaded), EnhanceError> {
    let empty = Loaded {
        level: 0,
        stat: ItemStat::default(),
    };
    let Some(value) = value else {
        return Ok((true, empty));
    };
    let units: Vec<u16> = value.encode_utf16().collect();
    let Some(level) = try_integer_argument(&units) else {
        return Ok((true, empty));
    };
    if level == 0 {
        return Ok((false, empty));
    }
    let current = current_level(level, applied_other, |l| info(l).is_some()).and_then(&info);
    let stat = match current {
        Some(rows) => bonus_stat(class_stat, rows, arithmetic)?,
        None => ItemStat::default(),
    };
    Ok((true, Loaded { level, stat }))
}

/// EnhanceRuneInfo of an enabled rune class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rune {
    pub relation: i32,
    pub min_level: i32,
    pub max_level: i32,
}

/// EnhanceRuneExtraInfo by ReleationNum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuneExtra {
    pub success_ratio: f64,
    pub use_item_success_ratio: bool,
    pub destroy_on_success: bool,
    pub reset_rank_on_fail: bool,
    pub force_rank_down_on_fail: i32,
}

/// One enabled AntiBindInfo row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AntiBind {
    pub target: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub count: i32,
    pub ap: i32,
}

/// AntiBindInfo.Contains: an integer value is range-checked (int.Parse of From/To, which
/// throws on a non-integer bound), any other value prefix/suffix-matched. `None` is the
/// source FormatException.
pub fn anti_bind_contains(info: &AntiBind, value: &str) -> Option<bool> {
    let parse = |s: &str| try_integer_argument(&s.encode_utf16().collect::<Vec<_>>());
    if let Some(n) = parse(value) {
        if let Some(from) = &info.from
            && n < parse(from)?
        {
            return Some(false);
        }
        if let Some(to) = &info.to
            && parse(to)? < n
        {
            return Some(false);
        }
    } else {
        if info
            .from
            .as_ref()
            .is_some_and(|f| !value.starts_with(&f.to_lowercase()))
        {
            return Some(false);
        }
        if info
            .to
            .as_ref()
            .is_some_and(|t| !value.ends_with(&t.to_lowercase()))
        {
            return Some(false);
        }
    }
    Some(true)
}

/// A MakeItemRecipe (single equippable target) as RecipeMaterialInfo rows: material
/// class, Num and whether it is gold (Ceiling10); priced when used.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recipe {
    pub materials: Vec<(String, i32)>,
}

/// EnhanceInfo.GetFailRewardDict in insertion order: OnFailMaterialNum1 of MaterialClass1,
/// then (int)((float)Num × OnFailRecipeRatio) of each non-gold recipe material
/// (AddOrIncrease). `price` turns a material row into its PriceCalc value.
pub fn fail_rewards(
    info: &EnhanceInfo,
    recipe: Option<&Recipe>,
    mut price: impl FnMut(&str, i32) -> Option<i32>,
    arithmetic: Arithmetic,
) -> Option<Vec<(String, i32)>> {
    let mut out: Vec<(String, i32)> = Vec::new();
    let add = |key: &str, n: i32, out: &mut Vec<(String, i32)>| match out
        .iter_mut()
        .find(|(k, _)| k == key)
    {
        Some(e) => e.1 = e.1.wrapping_add(n),
        None => out.push((key.to_owned(), n)),
    };
    if info.on_fail_material_num1 > 0 {
        // Dictionary.Add of a null key throws.
        let key = info.material1.as_deref()?;
        out.push((key.to_owned(), info.on_fail_material_num1));
    }
    if info.on_fail_recipe_ratio > 0.0
        && let Some(recipe) = recipe
    {
        // Materials is a Dictionary: rows of the same class are summed after pricing.
        let mut priced: Vec<(String, i32)> = Vec::new();
        for (class, num) in &recipe.materials {
            let value = price(class, *num)?;
            add(class, value, &mut priced);
        }
        for (class, value) in priced {
            if class == "gold" {
                continue;
            }
            // Same (int)((float)n × Single) shape as the measured Multiply.
            let n = multiply(value, info.on_fail_recipe_ratio, arithmetic);
            if n > 0 {
                add(&class, n, &mut out);
            }
        }
    }
    Some(out)
}
