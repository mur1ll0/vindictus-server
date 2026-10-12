//! Bounded reader of a private, versioned SQLite content subset; it never opens originals.
use crate::{
    domain::{
        dormant_rewards::{DormantReward, RewardCharacter},
        feature_policy::FeaturePolicy,
        legacy_user_data::LegacyBytes,
    },
    error::Error,
    infrastructure::user_export::{PrivateText, hex, integer, nullable},
};
use serde::Deserialize;
use std::io::Read;
use zeroize::Zeroizing;
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyManifest {
    pub database_sha256: String,
    pub config_sha256: String,
    pub region: String,
    #[serde(deserialize_with = "nullable")]
    pub override_region: Option<String>,
    pub game_code: i32,
    pub server_code: i32,
    pub catalog_rows: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureRow {
    key: String,
    #[serde(deserialize_with = "nullable")]
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RewardRow {
    item_utf16: PrivateText,
    num: i32,
    min_level: i32,
    max_level: i32,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    manifest: PolicyManifest,
    features: Vec<FeatureRow>,
    rewards: Vec<RewardRow>,
}
pub struct RewardPolicyPackage {
    pub manifest: PolicyManifest,
    pub policy: FeaturePolicy,
    pub rewards: Vec<DormantReward>,
}
pub fn read_policy(reader: impl Read) -> Result<RewardPolicyPackage, Error> {
    let mut input = Zeroizing::new(Vec::new());
    reader
        .take(512 * 1024 + 1)
        .read_to_end(&mut input)
        .map_err(|_| Error::RewardPolicyExport)?;
    if input.len() > 512 * 1024 {
        return Err(Error::RewardPolicyExport);
    }
    let raw: Export = serde_json::from_slice(&input).map_err(|_| Error::RewardPolicyExport)?;
    let valid_hash =
        |value: &str| value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit());
    let valid_region = |value: &str| {
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
    };
    if raw.format_version != 1
        || raw.source != "heroesContents.dormant-reward-policy"
        || raw.rewards.len() > 1000
        || raw.manifest.catalog_rows != raw.rewards.len()
        || !valid_hash(&raw.manifest.database_sha256)
        || !valid_hash(&raw.manifest.config_sha256)
        || !valid_region(&raw.manifest.region)
        || raw
            .manifest
            .override_region
            .as_deref()
            .is_some_and(|v| !valid_region(v))
    {
        return Err(Error::RewardPolicyExport);
    }
    let policy = FeaturePolicy::new(raw.features.into_iter().map(|v| (v.key, v.value)))?;
    let rewards = raw
        .rewards
        .into_iter()
        .map(|row| {
            if row
                .feature
                .as_ref()
                .is_some_and(|v| v.encode_utf16().count() > 50)
            {
                return Err(Error::RewardPolicyExport);
            }
            Ok(DormantReward {
                item_class: LegacyBytes::from_bytes(hex(row.item_utf16, 100)?)?,
                num: row.num,
                min_level: row.min_level,
                max_level: row.max_level,
                feature: row.feature,
            })
        })
        .collect::<Result<Vec<_>, Error>>()
        .map_err(|_| Error::RewardPolicyExport)?;
    Ok(RewardPolicyPackage {
        manifest: raw.manifest,
        policy,
        rewards,
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterRow {
    id: PrivateText,
    level: i32,
    #[serde(deserialize_with = "nullable")]
    name_utf16: Option<PrivateText>,
}
pub struct InspectionCharacter {
    pub id: i64,
    pub level: i32,
    pub name: Option<LegacyBytes<50, true>>,
}
impl InspectionCharacter {
    pub fn as_character(&self) -> RewardCharacter<'_> {
        RewardCharacter {
            id: self.id,
            level: self.level,
            name: self.name.as_ref(),
        }
    }
}
pub fn read_characters(reader: impl Read) -> Result<Vec<InspectionCharacter>, Error> {
    let mut input = Zeroizing::new(Vec::new());
    reader
        .take(512 * 1024 + 1)
        .read_to_end(&mut input)
        .map_err(|_| Error::RewardPolicyExport)?;
    if input.len() > 512 * 1024 {
        return Err(Error::RewardPolicyExport);
    }
    let rows: Vec<CharacterRow> =
        serde_json::from_slice(&input).map_err(|_| Error::RewardPolicyExport)?;
    if rows.len() > 1000 {
        return Err(Error::RewardPolicyExport);
    }
    rows.into_iter()
        .map(|r| {
            Ok(InspectionCharacter {
                id: integer(r.id)?,
                level: r.level,
                name: r
                    .name_utf16
                    .map(|v| Ok::<_, Error>(LegacyBytes::from_bytes(hex(v, 100)?)?))
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()
        .map_err(|_| Error::RewardPolicyExport)
}
