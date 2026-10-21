//! Private material snapshot pinned to the original equipment projection.
use crate::{
    application::item_catalog_inspection,
    domain::{
        item_catalog::ItemCatalog,
        item_colors::{ColorRandom, Materials},
        legacy_user_data::LegacyBytes,
    },
    error::Error,
    infrastructure::{
        item_catalog::{self, Package as CatalogPackage},
        user_export::{PrivateText, hex, nullable},
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
    materials: [Option<PrivateText>; 3],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMaterial {
    name_utf16: PrivateText,
    #[serde(deserialize_with = "nullable")]
    raw_hex: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    sha256: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    rows: Vec<RawRow>,
    materials: Vec<RawMaterial>,
}
struct Row {
    key: Option<LegacyBytes<1024, true>>,
    feature: Option<String>,
    names: [Option<Zeroizing<Vec<u16>>>; 3],
}
pub struct Package {
    db_hash: String,
    config_hash: String,
    rows: Vec<Row>,
    materials: Materials,
    names: Vec<Zeroizing<Vec<u16>>>,
}
pub struct ColorCatalog<'a> {
    catalog: ItemCatalog<'a>,
    equipment: BTreeMap<&'a [u8], usize>,
    package: &'a Package,
}
#[derive(serde::Serialize)]
pub struct Inspection {
    equipment_rows: usize,
    effective_equipment: usize,
    material_count: usize,
    total_colors: usize,
    source_projection_sha256: String,
    catalog_projection_sha256: String,
    source_catalog_matched: bool,
    database_mutations: bool,
    character_creation_available: bool,
}
fn units(value: Option<PrivateText>) -> Result<Option<Zeroizing<Vec<u16>>>, Error> {
    value
        .map(|s| {
            let data = hex(s, 512).map_err(|_| Error::ItemColorsExport)?;
            if !data.len().is_multiple_of(2) {
                return Err(Error::ItemColorsExport);
            }
            Ok(Zeroizing::new(
                data.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect(),
            ))
        })
        .transpose()
}
impl Package {
    pub fn read(reader: impl Read) -> Result<Self, Error> {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::ItemColorsExport)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(Error::ItemColorsExport);
        }
        let raw: Export = serde_json::from_slice(&bytes).map_err(|_| Error::ItemColorsExport)?;
        let hash = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
        if raw.format_version != 1
            || raw.source != "heroesContents.item-colors"
            || raw.rows.len() > 50000
            || raw.materials.len() > 256
            || !hash(&raw.database_sha256)
            || !hash(&raw.config_sha256)
        {
            return Err(Error::ItemColorsExport);
        }
        let mut materials = Materials::default();
        let mut names = Vec::new();
        for entry in raw.materials {
            let name = units(Some(entry.name_utf16))?.ok_or(Error::ItemColorsExport)?;
            let data = entry
                .raw_hex
                .map(|v| {
                    hex(v, 196610)
                        .map(Zeroizing::new)
                        .map_err(|_| Error::ItemColorsExport)
                })
                .transpose()?;
            match (&data, &entry.sha256) {
                (Some(data), Some(h))
                    if hash(h)
                        && format!("{:x}", Sha256::digest(data.as_slice()))
                            .eq_ignore_ascii_case(h) => {}
                (None, None) => {}
                _ => return Err(Error::ItemColorsExport),
            }
            materials
                .insert(&name, data.as_ref().map(|v| v.as_slice()))
                .map_err(|_| Error::ItemColorsExport)?;
            names.push(name);
        }
        let rows = raw
            .rows
            .into_iter()
            .enumerate()
            .map(|(index, r)| {
                if r.source_index != index || r.feature.as_ref().is_some_and(|s| s.len() > 512) {
                    return Err(Error::ItemColorsExport);
                }
                let [a, b, c] = r.materials;
                let names = [units(a)?, units(b)?, units(c)?];
                for name in names.iter().flatten() {
                    materials.get(name).map_err(|_| Error::ItemColorsExport)?;
                }
                Ok(Row {
                    key: item_catalog::opaque(r.item_utf16).map_err(|_| Error::ItemColorsExport)?,
                    feature: r.feature,
                    names,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self {
            db_hash: raw.database_sha256,
            config_hash: raw.config_sha256,
            rows,
            materials,
            names,
        })
    }
    pub fn bind<'a>(&'a self, package: &'a CatalogPackage) -> Result<ColorCatalog<'a>, Error> {
        if !self
            .db_hash
            .eq_ignore_ascii_case(&package.manifest.database_sha256)
            || !self
                .config_hash
                .eq_ignore_ascii_case(&package.manifest.config_sha256)
            || self.rows.len() != package.equips.len()
        {
            return Err(Error::ItemColorsExport);
        }
        for (a, b) in self.rows.iter().zip(&package.equips) {
            if a.key.as_ref().map(LegacyBytes::as_bytes)
                != b.item_class.as_ref().map(LegacyBytes::as_bytes)
                || a.feature != b.feature
            {
                return Err(Error::ItemColorsExport);
            }
        }
        let catalog = item_catalog_inspection::catalog(package)?;
        let equipment = catalog.equipment().collect();
        Ok(ColorCatalog {
            catalog,
            equipment,
            package: self,
        })
    }
    pub fn inspect(&self, package: &CatalogPackage) -> Result<Inspection, Error> {
        let bound = self.bind(package)?;
        let mut digest = Sha256::new();
        let mut total = 0;
        digest.update(b"ItemColors\0v1");
        digest.update((self.names.len() as u32).to_le_bytes());
        let mut random = ColorRandom::from_seed(0);
        for name in &self.names {
            digest.update((name.len() as u32).to_le_bytes());
            for unit in name.iter() {
                digest.update(unit.to_le_bytes());
            }
            let material = self
                .materials
                .get(name)
                .map_err(|_| Error::ItemColorsExport)?;
            digest.update([u8::from(material.is_some())]);
            if let Some(material) = material {
                digest.update((material.colors().len() as u32).to_le_bytes());
                total += material.colors().len();
                for value in material.colors() {
                    digest.update(value.to_le_bytes());
                }
            }
            for _ in 0..32 {
                digest.update(
                    self.materials
                        .pick(Some(name), &mut random)
                        .map_err(|_| Error::ItemColorsExport)?
                        .to_le_bytes(),
                );
            }
        }
        Ok(Inspection {
            equipment_rows: self.rows.len(),
            effective_equipment: bound.equipment.len(),
            material_count: self.names.len(),
            total_colors: total,
            source_projection_sha256: format!("{:x}", digest.finalize()),
            catalog_projection_sha256: item_catalog_inspection::inspect(package)?.projection_sha256,
            source_catalog_matched: true,
            database_mutations: false,
            character_creation_available: false,
        })
    }
}
impl ColorCatalog<'_> {
    /// Known valid equipment class only. Names remain UTF16 ordinal, NULL distinct.
    pub fn get(&self, key: Option<&[u8]>) -> Option<[Option<&[u16]>; 3]> {
        let item = self.catalog.get(key)?;
        let row = self
            .package
            .rows
            .get(*self.equipment.get(item.binding.item_class)?)?;
        Some(
            row.names
                .each_ref()
                .map(|v| v.as_deref().map(|v| v.as_slice())),
        )
    }
    pub fn materials(&self) -> &Materials {
        &self.package.materials
    }
    /// Owned per-class material names of every effective equipment class, bound once.
    pub fn to_index(&self) -> ColorIndex {
        ColorIndex(
            self.catalog
                .items()
                .filter_map(|(key, _)| {
                    let names = self.get(Some(key))?;
                    Some((key.to_vec(), names.map(|n| n.map(<[u16]>::to_vec))))
                })
                .collect(),
        )
    }
}
#[derive(Default)]
pub struct ColorIndex(BTreeMap<Vec<u8>, [Option<Vec<u16>>; 3]>);
impl ColorIndex {
    pub fn get(&self, key: &[u8]) -> Option<[Option<&[u16]>; 3]> {
        self.0.get(key).map(|n| n.each_ref().map(|v| v.as_deref()))
    }
}
impl Package {
    pub fn materials(&self) -> &Materials {
        &self.materials
    }
}
