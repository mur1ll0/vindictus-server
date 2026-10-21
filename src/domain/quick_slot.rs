//! Gear.QuickSlots of StackableSlot: item classes (or skills) per consumable part.
//! Order follows ItemContents.GetEquipPartInfo; the persisted rows only hold classes.
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("quick slot rows are ambiguous or outside the bounded profile")]
pub struct QuickSlotError;

/// ItemOperationResult of StackableSlot.Set / Gear.Equip(part, itemClass).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    InvalidSlot,
    InvalidItemCategory,
}
impl Refusal {
    pub const fn name(self) -> &'static str {
        match self {
            Self::InvalidSlot => "EquipItemFail_InvalidSlot",
            Self::InvalidItemCategory => "EquipItemFail_InvalidItemCategory",
        }
    }
}

/// Persistence change produced by a transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Insert { part: u8, class: String },
    Update { part: u8, class: String },
    Delete { part: u8 },
}

struct Slot {
    part: u8,
    locked: bool,
    categories: Vec<String>,
    class: Option<String>,
}

pub struct QuickSlots {
    slots: Vec<Slot>,
}

impl QuickSlots {
    /// `parts`: (part, IsExpandedSlot-derived lock, categories) in content order.
    pub fn new(
        parts: impl IntoIterator<Item = (u8, bool, Vec<String>)>,
    ) -> Result<Self, QuickSlotError> {
        let mut slots: Vec<Slot> = Vec::new();
        for (part, locked, categories) in parts {
            if slots.len() >= 256 || slots.iter().any(|s| s.part == part) || categories.len() > 64 {
                return Err(QuickSlotError);
            }
            slots.push(Slot {
                part,
                locked,
                categories,
                class: None,
            });
        }
        Ok(Self { slots })
    }
    /// StackableSlot.Load(QuickSlot). The source ignores rows for unknown parts and
    /// second rows for a part; such hidden rows are refused instead of retained.
    pub fn load(&mut self, part: i32, class: String) -> Result<(), QuickSlotError> {
        if class.len() > 100 {
            return Err(QuickSlotError);
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|s| i32::from(s.part) == part)
            .ok_or(QuickSlotError)?;
        if slot.class.is_some() {
            return Err(QuickSlotError);
        }
        slot.class = Some(class);
        Ok(())
    }
    pub fn contains(&self, part: i32) -> bool {
        self.slots.iter().any(|s| i32::from(s.part) == part)
    }
    /// StackableSlot.Value: locked → null, empty → "", otherwise the class.
    fn value(slot: &Slot) -> Option<&str> {
        if slot.locked {
            None
        } else {
            Some(slot.class.as_deref().unwrap_or(""))
        }
    }
    /// Gear.ToQuickSlotInfo: every non-null Value, in slot order.
    pub fn info(&self) -> Vec<(i32, Option<String>)> {
        self.slots
            .iter()
            .filter_map(|s| Self::value(s).map(|v| (i32::from(s.part), Some(v.to_owned()))))
            .collect()
    }
    /// Gear.GetEquippedQuickSlot: first slot whose Value equals the class.
    pub fn equipped(&self, class: &str) -> Option<u8> {
        self.slots
            .iter()
            .find(|s| Self::value(s) == Some(class))
            .map(|s| s.part)
    }
    /// (part, Value) for unlocked slots, in slot order (Gear.QuickSlots enumeration).
    pub fn values(&self) -> impl Iterator<Item = (u8, &str)> {
        self.slots
            .iter()
            .filter_map(|s| Self::value(s).map(|v| (s.part, v)))
    }
    /// Gear.GetEquippableEmptySlots, quick-slot part: unlocked, empty, category accepted.
    pub fn empty_for(&self, category: &str) -> Vec<u8> {
        self.slots
            .iter()
            .filter(|s| {
                !s.locked && s.class.is_none() && s.categories.iter().any(|c| c == category)
            })
            .map(|s| s.part)
            .collect()
    }
    /// Gear.GetMaxQuickSlotID.
    pub fn max_part(&self) -> Option<u8> {
        self.slots.iter().map(|s| s.part).max()
    }
    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.slots.iter().filter_map(|s| s.class.as_deref())
    }
    /// StackableSlot.Clear; None when the part is not a quick slot.
    pub fn clear(&mut self, part: i32) -> Option<Option<Change>> {
        let slot = self.slots.iter_mut().find(|s| i32::from(s.part) == part)?;
        Some(
            slot.class
                .take()
                .map(|_| Change::Delete { part: slot.part }),
        )
    }
    /// Gear.EquipSkill after its duplicate check, i.e. StackableSlot.SetSkill: only the
    /// lock is checked (no category).
    pub fn set_skill(&mut self, part: i32, skill: &str) -> Result<Change, Refusal> {
        let slot = self
            .slots
            .iter_mut()
            .find(|s| i32::from(s.part) == part)
            .ok_or(Refusal::InvalidSlot)?;
        if slot.locked {
            return Err(Refusal::InvalidSlot);
        }
        let change = if slot.class.is_some() {
            Change::Update {
                part: slot.part,
                class: skill.to_owned(),
            }
        } else {
            Change::Insert {
                part: slot.part,
                class: skill.to_owned(),
            }
        };
        slot.class = Some(skill.to_owned());
        Ok(change)
    }
    /// Gear.Equip(part, itemClass) after its duplicate check, i.e. StackableSlot.Set.
    /// `category` is the ItemClassInfo.Category of the existing class.
    pub fn set(&mut self, part: i32, class: &str, category: &str) -> Result<Change, Refusal> {
        let slot = self
            .slots
            .iter_mut()
            .find(|s| i32::from(s.part) == part)
            .ok_or(Refusal::InvalidSlot)?;
        if slot.locked {
            return Err(Refusal::InvalidSlot);
        }
        if !slot.categories.iter().any(|c| c == category) {
            return Err(Refusal::InvalidItemCategory);
        }
        let change = if slot.class.is_some() {
            Change::Update {
                part: slot.part,
                class: class.to_owned(),
            }
        } else {
            Change::Insert {
                part: slot.part,
                class: class.to_owned(),
            }
        };
        slot.class = Some(class.to_owned());
        Ok(change)
    }
}
