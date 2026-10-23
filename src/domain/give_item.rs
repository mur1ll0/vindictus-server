//! ItemStorage.GiveVirtualItem planning (StorageHelper, ItemClassHelper and the
//! storages' BindVirtualItem) for attribute-less, non-expiring items of one owner.
//! Only the all-or-nothing overload is modelled: any failure applies nothing, and
//! leftovers are never converted into system items (the mail fallback is not migrated).
use super::item_location::ItemLocation;
use thiserror::Error;

/// ItemOperationResult failures reachable by this planner.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum GiveError {
    #[error("DestroyItemFail_InvalidRequest")]
    InvalidRequest,
    #[error("InvalidItemClass")]
    InvalidItemClass,
    #[error("DestroyItemFail_NotEnoughItem")]
    NotEnoughItem,
    #[error("GiveItemFail_ExceedBundleCount")]
    ExceedBundleCount,
    #[error("GiveItemFail_UniqueConstraint")]
    UniqueConstraint,
    #[error("GiveItemFail_NoEmptySlot")]
    NoEmptySlot,
    /// A source branch outside this slice (TirCoin, Story tab, expiring classes,
    /// source arithmetic that would loop or store negative counts).
    #[error("item request outside the migrated storage profile")]
    Unsupported,
}

/// Effective ItemClassInfo values used by the storages.
#[derive(Clone, Debug)]
pub struct ClassInfo {
    pub class: Vec<u8>,
    pub max_stack: i32,
    pub storage: u8,
    pub unique: i32,
    pub equippable: bool,
    pub expire_in: Option<i32>,
    /// FeatureMatrix ingame_cashshop_tircoin_system with a TirCoin class.
    pub tircoin: bool,
}
/// A located item of the owner (Restore storage excluded by the caller).
#[derive(Clone, Copy, Debug)]
pub struct Held {
    pub id: i64,
    pub count: i32,
    pub location: ItemLocation,
    pub bound: bool,
}
pub struct Give<'a> {
    pub class: &'a ClassInfo,
    pub count: i32,
    pub bound: bool,
}
pub struct Destroy<'a> {
    pub class: &'a ClassInfo,
    pub count: i32,
}
/// What the owner holds, grouped as ItemStorage.ItemClassDict.
pub struct Inventory<'a> {
    pub items: &'a [(Vec<u8>, Held)],
    /// Bag collections reporting IsAvailable, in ValidBagStorageIDs order.
    pub bags: &'a [u8],
    /// FeatureMatrix BundleExceed.
    pub bundle_exceed: i32,
    /// FeatureMatrix StoryTabInInventory.
    pub story_tab: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
    pub class: Vec<u8>,
    pub count: i32,
    pub bound: bool,
    pub location: ItemLocation,
    /// Index of the give request whose ItemRequestProperty initializes the item
    /// (the first of merged requests).
    pub request: usize,
}
/// Successful plan: VirtualModifyItem results then bound VirtualCreateItems,
/// in the source application order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub modified: Vec<(i64, i32)>,
    pub created: Vec<Created>,
}
impl Plan {
    /// StorageHelper.Destroyed: modified items whose count reaches zero.
    pub fn destroyed(&self) -> impl Iterator<Item = i64> + '_ {
        self.modified.iter().filter(|m| m.1 == 0).map(|m| m.0)
    }
}

const ACCOUNT: ItemLocation = ItemLocation {
    collection: 101,
    slot: -1,
};
const BAG_SIZE: i32 = 48;
/// StoryStorage: ItemLocation.StoryLocation(slot) in collection 103, StoryStorageSize 48.
pub const STORY_COLLECTION: u8 = 103;
const STORY_SIZE: i32 = 48;
const VALID_BAGS: [u8; 9] = [0, 1, 6, 2, 3, 4, 5, 7, 8];
const MAX_VIRTUAL: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Storage {
    Account,
    Cash,
    Story,
    Bag,
    Invalid,
}
/// StorageSetting.GetAvailableStorages; Restore is never bound by GiveVirtualItem.
fn storage_of(info: &ClassInfo, story_tab: bool) -> Storage {
    match info.storage {
        3 => Storage::Account,
        2 => Storage::Cash,
        1 => Storage::Bag,
        5 if story_tab => Storage::Story,
        _ => Storage::Invalid,
    }
}

