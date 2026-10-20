//! CalculateItemStat for an explicitly known empty equipped-item collection.
//! General equipped effects and native actor/SQL integration remain outside this kernel.
use std::collections::BTreeMap;
use thiserror::Error;

use super::{
    character_stats::{Arithmetic, ItemResult, Stats, empty_gear},
    skill_runtime::Cache,
};

#[derive(Debug, Error, PartialEq, Eq)]
#[error("base load is outside the admitted finite Double profile")]
pub struct LoadError;

#[derive(Clone, Copy)]
pub struct BaseLoad(f64);
impl BaseLoad {
    pub fn from_bits(bits: u64) -> Result<Self, LoadError> {
        let value = f64::from_bits(bits);
        if !value.is_finite() {
            return Err(LoadError);
        }
        Ok(Self(value))
    }
    pub fn value(self) -> f64 {
        self.0
    }
}

/// CalculateItemStat load portion after ApplyItemSkills produced the equipped
/// outputs. Nonempty x87 arithmetic is not inferred from the empty-Gear oracle.
/// The current native laboratory uses Framework x64. A base-provider failure is
/// returned to the transaction coordinator before publishing any character state.
pub fn equipped_x64(
    items: ItemResult,
    set_bonus: Stats,
    cache: &Cache,
    base: BaseLoad,
    arithmetic: Arithmetic,
) -> Result<Snapshot, LoadError> {
    if !matches!(arithmetic, Arithmetic::FrameworkX64) || !items.weight.is_finite() {
        return Err(LoadError);
    }
    let text = |s: &str| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    let bonus = cache.argument(
        &text("skill_weight_mastery"),
        &text("max_weight_increament_bonus"),
        0,
    );
    let medium = base.value() + f64::from(bonus);
    let heavy = medium * 1.2;
    let ratio = (items.weight - medium) * 100.0 / (heavy - medium);
    let penalty = if ratio.is_nan() {
        i32::MIN
    } else {
        ratio.clamp(0.0, 100.0) as i32
    };
    Ok(Snapshot {
        set_bonus,
        equipped_empty: false,
        item_stats: Some(items.stats),
        weight: items.weight,
        armor_hp: Some(items.armor_hp),
        armor_def: Some(items.armor_def),
        medium,
        heavy,
        penalty,
    })
}
/// The caller has established a non-null, empty Gear collection. A failed/missing
/// loader is not evidence of this state.
pub struct EmptyGear;

#[derive(Clone)]
pub struct Snapshot {
    pub set_bonus: Stats,
    pub equipped_empty: bool,
    pub item_stats: Option<Stats>,
    pub weight: f64,
    pub armor_hp: Option<BTreeMap<i32, i32>>,
    pub armor_def: Option<BTreeMap<i32, i32>>,
    pub medium: f64,
    pub heavy: f64,
    pub penalty: i32,
}
pub struct State {
    snapshot: Snapshot,
}
impl State {
    pub fn new(set_bonus: Stats) -> Self {
        Self {
            snapshot: Snapshot {
                set_bonus,
                equipped_empty: false,
                item_stats: None,
                weight: 0.0,
                armor_hp: None,
                armor_def: None,
                medium: 0.0,
                heavy: 0.0,
                penalty: 0,
            },
        }
    }
    pub fn restore(snapshot: Snapshot) -> Self {
        Self { snapshot }
    }
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    /// SetBonus and equipped collection are replaced before the equipped-collection-null guard.
    /// Item outputs replace their fields before querying medium load. A failed query
    /// retains those mutations and the previous medium/heavy/penalty fields.
    pub fn recalculate<E>(
        &mut self,
        set_bonus: Option<Stats>,
        equipped: Option<EmptyGear>,
        cache: &Cache,
        arithmetic: Arithmetic,
        base: impl FnOnce() -> Result<BaseLoad, E>,
    ) -> Result<(), E> {
        let s = &mut self.snapshot;
        if let Some(set_bonus) = set_bonus {
            s.set_bonus = set_bonus;
        }
        if equipped.is_some() {
            s.equipped_empty = true;
        }
        if !s.equipped_empty {
            return Ok(());
        }
        let items = empty_gear(&s.set_bonus);
        s.item_stats = Some(items.stats);
        s.weight = items.weight;
        s.armor_hp = Some(items.armor_hp);
        s.armor_def = Some(items.armor_def);
        let key = |v: &str| {
            v.encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        };
        let bonus = cache.argument(
            &key("skill_weight_mastery"),
            &key("max_weight_increament_bonus"),
            0,
        );
        s.medium = base()?.value() + f64::from(bonus);
        // Exact literal returned by original CharacterContents.GetHeavyLoadMultiplier.
        s.heavy = s.medium * 1.2_f64;
        // Empty Gear gives weight=0. x86 evaluates the numerator in the extended
        // exponent range before dividing by an infinite stored HeavyLoad. x64
        // instead overflows both numerator/denominator; measured result is NaN.
        let ratio = if matches!(arithmetic, Arithmetic::FrameworkX86) && s.heavy.is_infinite() {
            0.0
        } else {
            (s.weight - s.medium) * 100.0 / (s.heavy - s.medium)
        };
        // .NET Math.Min/Max propagate NaN; Rust min/max and casts do not.
        s.penalty = if ratio.is_nan() {
            i32::MIN
        } else {
            ratio.clamp(0.0, 100.0) as i32
        };
        Ok(())
    }
}
