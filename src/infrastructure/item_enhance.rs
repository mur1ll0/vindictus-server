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
    enchantable: u8,
    #[serde(deserialize_with = "nullable")]
    quality_type: Option<PrivateText>,
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
/// CraftShopInfo: ShopID, Order, RecipeID.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CraftShopRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    shop_utf16: Option<PrivateText>,
    order: i32,
    #[serde(deserialize_with = "nullable")]
    recipe_utf16: Option<PrivateText>,
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
struct EnchantRow {
    source_index: usize,
    // ID, EnchantType and Feature are exported but not read by the item service.
    #[serde(rename = "id")]
    _id: i32,
    #[serde(deserialize_with = "nullable")]
    enchant_utf16: Option<PrivateText>,
    is_prefix: bool,
    #[serde(rename = "enchant_type", deserialize_with = "nullable")]
    _enchant_type: Option<PrivateText>,
    level: i32,
    min_arg: i32,
    max_arg: i32,
    #[serde(deserialize_with = "nullable")]
    constraint: Option<PrivateText>,
    min_ratio: i32,
    max_ratio: i32,
    destruction: i32,
    mini_game: i32,
    duration: i32,
    ignore_max_armor: bool,
    #[serde(rename = "feature", deserialize_with = "nullable")]
    _feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnchantStatRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    enchant_utf16: Option<PrivateText>,
    order: i32,
    #[serde(deserialize_with = "nullable")]
    condition: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    stat: Option<PrivateText>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnchantDiceRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    min: i32,
    max: i32,
    // enchantDiceInfoDic keeps every row; SkillRank and Feature are not read.
    #[serde(rename = "skill_rank")]
    _skill_rank: i32,
    #[serde(rename = "feature", deserialize_with = "nullable")]
    _feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManufactureRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    manufacture_utf16: Option<PrivateText>,
    grade: i32,
    min: i32,
    max: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManufactureRecipeRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    manufacture_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    recipe_utf16: Option<PrivateText>,
    experience_required: i32,
    grade_required: i32,
    base_reward_exp: i32,
    // RecipeItemClass (recipe items) and AutoGiveQuest (GiveManufactureRecipesByQuestID)
    // are not consumed yet.
    #[serde(rename = "recipe_item_utf16", deserialize_with = "nullable")]
    _recipe_item: Option<PrivateText>,
    #[serde(rename = "auto_give_quest", deserialize_with = "nullable")]
    _auto_give_quest: Option<PrivateText>,
    always_listed: u8,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManufactureMaterialRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    recipe_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    num: i32,
    #[serde(rename = "type")]
    kind: u8,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManufactureDependencyRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    manufacture_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    required_utf16: Option<PrivateText>,
    required_experience: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QualityDiceRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_type_utf16: Option<PrivateText>,
    quality: i32,
    dice: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QualityStatRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_type_utf16: Option<PrivateText>,
    quality: i32,
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
struct SkillEnhanceRow {
    source_index: usize,
    group: i32,
    key: i32,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    max_success_ratio: i32,
    max_durability: i32,
    repair_value: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillEnhanceMaterialRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(rename = "type")]
    kind: u8,
    min: i32,
    max: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillEnhanceGroupRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    skill_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    groups: Option<PrivateText>,
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
    #[serde(default)]
    craft_shops: Vec<CraftShopRow>,
    anti_bind: Vec<AntiBindRow>,
    enchant: Vec<EnchantRow>,
    enchant_stats: Vec<EnchantStatRow>,
    enchant_dice: Vec<EnchantDiceRow>,
    manufacture: Vec<ManufactureRow>,
    manufacture_recipes: Vec<ManufactureRecipeRow>,
    manufacture_materials: Vec<ManufactureMaterialRow>,
    manufacture_dependencies: Vec<ManufactureDependencyRow>,
    quality_dice: Vec<QualityDiceRow>,
    quality_stats: Vec<QualityStatRow>,
    skill_enhance: Vec<SkillEnhanceRow>,
    skill_enhance_materials: Vec<SkillEnhanceMaterialRow>,
    skill_enhance_groups: Vec<SkillEnhanceGroupRow>,
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
    /// recipes: every enabled RecipeID with its Target (summed) and Material rows.
    recipes: BTreeMap<String, CraftRecipe>,
    /// craftShopInfoDic: (ShopID.ToLower(), Order) -> RecipeID, the first row wins.
    craft_shops: BTreeMap<(String, i32), String>,
    /// antiBindInfo: enabled rows in provider order.
    anti_bind: Vec<AntiBind>,
    /// FeatureMatrix CraftPriceRatio text.
    craft_price_ratio: String,
    /// The catalog policy plus the exported enhance/restore features.
    policy: Option<crate::domain::feature_policy::FeaturePolicy>,
    /// enchantInfoDic (every row; ToDictionary refuses a duplicate EnchantClass).
    enchant: BTreeMap<String, crate::domain::enchant::EnchantInfo>,
    /// enchantStatInfoDic: rows ordered by Order (stable) per EnchantClass.
    enchant_stats: BTreeMap<String, Vec<crate::domain::enchant::StatRow>>,
    /// enchantDiceInfoDic: (MinValue, MaxValue) by ItemClass.
    enchant_dice: BTreeMap<String, (i32, i32)>,
    /// GetEquipItemInfo().Enchantable of each effective equipment class.
    enchantable: BTreeMap<Vec<u8>, u8>,
    /// GetEquipItemInfo().QualityType of each effective equipment class.
    quality_types: BTreeMap<Vec<u8>, Option<String>>,
    /// ManufactureDataContainer.
    manufacture: crate::domain::manufacture::Contents,
    /// qualityDiceDic: ToDoubleDictionary (ItemType -> Quality -> Dice) in insertion order.
    quality_dice: BTreeMap<String, Vec<(i32, i32)>>,
    /// qualityStatInfoDic: ToDoubleListDictionary (ItemType -> Quality -> rows).
    quality_stats: BTreeMap<String, BTreeMap<i32, Vec<EnhanceStat>>>,
    /// CharacterContents.skillEnhanceInfo in provider order.
    skill_enhance: Vec<crate::domain::skill_enhance::Info>,
    /// skillEnhanceMaterialInfo: ToDictionary by MaterialItemClass.
    skill_enhance_materials: BTreeMap<String, crate::domain::skill_enhance::Material>,
    /// SkillInfo.GetAvailableSkillEnhanceGroup of the skills with a group text.
    skill_enhance_groups: BTreeMap<String, Vec<i32>>,
}

