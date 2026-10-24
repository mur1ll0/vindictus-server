//! Source-bound enhance contents exported by scripts/content/export_item_enhance.py: the
//! effective EquipItemInfo.EnhanceType and ItemClassInfo.MaxAntiBind of each class and
//! the ItemContents enhance, rune, MakeItemRecipe and AntiBindInfo data, with features
//! evaluated against the catalog policy.
use super::{
    item_catalog::Package as Catalog,
    minimum_character::MinimumError,
    user_export::{PrivateText, nullable},
};
use crate::domain::enhance::{
    AntiBind, EnhanceInfo, EnhanceStat, Recipe, Rune, RuneExtra, StatRows,
};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Equip {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    #[serde(deserialize_with = "nullable")]
    enhance_type: Option<PrivateText>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Info {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    enhance_type: Option<PrivateText>,
    level: i32,
    #[serde(deserialize_with = "nullable")]
    material1: Option<PrivateText>,
    num1: i32,
    #[serde(deserialize_with = "nullable")]
    material2: Option<PrivateText>,
    num2: i32,
    gold: i32,
    success_ratio: String,
    #[serde(deserialize_with = "nullable")]
    on_fail: Option<PrivateText>,
    on_fail_recipe_ratio: String,
    on_fail_material_num1: i32,
    effect: i32,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stat {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    enhance_type: Option<PrivateText>,
    level: i32,
    #[serde(deserialize_with = "nullable")]
    stat: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    value_type: Option<PrivateText>,
    value: String,
    #[serde(deserialize_with = "nullable")]
    base: Option<PrivateText>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Server {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    enhance_type: Option<PrivateText>,
    level: i32,
    success_ratio: String,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bonus {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    bonus: String,
    min: i16,
    max: i16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Instead {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    use_count: i32,
    slot: u8,
    min: i16,
    max: i16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Class {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    max_anti_bind: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuneRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    relation: i32,
    min: i32,
    max: i32,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuneExtraRow {
    source_index: usize,
    relation: i32,
    success_ratio: String,
    use_item_success_ratio: bool,
    destroy_on_success: bool,
    reset_rank_on_fail: bool,
    force_rank_down_on_fail: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    recipe_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    num: i32,
    #[serde(rename = "type")]
    kind: u8,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AntiBindRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    target: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    from_value: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    to_value: Option<PrivateText>,
    count: i32,
    ap: i32,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
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
    equips: Vec<Equip>,
    classes: Vec<Class>,
    enhance: Vec<Info>,
    stats: Vec<Stat>,
    server: Vec<Server>,
    bonus: Vec<Bonus>,
    instead: Vec<Instead>,
    runes: Vec<RuneRow>,
    rune_extra: Vec<RuneExtraRow>,
    recipes: Vec<RecipeRow>,
    anti_bind: Vec<AntiBindRow>,
    features: Vec<Feature>,
}

/// ItemContents enhance dictionaries bound to the catalog.
#[derive(Default)]
pub struct Contents {
    /// GetEquipItemInfo().EnhanceType of each effective equipment class.
    types: BTreeMap<Vec<u8>, Option<String>>,
    /// enhanceInfoDic: enabled rows, ToDoubleDictionary keeps the last duplicate.
    infos: BTreeMap<String, BTreeMap<i32, EnhanceInfo>>,
    /// enhanceStatInfoDic: every row, in provider order per (type, level).
    stats: BTreeMap<String, BTreeMap<i32, Vec<EnhanceStat>>>,
    /// enhanceServerInfoDic SuccessRatio of the enabled rows.
    server: BTreeMap<String, BTreeMap<i32, f32>>,
    /// enhanceBonusServerInfoDic / enhanceInsteadMaterialInfoDic in insertion order.
    bonus: Vec<(String, f64, i16, i16)>,
    instead: Vec<(String, i32, u8, i16, i16)>,
    /// ItemClassInfo.MaxAntiBind of each effective class.
    max_anti_bind: BTreeMap<Vec<u8>, i32>,
    /// enhanceRuneInfoDic: enabled rows, ToDictionary_OverwriteDuplicated.
    runes: BTreeMap<String, Rune>,
    /// enhanceRuneExtraInfoDic (ToDictionary: a duplicate refuses the load).
    rune_extra: BTreeMap<i32, RuneExtra>,
    /// makeItemRecipe: single equippable target recipes, last recipe wins.
    make_item_recipe: BTreeMap<String, Recipe>,
    /// antiBindInfo: enabled rows in provider order.
    anti_bind: Vec<AntiBind>,
    /// FeatureMatrix CraftPriceRatio text.
    craft_price_ratio: String,
}

/// RecipeID, Target rows (AddItem sums) and Material rows (priced when used).
type RecipeRows = (String, Vec<(String, i32)>, Vec<(String, i32)>);

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn units(value: &PrivateText) -> Result<Vec<u16>, MinimumError> {
    let key = super::item_catalog::opaque(Some(PrivateText(value.0.clone())))?.ok_or_else(fail)?;
    Ok(key
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|a| u16::from_le_bytes(*a))
        .collect())
}
fn text(value: Option<&PrivateText>) -> Result<Option<String>, MinimumError> {
    value
        .map(|v| String::from_utf16(&units(v)?).map_err(|_| fail()))
        .transpose()
}
/// Dictionary keys: a null key throws in Dictionary.Add/TryGetValue.
fn key(value: Option<&PrivateText>) -> Result<String, MinimumError> {
    text(value)?.ok_or_else(fail)
}
/// Big-endian Single bits (8 hex digits, checked by `read`).
fn single(bits: &str) -> Result<f32, MinimumError> {
    Ok(f32::from_bits(
        u32::from_str_radix(bits, 16).map_err(|_| fail())?,
    ))
}
fn double(bits: &str) -> Result<f64, MinimumError> {
    if bits.len() != 16 {
        return Err(fail());
    }
    Ok(f64::from_bits(
        u64::from_str_radix(bits, 16).map_err(|_| fail())?,
    ))
}
fn ordered<T>(rows: &[T], index: impl Fn(&T) -> usize) -> bool {
    rows.windows(2).all(|w| index(&w[0]) < index(&w[1]))
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
        let sizes = [
            p.equips.len(),
            p.classes.len(),
            p.enhance.len(),
            p.stats.len(),
            p.server.len(),
            p.bonus.len(),
            p.instead.len(),
            p.runes.len(),
            p.rune_extra.len(),
            p.recipes.len(),
            p.anti_bind.len(),
        ];
        if p.format_version != 2
            || p.source != "heroesContents.item-enhance"
            || sizes.iter().any(|n| *n > 65536)
            || p.features.len() > 256
            || !ordered(&p.equips, |r| r.source_index)
            || !ordered(&p.enhance, |r| r.source_index)
            || !ordered(&p.stats, |r| r.source_index)
            || !ordered(&p.server, |r| r.source_index)
            || !ordered(&p.bonus, |r| r.source_index)
            || !ordered(&p.instead, |r| r.source_index)
            || !ordered(&p.classes, |r| r.source_index)
            || !ordered(&p.runes, |r| r.source_index)
            || !ordered(&p.rune_extra, |r| r.source_index)
            || !ordered(&p.recipes, |r| r.source_index)
            || !ordered(&p.anti_bind, |r| r.source_index)
            || p.rune_extra.iter().any(|r| r.success_ratio.len() != 16)
            || [&p.success_hex(), &p.value_hex()]
                .iter()
                .any(|v| v.iter().any(|h| h.len() != 8))
        {
            return Err(fail());
        }
        Ok(p)
    }
    fn success_hex(&self) -> Vec<&str> {
        self.enhance
            .iter()
            .flat_map(|r| [r.success_ratio.as_str(), r.on_fail_recipe_ratio.as_str()])
            .chain(self.server.iter().map(|r| r.success_ratio.as_str()))
            .collect()
    }
    fn value_hex(&self) -> Vec<&str> {
        self.stats.iter().map(|r| r.value.as_str()).collect()
    }
    /// Binds to the catalog snapshot and evaluates the row features.
    pub fn bind(&self, catalog: &Catalog) -> Result<Contents, MinimumError> {
        if self.database_sha256 != catalog.manifest.database_sha256
            || self.config_sha256 != catalog.manifest.config_sha256
            || self.equips.len() != catalog.equips.len()
            || self.classes.len() != catalog.classes.len()
        {
            return Err(fail());
        }
        for (a, b) in self.classes.iter().zip(&catalog.classes) {
            let key = a.item_utf16.as_ref().map(units).transpose()?;
            let expected = b.item_class.as_ref().map(|k| {
                k.as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|u| u16::from_le_bytes(*u))
                    .collect::<Vec<_>>()
            });
            if key != expected || a.feature != b.feature {
                return Err(fail());
            }
        }
        for (a, b) in self.equips.iter().zip(&catalog.equips) {
            let key = a.item_utf16.as_ref().map(units).transpose()?;
            let expected = b.item_class.as_ref().map(|k| {
                k.as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|u| u16::from_le_bytes(*u))
                    .collect::<Vec<_>>()
            });
            if key != expected || a.feature != b.feature {
                return Err(fail());
            }
        }
        // The catalog policy plus the enhance features; a key both carry must agree.
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
        let policy =
            crate::domain::feature_policy::FeaturePolicy::npc_source(rows).map_err(|_| fail())?;
        let mut out = Contents::default();
        let effective = crate::application::item_catalog_inspection::catalog(catalog)?;
        for (class, index) in effective.equipment() {
            let row = self.equips.get(index).ok_or_else(fail)?;
            out.types
                .insert(class.to_vec(), text(row.enhance_type.as_ref())?);
        }
        for (class, item) in effective.items() {
            let row = self.classes.get(item.source_index).ok_or_else(fail)?;
            out.max_anti_bind.insert(class.to_vec(), row.max_anti_bind);
        }
        for r in &self.runes {
            if policy
                .is_enabled(r.feature.as_deref())
                .map_err(|_| fail())?
            {
                out.runes.insert(
                    key(r.item_utf16.as_ref())?,
                    Rune {
                        relation: r.relation,
                        min_level: r.min,
                        max_level: r.max,
                    },
                );
            }
        }
        for r in &self.rune_extra {
            let extra = RuneExtra {
                success_ratio: double(&r.success_ratio)?,
                use_item_success_ratio: r.use_item_success_ratio,
                destroy_on_success: r.destroy_on_success,
                reset_rank_on_fail: r.reset_rank_on_fail,
                force_rank_down_on_fail: r.force_rank_down_on_fail,
            };
            if out.rune_extra.insert(r.relation, extra).is_some() {
                return Err(fail());
            }
        }
        // ItemContents recipes: enabled RecipeMaterialInfo rows of known classes, by
        // RecipeID in first-seen order; Target (0) and Material (1) rows.
        let mut recipes: Vec<RecipeRows> = Vec::new();
        for r in &self.recipes {
            if !policy
                .is_enabled(r.feature.as_deref())
                .map_err(|_| fail())?
            {
                continue;
            }
            let id = key(r.recipe_utf16.as_ref())?;
            let position = match recipes.iter().position(|(k, _, _)| *k == id) {
                Some(p) => p,
                None => {
                    recipes.push((id, Vec::new(), Vec::new()));
                    recipes.len() - 1
                }
            };
            let item_bytes = super::item_catalog::opaque(
                r.item_utf16.as_ref().map(|t| PrivateText(t.0.clone())),
            )?;
            if effective
                .get(item_bytes.as_ref().map(|b| b.as_bytes()))
                .is_none()
            {
                continue;
            }
            let item = key(r.item_utf16.as_ref())?;
            match r.kind {
                0 => {
                    let targets = &mut recipes[position].1;
                    match targets.iter_mut().find(|(k, _)| *k == item) {
                        Some(t) => t.1 = t.1.wrapping_add(r.num),
                        None => targets.push((item, r.num)),
                    }
                }
                1 => recipes[position].2.push((item, r.num)),
                _ => {}
            }
        }
        for (_, targets, materials) in recipes {
            if let [(target, 1)] = targets.as_slice() {
                let bytes: Vec<u8> = target.encode_utf16().flat_map(u16::to_le_bytes).collect();
                if out.types.contains_key(&bytes) {
                    out.make_item_recipe
                        .insert(target.clone(), Recipe { materials });
                }
            }
        }
        for r in &self.anti_bind {
            if policy
                .is_enabled(r.feature.as_deref())
                .map_err(|_| fail())?
            {
                out.anti_bind.push(AntiBind {
                    target: text(r.target.as_ref())?,
                    from: text(r.from_value.as_ref())?,
                    to: text(r.to_value.as_ref())?,
                    count: r.count,
                    ap: r.ap,
                });
            }
        }
        out.craft_price_ratio = self
            .features
            .iter()
            .find(|f| f.key == "CraftPriceRatio")
            .and_then(|f| f.value.clone())
            .unwrap_or_default();
        for r in &self.enhance {
            if !policy
                .is_enabled(r.feature.as_deref())
                .map_err(|_| fail())?
            {
                continue;
            }
            out.infos
                .entry(key(r.enhance_type.as_ref())?)
                .or_default()
                .insert(
                    r.level,
                    EnhanceInfo {
                        level: r.level,
                        material1: text(r.material1.as_ref())?,
                        num1: r.num1,
                        material2: text(r.material2.as_ref())?,
                        num2: r.num2,
                        gold: r.gold,
                        success_ratio: single(&r.success_ratio)?,
                        on_fail: text(r.on_fail.as_ref())?,
                        on_fail_recipe_ratio: single(&r.on_fail_recipe_ratio)?,
                        on_fail_material_num1: r.on_fail_material_num1,
                        effect: r.effect,
                    },
                );
        }
        for r in &self.stats {
            out.stats
                .entry(key(r.enhance_type.as_ref())?)
                .or_default()
                .entry(r.level)
                .or_default()
                .push(EnhanceStat {
                    stat: r.stat.as_ref().map(units).transpose()?,
                    value_type: text(r.value_type.as_ref())?,
                    value: single(&r.value)?,
                    base: r.base.as_ref().map(units).transpose()?,
                });
        }
        for r in &self.server {
            if !policy
                .is_enabled(r.feature.as_deref())
                .map_err(|_| fail())?
            {
                continue;
            }
            out.server
                .entry(key(r.enhance_type.as_ref())?)
                .or_default()
                .insert(r.level, single(&r.success_ratio)?);
        }
        for r in &self.bonus {
            out.bonus
                .push((key(r.item_utf16.as_ref())?, double(&r.bonus)?, r.min, r.max));
        }
        for r in &self.instead {
            out.instead.push((
                key(r.item_utf16.as_ref())?,
                r.use_count,
                r.slot,
                r.min,
                r.max,
            ));
        }
        Ok(out)
    }
}

impl Contents {
    /// GetEquipItemInfo().EnhanceType; `None` for unknown classes and null types.
    pub fn enhance_type(&self, class: &[u8]) -> Option<&str> {
        self.types.get(class)?.as_deref()
    }
    /// ItemContents.GetEnhanceInfo(item, level) for a non-combined item.
    pub fn info(&self, class: &[u8], level: i32) -> Option<&EnhanceInfo> {
        self.infos.get(self.enhance_type(class)?)?.get(&level)
    }
    /// GetEnhanceInfo then GetEnhanceStatInfo, as enhance::load consumes them.
    pub fn stat_rows(&self, class: &[u8], level: i32) -> Option<StatRows<'_>> {
        let kind = self.enhance_type(class)?;
        self.infos.get(kind)?.get(&level)?;
        Some(
            self.stats
                .get(kind)
                .and_then(|l| l.get(&level))
                .map(Vec::as_slice),
        )
    }
    /// ItemContents.GetEnhanceMaxLevel: the largest enabled level of the type, -1 without.
    pub fn max_level(&self, class: &[u8]) -> i32 {
        self.enhance_type(class)
            .and_then(|k| self.infos.get(k))
            .and_then(|l| l.keys().next_back().copied())
            .unwrap_or(-1)
    }
    /// GetEnhanceServerInfo(enhanceType, level).SuccessRatio.
    pub fn server_ratio(&self, enhance_type: &str, level: i32) -> Option<f32> {
        if enhance_type.is_empty() {
            return None;
        }
        self.server.get(enhance_type)?.get(&level).copied()
    }
    /// GetEnhanceBonusServerInfo(material, level).Bonus: first range match.
    pub fn bonus(&self, material: &str, level: i32) -> Option<f64> {
        self.bonus
            .iter()
            .find(|(c, _, min, max)| {
                c == material && i32::from(*min) <= level && i32::from(*max) >= level
            })
            .map(|b| b.1)
    }
    /// ItemClassInfo.MaxAntiBind (0 for unknown classes is never used: callers own items).
    pub fn max_anti_bind(&self, class: &[u8]) -> Option<i32> {
        self.max_anti_bind.get(class).copied()
    }
    /// GetEnhanceRuneInfo and GetEnhanceRuneExtraInfo.
    pub fn rune(&self, class: &str) -> Option<&Rune> {
        self.runes.get(class)
    }
    pub fn rune_extra(&self, relation: i32) -> Option<&RuneExtra> {
        self.rune_extra.get(&relation)
    }
    /// GetMakeItemRecipe(itemClass).
    pub fn make_item_recipe(&self, class: &str) -> Option<&Recipe> {
        self.make_item_recipe.get(class)
    }
    pub fn anti_bind(&self) -> &[AntiBind] {
        &self.anti_bind
    }
    /// GetFloat("CraftPriceRatio") with ItemContents replacing 0 by 1.
    pub fn craft_price_ratio(&self) -> Option<f32> {
        crate::domain::shop_price::ratio(&self.craft_price_ratio)
    }
    /// GetEnhanceInsteadMaterial(material, level): (ItemClass, UseCount).
    pub fn instead(&self, material: &str, level: i32) -> Option<(&str, i32)> {
        self.instead
            .iter()
            .find(|(c, _, _, min, max)| {
                c == material && i32::from(*min) <= level && i32::from(*max) >= level
            })
            .map(|i| (i.0.as_str(), i.1))
    }
}
