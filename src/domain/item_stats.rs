//! Recovered ItemStat arithmetic and base EquipItemInfo projection; no item delivery.
use thiserror::Error;
use zeroize::Zeroizing;

pub const EQUIPMENT_STATS: [&str; 18] = [
    "STR",
    "DEX",
    "INT",
    "LUCK",
    "WILL",
    "HP",
    "STAMINA",
    "ATK",
    "DEF",
    "MATK",
    "ATK_Speed",
    "Critical",
    "Balance",
    "Res_Critical",
    "PVP_ATK",
    "PVP_MATK",
    "PVP_DEF",
    "TOWN_SPEED",
];
const SPECIAL: [&[u16]; 4] = [
    &[77, 97, 120, 68, 117, 114, 97, 98, 105, 108, 105, 116, 121],
    &[87, 101, 105, 103, 104, 116],
    &[68, 69, 70, 95, 68, 101, 115, 116, 114, 111, 121, 101, 100],
    &[65, 114, 109, 111, 114, 72, 80],
];
const MAX_ENTRIES: usize = 256;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StatError {
    #[error("item stats exceed verified entry/key bounds")]
    Bound,
    #[error("item stats contain a duplicate dictionary key")]
    Duplicate,
    #[error("item stat dictionary key is null")]
    NullKey,
}
#[derive(Clone, Default, PartialEq, Eq)]
struct Entries(Vec<(Zeroizing<Vec<u16>>, i32)>);
impl Entries {
    fn validate(key: &[u16]) -> Result<(), StatError> {
        if key.len() > 256 {
            return Err(StatError::Bound);
        }
        Ok(())
    }
    fn get(&self, key: &[u16]) -> Option<i32> {
        self.0
            .iter()
            .find(|(k, _)| k.as_slice() == key)
            .map(|(_, v)| *v)
    }
    fn put(&mut self, key: &[u16], value: i32) -> Result<(), StatError> {
        Self::validate(key)?;
        if let Some((_, v)) = self.0.iter_mut().find(|(k, _)| k.as_slice() == key) {
            *v = value;
        } else {
            if self.0.len() == MAX_ENTRIES {
                return Err(StatError::Bound);
            }
            self.0.push((Zeroizing::new(key.to_vec()), value));
        }
        Ok(())
    }
    fn raw(rows: &[(Vec<u16>, i32)]) -> Result<Self, StatError> {
        let mut result = Self::default();
        for (key, value) in rows {
            if result.get(key).is_some() {
                return Err(StatError::Duplicate);
            }
            result.put(key, *value)?;
        }
        Ok(result)
    }
}
/// No Debug/Serialize: content values stay private. Insertion order is retained.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ItemStat {
    character: Entries,
    effects: Entries,
    // MaxDurability, Weight, DEF_Destroyed, ArmorHP, in source ToDictionary order.
    fields: [i32; 4],
}
pub struct StatEntry<'a> {
    pub name: &'a [u16],
    pub value: i32,
}
impl ItemStat {
    /// Raw source dictionaries, including names colliding with special fields.
    pub fn from_raw(
        character: &[(Vec<u16>, i32)],
        effects: &[(Vec<u16>, i32)],
        fields: [i32; 4],
    ) -> Result<Self, StatError> {
        Ok(Self {
            character: Entries::raw(character)?,
            effects: Entries::raw(effects)?,
            fields,
        })
    }
    pub fn fields(&self) -> [i32; 4] {
        self.fields
    }
    pub fn character_entries(&self) -> impl Iterator<Item = StatEntry<'_>> {
        self.character
            .0
            .iter()
            .map(|(k, v)| StatEntry { name: k, value: *v })
    }
    pub fn status_entries(&self) -> impl Iterator<Item = StatEntry<'_>> {
        self.effects
            .0
            .iter()
            .map(|(k, v)| StatEntry { name: k, value: *v })
    }
    pub fn get(&self, name: Option<&[u16]>) -> i32 {
        let Some(name) = name else {
            return 0;
        };
        SPECIAL
            .iter()
            .position(|s| *s == name)
            .map_or_else(|| self.character.get(name).unwrap_or(0), |i| self.fields[i])
    }
    pub fn set(&mut self, name: Option<&[u16]>, value: i32) -> Result<(), StatError> {
        let name = name.ok_or(StatError::NullKey)?;
        if let Some(i) = SPECIAL.iter().position(|s| *s == name) {
            self.fields[i] = value;
            Ok(())
        } else {
            self.character.put(name, value)
        }
    }
    pub fn add_status(&mut self, name: Option<&[u16]>, value: i32) -> Result<(), StatError> {
        let name = name.ok_or(StatError::NullKey)?;
        self.effects.put(
            name,
            self.effects.get(name).map_or(value, |old| old.max(value)),
        )
    }
    /// Resource bounds are checked on a temporary result; no partially applied addition.
    pub fn add(&mut self, rhs: Option<&Self>) -> Result<(), StatError> {
        let Some(rhs) = rhs else {
            return Ok(());
        };
        let mut next = self.clone();
        for entry in rhs.character_entries() {
            next.character.put(
                entry.name,
                next.character
                    .get(entry.name)
                    .unwrap_or(0)
                    .wrapping_add(entry.value),
            )?;
        }
        for entry in rhs.status_entries() {
            next.add_status(Some(entry.name), entry.value)?;
        }
        for (value, delta) in next.fields.iter_mut().zip(rhs.fields) {
            *value = value.wrapping_add(delta);
        }
        *self = next;
        Ok(())
    }
    pub fn copy_from(&mut self, rhs: &Self) {
        *self = rhs.clone();
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    /// Only rhs.CharacterStats keys are zeroed/inserted; fields/effects stay unchanged.
    pub fn replace_stat_to_zero(&mut self, rhs: Option<&Self>) -> Result<(), StatError> {
        let Some(rhs) = rhs else {
            return Ok(());
        };
        let mut next = self.clone();
        for entry in rhs.character_entries() {
            next.character.put(entry.name, 0)?;
        }
        *self = next;
        Ok(())
    }
    /// The source Add of special field names throws on a raw character-key collision.
    pub fn dictionary(&self) -> Result<Vec<StatEntry<'_>>, StatError> {
        if SPECIAL.iter().any(|key| self.character.get(key).is_some()) {
            return Err(StatError::Duplicate);
        }
        let mut entries = self.character_entries().collect::<Vec<_>>();
        entries.extend(
            SPECIAL
                .iter()
                .zip(self.fields)
                .map(|(name, value)| StatEntry { name, value }),
        );
        Ok(entries)
    }
    /// Source cast precedes multiplication. Out-of-range/NaN cast behavior is pinned
    /// to the measured Framework4.8 x86/x64 conversion; Rust's saturating cast differs.
    pub fn from_equipment(
        values: [i32; 18],
        max_durability: i32,
        weight: f64,
        armor_hp: i32,
    ) -> Result<Self, StatError> {
        let mut result = Self::default();
        for (name, value) in EQUIPMENT_STATS.iter().zip(values) {
            result.set(Some(&name.encode_utf16().collect::<Vec<_>>()), value)?;
        }
        let trunc = weight.trunc();
        let integer =
            if trunc.is_finite() && trunc >= f64::from(i32::MIN) && trunc <= f64::from(i32::MAX) {
                trunc as i32
            } else {
                i32::MIN
            };
        result.fields = [max_durability, integer.wrapping_mul(1000), 0, armor_hp];
        Ok(result)
    }
}
