//! Source-bound random item contents exported by scripts/content/export_random_item.py:
//! RandomItemContents (RandomItem by ItemClass, the feature-enabled RandomItemRecipe rows
//! by RandomItemID and RecipeClassName, and the RandomItemRecipeClass rows they keep),
//! RandomItemContentsV2 (RandomItemV2 by ItemClass and RandomItemData by RandomItemID),
//! ItemClassInfo.Level and the features they read.
use super::{
    item_catalog::Package as Catalog,
    minimum_character::MinimumError,
    user_export::{PrivateText, nullable},
};
use crate::domain::{
    feature_policy::FeaturePolicy,
    random_item::{RandomBox, Recipe, RecipeClass, V2Data, V2Recipe},
};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoxRow {
    id: i32,
    server_code: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    key_utf16: Option<PrivateText>,
    visual: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassRow {
    id: i32,
    #[serde(deserialize_with = "nullable")]
    name_utf16: Option<PrivateText>,
    expected: Option<i32>,
    probability: String,
    number: i32,
    #[serde(deserialize_with = "nullable")]
    rule: Option<PrivateText>,
}
/// RandomItemID, RecipeClassName, CharacterType, ItemClass, Number, Probability, Color1-3,
/// NotifyMessage, IsVariable, HaveItem, DontHaveItem, Feature.
#[derive(Deserialize)]
struct RecipeRow(
    i32,
    Option<PrivateText>,
    Option<i32>,
    Option<PrivateText>,
    i32,
    String,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<PrivateText>,
    i32,
    String,
    String,
    Option<String>,
);
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct V2Row {
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    key_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    random_item_utf16: Option<PrivateText>,
    show_pop_up: bool,
    titicore: bool,
    cool_time: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    exclusive_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
/// RandomItemID, ItemClass, Count, RecipeType, Probability, RareItemID, Cond_Character,
/// Cond_Item, Cond_Feature, Color1-3, CharacterBind, ExpireBySec, Announce, Message.
#[derive(Deserialize)]
struct V2RecipeRow(
    Option<PrivateText>,
    Option<PrivateText>,
    i32,
    u8,
    String,
    Option<i32>,
    Option<PrivateText>,
    Option<PrivateText>,
    Option<String>,
    i32,
    i32,
    i32,
    bool,
    Option<i32>,
    Option<PrivateText>,
    Option<PrivateText>,
);

/// A RandomItemV2 row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V2Box {
    pub key: Option<String>,
    pub random_item: String,
    pub show_pop_up: bool,
    pub titicore: bool,
    pub cool_time: Option<i32>,
    pub exclusive: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feature {
    key: String,
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    levels: Vec<i32>,
    boxes: Vec<BoxRow>,
    classes: Vec<ClassRow>,
    recipes: Vec<RecipeRow>,
    v2_boxes: Vec<V2Row>,
    v2_recipes: Vec<V2RecipeRow>,
    features: Vec<Feature>,
}

/// RandomItemContents bound to the catalog.
pub struct Contents {
    boxes: BTreeMap<String, Vec<(RandomBox, Option<String>)>>,
    classes: BTreeMap<i32, Vec<RecipeClass>>,
    recipes: BTreeMap<(i32, String), Vec<Recipe>>,
    v2: BTreeMap<String, Vec<V2Box>>,
    v2_data: BTreeMap<String, V2Data>,
    levels: BTreeMap<Vec<u8>, i32>,
    policy: FeaturePolicy,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}
/// UTF-16LE hex up to 4096 bytes (RandomItemRecipe.ItemClass is nvarchar(1024)).
fn text(value: Option<&PrivateText>) -> Result<Option<String>, MinimumError> {
    value
        .map(|v| {
            let hex = v.0.as_bytes();
            if hex.len() > 8192
                || !hex.len().is_multiple_of(4)
                || !hex.iter().all(u8::is_ascii_hexdigit)
            {
                return Err(fail());
            }
            let units = hex
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| {
                    let digits = std::str::from_utf8(c).map_err(|_| fail())?;
                    let [lo, hi] = u16::from_str_radix(digits, 16)
                        .map_err(|_| fail())?
                        .to_be_bytes();
                    Ok(u16::from_le_bytes([lo, hi]))
                })
                .collect::<Result<Vec<_>, MinimumError>>()?;
            String::from_utf16(&units).map_err(|_| fail())
        })
        .transpose()
}
/// A dictionary key: a null key throws in ToListDictionary/ToDoubleListDictionary.
fn key(value: Option<&PrivateText>) -> Result<String, MinimumError> {
    text(value)?.ok_or_else(fail)
}
fn double(bits: &str) -> Result<f64, MinimumError> {
    if bits.len() != 16 {
        return Err(fail());
    }
    Ok(f64::from_bits(
        u64::from_str_radix(bits, 16).map_err(|_| fail())?,
    ))
}

impl Profile {
    pub fn read(input: impl Read) -> Result<Self, MinimumError> {
        let mut bytes = Zeroizing::new(Vec::new());
        input
            .take(48 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 48 * 1024 * 1024 {
            return Err(fail());
        }
        let p: Self = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        if p.format_version != 2
            || p.source != "heroesContents.random-item"
            || p.boxes.len() > 65536
            || p.classes.len() > 65536
            || p.recipes.len() > 262_144
            || p.v2_boxes.len() > 65536
            || p.v2_recipes.len() > 262_144
            || p.features.len() > 512
        {
            return Err(fail());
        }
        Ok(p)
    }

    /// Binds to the catalog snapshot and evaluates the row features.
    pub fn bind(&self, catalog: &Catalog) -> Result<Contents, MinimumError> {
        if self.database_sha256 != catalog.manifest.database_sha256
            || self.config_sha256 != catalog.manifest.config_sha256
            || self.levels.len() != catalog.classes.len()
        {
            return Err(fail());
        }
        let mut rows: BTreeMap<String, Option<String>> = catalog
            .policy
            .entries()
            .map(|(k, v)| (k.to_owned(), Some(v.to_owned())))
            .collect();
        for f in &self.features {
            let value = f.value.clone().unwrap_or_default();
            match rows.get(&f.key) {
                Some(existing) if existing.as_deref().unwrap_or("") != value => {
                    return Err(fail());
                }
                Some(_) => {}
                None => {
                    rows.insert(f.key.clone(), Some(value));
                }
            }
        }
        let policy = FeaturePolicy::npc_source(rows).map_err(|_| fail())?;
        let effective = crate::application::item_catalog_inspection::catalog(catalog)?;
        let mut levels = BTreeMap::new();
        for (class, item) in effective.items() {
            levels.insert(
                class.to_vec(),
                *self.levels.get(item.source_index).ok_or_else(fail)?,
            );
        }
        let mut boxes: BTreeMap<String, Vec<(RandomBox, Option<String>)>> = BTreeMap::new();
        for b in &self.boxes {
            boxes.entry(key(b.item_utf16.as_ref())?).or_default().push((
                RandomBox {
                    id: b.id,
                    server_code: b.server_code,
                    key: text(b.key_utf16.as_ref())?,
                    visual: b.visual,
                },
                b.feature.clone(),
            ));
        }
        let mut recipes: BTreeMap<(i32, String), Vec<Recipe>> = BTreeMap::new();
        for r in &self.recipes {
            if !policy.is_enabled(r.13.as_deref()).map_err(|_| fail())? {
                continue;
            }
            recipes
                .entry((r.0, key(r.1.as_ref())?))
                .or_default()
                .push(Recipe {
                    character: r.2,
                    item_class: key(r.3.as_ref())?,
                    number: r.4,
                    probability: double(&r.5)?,
                    colors: [r.6, r.7, r.8],
                    notify: text(r.9.as_ref())?,
                    variable: r.10,
                    have: double(&r.11)?,
                    dont_have: double(&r.12)?,
                });
        }
        let mut classes: BTreeMap<i32, Vec<RecipeClass>> = BTreeMap::new();
        for c in &self.classes {
            // ClassName.StartsWith throws on a null name.
            let name = key(c.name_utf16.as_ref())?;
            if !(recipes.contains_key(&(c.id, name.clone())) || name.starts_with('$')) {
                continue;
            }
            classes.entry(c.id).or_default().push(RecipeClass {
                name,
                expected: c.expected,
                probability: double(&c.probability)?,
                number: c.number,
                rule: text(c.rule.as_ref())?,
            });
        }
        // Only the feature-enabled rows are ever returned by RandomItemContentsV2.
        let mut v2: BTreeMap<String, Vec<V2Box>> = BTreeMap::new();
        for r in &self.v2_boxes {
            if policy
                .is_enabled(r.feature.as_deref())
                .map_err(|_| fail())?
            {
                v2.entry(key(r.item_utf16.as_ref())?)
                    .or_default()
                    .push(V2Box {
                        key: text(r.key_utf16.as_ref())?,
                        random_item: key(r.random_item_utf16.as_ref())?,
                        show_pop_up: r.show_pop_up,
                        titicore: r.titicore,
                        cool_time: r.cool_time,
                        exclusive: text(r.exclusive_utf16.as_ref())?,
                    });
            }
        }
        let mut v2_data: BTreeMap<String, V2Data> = BTreeMap::new();
        for r in &self.v2_recipes {
            let recipe = V2Recipe {
                item_class: text(r.1.as_ref())?,
                count: r.2,
                probability: double(&r.4)?,
                rare: r.5,
                character: text(r.6.as_ref())?,
                item: text(r.7.as_ref())?,
                feature_enabled: match &r.8 {
                    None => true,
                    Some(f) => policy.is_enabled(Some(f)).map_err(|_| fail())?,
                },
                colors: [r.9, r.10, r.11],
                bind: r.12,
                expire: r.13,
                announce: text(r.14.as_ref())?,
                message: text(r.15.as_ref())?,
            };
            let data = v2_data.entry(key(r.0.as_ref())?).or_default();
            match r.3 {
                0 => data.package.push(recipe),
                1 => data.gachapon.push(recipe),
                2 => data.sequence.push(recipe),
                _ => {}
            }
        }
        Ok(Contents {
            boxes,
            classes,
            recipes,
            v2,
            v2_data,
            levels,
            policy,
        })
    }
}

impl Contents {
    pub fn policy(&self) -> &FeaturePolicy {
        &self.policy
    }
    /// RandomItemContentsV2.IsRandomItem.
    pub fn is_v2(&self, class: &str) -> bool {
        self.v2.contains_key(class)
    }
    /// RandomItemContentsV2.GetRandomItemKeyList.
    pub fn v2_keys(&self, class: &str) -> Vec<String> {
        self.v2
            .get(class)
            .into_iter()
            .flatten()
            .filter_map(|b| b.key.clone())
            .collect()
    }
    /// RandomItemContentsV2.GetRandomItemInfo(itemClass, keyItemClass): the first row with
    /// that key (rows of one key share a group, so provider order decides).
    pub fn v2_box(&self, class: &str, key: Option<&str>) -> Option<&V2Box> {
        self.v2
            .get(class)
            .into_iter()
            .flatten()
            .find(|b| b.key.as_deref() == key)
    }
    /// RandomItemContentsV2.GetRandomItemData.
    pub fn v2_data(&self, id: &str) -> Option<&V2Data> {
        self.v2_data.get(id)
    }
    /// RandomItemContents.IsRandomItem.
    pub fn is_random_item(&self, class: &str) -> bool {
        self.boxes.contains_key(class)
    }
    /// GetRandomItemKeys: the KeyItemClass of the enabled rows.
    pub fn keys(&self, class: &str) -> Result<Vec<String>, MinimumError> {
        let mut out = Vec::new();
        for (b, feature) in self.boxes.get(class).into_iter().flatten() {
            if self
                .policy
                .is_enabled(feature.as_deref())
                .map_err(|_| fail())?
                && let Some(k) = &b.key
            {
                out.push(k.clone());
            }
        }
        Ok(out)
    }
    /// GetRandomItem(itemClass, keyItemClass): the first enabled row with that key.
    pub fn random_item(
        &self,
        class: &str,
        key: Option<&str>,
    ) -> Result<Option<&RandomBox>, MinimumError> {
        for (b, feature) in self.boxes.get(class).into_iter().flatten() {
            if b.key.as_deref() == key
                && self
                    .policy
                    .is_enabled(feature.as_deref())
                    .map_err(|_| fail())?
            {
                return Ok(Some(b));
            }
        }
        Ok(None)
    }
    /// GetRandomItemRecipeClass (empty without rows).
    pub fn recipe_classes(&self, id: i32) -> &[RecipeClass] {
        self.classes.get(&id).map_or(&[], Vec::as_slice)
    }
    /// GetRandomItemRecipe.
    pub fn recipes(&self, id: i32, class: &str) -> Vec<Recipe> {
        self.recipes
            .get(&(id, class.to_owned()))
            .cloned()
            .unwrap_or_default()
    }
    /// ItemClassInfo.Level of an effective class.
    pub fn level(&self, class: &[u8]) -> Option<i32> {
        self.levels.get(class).copied()
    }
}
