//! Readonly skill rank/constraint subset bound to the item catalog content hashes,
//! its feature policy and the recovered implementation sources.
use super::{
    item_catalog::Package as Catalog,
    minimum_character::MinimumError,
    user_export::{PrivateText, nullable},
};
use crate::domain::skill_list::{RankInfo, SkillContent};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Skill {
    source_index: usize,
    skill_utf16: PrivateText,
    class_restriction: i32,
    use_type: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rank {
    source_index: usize,
    skill_utf16: PrivateText,
    rank: i32,
    level: i32,
    required_ap: i32,
    require_unlock: bool,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Constraint {
    source_index: usize,
    skill_utf16: PrivateText,
    rank: i32,
    required_skill_utf16: PrivateText,
    required_rank: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    implementations: BTreeMap<String, String>,
    skills: Vec<Skill>,
    ranks: Vec<Rank>,
    constraints: Vec<Constraint>,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn text(value: &PrivateText) -> Result<String, MinimumError> {
    let key = super::item_catalog::opaque(Some(PrivateText(value.0.clone())))?.ok_or_else(fail)?;
    let bytes = key.as_bytes();
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
fn ordered<T>(rows: &[T], index: impl Fn(&T) -> usize) -> bool {
    rows.windows(2).all(|w| index(&w[0]) < index(&w[1]))
}

impl Profile {
    pub fn read(input: impl Read) -> Result<Self, MinimumError> {
        let mut bytes = Zeroizing::new(Vec::new());
        input
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(fail());
        }
        let p: Self = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        let sources: serde_json::Value =
            serde_json::from_str(include_str!("../domain/skill_list_sources.json"))
                .map_err(|_| fail())?;
        let expected: BTreeMap<_, _> = sources
            .as_object()
            .ok_or_else(fail)?
            .iter()
            .map(|(k, v)| (k.clone(), v["sha256"].as_str().unwrap_or("").to_owned()))
            .collect();
        if p.format_version != 2
            || p.source != "heroesContents.skill-ranks"
            || p.implementations != expected
            || p.skills.is_empty()
            || p.skills.len() > 4096
            || p.ranks.len() > 65536
            || p.constraints.len() > 65536
            || !ordered(&p.skills, |s| s.source_index)
            || !ordered(&p.ranks, |r| r.source_index)
            || !ordered(&p.constraints, |c| c.source_index)
        {
            return Err(fail());
        }
        p.contents_unbound()?;
        Ok(p)
    }
    fn contents_unbound(&self) -> Result<BTreeMap<String, SkillContent>, MinimumError> {
        let mut out: BTreeMap<String, SkillContent> = BTreeMap::new();
        for s in &self.skills {
            // SkillInfo is a ToDictionary: duplicate IDs throw in the source.
            let entry = out.entry(text(&s.skill_utf16)?).or_default();
            if entry
                .class_restriction
                .replace(s.class_restriction)
                .is_some()
            {
                return Err(fail());
            }
            entry.use_type = Some(s.use_type);
        }
        for c in &self.constraints {
            out.entry(text(&c.skill_utf16)?)
                .or_default()
                .constraints
                .entry(c.rank)
                .or_default()
                .push((text(&c.required_skill_utf16)?, c.required_rank));
        }
        Ok(out)
    }
    /// Contents with each rank's feature evaluated against the bound catalog policy.
    pub fn contents(
        &self,
        catalog: &Catalog,
    ) -> Result<BTreeMap<String, SkillContent>, MinimumError> {
        if self.database_sha256 != catalog.manifest.database_sha256
            || self.config_sha256 != catalog.manifest.config_sha256
        {
            return Err(fail());
        }
        let mut out = self.contents_unbound()?;
        for r in &self.ranks {
            let feature_enabled = catalog.policy.is_enabled(r.feature.as_deref())?;
            // ToDoubleDictionary keeps the last duplicate (SkillID, Rank).
            out.entry(text(&r.skill_utf16)?).or_default().ranks.insert(
                r.rank,
                RankInfo {
                    level: r.level,
                    required_ap: r.required_ap,
                    require_unlock: r.require_unlock,
                    feature_enabled,
                },
            );
        }
        Ok(out)
    }
}
