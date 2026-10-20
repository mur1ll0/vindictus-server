//! Normal empty inventory preparation with source CreateItem emission points.
use crate::{
    application::character_creation::EntityAuthority,
    domain::{
        initial_inventory::{
            self, InitialRequest, InventoryClock, InventoryError, ItemRow, PreparedInventory,
        },
        item_colors::{ColorRandom, Materials},
        item_logs::{Counts, Data, Ledger, Manager},
        item_time::ItemTimeError,
        legacy_user_data::SqlDateTime2,
    },
};
use std::collections::BTreeMap;
pub struct DeliveryClock<'a> {
    pub authority: &'a mut EntityAuthority,
    pub utc: &'a mut (dyn FnMut() -> Result<SqlDateTime2, ItemTimeError> + Send),
    pub civil: &'a mut (dyn FnMut() -> Result<SqlDateTime2, InventoryError> + Send),
}
pub struct Inputs<'a> {
    pub requests: &'a [InitialRequest<'a>],
    pub materials: &'a Materials,
    pub maximum_durability: i32,
    pub bundle_exceed: i32,
}
pub struct Journals {
    pub operation: i64,
    pub ledger: i64,
    pub auto_logging: bool,
    pub shared_inventory: bool,
}
pub struct Prepared {
    pub(crate) shared_inventory: bool,
    pub(crate) inventory: PreparedInventory,
    pub(crate) manager: Manager,
    pub(crate) ledgers: Vec<Ledger>,
    pub(crate) counts: BTreeMap<Vec<u8>, Option<i32>>,
}
impl Prepared {
    pub fn inventory(&self) -> &PreparedInventory {
        &self.inventory
    }
    pub fn events(&mut self) -> &[crate::domain::item_logs::Record] {
        self.manager
            .prepare_write(true, |d| Counts {
                inventory: self.counts.get(d.class()).copied().flatten(),
                shared: if self.shared_inventory
                    && self.counts.get(d.class()).copied().flatten().is_some()
                {
                    Some(0)
                } else {
                    None
                },
            })
            .unwrap_or(&[])
    }
    pub fn ledgers(&self) -> &[Ledger] {
        &self.ledgers
    }
}
struct Clock<'a, 'b> {
    clock: &'a mut DeliveryClock<'b>,
    manager: &'a mut Manager,
    enabled: bool,
}
impl InventoryClock for Clock<'_, '_> {
    fn allocate(&mut self) -> Result<i64, InventoryError> {
        let now = (self.clock.utc)().map_err(InventoryError::Time)?;
        self.clock
            .authority
            .allocate(now)
            .map(|v| v.entity_time.ticks())
            .map_err(|_| InventoryError::Clock)
    }
    fn utc(&mut self) -> Result<SqlDateTime2, ItemTimeError> {
        (self.clock.utc)()
    }
    fn created_before_location(&mut self, row: &ItemRow) -> Result<(), InventoryError> {
        if !self.enabled {
            return Ok(());
        }
        let (colors, reduced, bonus) = row
            .equipment()
            .map_or(([-1; 3], 0, 0), |e| (e.colors, e.reduced, e.bonus));
        let data = Data::new(
            row.id(),
            row.class().to_vec(),
            row.count(),
            colors,
            reduced,
            bonus,
            true,
        )
        .map_err(|_| InventoryError::Bound)?;
        let civil = (self.clock.civil)()?;
        self.manager
            .add_detail(data, 1, true, || civil)
            .map_err(|_| InventoryError::Bound)
    }
}
pub fn prepare(
    inputs: Inputs<'_>,
    owner: i64,
    journals: Journals,
    random: &mut ColorRandom,
    clock: &mut DeliveryClock<'_>,
) -> Result<Prepared, InventoryError> {
    if inputs.requests.is_empty()
        || inputs.requests.len() > 4096
        || inputs.requests.iter().any(|r| r.count <= 0)
    {
        return Err(InventoryError::Bound);
    }
    // GiveItemRequest/LogData are captured before any item allocation or generated colors.
    let mut ledgers = Vec::with_capacity(inputs.requests.len());
    let mut counts = BTreeMap::new();
    for r in inputs.requests {
        let count = if r.max_stack > 1 { Some(r.count) } else { None };
        if counts.insert(r.class.to_vec(), count).is_some() {
            return Err(InventoryError::Bound);
        }
        ledgers.push(Ledger::creation(
            journals.ledger,
            Data::requested(r.class.to_vec(), r.count).map_err(|_| InventoryError::Bound)?,
            count,
        ));
    }
    let mut manager = Manager::default();
    manager.set_journal(journals.operation, 19);
    let inventory = initial_inventory::compose(
        inputs.requests,
        owner,
        inputs.materials,
        random,
        inputs.maximum_durability,
        inputs.bundle_exceed,
        &mut Clock {
            clock,
            manager: &mut manager,
            enabled: journals.auto_logging,
        },
    )?;
    Ok(Prepared {
        shared_inventory: journals.shared_inventory,
        inventory,
        manager,
        ledgers,
        counts,
    })
}
