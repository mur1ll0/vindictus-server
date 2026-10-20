//! EquippableSlot state transitions recovered from the original PlayerService.
//! This kernel emits effects for its caller; it does not perform SQL, attribute
//! expiration, enhancement callbacks, character constraints or network delivery.
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Success,
    InvalidItemClass,
    EquipSystem,
    InvalidSlot,
    InvalidCategory,
    EnhanceSystem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    CheckExpired,
    RegisterChange,
    EnhanceOther,
    UpdateStat,
    UpdateItem,
    AddExpandedSlot,
    UpdateLockedParts,
}

/// Class/category bytes use the source ordinal UTF16 representation, including
/// unpaired surrogates. No culture comparison or display-string conversion occurs.
#[derive(Clone, PartialEq, Eq)]
pub struct Item {
    pub id: i64,
    pub category: Vec<u8>,
    pub equippable: bool,
    pub equipped_part: Option<u8>,
    pub resolved_part: Option<i32>,
    pub bad_status: bool,
    pub enhance_other: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("equipment slot exceeds the bounded source profile")]
pub struct ProfileError;

pub struct Slot {
    part: u8,
    locked: bool,
    disabled: bool,
    categories: BTreeSet<Vec<u8>>,
    value: Option<Item>,
}

impl Slot {
    /// The first provider row determines IsExpandedSlot; the caller supplies its
    /// observed ordering. Duplicate/null category keys are not repaired.
    pub fn new(
        part: u8,
        rows: impl IntoIterator<Item = (Option<Vec<u8>>, bool)>,
    ) -> Result<Self, ProfileError> {
        let mut categories = BTreeSet::new();
        let mut locked = None;
        for (category, expanded) in rows {
            if categories.len() == 4096 {
                return Err(ProfileError);
            }
            let category = category.ok_or(ProfileError)?;
            Self::validate_category(&category)?;
            if !categories.insert(category) {
                return Err(ProfileError);
            }
            locked.get_or_insert(expanded);
        }
        Ok(Self {
            part,
            locked: locked.ok_or(ProfileError)?,
            disabled: false,
            categories,
            value: None,
        })
    }
    fn validate_category(category: &[u8]) -> Result<(), ProfileError> {
        if category.len() > 1024 || !category.len().is_multiple_of(2) {
            return Err(ProfileError);
        }
        Ok(())
    }
    pub fn part(&self) -> u8 {
        self.part
    }
    pub fn locked(&self) -> bool {
        self.locked
    }
    pub fn disabled(&self) -> bool {
        self.disabled
    }
    pub fn value(&self) -> Option<&Item> {
        self.value.as_ref()
    }
    /// Source Load permits locked slots and does not evaluate constraints. Gear's
    /// outer loader must independently establish IsEquipped before calling it.
    pub fn load(&mut self, item: Item) -> Result<bool, ProfileError> {
        Self::validate_category(&item.category)?;
        if self.value.is_none() && item.resolved_part.is_none_or(|p| p == i32::from(self.part)) {
            self.disabled = item.bad_status;
            self.value = Some(item);
            return Ok(true);
        }
        Ok(false)
    }
    /// The order of refusals matters: locked, null, class, existing part, category.
    pub fn check(&self, item: Option<&Item>) -> Result<Operation, ProfileError> {
        if let Some(item) = item {
            Self::validate_category(&item.category)?;
        }
        Ok(if self.locked {
            Operation::InvalidSlot
        } else if let Some(item) = item {
            if !item.equippable {
                Operation::InvalidCategory
            } else if item.equipped_part.is_some() {
                Operation::EquipSystem
            } else if !self.categories.contains(&item.category) {
                Operation::InvalidCategory
            } else {
                Operation::Success
            }
        } else {
            Operation::EquipSystem
        })
    }
    pub fn set(&mut self, item: Option<Item>) -> Result<(Operation, Vec<Effect>), ProfileError> {
        if let Some(item) = &item {
            Self::validate_category(&item.category)?;
        }
        if self.value.is_some() {
            return Ok((Operation::EnhanceSystem, Vec::new()));
        }
        let result = self.check(item.as_ref())?;
        if result != Operation::Success {
            return Ok((result, Vec::new()));
        }
        let mut item = item.ok_or(ProfileError)?;
        item.equipped_part = Some(self.part);
        self.disabled = item.bad_status;
        let mut effects = vec![Effect::CheckExpired, Effect::RegisterChange];
        if item.enhance_other {
            effects.push(Effect::EnhanceOther);
        }
        effects.push(Effect::UpdateStat);
        self.value = Some(item);
        Ok((Operation::Success, effects))
    }
    /// Returns the cleared item so the storage coordinator can retain its identity
    /// and location. Source changes only EquippedPart; it never moves a bag slot.
    pub fn clear(&mut self) -> (Option<Item>, Vec<Effect>) {
        let Some(mut item) = self.value.take() else {
            return (None, Vec::new());
        };
        item.equipped_part = None;
        self.disabled = false;
        let mut effects = vec![Effect::RegisterChange];
        if item.enhance_other {
            effects.push(Effect::EnhanceOther);
        }
        effects.push(Effect::UpdateStat);
        (Some(item), effects)
    }
    /// Caller provides BadStatus sampled before CheckAttributeExpired, matching
    /// the original Reset ordering rather than silently refreshing it afterwards.
    pub fn reset(&mut self, bad_status: bool) -> (Operation, Vec<Effect>) {
        let Some(item) = &mut self.value else {
            return (Operation::InvalidItemClass, Vec::new());
        };
        item.bad_status = bad_status;
        self.disabled = bad_status;
        let mut effects = vec![
            Effect::UpdateItem,
            Effect::CheckExpired,
            Effect::RegisterChange,
        ];
        if item.enhance_other {
            effects.push(Effect::EnhanceOther);
        }
        effects.push(Effect::UpdateStat);
        (Operation::Success, effects)
    }
    /// Loading an expanded slot never adds a persistence row; explicit Unlock does.
    pub fn load_expansion(&mut self, part: i32) -> Vec<Effect> {
        if part != i32::from(self.part) {
            return Vec::new();
        }
        self.locked = false;
        vec![Effect::UpdateLockedParts]
    }
    pub fn unlock(&mut self) -> Vec<Effect> {
        if !self.locked {
            return Vec::new();
        }
        self.locked = false;
        vec![Effect::AddExpandedSlot, Effect::UpdateLockedParts]
    }
}
