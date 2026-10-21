//! Empty normal inventory composition. Not a complete character initializer.
use super::{
    item_colors::{self, ColorRandom, Look, Materials},
    item_durability::{self, EquipmentDurability},
    item_location::ItemLocation,
    item_stats::ItemStat,
    item_time::{self, ItemTimeError},
    legacy_user_data::{LegacyBytes, SqlDateTime2},
};
use thiserror::Error;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryError {
    #[error("initial inventory exceeds verified bounds or source count rules")]
    Bound,
    #[error("initial inventory violates source Unique or BundleExceed")]
    Limit,
    #[error("storage is outside the empty Fiona profile")]
    Storage,
    #[error("initial bag has no empty slot")]
    Full,
    #[error("source entity allocator or clock failed")]
    Clock,
    #[error(transparent)]
    Color(#[from] item_colors::ColorError),
    #[error(transparent)]
    Time(#[from] ItemTimeError),
}
pub struct InitialRequest<'a> {
    pub class: &'a [u8],
    pub count: i32,
    pub max_stack: i32,
    pub unique: i32,
    pub storage: u8,
    pub expire_in: Option<i32>,
    pub equipment: bool,
    pub base: &'a ItemStat,
    pub materials: [Option<&'a [u16]>; 3],
    pub gold: bool,
}
#[derive(PartialEq, Eq)]
pub struct Equipment {
    pub colors: [i32; 3],
    pub reduced: i32,
    pub bonus: i32,
}
#[derive(PartialEq, Eq)]
pub struct ItemRow {
    pub(crate) id: i64,
    pub(crate) class: LegacyBytes<50, true>,
    pub(crate) count: i32,
    pub(crate) location: ItemLocation,
    pub(crate) expire: Option<i64>,
    pub(crate) checksum: Option<i32>,
    pub(crate) equipment: Option<Equipment>,
}
impl ItemRow {
    pub fn id(&self) -> i64 {
        self.id
    }
    pub fn class(&self) -> &[u8] {
        self.class.as_bytes()
    }
    pub fn count(&self) -> i32 {
        self.count
    }
    pub fn location(&self) -> ItemLocation {
        self.location
    }
    pub fn expiry(&self) -> Option<i64> {
        self.expire
    }
    pub fn checksum(&self) -> Option<i32> {
        self.checksum
    }
    pub fn equipment(&self) -> Option<&Equipment> {
        self.equipment.as_ref()
    }
}
pub struct PreparedInventory {
    pub(crate) owner: i64,
    pub(crate) rows: Vec<ItemRow>,
}
impl PreparedInventory {
    pub fn owner(&self) -> i64 {
        self.owner
    }
    pub fn items(&self) -> &[ItemRow] {
        &self.rows
    }
}
/// Source Checker/Crc32 for the explicit en-US numeric formatting profile.
/// Gold selection belongs to the pinned source ItemClassConst, not this function.
pub fn gold_checksum(id: i64, owner: Option<i64>, class: &[u16], count: i32) -> i32 {
    let mixed = id ^ owner.unwrap_or_default() ^ 634486454818966544_i64;
    let text = format!("{mixed}{}{count}", String::from_utf16_lossy(class));
    let mut crc = u32::MAX;
    for b in text.bytes() {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
        }
    }
    (!crc) as i32
}
/// Item.Initialize for equipment with request colors -1 and no durability changes:
/// SetColor draws each material color, then MaxDurability/Durability and IsValid.
pub fn new_equipment(
    base: &ItemStat,
    names: [Option<&[u16]>; 3],
    materials: &Materials,
    random: &mut ColorRandom,
    maximum_durability: i32,
) -> Result<Equipment, InventoryError> {
    let mut colors = [0; 3];
    item_colors::set_colors(
        &mut colors,
        [Some(-1); 3],
        true,
        Look::Equipment(names),
        materials,
        random,
    )?;
    let (mut durability, _) =
        item_durability::initialize(Some(base), base, 0, 0, maximum_durability)
            .ok_or(InventoryError::Bound)?;
    validate_durability(&mut durability, maximum_durability);
    Ok(Equipment {
        colors,
        reduced: durability.reduced(),
        bonus: durability.bonus(),
    })
}
/// IsValid's matching-equipment/new-null-Part branch; source profiler only measures.
pub fn validate_durability(state: &mut EquipmentDurability, limit: i32) {
    if state.maximum() < 0 {
        state.set_maximum(0, limit);
    }
    if state.maximum() > limit {
        state.set_maximum(limit, limit);
    }
    if state.maximum() < state.current() {
        state.set_current(state.maximum());
    }
    if state.current() < 0 {
        state.set_current(0);
    }
}
pub trait InventoryClock {
    fn allocate(&mut self) -> Result<i64, InventoryError>;
    fn utc(&mut self) -> Result<SqlDateTime2, ItemTimeError>;
    /// ItemFactory.MakeItem emits CreateItem after Initialize, before SetLocation/RegisterCreate.
    fn created_before_location(&mut self, _item: &ItemRow) -> Result<(), InventoryError> {
        Ok(())
    }
}
/// Requests are the already Feature-selected, AddOrIncrease-grouped dictionary.
/// Empty source storages are Cash100 and default Bag0/1/6. No attributes, deletes,
/// existing stacks, expanded bags or arbitrary source profiles are inferred here.
/// Allocator and RNG retain consumed state on a late failure, as source Undo does.
pub fn compose(
    requests: &[InitialRequest<'_>],
    owner: i64,
    materials: &Materials,
    random: &mut ColorRandom,
    maximum_durability: i32,
    bundle_exceed: i32,
    clock: &mut impl InventoryClock,
) -> Result<PreparedInventory, InventoryError> {
    if owner <= 0 || requests.len() > 4096 {
        return Err(InventoryError::Bound);
    }
    let mut virtuals = Vec::new();
    for (index, r) in requests.iter().enumerate() {
        if r.class.len() > 100
            || !r.class.len().is_multiple_of(2)
            || r.max_stack <= 0
            || r.count < 0
            || requests[..index].iter().any(|p| p.class == r.class)
        {
            return Err(InventoryError::Bound);
        }
        if !matches!(r.storage, 1 | 2) {
            return Err(InventoryError::Storage);
        }
        if (bundle_exceed > 0 && r.count / r.max_stack > bundle_exceed)
            || (r.unique > 0 && r.count > r.unique)
        {
            return Err(InventoryError::Limit);
        }
        let mut left = r.count;
        while left > 0 {
            if virtuals.len() >= 4096 {
                return Err(InventoryError::Bound);
            }
            let count = left.min(r.max_stack);
            virtuals.push((r, count));
            left -= count;
        }
    }
    // StoragePriorityOrder binds Cash first, then Bag, retaining source class order.
    let mut placed = Vec::with_capacity(virtuals.len());
    let mut cash = 0;
    let mut bag = 0;
    for storage in [2, 1] {
        for &(r, count) in &virtuals {
            if r.storage != storage {
                continue;
            }
            let location = if storage == 2 {
                let slot = cash;
                cash += 1;
                ItemLocation {
                    collection: 100,
                    slot,
                }
            } else {
                if bag >= 144 {
                    return Err(InventoryError::Full);
                }
                let collection = [0, 1, 6][bag / 48];
                let slot = (bag % 48) as i32;
                bag += 1;
                ItemLocation { collection, slot }
            };
            placed.push((r, count, location));
        }
    }
    let mut rows = Vec::with_capacity(placed.len());
    for (r, count, location) in placed {
        let id = clock.allocate()?;
        SqlDateTime2::from_ticks(id).map_err(|_| InventoryError::Clock)?;
        if rows.last().is_some_and(|last: &ItemRow| last.id >= id) {
            return Err(InventoryError::Clock);
        }
        // Num setter runs after the allocation, before attributes/colors.
        if r.equipment && count > 1 {
            return Err(InventoryError::Bound);
        }
        let expiry = item_time::initialize_expiry(None, false, r.expire_in, &mut || clock.utc())?
            .expiry
            .map(|v| v.ticks());
        let equipment = if r.equipment {
            Some(new_equipment(
                r.base,
                r.materials,
                materials,
                random,
                maximum_durability,
            )?)
        } else {
            None
        };
        let checksum = if r.gold {
            let units = r
                .class
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes(*b))
                .collect::<Vec<_>>();
            Some(gold_checksum(id, Some(owner), &units, count))
        } else {
            None
        };
        let row = ItemRow {
            id,
            class: LegacyBytes::from_bytes(r.class.to_vec()).map_err(|_| InventoryError::Bound)?,
            count,
            location,
            expire: expiry,
            checksum,
            equipment,
        };
        clock.created_before_location(&row)?;
        rows.push(row);
    }
    Ok(PreparedInventory { owner, rows })
}
