//! Private projection reader. Never opens the source SQLite or persists catalog state.
use crate::{
    domain::{feature_policy::FeaturePolicy, legacy_user_data::LegacyBytes},
    error::Error,
    infrastructure::user_export::{PrivateText, nullable},
};
use serde::Deserialize;
use std::io::Read;
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogManifest {
    pub database_sha256: String,
    pub config_sha256: String,
    pub region: String,
    #[serde(deserialize_with = "nullable")]
    pub override_region: Option<String>,
    pub equip_rows: usize,
    pub class_rows: usize,
    pub extra_rows: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feature {
    key: String,
    #[serde(deserialize_with = "nullable")]
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEquip {
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClass {
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    max_stack: i32,
    storage: u8,
    #[serde(deserialize_with = "nullable")]
    bind: Option<String>,
    #[serde(deserialize_with = "nullable")]
    trade_on_bind: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    manifest: CatalogManifest,
    features: Vec<Feature>,
    equips: Vec<RawEquip>,
    classes: Vec<RawClass>,
    extras: Vec<RawEquip>,
}
pub struct Equip {
    pub item_class: Option<LegacyBytes<1024, true>>,
    pub feature: Option<String>,
}
pub struct Class {
    pub item_class: Option<LegacyBytes<1024, true>>,
    pub feature: Option<String>,
    pub max_stack: i32,
    pub storage: u8,
    pub bind: Option<String>,
    pub trade_on_bind: Option<String>,
}
pub struct Package {
    pub manifest: CatalogManifest,
    pub policy: FeaturePolicy,
    pub equips: Vec<Equip>,
    pub classes: Vec<Class>,
    pub extras: Vec<Equip>,
}
pub(crate) fn opaque(value: Option<PrivateText>) -> Result<Option<LegacyBytes<1024, true>>, Error> {
    value
        .as_ref()
        .map(|v| {
            if v.0.len() > 2048 || !v.0.len().is_multiple_of(4) {
                return Err(Error::ItemCatalogExport);
            }
            let bytes =
                v.0.as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| {
                        let a = (p[0] as char)
                            .to_digit(16)
                            .ok_or(Error::ItemCatalogExport)?;
                        let b = (p[1] as char)
                            .to_digit(16)
                            .ok_or(Error::ItemCatalogExport)?;
                        Ok((a * 16 + b) as u8)
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
            LegacyBytes::from_bytes(bytes).map_err(|_| Error::ItemCatalogExport)
        })
        .transpose()
}
fn expression(value: &Option<String>) -> Result<(), Error> {
    if value.as_ref().is_some_and(|v| v.len() > 512) {
        return Err(Error::ItemCatalogExport);
    }
    Ok(())
}
fn equips(rows: Vec<RawEquip>) -> Result<Vec<Equip>, Error> {
    rows.into_iter()
        .map(|r| {
            expression(&r.feature)?;
            Ok(Equip {
                item_class: opaque(r.item_utf16)?,
                feature: r.feature,
            })
        })
        .collect()
}
pub fn read_package(reader: impl Read) -> Result<Package, Error> {
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::ItemCatalogExport)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(Error::ItemCatalogExport);
    }
    let raw: Export = serde_json::from_slice(&bytes).map_err(|_| Error::ItemCatalogExport)?;
    let hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    let region = |s: &str| {
        !s.is_empty()
            && s.len() <= 64
            && s.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
    };
    if raw.format_version != 1
        || raw.source != "heroesContents.item-catalog-projection"
        || raw.equips.len() > 50000
        || raw.classes.len() > 50000
        || raw.extras.len() > 50000
        || raw.features.len() > 128
        || raw.manifest.equip_rows != raw.equips.len()
        || raw.manifest.class_rows != raw.classes.len()
        || raw.manifest.extra_rows != raw.extras.len()
        || !hash(&raw.manifest.database_sha256)
        || !hash(&raw.manifest.config_sha256)
        || !region(&raw.manifest.region)
        || raw
            .manifest
            .override_region
            .as_ref()
            .is_some_and(|r| !region(r))
    {
        return Err(Error::ItemCatalogExport);
    }
    let policy = FeaturePolicy::new(raw.features.into_iter().map(|f| (f.key, f.value)))?;
    let classes = raw
        .classes
        .into_iter()
        .map(|r| {
            expression(&r.feature)?;
            expression(&r.bind)?;
            expression(&r.trade_on_bind)?;
            Ok(Class {
                item_class: opaque(r.item_utf16)?,
                feature: r.feature,
                max_stack: r.max_stack,
                storage: r.storage,
                bind: r.bind,
                trade_on_bind: r.trade_on_bind,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(Package {
        manifest: raw.manifest,
        policy,
        equips: equips(raw.equips)?,
        classes,
        extras: equips(raw.extras)?,
    })
}
