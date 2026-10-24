//! The PREFIX/SUFFIX attribute entity of equipment: Enchant.TryLoad and
//! UpdateBonusStat(null) with EnchantStatBonus/EnchantConditionElement, EnchantLevel,
//! and EnchantInfo.IsAvailableTo.
use super::{
    item_class_ex::try_integer_argument,
    item_stats::{ItemStat, StatError},
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnchantError {
    /// EnchantInfo without EnchantStatInfo rows, an empty condition, a null stat text,
    /// a repeated Order (Dictionary.Add) or a format string beyond "{0}".
    #[error("enchant row outside the migrated source profile")]
    Source,
    #[error(transparent)]
    Stat(#[from] StatError),
}

/// EnchantInfo columns used by the item service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnchantInfo {
    pub class: String,
    pub is_prefix: bool,
    pub level: i32,
    pub min_arg: i32,
    pub max_arg: i32,
    pub constraint: Option<String>,
    pub min_ratio: i32,
    pub max_ratio: i32,
    pub destruction: i32,
    pub mini_game: i32,
    pub duration: i32,
    pub ignore_max_armor: bool,
}

/// EnchantStatInfo, ordered by Order (stable) per EnchantClass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatRow {
    pub order: i32,
    pub condition: Option<String>,
    pub stat: Option<String>,
}

/// The character the conditions read (ItemOwner.Comm).
pub trait Context {
    fn level(&self) -> i32;
    fn class(&self) -> i32;
    fn skill_rank(&self, skill: Option<&str>) -> i32;
    fn title_acquired(&self, title: i32) -> bool;
    fn selected_title(&self) -> Option<i32>;
}

