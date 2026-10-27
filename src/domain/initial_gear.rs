//! Source-derived empty Gear projection for a fresh, unequipped creation inventory.
//! Equippable item type is not evidence that Equippable.Part is assigned.
use super::{
    character_stats::{ItemResult, Stats, empty_gear},
    equipped_skills::{self, Equipment, EquipmentError},
    initial_inventory::PreparedInventory,
    skill_runtime::Cache,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub struct SlotPart {
    pub slot: u8,
    pub costume_part: Option<i32>,
    pub decoration: bool,
    pub consumable: bool,
}
#[derive(Debug, Error, PartialEq, Eq)]
#[error("new unequipped Gear projection exceeds its admitted source profile")]
pub struct GearError;
/// All other EquippedItemInfo fields have their original empty-slot initializer values:
/// CostumeSN=-1; strings empty; stats/numeric/colors zero; Effect=null; IsAvatar=false.
pub struct EmptyInfo {
    pub part: i32,
    pub costume_part: i32,
    pub decoration: bool,
}
pub struct Projection {
    rows: Vec<EmptyInfo>,
    gear_slots: usize,
    set_bonus: Stats,
    set_skills: BTreeMap<Vec<u8>, i32>,
}
impl Projection {
    /// Metadata order is the original double-dictionary insertion order. Quick slots
    /// are selected by each group's FIRST IsConsumable; costume dedup is per slot.
    /// The caller proves source key uniqueness before supplying these projected rows.
    pub fn from_empty_slots(parts: &[SlotPart]) -> Result<Self, GearError> {
        if parts.len() > 4096 {
            return Err(GearError);
        }
        let mut groups: Vec<(u8, Vec<&SlotPart>)> = Vec::new();
        for row in parts {
            if let Some((_, rows)) = groups.iter_mut().find(|(slot, _)| *slot == row.slot) {
                rows.push(row);
            } else {
                groups.push((row.slot, vec![row]));
            }
        }
        let mut rows = Vec::new();
        let mut gear_slots = 0;
        for (slot, parts) in groups {
            if parts.first().is_some_and(|p| p.consumable) {
                continue;
            }
            gear_slots += 1;
            let mut sent = BTreeSet::new();
            for part in parts {
                if let Some(costume_part) = part.costume_part
                    && sent.insert(costume_part)
                {
                    if rows.len() == 128 {
                        return Err(GearError);
                    }
                    rows.push(EmptyInfo {
                        part: i32::from(slot),
                        costume_part,
                        decoration: part.decoration,
                    });
                }
            }
        }
        Ok(Self {
            rows,
            gear_slots,
            set_bonus: Stats::default(),
            set_skills: BTreeMap::new(),
        })
    }
    /// PreparedInventory is the fresh factory batch whose writer always inserts
    /// Equippable.Part=NULL. This is not a loader or proof about arbitrary stored rows.
    /// Never use collection/type alone to decide whether an existing item is equipped.
    pub fn admit_new_inventory(&self, inventory: &PreparedInventory) -> Result<(), GearError> {
        if inventory.items().len() > 4096
            || inventory.items().iter().any(|r| {
                // AccountStorage (101, -1) holds an unequipped item such as gold.
                let l = r.location();
                !(matches!(l.collection, 0 | 1 | 6 | 100) && l.slot >= 0
                    || l.collection == 101 && l.slot == -1)
            })
        {
            return Err(GearError);
        }
        Ok(())
    }
    pub fn gear_slots(&self) -> usize {
        self.gear_slots
    }
    pub fn rows(&self) -> &[EmptyInfo] {
        &self.rows
    }
    pub fn set_bonus(&self) -> &Stats {
        &self.set_bonus
    }
    pub fn set_skills(&self) -> &BTreeMap<Vec<u8>, i32> {
        &self.set_skills
    }
    /// Unlike an empty IList, the source empty Gear contains placeholders. Their
    /// armor-map entries must survive even though stats and weight are zero.
    pub fn item_stats(&self, cache: &Cache) -> Result<ItemResult, EquipmentError> {
        let mut result = empty_gear(&self.set_bonus);
        self.apply_item_stats(cache, &mut result)?;
        Ok(result)
    }
    /// Source out-parameter API: duplicate failures retain partially accumulated maps.
    pub fn apply_item_stats(
        &self,
        cache: &Cache,
        result: &mut ItemResult,
    ) -> Result<(), EquipmentError> {
        let zero = Stats::default();
        let items: Vec<_> = self
            .rows
            .iter()
            .map(|r| Equipment {
                stats: &zero,
                item_class: "",
                equip_class: "",
                weight: 0.0,
                armor_hp: 0,
                max_durability: 0,
                costume_part: r.costume_part,
                destroyed_def: 0,
                decoration: r.decoration,
            })
            .collect();
        equipped_skills::apply(&self.set_bonus, &items, cache, result, |_| {
            Err(EquipmentError::Category)
        })
    }
}
