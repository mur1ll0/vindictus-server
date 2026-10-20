//! Readonly equipment metadata subset, bound to the existing catalog/source hashes.
use super::{
    item_catalog::{self, Package as Catalog},
    minimum_character::MinimumError,
    user_export::{PrivateText, nullable},
};
use crate::domain::equipped_gear::{self, SetBonus, SetItem};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Class {
    source_index: usize,
    item_utf16: PrivateText,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    category_utf16: PrivateText,
    level: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Equip {
    source_index: usize,
    item_utf16: PrivateText,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    equip_utf16: PrivateText,
    serial: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    item_utf16: PrivateText,
    set_utf16: PrivateText,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Effect {
    set_utf16: PrivateText,
    count: i32,
    target_utf16: PrivateText,
    amount: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    implementations: BTreeMap<String, String>,
    classes: Vec<Class>,
    equips: Vec<Equip>,
    sets: Vec<Set>,
    effects: Vec<Effect>,
}
pub struct Metadata {
    pub category: String,
    pub level: i32,
    pub equip_class: String,
    pub serial: i32,
    pub sets: Vec<Option<String>>,
}
fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn raw(text: &PrivateText) -> Result<Vec<u8>, MinimumError> {
    let key = item_catalog::opaque(Some(PrivateText(text.0.clone())))?.ok_or_else(fail)?;
    Ok(key.as_bytes().to_vec())
}
fn text(value: &PrivateText) -> Result<String, MinimumError> {
    let bytes = raw(value)?;
    String::from_utf16(
        &bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}
impl Profile {
    pub fn read(input: impl Read) -> Result<Self, MinimumError> {
        let mut bytes = Zeroizing::new(Vec::new());
        input
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(fail());
        }
        let p: Self = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        let sources: serde_json::Value =
            serde_json::from_str(include_str!("../domain/equipped_gear_sources.json"))
                .map_err(|_| fail())?;
        let expected: BTreeMap<_, _> = sources
            .as_object()
            .ok_or_else(fail)?
            .iter()
            .map(|(k, v)| (k.clone(), v["sha256"].as_str().unwrap_or("").to_owned()))
            .collect();
        if p.format_version != 1
            || p.source != "heroesContents.normal-equipment"
            || p.implementations != expected
            || p.classes.is_empty()
            || p.classes.len() > 50000
            || p.equips.is_empty()
            || p.equips.len() > 50000
            || p.sets.len() > 50000
            || p.effects.len() > 50000
        {
            return Err(fail());
        }
        // Validate representations before the profile can be attached. Source feature
        // selection and exact-count effect lookup remain lazy, like the original.
        for c in &p.classes {
            raw(&c.item_utf16)?;
            text(&c.category_utf16)?;
        }
        for e in &p.equips {
            raw(&e.item_utf16)?;
            text(&e.equip_utf16)?;
        }
        for s in &p.sets {
            raw(&s.item_utf16)?;
            text(&s.set_utf16)?;
        }
        for e in &p.effects {
            text(&e.set_utf16)?;
            text(&e.target_utf16)?;
        }
        Ok(p)
    }
    pub fn bind(&self, catalog: &Catalog) -> Result<(), MinimumError> {
        if self.database_sha256 != catalog.manifest.database_sha256
            || self.config_sha256 != catalog.manifest.config_sha256
        {
            return Err(fail());
        }
        if self
            .classes
            .windows(2)
            .any(|r| r[0].source_index >= r[1].source_index)
            || self
                .equips
                .windows(2)
                .any(|r| r[0].source_index >= r[1].source_index)
        {
            return Err(fail());
        }
        let mut keys = BTreeSet::new();
        for c in &self.classes {
            let source = catalog.classes.get(c.source_index).ok_or_else(fail)?;
            if !keys.insert(c.source_index)
                || source.item_class.as_ref().map(|k| k.as_bytes())
                    != Some(raw(&c.item_utf16)?.as_slice())
                || source.feature != c.feature
            {
                return Err(fail());
            }
        }
        keys.clear();
        for e in &self.equips {
            let source = catalog.equips.get(e.source_index).ok_or_else(fail)?;
            if !keys.insert(e.source_index)
                || source.item_class.as_ref().map(|k| k.as_bytes())
                    != Some(raw(&e.item_utf16)?.as_slice())
                || source.feature != e.feature
            {
                return Err(fail());
            }
        }
        Ok(())
    }
    pub fn metadata(&self, catalog: &Catalog, key: &[u8]) -> Result<Metadata, MinimumError> {
        self.bind(catalog)?;
        let effective = crate::application::item_catalog_inspection::catalog(catalog)?;
        let item = effective.get(Some(key)).ok_or_else(fail)?;
        if !item.binding.equippable {
            return Err(fail());
        }
        let c = self
            .classes
            .iter()
            .find(|c| c.source_index == item.source_index)
            .ok_or_else(fail)?;
        let mut selected = None;
        for e in &self.equips {
            if raw(&e.item_utf16)? == key && catalog.policy.is_enabled(e.feature.as_deref())? {
                selected = Some(e)
            }
        }
        let e = selected.ok_or_else(fail)?;
        let sets = self
            .sets
            .iter()
            .filter_map(|s| match raw(&s.item_utf16) {
                Ok(k) if k == key => Some(text(&s.set_utf16).map(Some)),
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Metadata {
            category: text(&c.category_utf16)?,
            level: c.level,
            equip_class: text(&e.equip_utf16)?,
            serial: e.serial,
            sets,
        })
    }
    pub fn bonus(
        &self,
        items: &[equipped_gear::Item],
        metadata: &[Metadata],
    ) -> Result<SetBonus, MinimumError> {
        if items.len() != metadata.len() {
            return Err(fail());
        }
        let set_items: Vec<_> = items
            .iter()
            .zip(metadata)
            .map(|(i, m)| SetItem {
                class: &i.class,
                set_ids: &m.sets,
                disabled: i.disabled,
            })
            .collect();
        equipped_gear::set_bonus(&set_items, |id, count| {
            let mut rows = Vec::new();
            for e in &self.effects {
                if text(&e.set_utf16).map_err(|_| equipped_gear::GearError)? == id
                    && i32::try_from(count).ok() == Some(e.count)
                {
                    rows.push((
                        text(&e.target_utf16).map_err(|_| equipped_gear::GearError)?,
                        e.amount,
                    ));
                }
            }
            Ok(rows)
        })
        .map_err(|_| fail())
    }
}
