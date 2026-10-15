//! Compose a source-bound Fiona inventory using the caller's shared entity authority.
use crate::{
    application::character_creation::EntityAuthority,
    domain::{
        initial_inventory::{self, InventoryClock, InventoryError, PreparedInventory},
        item_colors::ColorRandom,
        item_time::ItemTimeError,
        legacy_user_data::SqlDateTime2,
    },
    error::Error,
    infrastructure::{
        fiona_inventory_profile::Profile, item_catalog::Package as Catalog,
        item_colors::Package as Colors, item_stats::Package as Stats,
    },
};
struct Clock<'a, F> {
    authority: &'a mut EntityAuthority,
    utc: &'a mut F,
}
impl<F: FnMut() -> Result<SqlDateTime2, ItemTimeError>> InventoryClock for Clock<'_, F> {
    fn allocate(&mut self) -> Result<i64, InventoryError> {
        let now = (self.utc)().map_err(InventoryError::Time)?;
        self.authority
            .allocate(now)
            .map(|v| v.entity_time.ticks())
            .map_err(|_| InventoryError::Clock)
    }
    fn utc(&mut self) -> Result<SqlDateTime2, ItemTimeError> {
        (self.utc)()
    }
}
pub struct Inputs<'a> {
    pub profile: &'a Profile,
    pub catalog: &'a Catalog,
    pub stats: &'a Stats,
    pub colors: &'a Colors,
}
pub fn compose(
    inputs: Inputs<'_>,
    owner: i64,
    authority: &mut EntityAuthority,
    random: &mut ColorRandom,
    utc: &mut impl FnMut() -> Result<SqlDateTime2, ItemTimeError>,
) -> Result<PreparedInventory, Error> {
    let stats = inputs.stats.bind(inputs.catalog)?;
    let colors = inputs.colors.bind(inputs.catalog)?;
    let requests = inputs.profile.requests(inputs.catalog, &stats, &colors)?;
    let (maximum, bundle) = inputs.profile.limits()?;
    let mut clock = Clock { authority, utc };
    Ok(initial_inventory::compose(
        &requests,
        owner,
        colors.materials(),
        random,
        maximum,
        bundle,
        &mut clock,
    )?)
}

/// Public diagnostic of an already composed private batch; never emits item IDs/classes.
#[derive(serde::Serialize)]
pub struct Inspection {
    pub items: usize,
    pub equipment: usize,
    pub cash_items: usize,
    pub bag_items: usize,
    pub expiry_rows: usize,
    pub checksum_rows: usize,
    pub projection_sha256: String,
    pub database_mutations: bool,
    pub character_creation_available: bool,
}
pub fn inspect(inventory: &PreparedInventory) -> Inspection {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"FionaInventory\0v1");
    h.update(inventory.owner().to_le_bytes());
    h.update((inventory.items().len() as u32).to_le_bytes());
    for r in inventory.items() {
        h.update(r.id().to_le_bytes());
        h.update((r.class().len() as u32).to_le_bytes());
        h.update(r.class());
        h.update(r.count().to_le_bytes());
        h.update([r.location().collection]);
        h.update(r.location().slot.to_le_bytes());
        match r.expiry() {
            None => h.update([0]),
            Some(v) => {
                h.update([1]);
                h.update(v.to_le_bytes());
            }
        }
        h.update([1]); // New normal requests are unbound/tradable.
        match r.checksum() {
            None => h.update([0]),
            Some(v) => {
                h.update([1]);
                h.update(v.to_le_bytes());
            }
        }
        match r.equipment() {
            None => h.update([0]),
            Some(e) => {
                h.update([1]);
                for c in e.colors {
                    h.update(c.to_le_bytes());
                }
                h.update(e.reduced.to_le_bytes());
                h.update(e.bonus.to_le_bytes());
            }
        }
    }
    Inspection {
        items: inventory.items().len(),
        equipment: inventory
            .items()
            .iter()
            .filter(|r| r.equipment().is_some())
            .count(),
        cash_items: inventory
            .items()
            .iter()
            .filter(|r| r.location().collection == 100)
            .count(),
        bag_items: inventory
            .items()
            .iter()
            .filter(|r| [0, 1, 6].contains(&r.location().collection))
            .count(),
        expiry_rows: inventory
            .items()
            .iter()
            .filter(|r| r.expiry().is_some())
            .count(),
        checksum_rows: inventory
            .items()
            .iter()
            .filter(|r| r.checksum().is_some())
            .count(),
        projection_sha256: format!("{:x}", h.finalize()),
        database_mutations: false,
        character_creation_available: false,
    }
}
