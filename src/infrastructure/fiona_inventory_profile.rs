//! Source-bound normal Fiona defaults. This does not enable TCP Create.
use crate::{
    domain::{
        feature_policy::FeaturePolicy,
        initial_inventory::{InitialRequest, InventoryError},
        legacy_user_data::LegacyBytes,
    },
    error::Error,
    infrastructure::{
        item_catalog::{self, Package as Catalog},
        item_colors::ColorCatalog,
        item_stats::StatsCatalog,
        user_export::{PrivateText, nullable},
    },
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
struct RawClass {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    #[serde(deserialize_with = "nullable")]
    category_utf16: Option<PrivateText>,
    unique: i32,
    #[serde(deserialize_with = "nullable")]
    expire_in: Option<i32>,
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
struct Export {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    constants_sha256: String,
    gold_utf16: PrivateText,
    tircoin_utf16: PrivateText,
    features: Vec<Feature>,
    classes: Vec<RawClass>,
    requests: Vec<RawRequest>,
    variable_keys: Vec<Option<PrivateText>>,
}
struct Class {
    key: Option<LegacyBytes<1024, true>>,
    feature: Option<String>,
    category: Option<LegacyBytes<1024, true>>,
    unique: i32,
    expire: Option<i32>,
}
struct Request {
    key: Option<LegacyBytes<1024, true>>,
    num: i32,
    feature: Option<String>,
}
pub struct Profile {
    db: String,
    config: String,
    pub policy: FeaturePolicy,
    classes: Vec<Class>,
    requests: Vec<Request>,
    variables: Vec<Option<LegacyBytes<1024, true>>>,
    gold: LegacyBytes<1024, true>,
    tircoin: LegacyBytes<1024, true>,
}
impl Profile {
    pub fn read(reader: impl Read) -> Result<Self, Error> {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::FionaInventoryProfile)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(Error::FionaInventoryProfile);
        }
        let raw: Export =
            serde_json::from_slice(&bytes).map_err(|_| Error::FionaInventoryProfile)?;
        let hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if raw.format_version != 1
            || raw.source != "heroesContents.fiona-inventory"
            || !hash(&raw.database_sha256)
            || !hash(&raw.config_sha256)
            || !hash(&raw.constants_sha256)
            || raw.classes.len() > 50000
            || raw.requests.len() > 4096
            || raw.variable_keys.len() > 50000
        {
            return Err(Error::FionaInventoryProfile);
        }
        let classes = raw
            .classes
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                if r.source_index != i || r.feature.as_ref().is_some_and(|s| s.len() > 512) {
                    return Err(Error::FionaInventoryProfile);
                }
                Ok(Class {
                    key: item_catalog::opaque(r.item_utf16)?,
                    feature: r.feature,
                    category: item_catalog::opaque(r.category_utf16)?,
                    unique: r.unique,
                    expire: r.expire_in,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let requests = raw
            .requests
            .into_iter()
            .map(|r| {
                if r.feature.as_ref().is_some_and(|s| s.len() > 512) {
                    return Err(Error::FionaInventoryProfile);
                }
                Ok(Request {
                    key: item_catalog::opaque(r.item_utf16)?,
                    num: r.num,
                    feature: r.feature,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let variables = raw
            .variable_keys
            .into_iter()
            .map(item_catalog::opaque)
            .collect::<Result<Vec<_>, _>>()?;
        let gold =
            item_catalog::opaque(Some(raw.gold_utf16))?.ok_or(Error::FionaInventoryProfile)?;
        let tircoin =
            item_catalog::opaque(Some(raw.tircoin_utf16))?.ok_or(Error::FionaInventoryProfile)?;
        for key in [&gold, &tircoin] {
            if key.as_bytes().is_empty()
                || key.as_bytes().len() > 100
                || key
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .any(|b| b[1] != 0 || !(b[0] == b'_' || b[0].is_ascii_alphanumeric()))
            {
                return Err(Error::FionaInventoryProfile);
            }
        }
        let policy = FeaturePolicy::new(raw.features.into_iter().map(|r| (r.key, r.value)))?;
        Ok(Self {
            db: raw.database_sha256,
            config: raw.config_sha256,
            classes,
            requests,
            variables,
            gold,
            tircoin,
            policy,
        })
    }
    pub fn bind(&self, catalog: &Catalog) -> Result<(), Error> {
        if !self
            .db
            .eq_ignore_ascii_case(&catalog.manifest.database_sha256)
            || !self
                .config
                .eq_ignore_ascii_case(&catalog.manifest.config_sha256)
            || self.classes.len() != catalog.classes.len()
        {
            return Err(Error::FionaInventoryProfile);
        }
        for (a, b) in self.classes.iter().zip(&catalog.classes) {
            if a.key.as_ref().map(LegacyBytes::as_bytes)
                != b.item_class.as_ref().map(LegacyBytes::as_bytes)
                || a.feature != b.feature
            {
                return Err(Error::FionaInventoryProfile);
            }
        }
        let own = self.policy.entries().collect::<BTreeMap<_, _>>();
        if catalog
            .policy
            .entries()
            .any(|(k, v)| own.get(k) != Some(&v))
        {
            return Err(Error::FionaInventoryProfile);
        }
        for k in [
            "BundleExceed",
            "MaxDurabilityLimit",
            "CheckEquipItemInfoVaild",
            "AvatarSynthesis_V2",
            "ingame_cashshop_tircoin_system",
        ] {
            if !own.contains_key(k) {
                return Err(Error::FionaInventoryProfile);
            }
        }
        Ok(())
    }
    pub fn requests<'a>(
        &'a self,
        catalog: &'a Catalog,
        stats: &'a StatsCatalog<'a>,
        colors: &'a ColorCatalog<'a>,
    ) -> Result<Vec<InitialRequest<'a>>, Error> {
        self.bind(catalog)?;
        let mut grouped: Vec<(usize, i32)> = Vec::new();
        for (i, r) in self.requests.iter().enumerate() {
            if !self.policy.is_enabled(r.feature.as_deref())? {
                continue;
            }
            let key = r
                .key
                .as_ref()
                .ok_or(Error::FionaInventoryProfile)?
                .as_bytes();
            if let Some((_, count)) = grouped.iter_mut().find(|(j, _)| {
                self.requests[*j].key.as_ref().map(LegacyBytes::as_bytes) == Some(key)
            }) {
                *count = count.wrapping_add(r.num);
            } else {
                grouped.push((i, r.num));
            }
        }
        let bracelet = "BRACELET"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        grouped
            .into_iter()
            .map(|(i, count)| {
                let key = self.requests[i]
                    .key
                    .as_ref()
                    .ok_or(Error::FionaInventoryProfile)?
                    .as_bytes();
                let units = key
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes(*b))
                    .collect::<Vec<_>>();
                // Plain ASCII names have unambiguous en-US prefix behavior, no parser fallback.
                if units.is_empty()
                    || units
                        .iter()
                        .any(|&u| !(u == 95 || u <= 127 && (u as u8).is_ascii_alphanumeric()))
                    || units.starts_with(&"avatar".encode_utf16().collect::<Vec<_>>())
                    || self.variables.iter().flatten().any(|k| k.as_bytes() == key)
                {
                    return Err(Error::FionaInventoryProfile);
                }
                if self
                    .policy
                    .is_enabled(Some("ingame_cashshop_tircoin_system"))?
                    && key
                        .windows(self.tircoin.as_bytes().len().max(1))
                        .any(|part| part == self.tircoin.as_bytes())
                {
                    return Err(Error::FionaInventoryProfile);
                }
                let item = stats
                    .catalog()
                    .get(Some(key))
                    .ok_or(Error::FionaInventoryProfile)?;
                let meta = self
                    .classes
                    .get(item.source_index)
                    .ok_or(Error::FionaInventoryProfile)?;
                if meta
                    .category
                    .as_ref()
                    .is_some_and(|v| v.as_bytes() == bracelet)
                {
                    return Err(Error::FionaInventoryProfile);
                }
                let base = stats.get(Some(key)).ok_or(Error::FionaInventoryProfile)?;
                let names = if item.binding.equippable {
                    colors.get(Some(key)).ok_or(Error::FionaInventoryProfile)?
                } else {
                    [None; 3]
                };
                Ok(InitialRequest {
                    class: key,
                    count,
                    max_stack: item.max_stack,
                    unique: meta.unique,
                    storage: item.storage,
                    expire_in: meta.expire,
                    equipment: item.binding.equippable,
                    base,
                    materials: names,
                    gold: key == self.gold.as_bytes(),
                })
            })
            .collect()
    }
    pub fn limits(&self) -> Result<(i32, i32), Error> {
        Ok((
            self.policy.get_integer("MaxDurabilityLimit")?,
            self.policy.get_integer("BundleExceed")?,
        ))
    }
    pub(crate) fn category(&self, index: usize) -> Option<&[u8]> {
        self.classes
            .get(index)?
            .category
            .as_ref()
            .map(LegacyBytes::as_bytes)
    }
    pub fn candidates(&self) -> usize {
        self.requests.len()
    }
}
impl From<InventoryError> for Error {
    fn from(value: InventoryError) -> Self {
        Self::InitialInventory(value)
    }
}
