//! Equipment projection: Gear.ToEquippedItemInfos with the avatar overlay. An avatar piece sits
//! at its normal part + 50 (GetAvatarPart, FeatureMatrix Avatar) and its ItemClassInfo.ExtraData
//! names what it shows: a CostumeKey/DecorationKey hides that costume part, an item class puts
//! that class's CostumeType on the costume part of its category, with the avatar's colors and
//! IsAvatar (Gloss 0). Avatar hiding (AvatarHideInfo) and AvatarSlotBonus are not migrated.
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
    /// What an avatar piece (part 51-99) shows, from its ExtraData; empty for the others.
    pub looks: Vec<AvatarLook>,
}
/// One ExtraData entry of an avatar piece, resolved by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AvatarLook {
    /// CostumeKey or DecorationKey name: the costume part shows nothing (CostumeSN -1).
    Hide { costume: i32, decoration: bool },
    /// An item class: its EquipItemInfo.CostumeType on the costume part of its category.
    Show {
        costume: i32,
        decoration: bool,
        serial: i32,
    },
}
/// Gear.GetAvatarPart: the avatar part of a normal part 1-49.
pub const AVATAR_OFFSET: u8 = 50;
/// CostumeManager.CostumeKeyDict (PlayerService CostumeKey) by name, ignoring case.
pub fn costume_key(name: &str) -> Option<i32> {
    const KEYS: [&str; 10] = [
        "Base",
        "InnerArmor",
        "Face",
        "Hair",
        "UpperBody",
        "LowerBody",
        "Head",
        "Foot",
        "Hand",
        "Motion",
    ];
    KEYS.iter()
        .position(|k| k.eq_ignore_ascii_case(name))
        .and_then(|i| i32::try_from(i).ok())
}
/// CostumeManager.DecorationKeyDict (PlayerService DecorationKey) by name, ignoring case.
pub fn decoration_key(name: &str) -> Option<i32> {
    const KEYS: [&str; 7] = [
        "EyeBrows",
        "MakeUp",
        "FacePainting",
        "Scar",
        "Beard",
        "Piercing",
        "BodyPainting",
    ];
    KEYS.iter()
        .position(|k| k.eq_ignore_ascii_case(name))
        .and_then(|i| i32::try_from(i).ok())
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
    /// EquippedItemInfo.IsAvatar: an avatar piece's look is on this costume part.
    pub is_avatar: bool,
    /// ItemClassInfo.Category of the item (empty for an empty slot): ApplyItemSkills' lazy
    /// lookup for avatar_ classes.
    pub category: String,
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
            // CostumeDataExtension.ToCostume: Gloss = IsAvatar ? 0 : durability gloss.
            gloss: Some(if self.is_avatar { 0 } else { gloss }),
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
            is_avatar: false,
            category: String::new(),
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
            if item.part >= 100
                || (item.part <= AVATAR_OFFSET && !item.looks.is_empty())
                || item.looks.len() > 64
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
                        is_avatar: false,
                        category: item.category.clone(),
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
        apply_avatars(&mut infos, items);
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
        equipped_skills::apply(set, &items, cache, &mut out, |class| {
            // SkillEffect.ApplyItemSkills: ItemClassInfo.Category of an avatar_ class.
            self.infos
                .iter()
                .find(|i| i.class == class)
                .map(|i| i.category.clone())
                .ok_or(EquipmentError::Category)
        })?;
        Ok(out)
    }
}

/// Gear.ToEquippedItemInfos: the avatar pieces in normal part order, later ones winning a costume
/// part (Dictionary assignment), overlay the infos of their costume parts.
fn apply_avatars(infos: &mut [Info], items: &[Item]) {
    let mut avatars: Vec<&Item> = items
        .iter()
        .filter(|i| i.part > AVATAR_OFFSET && !i.disabled && !i.looks.is_empty())
        .collect();
    avatars.sort_by_key(|i| i.part);
    let mut shown: BTreeMap<(bool, i32), (Option<i32>, [i32; 3])> = BTreeMap::new();
    for item in avatars {
        for look in &item.looks {
            let (key, serial) = match *look {
                AvatarLook::Hide {
                    costume,
                    decoration,
                } => ((decoration, costume), None),
                AvatarLook::Show {
                    costume,
                    decoration,
                    serial,
                } => ((decoration, costume), Some(serial)),
            };
            shown.insert(key, (serial, item.colors));
        }
    }
    for info in infos.iter_mut() {
        if let Some((serial, colors)) = shown.get(&(info.decoration, info.costume)) {
            info.serial = serial.unwrap_or(-1);
            info.colors = *colors;
            info.is_avatar = true;
            if !info.decoration && serial.is_some() && info.armor_hp == 0 {
                info.armor_hp = -1;
            }
        }
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