/// RecipeID, Target rows (AddItem sums) and Material rows (priced when used).
type RecipeRows = (String, Vec<(String, i32)>, Vec<(String, i32)>);

/// An ItemContents Recipe: Target rows summed in first-seen order and the Material rows
/// before PriceCalc (AddMaterial sums the priced values).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftRecipe {
    pub targets: Vec<(String, i32)>,
    pub materials: Vec<(String, i32)>,
}

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
/// EnchantInfo.ItemConstraint: UTF-16LE hex up to 16384 bytes (catalog texts stop at 1024).
fn long_text(value: Option<&PrivateText>) -> Result<Option<String>, MinimumError> {
    value
        .map(|v| {
            let hex = v.0.as_bytes();
            if hex.len() > 32768 || !hex.len().is_multiple_of(4) {
                return Err(fail());
            }
            let units = hex
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| {
                    if !c.iter().all(u8::is_ascii_hexdigit) {
                        return Err(fail());
                    }
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
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 32 * 1024 * 1024 {
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
        let manufacture_rows = p.manufacture.len()
            + p.manufacture_recipes.len()
            + p.manufacture_materials.len()
            + p.manufacture_dependencies.len()
            + p.quality_dice.len()
            + p.quality_stats.len()
            + p.skill_enhance.len()
            + p.skill_enhance_materials.len()
            + p.skill_enhance_groups.len();
        if p.format_version != 7
            || p.craft_shops.len() > 65536
            || !ordered(&p.craft_shops, |r| r.source_index)
            || manufacture_rows > 65536
            || !ordered(&p.manufacture, |r| r.source_index)
            || !ordered(&p.manufacture_recipes, |r| r.source_index)
            || !ordered(&p.manufacture_materials, |r| r.source_index)
            || !ordered(&p.manufacture_dependencies, |r| r.source_index)
            || !ordered(&p.quality_dice, |r| r.source_index)
            || !ordered(&p.quality_stats, |r| r.source_index)
            || !ordered(&p.skill_enhance, |r| r.source_index)
            || !ordered(&p.skill_enhance_materials, |r| r.source_index)
            || !ordered(&p.skill_enhance_groups, |r| r.source_index)
            || p.quality_stats.iter().any(|r| r.value.len() != 8)
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
            || !ordered(&p.enchant, |r| r.source_index)
            || !ordered(&p.enchant_stats, |r| r.source_index)
            || !ordered(&p.enchant_dice, |r| r.source_index)
            || p.enchant.len() + p.enchant_stats.len() + p.enchant_dice.len() > 65536
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
            out.enchantable.insert(class.to_vec(), row.enchantable);
            out.quality_types
                .insert(class.to_vec(), text(row.quality_type.as_ref())?);
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
        for c in &self.craft_shops {
            // ShopID.ToLower() throws on a null ShopID.
            let shop = key(c.shop_utf16.as_ref())?.to_lowercase();
            let recipe = key(c.recipe_utf16.as_ref())?;
            out.craft_shops.entry((shop, c.order)).or_insert(recipe);
        }
        for (id, targets, materials) in recipes {
            out.recipes.insert(
                id,
                CraftRecipe {
                    targets: targets.clone(),
                    materials: materials.clone(),
                },
            );
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
        for r in &self.enchant {
            let info = crate::domain::enchant::EnchantInfo {
                class: key(r.enchant_utf16.as_ref())?,
                is_prefix: r.is_prefix,
                level: r.level,
                min_arg: r.min_arg,
                max_arg: r.max_arg,
                constraint: long_text(r.constraint.as_ref())?,
                min_ratio: r.min_ratio,
                max_ratio: r.max_ratio,
                destruction: r.destruction,
                mini_game: r.mini_game,
                duration: r.duration,
                ignore_max_armor: r.ignore_max_armor,
            };
            if out.enchant.insert(info.class.clone(), info).is_some() {
                return Err(fail());
            }
        }
        for r in &self.enchant_stats {
            out.enchant_stats
                .entry(key(r.enchant_utf16.as_ref())?)
                .or_default()
                .push(crate::domain::enchant::StatRow {
                    order: r.order,
                    condition: text(r.condition.as_ref())?,
                    stat: text(r.stat.as_ref())?,
                });
        }
        for rows in out.enchant_stats.values_mut() {
            rows.sort_by_key(|r| r.order);
        }
        for r in &self.enchant_dice {
            if out
                .enchant_dice
                .insert(key(r.item_utf16.as_ref())?, (r.min, r.max))
                .is_some()
            {
                return Err(fail());
            }
        }
        let known: std::collections::BTreeSet<Vec<u8>> =
            effective.items().map(|(class, _)| class.to_vec()).collect();
        let utf16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(u16::to_le_bytes).collect() };
        let recipes = self
            .manufacture_recipes
            .iter()
            .map(|r| {
                Ok(crate::domain::manufacture::RecipeRow {
                    manufacture_id: key(r.manufacture_utf16.as_ref())?,
                    recipe_id: key(r.recipe_utf16.as_ref())?,
                    experience_required: r.experience_required,
                    grade_required: r.grade_required,
                    base_reward_exp: r.base_reward_exp,
                    always_listed: r.always_listed,
                    feature: r.feature.clone(),
                })
            })
            .collect::<Result<Vec<_>, MinimumError>>()?;
        let materials = self
            .manufacture_materials
            .iter()
            .map(|r| {
                Ok((
                    key(r.recipe_utf16.as_ref())?,
                    key(r.item_utf16.as_ref())?,
                    r.num,
                    r.kind,
                ))
            })
            .collect::<Result<Vec<_>, MinimumError>>()?;
        let infos = self
            .manufacture
            .iter()
            .map(|r| {
                Ok((
                    key(r.manufacture_utf16.as_ref())?,
                    crate::domain::manufacture::Interval {
                        grade: r.grade,
                        min: r.min,
                        max: r.max,
                    },
                ))
            })
            .collect::<Result<Vec<_>, MinimumError>>()?;
        let dependencies = self
            .manufacture_dependencies
            .iter()
            .map(|r| {
                Ok((
                    key(r.manufacture_utf16.as_ref())?,
                    key(r.required_utf16.as_ref())?,
                    r.required_experience,
                ))
            })
            .collect::<Result<Vec<_>, MinimumError>>()?;
        out.manufacture = crate::domain::manufacture::Contents::build(
            recipes,
            materials,
            infos,
            dependencies,
            |class| known.contains(&utf16(class)),
        )
        .map_err(|_| fail())?;
        for r in &self.quality_dice {
            let dice = out
                .quality_dice
                .entry(key(r.item_type_utf16.as_ref())?)
                .or_default();
            // ToDoubleDictionary logs a repeated Quality and overwrites its value.
            match dice.iter_mut().find(|(q, _)| *q == r.quality) {
                Some(entry) => entry.1 = r.dice,
                None => dice.push((r.quality, r.dice)),
            }
        }
        for r in &self.quality_stats {
            out.quality_stats
                .entry(key(r.item_type_utf16.as_ref())?)
                .or_default()
                .entry(r.quality)
                .or_default()
                .push(EnhanceStat {
                    stat: r.stat.as_ref().map(units).transpose()?,
                    value_type: text(r.value_type.as_ref())?,
                    value: single(&r.value)?,
                    base: r.base.as_ref().map(units).transpose()?,
                });
        }
        for r in &self.skill_enhance {
            out.skill_enhance.push(crate::domain::skill_enhance::Info {
                group: r.group,
                key: r.key,
                item_class: text(r.item_utf16.as_ref())?,
                max_success_ratio: r.max_success_ratio,
                max_durability: r.max_durability,
                repair_value: r.repair_value,
            });
        }
        for r in &self.skill_enhance_materials {
            let class = key(r.item_utf16.as_ref())?;
            let material = crate::domain::skill_enhance::Material {
                item_class: class.clone(),
                kind: r.kind,
                min: r.min,
                max: r.max,
            };
            if out
                .skill_enhance_materials
                .insert(class, material)
                .is_some()
            {
                return Err(fail());
            }
        }
        for r in &self.skill_enhance_groups {
            let groups =
                crate::domain::skill_enhance::parse_groups(text(r.groups.as_ref())?.as_deref());
            // skillInfoDic is a ToDictionary by SkillID.
            if out
                .skill_enhance_groups
                .insert(key(r.skill_utf16.as_ref())?, groups)
                .is_some()
            {
                return Err(fail());
            }
        }
        out.policy = Some(policy);
        Ok(out)
    }
}

impl Contents {
    /// GetSkillEnhanceInfo(groupKey, indexKey): the last matching row.
    pub fn skill_enhance(
        &self,
        group: i32,
        key: i32,
    ) -> Option<&crate::domain::skill_enhance::Info> {
        self.skill_enhance
            .iter()
            .rev()
            .find(|i| i.group == group && i.key == key)
    }
    /// GetSkillEnhanceInfo(itemClass): the last row with that ItemClass (ignore case).
    pub fn skill_enhance_of_item(
        &self,
        class: &str,
    ) -> Option<&crate::domain::skill_enhance::Info> {
        self.skill_enhance.iter().rev().find(|i| {
            i.item_class
                .as_deref()
                .is_some_and(|c| c.to_lowercase() == class.to_lowercase())
        })
    }
    /// GetSkillEnhanceMaterialInfo.
    pub fn skill_enhance_material(
        &self,
        class: &str,
    ) -> Option<&crate::domain::skill_enhance::Material> {
        self.skill_enhance_materials.get(class)
    }
    /// The enhance groups a skill accepts (empty without SkillEnhanceGroup).
    pub fn skill_enhance_groups(&self, skill: &str) -> Vec<i32> {
        self.skill_enhance_groups
            .get(skill)
            .cloned()
            .unwrap_or_default()
    }
    /// ManufactureDataContainer.
    pub fn manufacture(&self) -> &crate::domain::manufacture::Contents {
        &self.manufacture
    }
    /// GetEquipItemInfo().QualityType; `None` for unknown classes and null types.
    pub fn quality_type(&self, class: &[u8]) -> Option<&str> {
        self.quality_types.get(class)?.as_deref()
    }
    /// GetQualityStatInfo of a non-combined item: the rows of its QualityType ("" when
    /// null) at the level, empty when missing or for a level of 0 or less.
    pub fn quality_stat_rows(&self, class: &[u8], level: i32) -> &[EnhanceStat] {
        if level <= 0 {
            return &[];
        }
        self.quality_stats
            .get(self.quality_type(class).unwrap_or(""))
            .and_then(|l| l.get(&level))
            .map_or(&[], Vec::as_slice)
    }
    /// GetQualityDiceInfo: (Quality, Dice) in insertion order (empty when unknown).
    pub fn quality_dice(&self, item_type: &str) -> &[(i32, i32)] {
        self.quality_dice.get(item_type).map_or(&[], Vec::as_slice)
    }
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
    /// Panel tunings (B4.5): EnhanceServerInfo SuccessRatio and EnchantInfo Min/Max ratios
    /// of existing rows.
    pub fn with_overlay(mut self, overlay: &crate::domain::content_overlay::Overlay) -> Self {
        for (target, ratio) in &overlay.enhance_ratios {
            if let Some((kind, level)) = crate::domain::content_overlay::enhance_target(target)
                && let Some(value) = self.server.get_mut(kind).and_then(|l| l.get_mut(&level))
            {
                *value = *ratio;
            }
        }
        for (class, [min, max]) in &overlay.enchant_ratios {
            if let Some(info) = self.enchant.get_mut(class) {
                info.min_ratio = *min;
                info.max_ratio = *max;
            }
        }
        self
    }
    /// EnhanceServerInfo rows: (EnhanceType, level, SuccessRatio), for the panel.
    pub fn server_ratios(&self) -> impl Iterator<Item = (&str, i32, f32)> {
        self.server
            .iter()
            .flat_map(|(k, levels)| levels.iter().map(move |(l, r)| (k.as_str(), *l, *r)))
    }
    /// EnchantInfo rows, for the panel.
    pub fn enchants(&self) -> impl Iterator<Item = &crate::domain::enchant::EnchantInfo> {
        self.enchant.values()
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
    /// ItemContents.GetCraftShopInfo(shopID, order).RecipeID.
    pub fn craft_shop(&self, shop: &str, order: i32) -> Option<&str> {
        self.craft_shops
            .get(&(shop.to_lowercase(), order))
            .map(String::as_str)
    }
    /// ItemContents.GetRecipe(id).
    pub fn recipe(&self, id: &str) -> Option<&CraftRecipe> {
        self.recipes.get(id)
    }
    pub fn anti_bind(&self) -> &[AntiBind] {
        &self.anti_bind
    }
    /// FeatureMatrix of the enhance processor (catalog features plus the exported ones).
    pub fn policy(&self) -> Option<&crate::domain::feature_policy::FeaturePolicy> {
        self.policy.as_ref()
    }
    /// GetEquipItemInfo().Enchantable (`None` for non-equipment).
    pub fn enchantable(&self, class: &[u8]) -> Option<u8> {
        self.enchantable.get(class).copied()
    }
    /// GetEnchantInfo, GetEnchantStatInfo and GetEnchantDiceInfo.
    pub fn enchant(&self, class: &str) -> Option<&crate::domain::enchant::EnchantInfo> {
        self.enchant.get(class)
    }
    pub fn enchant_stats(&self, class: &str) -> Option<&[crate::domain::enchant::StatRow]> {
        self.enchant_stats.get(class).map(Vec::as_slice)
    }
    pub fn enchant_dice(&self, class: &str) -> Option<(i32, i32)> {
        self.enchant_dice.get(class).copied()
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