struct Helper<'a> {
    info: &'a ClassInfo,
    items: Vec<Held>,
    destroys: Vec<i32>,
    gives: Vec<(i32, bool, usize)>,
    modified: Vec<(Held, i32)>,
    created: Vec<(i32, bool, usize)>,
    unique_fail: bool,
}
impl<'a> Helper<'a> {
    fn new(info: &'a ClassInfo, all: &[(Vec<u8>, Held)]) -> Self {
        Self {
            info,
            items: all
                .iter()
                .filter(|(c, _)| *c == info.class)
                .map(|(_, h)| *h)
                .collect(),
            destroys: Vec::new(),
            gives: Vec::new(),
            modified: Vec::new(),
            created: Vec::new(),
            unique_fail: false,
        }
    }
    fn modified_mut(&mut self, id: i64) -> Option<&mut i32> {
        self.modified
            .iter_mut()
            .find(|(h, _)| h.id == id)
            .map(|(_, n)| n)
    }
    fn num(&self, item: &Held) -> i32 {
        self.modified
            .iter()
            .find(|(h, _)| h.id == item.id)
            .map_or(item.count, |m| m.1)
    }
    /// ItemTraitExtension.IsStackableWith(Item, GiveItemRequest) without attributes or expiry.
    fn stackable(&self, item: &Held, bound: bool) -> bool {
        !self.info.equippable && (self.info.storage == 3 || item.bound == bound)
    }
    fn destroy_item(&mut self, item: Held, num: i32) -> i32 {
        if let Some(modified) = self.modified_mut(item.id) {
            // Source subtracts the requested num, not the destroyed amount.
            let result = (*modified).min(num);
            *modified -= num;
            return result;
        }
        let taken = item.count.min(num);
        self.modified.push((item, item.count - taken));
        taken
    }
    fn add_item(&mut self, item: Held, num: i32, unique_left: &mut i32) -> i32 {
        let max = self.info.max_stack;
        if let Some(modified) = self.modified_mut(item.id) {
            let added = (max - *modified).min(num).min(*unique_left);
            if added != 0 {
                *modified += added;
                *unique_left -= added;
            }
            return added;
        }
        let added = (max - item.count).min(num).min(*unique_left);
        if added != 0 {
            self.modified.push((item, item.count + added));
            *unique_left -= added;
        }
        added
    }
    /// DestroyComparer for equal item value and no expiry: bound first, then Num
    /// ascending, then Slot descending; OrderBy is stable.
    fn destroy_order(&self) -> Vec<Held> {
        let mut items = self.items.clone();
        items.sort_by(|x, y| {
            y.bound
                .cmp(&x.bound)
                .then(x.count.cmp(&y.count))
                .then(y.location.slot.cmp(&x.location.slot))
        });
        items
    }
    /// StackComparer: Num descending, stable.
    fn stack_order(&self) -> Vec<Held> {
        let mut items = self.items.clone();
        items.sort_by_key(|x| std::cmp::Reverse(x.count));
        items
    }
    fn process(&mut self, bundle_exceed: i32) -> Result<(), GiveError> {
        for num in self.destroys.clone() {
            let mut num = num;
            for item in self.destroy_order() {
                num -= self.destroy_item(item, num);
                if num == 0 {
                    break;
                }
            }
            if num != 0 {
                return Err(GiveError::NotEnoughItem);
            }
        }
        let stack = if self.info.max_stack <= 0 {
            1
        } else {
            self.info.max_stack
        };
        if bundle_exceed > 0 && self.gives.iter().any(|g| g.0 / stack > bundle_exceed) {
            return Err(GiveError::ExceedBundleCount);
        }
        // ItemFactory.MergeDuplicatedRequest: stackable requests of the class are summed.
        let mut merged: Vec<(i32, bool, usize)> = Vec::new();
        for &(num, bound, request) in &self.gives {
            match merged
                .iter_mut()
                .find(|m| !self.info.equippable && (self.info.storage == 3 || m.1 == bound))
            {
                Some(m) => m.0 = m.0.checked_add(num).ok_or(GiveError::Unsupported)?,
                None => merged.push((num, bound, request)),
            }
        }
        let count = self
            .items
            .iter()
            .try_fold(0i32, |sum, i| sum.checked_add(self.num(i)))
            .ok_or(GiveError::Unsupported)?;
        let mut unique_left = if self.info.unique <= 0 {
            i32::MAX
        } else {
            self.info.unique - count
        };
        for (num, bound, request) in merged {
            let mut num = num;
            if unique_left > 0 {
                for item in self.stack_order() {
                    if self.stackable(&item, bound) {
                        num -= self.add_item(item, num, &mut unique_left);
                        if unique_left == 0 {
                            break;
                        }
                    }
                }
            }
            while num > 0 {
                // CreateItem: a negative unique_left would loop forever in the source.
                if unique_left < 0 || self.info.max_stack <= 0 {
                    return Err(GiveError::Unsupported);
                }
                let mut n = num.min(self.info.max_stack);
                if unique_left == 0 {
                    self.unique_fail = true;
                } else {
                    n = n.min(unique_left);
                    self.created.push((n, bound, request));
                    unique_left -= n;
                }
                num -= n;
                if self.created.len() > MAX_VIRTUAL {
                    return Err(GiveError::Unsupported);
                }
            }
        }
        if self.unique_fail {
            return Err(GiveError::UniqueConstraint);
        }
        Ok(())
    }
}