/// string.Format(format, arg) for the "{0}" placeholders the content uses.
fn safe_format(format: &str, arg: i32) -> Result<String, EnchantError> {
    let replaced = format.replace("{0}", &arg.to_string());
    if replaced.contains('{') || replaced.contains('}') {
        return Err(EnchantError::Source);
    }
    Ok(replaced)
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Condition(Vec<String>);
impl Condition {
    fn int(&self, index: usize) -> i32 {
        self.0
            .get(index)
            .and_then(|p| try_integer_argument(&units(p)))
            .unwrap_or(0)
    }
    fn check(&self, ctx: &dyn Context, equip_class: Option<&str>) -> bool {
        match self.0[0].to_lowercase().as_str() {
            "level>=" => ctx.level() >= self.int(1),
            "level<=" => ctx.level() <= self.int(1),
            // Func_skill_less_than compares with >= as well.
            "skillrank>=" | "skillrank<=" => {
                ctx.skill_rank(self.0.get(1).map(String::as_str)) >= self.int(2)
            }
            "title_acquired" => self
                .0
                .get(1)
                .and_then(|p| try_integer_argument(&units(p)))
                .is_some_and(|t| ctx.title_acquired(t)),
            "title_set" => self
                .0
                .get(1)
                .and_then(|p| try_integer_argument(&units(p)))
                .is_some_and(|t| ctx.selected_title() == Some(t)),
            "class" => ctx.class() == self.int(1),
            "equip_class" => self.0[1..]
                .iter()
                .any(|p| equip_class.is_some_and(|e| e == p)),
            _ => false,
        }
    }
}

/// EnchantStatBonus: the '&' conditions and the "Stat+N"/"@effect+N" bonus.
#[derive(Clone)]
struct Bonus {
    order: i32,
    conditions: Vec<Condition>,
    stat: ItemStat,
}
impl Bonus {
    fn parse(row: &StatRow, arg: i32) -> Result<Self, EnchantError> {
        let mut conditions = Vec::new();
        if let Some(text) = &row.condition {
            for part in safe_format(text, arg)?.split('&') {
                let tokens: Vec<String> = part
                    .split(['(', ')', ','])
                    .filter(|t| !t.is_empty())
                    .map(|t| t.trim().to_owned())
                    .collect();
                // parameters[0] of an empty list throws.
                if tokens.is_empty() {
                    return Err(EnchantError::Source);
                }
                conditions.push(Condition(tokens));
            }
        }
        let mut stat = ItemStat::default();
        let text = safe_format(row.stat.as_deref().ok_or(EnchantError::Source)?, arg)?;
        for part in text.split([',', ';']) {
            let Some(index) = part.find(['+', '-']) else {
                continue;
            };
            let Some(value) = try_integer_argument(&units(&part[index..])) else {
                continue;
            };
            if let Some(effect) = part.strip_prefix('@') {
                stat.add_status(Some(&units(&effect[..index - 1])), value)?;
            } else {
                stat.set(Some(&units(&part[..index])), value)?;
            }
        }
        Ok(Self {
            order: row.order,
            conditions,
            stat,
        })
    }
    fn check(&self, ctx: &dyn Context, equip_class: Option<&str>) -> bool {
        self.conditions.iter().all(|c| c.check(ctx, equip_class))
    }
}

/// The entity after TryLoad on a fresh Enchant.
#[derive(Clone, PartialEq, Eq)]
pub struct Loaded {
    /// Stat (null when the timed enchant is disabled).
    pub stat: Option<ItemStat>,
    /// StatBonusStatus in insertion (Order) order: SlotInfo Prefix/SuffixEnchantStatus.
    pub status: Vec<(i32, bool)>,
    /// EnchantLevel (0 when disabled, expired or unknown).
    pub level: i32,
    /// Arg2 > 0: the enchant expires at this Unix second.
    pub expired: bool,
}

/// Enchant.TryLoad(value, arg, arg2). `info`/`rows` are GetEnchantInfo(value) and its
/// GetEnchantStatInfo (`None` when absent: the source foreach then throws).
pub fn load(
    info: Option<&EnchantInfo>,
    rows: Option<&[StatRow]>,
    arg: i32,
    arg2: i32,
    now_unix: i64,
    equip_class: Option<&str>,
    ctx: &dyn Context,
) -> Result<Loaded, EnchantError> {
    // ExpireTime = epoch + arg2 seconds; IsExpired = ExpireTime < UtcNow.
    let expired = arg2 != 0 && i64::from(arg2) < now_unix;
    let mut bonuses: Vec<Bonus> = Vec::new();
    if info.is_some() {
        for row in rows.ok_or(EnchantError::Source)? {
            if bonuses.iter().any(|b| b.order == row.order) {
                return Err(EnchantError::Source);
            }
            bonuses.push(Bonus::parse(row, arg)?);
        }
    }
    if expired {
        return Ok(Loaded {
            stat: None,
            status: Vec::new(),
            level: 0,
            expired,
        });
    }
    let status: Vec<(i32, bool)> = bonuses
        .iter()
        .map(|b| (b.order, b.check(ctx, equip_class)))
        .collect();
    let mut stat = ItemStat::default();
    for (bonus, (_, ok)) in bonuses.iter().zip(&status) {
        if *ok {
            stat.add(Some(&bonus.stat))?;
        }
    }
    Ok(Loaded {
        stat: Some(stat),
        status,
        level: info.map_or(0, |i| i.level),
        expired,
    })
}

/// EnchantInfo.IsAvailableTo: every '&' group of ItemConstraint must have a token that
/// contains the item class, its category or its EquipClass (ordinal substring).
pub fn is_available_to(
    info: &EnchantInfo,
    class: &str,
    category: &str,
    equip_class: Option<&str>,
) -> bool {
    let Some(constraint) = &info.constraint else {
        return true;
    };
    constraint.split('&').map(str::trim).all(|group| {
        group.split(['(', ')', ',']).any(|t| {
            !t.is_empty()
                && (t.contains(class)
                    || t.contains(category)
                    || equip_class.is_some_and(|e| t.contains(e)))
        })
    })
}

/// EnchantSession.SuccessRatio for a roll total (EnchantSuccessBonus percent).
pub fn success_ratio(info: &EnchantInfo, current: i32, goal: i32, bonus: i32) -> f32 {
    let value = if current <= goal {
        let span = (info.max_ratio - info.min_ratio) as f32;
        let progress = current as f32 / goal as f32;
        (info.min_ratio as f32 + span * progress) / 100.0 + bonus as f32 / 100.0
    } else {
        info.min_ratio as f32 / 100.0 + bonus as f32 / 100.0
    };
    if value > 1.0 { 1.0 } else { value }
}

/// EnchantItemResult values.
pub mod result {
    pub const ENCHANT_SUCCESS: i32 = 0;
    pub const ENCHANT_FAILED: i32 = 1;
    pub const ROLL_SUCCESS: i32 = 2;
    pub const CANCELED: i32 = 3;
    pub const OPEN_SESSION_FAIL: i32 = 4;
    pub const INVALID_DICE_CLASS: i32 = 5;
    pub const NO_MORE_ROLL_AVAILABLE: i32 = 6;
    pub const NO_DICE_ITEM: i32 = 7;
    pub const INVALID_SCROLL_ITEM: i32 = 8;
    pub const INVALID_TARGET_ITEM: i32 = 9;
    pub const SCROLL_EXPIRED: i32 = 11;
    pub const ITEM_DESTROYED: i32 = 12;
    pub const ITEM_PROTECTED: i32 = 13;
    pub const SCROLL_PROTECTED: i32 = 14;
    pub const RUNE_EXPIRED: i32 = 16;
}

/// ItemClassConst indestructible scrolls (the session's IndestructibleScroll class).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protection {
    None,
    /// enchant_indestructible_scroll_perfect(_gift): always succeeds.
    Perfect,
    /// enchant_indestructible_scroll(_event): protects the item, both scrolls consumed.
    Item,
    /// enchant_indestructible_scroll_unstable: consumed on success too.
    Unstable,
    /// enchant_indestructible_scroll_lvl2: protects the enchant scroll.
    Scroll,
}
impl Protection {
    pub fn of(class: Option<&str>) -> Self {
        match class {
            Some(
                "enchant_indestructible_scroll_perfect"
                | "enchant_indestructible_scroll_perfect_gift",
            ) => Self::Perfect,
            Some("enchant_indestructible_scroll" | "enchant_indestructible_scroll_event") => {
                Self::Item
            }
            Some("enchant_indestructible_scroll_unstable") => Self::Unstable,
            Some("enchant_indestructible_scroll_lvl2") => Self::Scroll,
            _ => Self::None,
        }
    }
}

