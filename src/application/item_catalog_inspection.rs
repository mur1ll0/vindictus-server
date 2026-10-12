//! Public catalog diagnostic exposes only counts and a canonical projection digest.
use crate::{
    domain::{
        item_catalog::{CatalogCounts, ClassRow, EquipRow, ExtraRow, ItemCatalog},
        legacy_user_data::LegacyBytes,
    },
    error::Error,
    infrastructure::item_catalog::Package,
};
use sha2::{Digest, Sha256};
#[derive(serde::Serialize)]
pub struct Inspection {
    pub counts: CatalogCounts,
    pub projection_sha256: String,
    pub full_startup_verified: bool,
    pub equipment_stats_loaded: bool,
    pub random_rates_loaded: bool,
    pub database_mutations: bool,
    pub game_authentication_available: bool,
}
fn field(digest: &mut Sha256, value: Option<&[u8]>) {
    match value {
        None => digest.update([0]),
        Some(b) => {
            digest.update([1]);
            digest.update((b.len() as u32).to_le_bytes());
            digest.update(b);
        }
    }
}
pub fn inspect(package: &Package) -> Result<Inspection, Error> {
    let equip = package
        .equips
        .iter()
        .map(|r| EquipRow {
            item_class: r.item_class.as_ref().map(LegacyBytes::as_bytes),
            feature: r.feature.as_deref(),
        })
        .collect::<Vec<_>>();
    let rows = package
        .classes
        .iter()
        .map(|r| ClassRow {
            item_class: r.item_class.as_ref().map(LegacyBytes::as_bytes),
            feature: r.feature.as_deref(),
            max_stack: r.max_stack,
            storage: r.storage,
            bind: r.bind.as_deref(),
            trade_on_bind: r.trade_on_bind.as_deref(),
        })
        .collect::<Vec<_>>();
    let extra = package
        .extras
        .iter()
        .map(|r| ExtraRow {
            item_class: r.item_class.as_ref().map(LegacyBytes::as_bytes),
            feature: r.feature.as_deref(),
        })
        .collect::<Vec<_>>();
    let catalog = ItemCatalog::load_with_policy(&equip, &rows, &extra, &package.policy)?;
    let mut digest = Sha256::new();
    digest.update(b"ItemCatalogProjection\0v1");
    for (key, index) in catalog.equipment() {
        digest.update(b"E");
        field(&mut digest, Some(key));
        digest.update((index as u32).to_le_bytes());
    }
    for (key, item) in catalog.items() {
        digest.update(b"I");
        field(&mut digest, Some(key));
        digest.update((item.source_index as u32).to_le_bytes());
        digest.update(item.max_stack.to_le_bytes());
        digest.update([item.storage, u8::from(item.binding.equippable)]);
        field(&mut digest, item.binding.bind.map(str::as_bytes));
        field(&mut digest, item.binding.trade_on_bind.map(str::as_bytes));
    }
    for (key, index) in catalog.extras() {
        digest.update(b"X");
        field(&mut digest, Some(key));
        digest.update((index as u32).to_le_bytes());
    }
    Ok(Inspection {
        counts: catalog.counts,
        projection_sha256: format!("{:x}", digest.finalize()),
        full_startup_verified: false,
        equipment_stats_loaded: false,
        random_rates_loaded: false,
        database_mutations: false,
        game_authentication_available: false,
    })
}
