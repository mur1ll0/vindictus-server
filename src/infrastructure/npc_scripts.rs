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
        let policy = crate::domain::feature_policy::FeaturePolicy::npc_source(
            export.features.into_iter().map(|f| (f.key, f.value)),
        )
        .map_err(|_| fail())?;
        Ok(Self {
            policy,
            database_sha256: export.manifest.database_sha256,
            config_sha256: export.manifest.config_sha256,
            scripts,
        })
    }
    pub fn bind(&self, catalog: &Catalog) -> Result<(), MinimumError> {
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
