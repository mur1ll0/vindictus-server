//! Private NPC script projection (NpcTalkInfo.xml defaults and StoryLineInfo XML) bound
//! to the item catalog's content hashes. Never opens the original XML directory.
use super::{item_catalog::Package as Catalog, minimum_character::MinimumError};
use crate::domain::npc_script::{DefaultScript, Element, NpcScripts, StoryLineInfo};
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    database_sha256: String,
    config_sha256: String,
    scripts_sha256: String,
    defaults: usize,
    storylines: usize,
    coerced_integers: usize,
    shops: usize,
    coerced_shop_integers: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feature {
    key: String,
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDefault {
    feature: String,
    path: String,
    tree: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
struct RawInfo {
    #[serde(rename = "StoryLineID")]
    story_line_id: String,
    story_line_name: Option<String>,
    xml_path: Option<String>,
    npc: Option<String>,
    category: Option<String>,
    stage: Option<String>,
    reward_gold: i32,
    reward_exp: i32,
    reward_ap: i32,
    reward_title: i32,
    reward_item: Option<String>,
    reward_item_count: i32,
    reward_skill: Option<String>,
    reward_skill_rank: i32,
    auto_give_level: i32,
    class_restriction: i32,
    auto_process: bool,
    auto_reveal: bool,
    season: i32,
    episode: Option<String>,
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
struct RawShop {
    #[serde(rename = "ShopID")]
    shop_id: String,
    order: i16,
    item_class: String,
    count: i32,
    price_item_class: String,
    price_count: i32,
    #[serde(rename = "AppearQuestID")]
    appear_quest_id: Option<String>,
    character_bind: bool,
    #[serde(rename = "ID")]
    id: Option<i32>,
    restriction_reset_time: Option<String>,
    restriction_period_day: i32,
    restriction_count: i32,
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRepair {
    enhance: Vec<(i32, i32)>,
    quality: Vec<(i32, i32)>,
    enchant: Vec<(i32, i32, i32)>,
    equip_class: Vec<(Option<String>, i32)>,
    equip_item_class: Vec<Option<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStoryLine {
    info: RawInfo,
    tree: Option<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    manifest: Manifest,
    features: Vec<Feature>,
    defaults: Vec<RawDefault>,
    storylines: Vec<RawStoryLine>,
    shops: Vec<RawShop>,
    repair: RawRepair,
    #[serde(default)]
    quests: Option<crate::domain::quest::QuestTables>,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// [name, [[attr, value]...], text, [children], tail], depth-bounded.
fn element(v: &Value, depth: usize) -> Result<Element, MinimumError> {
    let a = v
        .as_array()
        .filter(|a| a.len() == 5 && depth < 256)
        .ok_or_else(fail)?;
    let text = |x: &Value| x.as_str().map(str::to_owned).ok_or_else(fail);
    let attributes = a[1]
        .as_array()
        .ok_or_else(fail)?
        .iter()
        .map(|p| {
            let p = p.as_array().filter(|p| p.len() == 2).ok_or_else(fail)?;
            Ok((text(&p[0])?, text(&p[1])?))
        })
        .collect::<Result<Vec<_>, MinimumError>>()?;
    let children = a[3]
        .as_array()
        .ok_or_else(fail)?
        .iter()
        .map(|c| element(c, depth + 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Element {
        name: text(&a[0])?,
        attributes,
        text: text(&a[2])?,
        children,
        tail: text(&a[4])?,
    })
}

pub struct Profile {
    database_sha256: String,
    config_sha256: String,
    pub scripts: NpcScripts,
    /// FeatureMatrix subset referenced by the scripts (same region resolution).
    pub policy: crate::domain::feature_policy::FeaturePolicy,
    /// ShopInfo rows in table order; prices are resolved per arithmetic profile.
    pub shop_rows: Vec<crate::domain::shop::ShopRow>,
    /// Repair dictionaries (RepairPriceRate applied from `policy`).
    pub repair: crate::domain::repair::RepairTables,
    /// EquipItemInfo.EquipClass aligned with the catalog equips rows.
    pub equip_item_class: Vec<Option<String>>,
    /// QuestContents after Load (absent from profiles exported before the quest tables).
    pub quests: Option<crate::domain::quest::QuestContents>,
}
impl Profile {
    pub fn read(reader: impl Read) -> Result<Self, MinimumError> {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err(fail());
        }
        let export: Export = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        if export.format_version != 1
            || export.source != "heroes.npc-scripts"
            || export.defaults.len() != export.manifest.defaults
            || export.storylines.len() != export.manifest.storylines
            || export.shops.len() != export.manifest.shops
            || export.shops.len() > 100_000
            || export.manifest.coerced_shop_integers > export.shops.len() * 4
            || export.manifest.scripts_sha256.len() != 64
            || export.manifest.coerced_integers > export.storylines.len() * 9
        {
            return Err(fail());
        }
        let defaults = export
            .defaults
            .iter()
            .map(|d| {
                if d.path.is_empty() {
                    return Err(fail());
                }
                Ok((d.feature.clone(), element(&d.tree, 0)?))
            })
            .collect::<Result<Vec<_>, MinimumError>>()?;
        let mut trees = Vec::with_capacity(export.storylines.len());
        for s in &export.storylines {
            let r = &s.info;
            let _ = (&r.story_line_name, &r.stage, r.season);
            trees.push((
                StoryLineInfo {
                    id: r.story_line_id.clone(),
                    xml_path: r.xml_path.clone(),
                    npc: r.npc.clone(),
                    category: r.category.clone(),
                    reward_gold: r.reward_gold,
                    reward_exp: r.reward_exp,
                    reward_ap: r.reward_ap,
                    reward_title: r.reward_title,
                    reward_item: r.reward_item.clone(),
                    reward_item_count: r.reward_item_count,
                    reward_skill: r.reward_skill.clone(),
                    reward_skill_rank: r.reward_skill_rank,
                    auto_give_level: r.auto_give_level,
                    class_restriction: r.class_restriction,
                    auto_process: r.auto_process,
                    auto_reveal: r.auto_reveal,
                    episode: r.episode.clone(),
                    feature: r.feature.clone(),
                },
                s.tree.as_ref().map(|t| element(t, 0)).transpose()?,
            ));
        }
        let scripts = NpcScripts::load(
            &defaults
                .iter()
                .map(|(feature, root)| DefaultScript { feature, root })
                .collect::<Vec<_>>(),
            trees.iter().map(|(i, t)| (i.clone(), t.as_ref())).collect(),
        )
        .map_err(|_| fail())?;
        let shop_rows = export
            .shops
            .into_iter()
            .map(|r| {
                // AppearQuestID, ID, reset time and period only matter for restrictions.
                let _ = (
                    &r.appear_quest_id,
                    r.id,
                    &r.restriction_reset_time,
                    r.restriction_period_day,
                );
                crate::domain::shop::ShopRow {
                    shop_id: r.shop_id,
                    order: r.order,
                    item_class: r.item_class,
                    count: r.count,
                    price_item_class: r.price_item_class,
                    price_count: r.price_count,
                    character_bind: r.character_bind,
                    restriction_count: r.restriction_count,
                    feature: r.feature,
                }
            })
            .collect();
        let policy = crate::domain::feature_policy::FeaturePolicy::npc_source(
            export.features.into_iter().map(|f| (f.key, f.value)),
        )
        .map_err(|_| fail())?;
        // ToDictionary keys: a duplicate throws in the source loader.
        fn unique<K: Ord, V>(
            rows: impl IntoIterator<Item = (K, V)>,
        ) -> Result<std::collections::BTreeMap<K, V>, MinimumError> {
            let mut out = std::collections::BTreeMap::new();
            for (k, v) in rows {
                if out.insert(k, v).is_some() {
                    return Err(fail());
                }
            }
            Ok(out)
        }
        let raw = export.repair;
        let repair = crate::domain::repair::RepairTables {
            enhance: unique(raw.enhance)?,
            quality: unique(raw.quality)?,
            enchant: unique(raw.enchant.into_iter().map(|(l, p, s)| (l, (p, s))))?,
            equip_class: unique(
                raw.equip_class
                    .into_iter()
                    .map(|(c, p)| Ok((c.ok_or_else(fail)?.to_lowercase(), p)))
                    .collect::<Result<Vec<_>, MinimumError>>()?,
            )?,
            rate: policy.get_integer("RepairPriceRate").unwrap_or(0),
        };
        let equip_item_class = raw.equip_item_class;
        let quests = export
            .quests
            .map(|t| crate::domain::quest::QuestContents::load(t, &policy))
            .transpose()
            .map_err(|_| fail())?;
        Ok(Self {
            quests,
            repair,
            equip_item_class,
            shop_rows,
            policy,
            database_sha256: export.manifest.database_sha256,
            config_sha256: export.manifest.config_sha256,
            scripts,
        })
    }
    pub fn bind(&self, catalog: &Catalog) -> Result<(), MinimumError> {
        if self.equip_item_class.len() != catalog.equips.len() {
            return Err(fail());
        }
        if self.database_sha256 != catalog.manifest.database_sha256
            || self.config_sha256 != catalog.manifest.config_sha256
        {
            return Err(fail());
        }
        // Keys present in both projections resolve to the same value.
        let own: std::collections::BTreeMap<_, _> = self.policy.entries().collect();
        if catalog
            .policy
            .entries()
            .any(|(k, v)| own.get(k).is_some_and(|o| *o != v))
        {
            return Err(fail());
        }
        Ok(())
    }
}
