//! Normal equipment projection. Avatar replacement/ExtraData overlays are a
//! separate profile: callers must establish their absence before using this kernel.
use super::{
    character_stats::{ItemResult, Stats, empty_gear},
    equipped_skills::{self, Equipment, EquipmentError},
    skill_runtime::Cache,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Clone)]
pub struct Part {
    pub slot: u8,
    pub category: String,
    pub costume: Option<i32>,
    pub decoration: bool,
    pub consumable: bool,
}
#[derive(Clone)]
pub struct Item {
    pub id: i64,
    pub part: u8,
    pub category: String,
    pub class: String,
    pub equip_class: String,
    pub serial: i32,
    pub level: i32,
    pub colors: [i32; 3],
    pub weight: i32,
    pub durability: i32,
    pub maximum: i32,
    pub armor_hp: i32,
    pub stats: Stats,
    pub destroyed_def: i32,
    pub effect: Option<i32>,
    pub disabled: bool,
}
#[derive(Clone)]
pub struct Info {
    pub part: i32,
    pub costume: i32,
    pub decoration: bool,
    pub serial: i32,
    pub colors: [i32; 3],
    pub level: i32,
    pub weight: f64,
    pub durability: i32,
    pub maximum: i32,
    pub armor_hp: i32,
    pub stats: Stats,
    pub destroyed_def: i32,
    pub effect: Option<i32>,
    pub class: String,
    pub equip_class: String,
}
impl Info {
    /// Original ToCostume Double expressions for the explicit Framework x64 profile.
    pub fn costume_x64(&self) -> super::character_costume::Costume {
        let ratio = (f64::from(self.durability) / 10000.0).min(1.0);
        let gloss = (255.0 - ((ratio - 0.2) / 0.39999999999999997).clamp(0.0, 1.0) * 255.0) as u8;
        super::character_costume::Costume {
            is_base: false,
            kind: u8::from(self.decoration),
            key: self.costume as u8,
            serial: Some(self.serial),
            colors: self.colors.map(Some),
            gloss: Some(gloss),
            effect: self.effect,
        }
    }
    fn empty(part: &Part) -> Self {
        Self {
            part: i32::from(part.slot),
            costume: part.costume.unwrap_or(-1),
            decoration: part.decoration,
            serial: -1,
            colors: [0; 3],
            level: 0,
            weight: 0.0,
            durability: 0,
            maximum: 0,
            armor_hp: 0,
            stats: Stats::default(),
            destroyed_def: 0,
            effect: None,
            class: String::new(),
            equip_class: String::new(),
        }
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
#[error("normal equipment projection has ambiguous or unsupported metadata/items")]
pub struct GearError;
pub struct Projection {
    pub infos: Vec<Info>,
    pub equipment: Vec<(i32, i64)>,
}
impl Projection {
    /// LoadItemCostumes changes item rows; base rows remain stored for fallback.
    /// This projects constructor/relogin state, not an existing CostumeInfo cache.
    pub fn load_costumes_x64(
        &self,
        set: &mut super::character_costume::CostumeSet,
    ) -> Result<(), super::character_costume::CostumeProfile> {
        for info in self.infos.iter().filter(|i| i.costume != -1) {
            let costume = info.costume_x64();
            if info.serial == -1 {
                set.remove(costume.identity());
            } else {
                set.set(costume)?;
            }
        }
        Ok(())
    }
    pub fn load(parts: &[Part], items: &[Item]) -> Result<Self, GearError> {
        if parts.len() > 4096 || items.len() > 4096 {
            return Err(GearError);
        }
        let mut groups: Vec<(u8, Vec<&Part>)> = Vec::new();
        let mut keys = BTreeSet::new();
        for p in parts {
            if !p.category.is_ascii()
                || p.category.len() > 256
                || !keys.insert((p.slot, &p.category))
            {
                return Err(GearError);
            }
            if let Some((_, rows)) = groups.iter_mut().find(|(slot, _)| *slot == p.slot) {
                rows.push(p)
            } else {
                groups.push((p.slot, vec![p]))
            }
        }
        let mut by_part = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for item in items {
            if item.part >= 50
                || item.class.contains("avatar_")
                || item.class.len() > 100
                || !item
                    .class
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || !item.equip_class.is_ascii()
                || item.equip_class.len() > 100
                || !ids.insert(item.id)
                || by_part.insert(item.part, item).is_some()
            {
                return Err(GearError);
            }
        }
        let mut infos = Vec::new();
        let mut equipment = Vec::new();
        for (slot, rows) in groups {
            if rows[0].consumable {
                if by_part.contains_key(&slot) {
                    return Err(GearError);
                }
                continue;
            }
            let mut sent = BTreeSet::new();
            if let Some(item) = by_part.remove(&slot) {
                let p = rows
                    .iter()
                    .find(|p| p.category == item.category)
                    .ok_or(GearError)?;
                equipment.push((i32::from(slot), item.id));
                if !item.disabled {
                    infos.push(Info {
                        part: i32::from(slot),
                        costume: p.costume.unwrap_or(-1),
                        decoration: p.decoration,
                        serial: if p.costume.is_some() { item.serial } else { -1 },
                        colors: if p.costume.is_some() {
                            item.colors
                        } else {
                            [-1; 3]
                        },
                        level: item.level,
                        // Original ItemStat.Weight is Int32: division occurs before conversion to Double.
                        weight: f64::from(item.weight / 1000),
                        durability: item.durability,
                        maximum: item.maximum,
                        armor_hp: item.armor_hp,
                        stats: item.stats.clone(),
                        destroyed_def: item.destroyed_def,
                        effect: item.effect,
                        class: item.class.clone(),
                        equip_class: item.equip_class.clone(),
                    });
                    if let Some(c) = p.costume {
                        sent.insert(c);
                    }
                }
            }
            for p in rows {
                if let Some(c) = p.costume
                    && sent.insert(c)
                {
                    infos.push(Info::empty(p))
                }
            }
        }
        if !by_part.is_empty() || infos.len() > 128 {
            return Err(GearError);
        }
        Ok(Self { infos, equipment })
    }
    pub fn item_stats(&self, set: &Stats, cache: &Cache) -> Result<ItemResult, EquipmentError> {
        let items: Vec<_> = self
            .infos
            .iter()
            .map(|i| Equipment {
                stats: &i.stats,
                item_class: &i.class,
                equip_class: &i.equip_class,
                weight: i.weight,
                armor_hp: i.armor_hp,
                max_durability: i.maximum,
                costume_part: i.costume,
                destroyed_def: i.destroyed_def,
                decoration: i.decoration,
            })
            .collect();
        let mut out = empty_gear(set);
        equipped_skills::apply(set, &items, cache, &mut out, |_| {
            Err(EquipmentError::Category)
        })?;
        Ok(out)
    }
}

/// Gear deduplicates each (SetID, ItemClass), not instances or BaseItemClass.
pub struct SetItem<'a> {
    pub class: &'a str,
    pub set_ids: &'a [Option<String>],
    pub disabled: bool,
}
pub struct SetBonus {
    pub stats: Stats,
    pub skills: BTreeMap<String, i32>,
}
pub fn set_bonus(
    items: &[SetItem<'_>],
    mut effects: impl FnMut(&str, usize) -> Result<Vec<(String, i32)>, GearError>,
) -> Result<SetBonus, GearError> {
    if items.len() > 4096 {
        return Err(GearError);
    }
    let mut sets: Vec<(&str, BTreeSet<&str>)> = Vec::new();
    for item in items.iter().filter(|i| !i.disabled) {
        if item.set_ids.len() > 4096 {
            return Err(GearError);
        }
        for set in item.set_ids {
            let set = set.as_deref().ok_or(GearError)?;
            if set.len() > 512 {
                return Err(GearError);
            }
            if let Some((_, classes)) = sets.iter_mut().find(|(s, _)| *s == set) {
                classes.insert(item.class);
            } else {
                sets.push((set, BTreeSet::from([item.class])));
            }
        }
    }
    let mut stats = [0_i32; 25];
    let mut skills = BTreeMap::new();
    let names: Vec<_> = Stats::default().entries().map(|(n, _)| n).collect();
    for (id, classes) in sets {
        let rows = effects(id, classes.len())?;
        if rows.len() > 4096 {
            return Err(GearError);
        }
        let mut keys = BTreeSet::new();
        for (key, value) in rows {
            if key.len() > 512 || !keys.insert(key.clone()) {
                return Err(GearError);
            }
            if [
                "STR", "DEX", "INT", "WILL", "LUCK", "HP", "STAMINA", "ATK", "DEF", "MATK",
            ]
            .contains(&key.as_str())
            {
                let i = names.iter().position(|n| *n == key).ok_or(GearError)?;
                stats[i] = stats[i].wrapping_add(value);
            } else if skills.insert(key, value).is_some() {
                return Err(GearError);
            }
        }
    }
    Ok(SetBonus {
        stats: Stats::from_values(stats),
        skills,
    })
}
