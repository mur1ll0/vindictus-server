//! Known-stat kernel recovered from SkillEffect and CalculateFinalStat.
//! No equipped-item loader, SQL writer, native actor or network messages.
use std::collections::BTreeMap;

use thiserror::Error;

use super::{character_base_stats::BaseStats, skill_runtime::Cache};

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Field {
    Atk,
    Matk,
    Def,
    Str,
    Dex,
    Int,
    Will,
    Luck,
    Hp,
    Stamina,
    HeavyLoad,
    LoadPenalty,
    MediumLoad,
    TotalLoad,
    AtkSpeed,
    AtkAbsolute,
    Critical,
    CritFactor,
    ResCritical,
    Balance,
    PvpAtk,
    PvpMatk,
    PvpDef,
    TownSpeed,
    SkillRankSum,
}
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Stats([i32; 25]);
impl Stats {
    pub fn from_values(values: [i32; 25]) -> Self {
        Self(values)
    }
    pub fn from_base(base: &BaseStats) -> Self {
        let mut s = Self::default();
        for (field, value) in [
            Field::Str,
            Field::Dex,
            Field::Int,
            Field::Will,
            Field::Luck,
            Field::Hp,
            Field::Stamina,
        ]
        .into_iter()
        .zip(base.values())
        {
            s.set(field, *value);
        }
        s
    }
    pub fn values(&self) -> &[i32; 25] {
        &self.0
    }
    pub fn get(&self, field: Field) -> i32 {
        self.0[field as usize]
    }
    pub fn set(&mut self, field: Field, value: i32) {
        self.0[field as usize] = value;
    }
    /// CharacterStats.Add uses unchecked Int32 additions.
    pub fn add(&self, rhs: &Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i].wrapping_add(rhs.0[i])))
    }
}
/// Architecture is explicit: Framework x86 retains wider expression intermediates.
#[derive(Clone, Copy)]
pub enum Arithmetic {
    FrameworkX86,
    FrameworkX64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StatError {
    #[error("armor definition map exceeds the admitted profile")]
    Profile,
}
pub struct ItemResult {
    pub stats: Stats,
    pub weight: f64,
    pub armor_hp: BTreeMap<i32, i32>,
    pub armor_def: BTreeMap<i32, i32>,
}
/// Original ApplyItemSkills with a proven empty equipped-item collection.
/// Caller must establish empty Gear; this is not a fallback for missing item data.
pub fn empty_gear(set_bonus: &Stats) -> ItemResult {
    ItemResult {
        stats: set_bonus.clone(),
        weight: 0.0,
        armor_hp: BTreeMap::new(),
        armor_def: BTreeMap::new(),
    }
}
pub struct FinalResult {
    pub stats: Stats,
    pub armor_def: BTreeMap<i32, i32>,
    pub destroyed_def: i32,
}
fn text(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn arg(cache: &Cache, skill: &str, key: &str) -> i32 {
    cache.argument(&text(skill), &text(key), 0)
}
fn clr_i32(v: f64) -> i32 {
    let v = v.trunc();
    if v.is_finite() && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&v) {
        v as i32
    } else {
        i32::MIN
    }
}
impl Arithmetic {
    fn sum_product(self, base: i32, factor: f32, multiplier: f32) -> i32 {
        clr_i32(match self {
            Self::FrameworkX86 => {
                f64::from(base as f32) + f64::from(factor) * f64::from(multiplier)
            }
            Self::FrameworkX64 => f64::from(base as f32 + factor * multiplier),
        })
    }
    fn product(self, factor: f32, multiplier: f32) -> i32 {
        self.sum_product(0, factor, multiplier)
    }
}
/// Source ApplyStatSkills. Only the known enum fields are admitted; arbitrary
/// CharacterStats string keys and general override dictionaries are outside this kernel.
pub fn apply_skills(
    total: &Stats,
    armor: &BTreeMap<i32, i32>,
    cache: &Cache,
    magic_critical: bool,
    arithmetic: Arithmetic,
) -> Result<FinalResult, StatError> {
    use Field::*;
    if armor.len() > 64 {
        return Err(StatError::Profile);
    }
    let mut s = total.clone();
    for (field, skill, key) in [
        (Str, "skill_str_mastery", "str_max_bonus"),
        (Dex, "skill_dex_mastery", "dex_max_bonus"),
        (Int, "skill_int_mastery", "int_max_bonus"),
        (Will, "skill_will_mastery", "will_max_bonus"),
        (Stamina, "skill_stamina_mastery", "stamina_max_bonus"),
    ] {
        s.set(field, total.get(field).wrapping_add(arg(cache, skill, key)));
    }
    let ratio = |field| 1.0_f32.min(s.get(field) as f32 / 2000.0_f32);
    let (strength, dexterity, intelligence, will) =
        (ratio(Str), ratio(Dex), ratio(Int), ratio(Will));
    s.set(
        Atk,
        arithmetic.sum_product(total.get(Atk), s.get(Str).wrapping_add(180) as f32, 2.7_f32),
    );
    s.set(
        Matk,
        arithmetic
            .sum_product(
                total.get(Matk),
                s.get(Int).wrapping_add(350) as f32,
                2.0_f32,
            )
            .wrapping_add(arg(cache, "skill_evy_magic_mastery", "magic_atk_bonus")),
    );
    s.set(
        Def,
        arithmetic
            .sum_product(total.get(Def), dexterity, 1000.0)
            .wrapping_add(arg(cache, "skill_defence_mastery", "def_bonus")),
    );
    s.set(
        Hp,
        arithmetic
            .sum_product(total.get(Hp), will, 1200.0)
            .wrapping_add(arg(cache, "skill_hp_mastery", "hp_max_bonus")),
    );
    s.set(
        Critical,
        3_i32
            .wrapping_add(arithmetic.product(will, 15.0))
            .wrapping_add(total.get(Critical))
            .wrapping_add(arg(cache, "skill_critical_hit", "frequency_bonus")),
    );
    s.set(
        CritFactor,
        100_i32
            .wrapping_add(arithmetic.product(
                if magic_critical {
                    intelligence
                } else {
                    strength
                },
                30.0,
            ))
            .wrapping_add(arg(cache, "skill_critical_hit", "dmg_bonus"))
            .wrapping_add(total.get(CritFactor)),
    );
    s.set(SkillRankSum, cache.rank_sum());
    let destroyed = arg(cache, "skill_stone_skin", "def_bonus");
    let mut result = BTreeMap::new();
    for (&part, &def) in armor {
        let value = if (4..=8).contains(&part) {
            if def <= destroyed {
                s.set(Def, s.get(Def).wrapping_add(destroyed.wrapping_sub(def)));
                0
            } else {
                def.wrapping_sub(destroyed)
            }
        } else {
            def
        };
        result.insert(part, value);
    }
    Ok(FinalResult {
        stats: s,
        armor_def: result,
        destroyed_def: destroyed,
    })
}
pub trait Callbacks {
    type Error;
    fn int_changed(&mut self, stats: &Stats) -> Result<(), Self::Error>;
    fn stat_updated(&mut self) -> Result<(), Self::Error>;
    fn microplay_updated(&mut self) -> Result<(), Self::Error>;
}
#[derive(Default)]
pub struct FinalState {
    result: Option<FinalResult>,
}
impl FinalState {
    pub fn restore(result: Option<FinalResult>) -> Self {
        Self { result }
    }
    pub fn result(&self) -> Option<&FinalResult> {
        self.result.as_ref()
    }
    /// CalculateFinalStat suffix after ApplyStatSkills: store, override, INT/AP,
    /// StatObservable, MicroPlayObservable. A callback error retains the new state
    /// and prevents later callbacks; no compensation or network push is invented.
    pub fn publish<C: Callbacks>(
        &mut self,
        next: FinalResult,
        overrides: &[(Field, i32)],
        callbacks: &mut C,
    ) -> Result<(), C::Error> {
        let previous_int = self.result.as_ref().map_or(0, |r| r.stats.get(Field::Int));
        let current = self.result.insert(next);
        for &(field, value) in overrides {
            current.stats.set(field, value);
        }
        if previous_int != current.stats.get(Field::Int) {
            callbacks.int_changed(&current.stats)?;
        }
        callbacks.stat_updated()?;
        callbacks.microplay_updated()
    }
}
