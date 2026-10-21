//! Private ItemClassInfo economy projection (Unique, ExpireIn, prices, requirements),
//! bound by row position to the effective item catalog. Never opens the source SQLite.
use super::{
    item_catalog::{self, Package},
    minimum_character::MinimumError,
    user_export::{PrivateText, nullable},
};
use crate::domain::legacy_user_data::LegacyBytes;
use serde::Deserialize;
use std::io::Read;
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    database_sha256: String,
    config_sha256: String,
    class_rows: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClass {
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    category_utf16: Option<PrivateText>,
    unique: i32,
    #[serde(deserialize_with = "nullable")]
    expire_in: Option<i32>,
    price: i32,
    sell_price: i32,
    required_level: i32,
    class_restriction: i32,
    use_in_town: i32,
    indestructible: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    manifest: Manifest,
    classes: Vec<RawClass>,
}
/// One ItemClassInfo row; the effective row is chosen by the catalog's source_index.
pub struct EconomyClass {
    item_class: Option<LegacyBytes<1024, true>>,
    pub category: Option<LegacyBytes<1024, true>>,
    pub unique: i32,
    pub expire_in: Option<i32>,
    pub price: i32,
    pub sell_price: i32,
    pub required_level: i32,
    pub class_restriction: i32,
    pub use_in_town: bool,
    /// ItemClassInfo.Indestructible (1 cannot be sold).
    pub indestructible: i32,
}
pub struct Profile {
    manifest: Manifest,
    classes: Vec<EconomyClass>,
}
impl Profile {
    pub fn read(reader: impl Read) -> Result<Self, MinimumError> {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| MinimumError::Conflict)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(MinimumError::Conflict);
        }
        let export: Export = serde_json::from_slice(&bytes).map_err(|_| MinimumError::Conflict)?;
        if export.format_version != 2
            || export.source != "heroesContents.item-economy"
            || export.classes.len() != export.manifest.class_rows
            || export.classes.len() > 50_000
        {
            return Err(MinimumError::Conflict);
        }
        let classes = export
            .classes
            .into_iter()
            .map(|r| {
                Ok(EconomyClass {
                    item_class: item_catalog::opaque(r.item_utf16)?,
                    category: item_catalog::opaque(r.category_utf16)?,
                    unique: r.unique,
                    expire_in: r.expire_in,
                    price: r.price,
                    sell_price: r.sell_price,
                    required_level: r.required_level,
                    class_restriction: r.class_restriction,
                    use_in_town: r.use_in_town != 0,
                    indestructible: r.indestructible,
                })
            })
            .collect::<Result<Vec<_>, crate::error::Error>>()?;
        Ok(Self {
            manifest: export.manifest,
            classes,
        })
    }
    /// Same source files and the same row sequence as the bound catalog.
    pub fn bind(&self, catalog: &Package) -> Result<(), MinimumError> {
        let same_rows = self.classes.len() == catalog.classes.len()
            && self.classes.iter().zip(&catalog.classes).all(|(e, c)| {
                e.item_class.as_ref().map(LegacyBytes::as_bytes)
                    == c.item_class.as_ref().map(LegacyBytes::as_bytes)
            });
        if !same_rows
            || self.manifest.database_sha256 != catalog.manifest.database_sha256
            || self.manifest.config_sha256 != catalog.manifest.config_sha256
        {
            return Err(MinimumError::Conflict);
        }
        Ok(())
    }
    pub fn get(&self, source_index: usize) -> Option<&EconomyClass> {
        self.classes.get(source_index)
    }
}