/// StorageHelper.Load/Process then BindVirtualItem in StoragePriorityOrder.
pub fn plan(
    inventory: &Inventory<'_>,
    destroys: &[Destroy<'_>],
    gives: &[Give<'_>],
) -> Result<Plan, GiveError> {
    let mut helpers: Vec<Helper<'_>> = Vec::new();
    for d in destroys {
        if d.count < 0 {
            return Err(GiveError::InvalidRequest);
        }
        index(&mut helpers, d.class, inventory)
            .destroys
            .push(d.count);
    }
    for (request, g) in gives.iter().enumerate() {
        if g.count < 0 {
            return Err(GiveError::InvalidRequest);
        }
        index(&mut helpers, g.class, inventory)
            .gives
            .push((g.count, g.bound, request));
    }
    for h in &helpers {
        if h.info.tircoin || h.info.expire_in.is_some() && !h.gives.is_empty() {
            return Err(GiveError::Unsupported);
        }
    }
    for h in &mut helpers {
        h.process(inventory.bundle_exceed)?;
    }
    let mut out = Plan::default();
    for h in &helpers {
        for (item, num) in &h.modified {
            if *num < 0 {
                return Err(GiveError::Unsupported);
            }
            out.modified.push((item.id, *num));
        }
    }
    let destroyed: Vec<i64> = out.destroyed().collect();
    let mut pending: Vec<(&ClassInfo, i32, bool, usize)> = helpers
        .iter()
        .flat_map(|h| h.created.iter().map(move |&(n, b, r)| (h.info, n, b, r)))
        .collect();
    for storage in [
        Storage::Account,
        Storage::Cash,
        Storage::Story,
        Storage::Bag,
    ] {
        let mut left = Vec::new();
        let mut free = free_slots(storage, inventory, &destroyed).into_iter();
        for (info, count, bound, request) in pending {
            if storage_of(info, inventory.story_tab) != storage {
                left.push((info, count, bound, request));
                continue;
            }
            let location = match storage {
                Storage::Account => {
                    let held = inventory
                        .items
                        .iter()
                        .any(|(c, h)| *c == info.class && !destroyed.contains(&h.id));
                    let twice = out
                        .created
                        .iter()
                        .any(|c| c.class == info.class && c.location == ACCOUNT);
                    if held {
                        left.push((info, count, bound, request));
                        continue;
                    }
                    if twice {
                        return Err(GiveError::Unsupported);
                    }
                    ACCOUNT
                }
                Storage::Story | Storage::Cash | Storage::Bag => match free.next() {
                    Some(l) => l,
                    None => {
                        left.push((info, count, bound, request));
                        continue;
                    }
                },
                Storage::Invalid => unreachable_storage()?,
            };
            out.created.push(Created {
                class: info.class.clone(),
                count,
                bound,
                location,
                request,
            });
        }
        pending = left;
        if pending.is_empty() {
            break;
        }
    }
    if !pending.is_empty() {
        return Err(GiveError::NoEmptySlot);
    }
    Ok(out)
}

fn unreachable_storage() -> Result<ItemLocation, GiveError> {
    Err(GiveError::Unsupported)
}

fn index<'h, 'a>(
    helpers: &'h mut Vec<Helper<'a>>,
    info: &'a ClassInfo,
    inventory: &Inventory<'_>,
) -> &'h mut Helper<'a> {
    let position = match helpers.iter().position(|h| h.info.class == info.class) {
        Some(p) => p,
        None => {
            helpers.push(Helper::new(info, inventory.items));
            helpers.len() - 1
        }
    };
    &mut helpers[position]
}

/// Cash: every slot from 0 not held (destroyed items still occupy it).
/// Bag: empty or destroyed slots of the available collections, in source order.
fn free_slots(storage: Storage, inventory: &Inventory<'_>, destroyed: &[i64]) -> Vec<ItemLocation> {
    let occupied = |collection: u8, slot: i32, count_destroyed: bool| {
        inventory.items.iter().any(|(_, h)| {
            h.location.collection == collection
                && h.location.slot == slot
                && (count_destroyed || !destroyed.contains(&h.id))
        })
    };
    match storage {
        Storage::Cash => (0..)
            .filter(|&slot| !occupied(100, slot, true))
            .take(MAX_VIRTUAL)
            .map(|slot| ItemLocation {
                collection: 100,
                slot,
            })
            .collect(),
        // StoryStorage.EmptySlots ignores the destroyed items (they still hold the slot).
        Storage::Story => (0..STORY_SIZE)
            .filter(|&slot| !occupied(STORY_COLLECTION, slot, true))
            .map(|slot| ItemLocation {
                collection: STORY_COLLECTION,
                slot,
            })
            .collect(),
        Storage::Bag => VALID_BAGS
            .iter()
            .filter(|c| inventory.bags.contains(c))
            .flat_map(|&collection| {
                (0..BAG_SIZE)
                    .filter(move |&slot| !occupied(collection, slot, false))
                    .map(move |slot| ItemLocation { collection, slot })
            })
            .collect(),
        _ => Vec::new(),
    }
}
