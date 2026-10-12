//! Effective catalog projection for existence, storage and binding; not full ItemContents.Load.
use super::{feature_policy::FeaturePolicy, item_binding::BindingInfo};
use std::collections::BTreeMap;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("item catalog exceeds verified representation bounds")]
    Bound,
    #[error("enabled catalog row has a null dictionary key")]
    NullKey,
    #[error(transparent)]
    Feature(Box<crate::error::Error>),
}
impl From<crate::error::Error> for CatalogError {
    fn from(error: crate::error::Error) -> Self {
        Self::Feature(Box::new(error))
    }
}
#[derive(Clone, Copy)]
pub struct EquipRow<'a> {
    pub item_class: Option<&'a [u8]>,
    pub feature: Option<&'a str>,
}
#[derive(Clone, Copy)]
pub struct ClassRow<'a> {
    pub item_class: Option<&'a [u8]>,
    pub feature: Option<&'a str>,
    pub max_stack: i32,
    pub storage: u8,
    pub bind: Option<&'a str>,
    pub trade_on_bind: Option<&'a str>,
}
#[derive(Clone, Copy)]
pub struct ExtraRow<'a> {
    pub item_class: Option<&'a [u8]>,
    pub feature: Option<&'a str>,
}
#[derive(Clone, Copy)]
pub struct EffectiveItem<'a> {
    pub binding: BindingInfo<'a>,
    pub max_stack: i32,
    pub storage: u8,
    pub source_index: usize,
}
#[derive(Default, serde::Serialize)]
pub struct CatalogCounts {
    pub equip_rows: usize,
    pub equip_enabled: usize,
    pub equip_unique: usize,
    pub class_rows: usize,
    pub class_disabled: usize,
    pub class_invalid: usize,
    pub class_accepted: usize,
    pub class_unique: usize,
    pub class_overwrites: usize,
    pub extra_rows: usize,
    pub extra_enabled: usize,
    pub extra_unique: usize,
    pub storage_remapped: usize,
}
pub struct ItemCatalog<'a> {
    equips: BTreeMap<&'a [u8], usize>,
    items: BTreeMap<&'a [u8], EffectiveItem<'a>>,
    extras: BTreeMap<&'a [u8], usize>,
    pub counts: CatalogCounts,
}
fn text(value: Option<&[u8]>) -> Result<(), CatalogError> {
    if value.is_some_and(|v| v.len() > 1024 || !v.len().is_multiple_of(2)) {
        return Err(CatalogError::Bound);
    }
    Ok(())
}
fn feature(value: Option<&str>) -> Result<(), CatalogError> {
    if value.is_some_and(|v| v.len() > 512) {
        return Err(CatalogError::Bound);
    }
    Ok(())
}
impl<'a> ItemCatalog<'a> {
    pub fn load_with_policy(
        equips: &[EquipRow<'a>],
        classes: &[ClassRow<'a>],
        extras: &[ExtraRow<'a>],
        policy: &FeaturePolicy,
    ) -> Result<Self, CatalogError> {
        Self::load(equips, classes, extras, &mut |expression| {
            Ok(policy.is_enabled(expression)?)
        })
    }
    /// Slice order is the provider's observed order, never sorted before last-wins selection.
    pub fn load(
        equip_rows: &[EquipRow<'a>],
        class_rows: &[ClassRow<'a>],
        extra_rows: &[ExtraRow<'a>],
        enabled: &mut impl FnMut(Option<&str>) -> Result<bool, CatalogError>,
    ) -> Result<Self, CatalogError> {
        if equip_rows.len() > 50000 || class_rows.len() > 50000 || extra_rows.len() > 50000 {
            return Err(CatalogError::Bound);
        }
        for row in equip_rows {
            text(row.item_class)?;
            feature(row.feature)?;
        }
        for row in class_rows {
            text(row.item_class)?;
            feature(row.feature)?;
            feature(row.bind)?;
            feature(row.trade_on_bind)?;
        }
        for row in extra_rows {
            text(row.item_class)?;
            feature(row.feature)?;
        }
        let mut result = Self {
            equips: BTreeMap::new(),
            items: BTreeMap::new(),
            extras: BTreeMap::new(),
            counts: CatalogCounts {
                equip_rows: equip_rows.len(),
                class_rows: class_rows.len(),
                extra_rows: extra_rows.len(),
                ..Default::default()
            },
        };
        for (i, row) in equip_rows.iter().enumerate() {
            if enabled(row.feature)? {
                result
                    .equips
                    .insert(row.item_class.ok_or(CatalogError::NullKey)?, i);
                result.counts.equip_enabled += 1;
            }
        }
        result.counts.equip_unique = result.equips.len();
        // GetItemStat is a separate unported startup dependency; this is a named projection.
        for (i, row) in class_rows.iter().enumerate() {
            if !enabled(row.feature)? {
                result.counts.class_disabled += 1;
                continue;
            }
            if row.max_stack <= 0 {
                result.counts.class_invalid += 1;
                continue;
            }
            let key = row.item_class.ok_or(CatalogError::NullKey)?;
            let equippable = result.equips.contains_key(key);
            if equippable && row.max_stack != 1 {
                result.counts.class_invalid += 1;
                continue;
            }
            let item = EffectiveItem {
                binding: BindingInfo {
                    item_class: key,
                    equippable,
                    bind: row.bind,
                    trade_on_bind: row.trade_on_bind,
                },
                max_stack: row.max_stack,
                storage: row.storage,
                source_index: i,
            };
            if result.items.insert(key, item).is_some() {
                result.counts.class_overwrites += 1;
            }
            result.counts.class_accepted += 1;
        }
        for (i, row) in extra_rows.iter().enumerate() {
            if enabled(row.feature)? {
                result
                    .extras
                    .insert(row.item_class.ok_or(CatalogError::NullKey)?, i);
                result.counts.extra_enabled += 1;
            }
        }
        let story = enabled(Some("StoryTabInInventory"))?;
        for item in result.items.values_mut() {
            if !story && item.storage == 5 {
                item.storage = 1;
                result.counts.storage_remapped += 1;
            }
        }
        result.counts.class_unique = result.items.len();
        result.counts.extra_unique = result.extras.len();
        Ok(result)
    }
    /// NULL lookup is absent, matching GetItemClassInfo before dictionary access.
    pub fn get(&self, key: Option<&[u8]>) -> Option<&EffectiveItem<'a>> {
        key.and_then(|k| self.items.get(k))
    }
    pub fn equipment(&self) -> impl Iterator<Item = (&'a [u8], usize)> + '_ {
        self.equips.iter().map(|(k, i)| (*k, *i))
    }
    pub fn items(&self) -> impl Iterator<Item = (&'a [u8], &EffectiveItem<'a>)> + '_ {
        self.items.iter().map(|(k, i)| (*k, i))
    }
    pub fn extras(&self) -> impl Iterator<Item = (&'a [u8], usize)> + '_ {
        self.extras.iter().map(|(k, i)| (*k, *i))
    }
}
