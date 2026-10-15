//! Bounded private ItemStat sidecar, pinned to the existing source catalog snapshot.
use crate::{
    application::item_catalog_inspection,
    domain::{item_catalog::ItemCatalog, item_stats::ItemStat, legacy_user_data::LegacyBytes},
    error::Error,
    infrastructure::{
        item_catalog::{self, Package as CatalogPackage},
        user_export::{PrivateText, nullable},
    },
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read};
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRow {
    source_index: usize,
    #[serde(deserialize_with = "nullable")]
    item_utf16: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
    stats: [i32; 18],
    max_durability: i32,
    weight_bits: PrivateText,
    armor_hp: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    rows: Vec<RawRow>,
}
struct Row {
    key: Option<LegacyBytes<1024, true>>,
    feature: Option<String>,
    stats: [i32; 18],
    max: i32,
    weight: f64,
    armor: i32,
}
pub struct Package {
    db_hash: String,
    config_hash: String,
    rows: Vec<Row>,
}
/// Original stats dictionary is built from effective equipment before class selection.
/// Known non-equipment classes use the source ItemStat.Empty; unknown classes are absent.
pub struct StatsCatalog<'a> {
    catalog: ItemCatalog<'a>,
    stats: BTreeMap<&'a [u8], ItemStat>,
    empty: ItemStat,
}
#[derive(serde::Serialize)]
pub struct Inspection {
    pub equipment_rows: usize,
    pub effective_equipment: usize,
    pub effective_classes: usize,
    pub classes_with_equipment_stats: usize,
    pub projection_sha256: String,
    pub source_rows_projection_sha256: String,
    pub source_catalog_matched: bool,
    pub database_mutations: bool,
    pub character_creation_available: bool,
}
impl Package {
    pub fn read(reader: impl Read) -> Result<Self, Error> {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::ItemStatsExport)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(Error::ItemStatsExport);
        }
        let raw: Export = serde_json::from_slice(&bytes).map_err(|_| Error::ItemStatsExport)?;
        let hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if raw.format_version != 1
            || raw.source != "heroesContents.item-stats"
            || raw.rows.len() > 50000
            || !hash(&raw.database_sha256)
            || !hash(&raw.config_sha256)
        {
            return Err(Error::ItemStatsExport);
        }
        let rows = raw
            .rows
            .into_iter()
            .enumerate()
            .map(|(index, r)| {
                if r.source_index != index
                    || r.feature.as_ref().is_some_and(|s| s.len() > 512)
                    || r.weight_bits.0.len() != 16
                    || !r.weight_bits.0.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(Error::ItemStatsExport);
                }
                let bits = u64::from_str_radix(&r.weight_bits.0, 16)
                    .map_err(|_| Error::ItemStatsExport)?;
                Ok(Row {
                    key: item_catalog::opaque(r.item_utf16)?,
                    feature: r.feature,
                    stats: r.stats,
                    max: r.max_durability,
                    weight: f64::from_bits(bits),
                    armor: r.armor_hp,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self {
            db_hash: raw.database_sha256,
            config_hash: raw.config_sha256,
            rows,
        })
    }
    pub fn bind<'a>(&self, package: &'a CatalogPackage) -> Result<StatsCatalog<'a>, Error> {
        if !self
            .db_hash
            .eq_ignore_ascii_case(&package.manifest.database_sha256)
            || !self
                .config_hash
                .eq_ignore_ascii_case(&package.manifest.config_sha256)
            || self.rows.len() != package.equips.len()
        {
            return Err(Error::ItemStatsExport);
        }
        for (a, b) in self.rows.iter().zip(&package.equips) {
            if a.key.as_ref().map(LegacyBytes::as_bytes)
                != b.item_class.as_ref().map(LegacyBytes::as_bytes)
                || a.feature != b.feature
            {
                return Err(Error::ItemStatsExport);
            }
        }
        let catalog = item_catalog_inspection::catalog(package)?;
        let mut stats = BTreeMap::new();
        for (key, index) in catalog.equipment() {
            let row = self.rows.get(index).ok_or(Error::ItemStatsExport)?;
            stats.insert(
                key,
                ItemStat::from_equipment(row.stats, row.max, row.weight, row.armor)?,
            );
        }
        Ok(StatsCatalog {
            catalog,
            stats,
            empty: ItemStat::default(),
        })
    }
    pub fn inspect(&self, package: &CatalogPackage) -> Result<Inspection, Error> {
        let bound = self.bind(package)?;
        let mut source = Sha256::new();
        source.update(b"ItemStatsSourceRows\0v1");
        source.update((self.rows.len() as u32).to_le_bytes());
        for row in &self.rows {
            let stat = ItemStat::from_equipment(row.stats, row.max, row.weight, row.armor)?;
            for entry in stat.character_entries() {
                source.update(entry.value.to_le_bytes());
            }
            for value in stat.fields() {
                source.update(value.to_le_bytes());
            }
        }
        let mut digest = Sha256::new();
        digest.update(b"ItemStats\0v1");
        for (key, stat) in &bound.stats {
            digest.update((key.len() as u32).to_le_bytes());
            digest.update(key);
            for entry in stat.character_entries() {
                digest.update((entry.name.len() as u32).to_le_bytes());
                for u in entry.name {
                    digest.update(u.to_le_bytes());
                }
                digest.update(entry.value.to_le_bytes());
            }
            for value in stat.fields() {
                digest.update(value.to_le_bytes());
            }
        }
        Ok(Inspection {
            equipment_rows: self.rows.len(),
            effective_equipment: bound.stats.len(),
            effective_classes: bound.catalog.counts.class_unique,
            classes_with_equipment_stats: bound
                .catalog
                .items()
                .filter(|(k, _)| bound.stats.contains_key(k))
                .count(),
            projection_sha256: format!("{:x}", digest.finalize()),
            source_rows_projection_sha256: format!("{:x}", source.finalize()),
            source_catalog_matched: true,
            database_mutations: false,
            character_creation_available: false,
        })
    }
}
impl StatsCatalog<'_> {
    pub fn get(&self, key: Option<&[u8]>) -> Option<&ItemStat> {
        let item = self.catalog.get(key)?;
        Some(
            self.stats
                .get(item.binding.item_class)
                .unwrap_or(&self.empty),
        )
    }
    pub fn catalog(&self) -> &ItemCatalog<'_> {
        &self.catalog
    }
}
