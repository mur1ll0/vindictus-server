//! Per-class initial contents (scripts/content/export_class_initial.py): for each BaseCharacter
//! of the CustomizeCharacter switch, the DefaultItemInfo requests, the INNERARMOR request and the
//! CharacterClassInfo rows. Request features are evaluated once, in the exported region.
use super::{
    item_catalog::{self, Package as Catalog},
    user_export::{PrivateText, nullable},
};
use crate::{
    domain::{feature_policy::FeaturePolicy, legacy_user_data::LegacyBytes},
    error::Error,
};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feature {
    key: String,
    #[serde(deserialize_with = "nullable")]
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    num: i32,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClass {
    character_class: u8,
    requests: Vec<RawRequest>,
    inner_item_utf16: PrivateText,
    inner_colors: [i32; 3],
    class_stats: Vec<[i32; 7]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    implementation_sha256: String,
    features: Vec<Feature>,
    classes: Vec<RawClass>,
}
/// One DefaultItemInfo row of a class, its feature already evaluated.
pub struct ClassRequest {
    pub key: Option<LegacyBytes<1024, true>>,
    pub num: i32,
    pub enabled: bool,
}
pub struct ClassInitial {
    pub requests: Vec<ClassRequest>,
    pub inner: LegacyBytes<1024, true>,
    pub inner_colors: [i32; 3],
    /// CharacterClassInfo by level: STR, DEX, INT, WILL, HP, STAMINA.
    pub class_stats: BTreeMap<i32, [i32; 6]>,
}
pub struct Package {
    db: String,
    config: String,
    pub classes: BTreeMap<u8, ClassInitial>,
}
fn fail() -> Error {
    Error::FionaInventoryProfile
}
impl Package {
    pub fn read(reader: impl Read) -> Result<Self, Error> {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(fail());
        }
        let raw: Export = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        let hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if raw.format_version != 1
            || raw.source != "heroesContents.class-initial"
            || !hash(&raw.database_sha256)
            || !hash(&raw.config_sha256)
            || !hash(&raw.implementation_sha256)
            || raw.features.len() > 256
            || raw.classes.len() > 10
        {
            return Err(fail());
        }
        let policy = FeaturePolicy::new(raw.features.into_iter().map(|f| (f.key, f.value)))?;
        let mut classes = BTreeMap::new();
        for c in raw.classes {
            if c.character_class > 9 || c.requests.len() > 4096 || c.class_stats.len() > 4096 {
                return Err(fail());
            }
            let requests = c
                .requests
                .into_iter()
                .map(|r| {
                    if r.feature.as_ref().is_some_and(|s| s.len() > 512) {
                        return Err(fail());
                    }
                    Ok(ClassRequest {
                        key: item_catalog::opaque(r.item_utf16)?,
                        num: r.num,
                        enabled: policy.is_enabled(r.feature.as_deref())?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let mut class_stats = BTreeMap::new();
            for [level, stats @ ..] in c.class_stats {
                if class_stats.insert(level, stats).is_some() {
                    return Err(fail());
                }
            }
            // GetCharacterStats falls back to level 1.
            if !class_stats.contains_key(&1) {
                return Err(fail());
            }
            let initial = ClassInitial {
                requests,
                inner: item_catalog::opaque(Some(c.inner_item_utf16))?.ok_or_else(fail)?,
                inner_colors: c.inner_colors,
                class_stats,
            };
            if classes.insert(c.character_class, initial).is_some() {
                return Err(fail());
            }
        }
        Ok(Self {
            db: raw.database_sha256,
            config: raw.config_sha256,
            classes,
        })
    }
    pub fn bind(&self, catalog: &Catalog) -> Result<(), Error> {
        if !self
            .db
            .eq_ignore_ascii_case(&catalog.manifest.database_sha256)
            || !self
                .config
                .eq_ignore_ascii_case(&catalog.manifest.config_sha256)
        {
            return Err(fail());
        }
        Ok(())
    }
}