pub trait Random {
    fn next_double(&mut self) -> f64;
    /// Random.Next(maxValue); `None` for a negative maximum.
    fn next(&mut self, maximum: i32) -> Option<i32>;
}

/// EnchantSession.DoEnchant after its item checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub result: i32,
    pub consume_scroll: bool,
    pub consume_rune: bool,
    /// TargetItem.Enchant(AttributeName, EnchantClass).
    pub enchanted: bool,
    pub bind: bool,
    /// SendDialog ItemProtected (target) or ScrollProtected (target, scroll).
    pub dialog: Option<&'static str>,
    pub destroyed: bool,
    pub max_durability_loss: i32,
}

pub fn do_enchant(
    info: &EnchantInfo,
    success_ratio: f32,
    protection: Protection,
    destructible: bool,
    random: &mut dyn Random,
) -> Option<Outcome> {
    let mut o = Outcome {
        result: result::ENCHANT_FAILED,
        consume_scroll: true,
        consume_rune: false,
        enchanted: false,
        bind: false,
        dialog: None,
        destroyed: false,
        max_durability_loss: 0,
    };
    let succeed = |o: &mut Outcome| {
        o.enchanted = true;
        o.bind = info.level >= 7;
        o.result = result::ENCHANT_SUCCESS;
    };
    if protection == Protection::Perfect {
        o.consume_rune = true;
        succeed(&mut o);
        return Some(o);
    }
    if random.next_double() <= f64::from(success_ratio) {
        o.consume_rune = protection == Protection::Unstable;
        succeed(&mut o);
        return Some(o);
    }
    match protection {
        Protection::Item | Protection::Unstable => {
            o.consume_rune = true;
            o.dialog = Some("GameUI_Heroes_EnchantResult_ItemProtected");
            o.result = result::ITEM_PROTECTED;
            return Some(o);
        }
        Protection::Scroll => {
            o.consume_scroll = false;
            o.consume_rune = true;
            o.dialog = Some("GameUI_Heroes_EnchantResult_ScrollProtected");
            o.result = result::SCROLL_PROTECTED;
            return Some(o);
        }
        _ => {}
    }
    if destructible
        && info.destruction > 0
        && random.next_double() <= f64::from(info.destruction as f32 / 100.0)
    {
        o.destroyed = true;
        o.result = result::ITEM_DESTROYED;
        return Some(o);
    }
    let threshold = 30_i32.wrapping_sub(2_i32.wrapping_mul(info.level.wrapping_sub(1)));
    if random.next(100)? >= threshold {
        let maximum = if info.level >= 7 {
            info.level.wrapping_mul(2)
        } else {
            info.level
        };
        o.max_durability_loss = 1_i32.wrapping_add(random.next(maximum)?).wrapping_mul(100);
    }
    Some(o)
}
