//! One bounded registry per laboratory runtime. No cross-process/distributed lease claims.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use thiserror::Error;

/// Limits operator configuration and prevents an unbounded ID reservation map.
pub const MAX_REGISTRY_CAPACITY: usize = 16_384;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("session registry capacity must be between 1 and 16384")]
    InvalidCapacity,
    #[error("user already has a laboratory session")]
    Duplicate,
    #[error("session registry is at capacity")]
    AtCapacity,
    #[error("session registry synchronization failed")]
    Poisoned,
}
struct Owner;
struct RegistryState {
    owners: HashMap<i32, Weak<Owner>>,
    capacity: usize,
}
/// Clone this handle for connections belonging to the SAME laboratory runtime.
/// Entries contain only signed user IDs and weak ownership; no names or passports.
#[derive(Clone)]
pub struct SessionRegistry(Arc<Mutex<RegistryState>>);

/// Non-cloneable ownership held by exactly one FrontendSession.
/// Dropping it releases the reservation without locking or an async destructor.
pub(crate) struct Reservation {
    _owner: Arc<Owner>,
}

impl SessionRegistry {
    pub fn with_capacity(capacity: usize) -> Result<Self, RegistryError> {
        if !(1..=MAX_REGISTRY_CAPACITY).contains(&capacity) {
            return Err(RegistryError::InvalidCapacity);
        }
        Ok(Self(Arc::new(Mutex::new(RegistryState {
            owners: HashMap::new(),
            capacity,
        }))))
    }
    pub(crate) fn reserve(&self, id: i32) -> Result<Reservation, RegistryError> {
        let mut state = self.0.lock().map_err(|_| RegistryError::Poisoned)?;
        if state
            .owners
            .get(&id)
            .is_some_and(|owner| owner.strong_count() > 0)
        {
            return Err(RegistryError::Duplicate);
        }
        if state.owners.len() >= state.capacity {
            state.owners.retain(|_, owner| owner.strong_count() > 0);
            if state.owners.len() >= state.capacity {
                return Err(RegistryError::AtCapacity);
            }
        }
        let owner = Arc::new(Owner);
        state.owners.insert(id, Arc::downgrade(&owner));
        Ok(Reservation { _owner: owner })
    }
    /// Aggregate diagnostic only. Cleanup of dead weak entries is bounded by capacity.
    pub fn active_count(&self) -> Result<usize, RegistryError> {
        let mut state = self.0.lock().map_err(|_| RegistryError::Poisoned)?;
        state.owners.retain(|_, owner| owner.strong_count() > 0);
        Ok(state.owners.len())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poison_fails_closed_and_guard_drop_does_not_lock() {
        let registry = SessionRegistry::with_capacity(1).unwrap();
        let reservation = registry.reserve(5).unwrap();
        let other = registry.clone();
        let _ = std::thread::spawn(move || {
            let _lock = other.0.lock().unwrap();
            panic!("synthetic poison");
        })
        .join();
        assert!(matches!(registry.reserve(6), Err(RegistryError::Poisoned)));
        assert_eq!(registry.active_count(), Err(RegistryError::Poisoned));
        drop(reservation);
    }
}

#[cfg(test)]
mod signed_tests {
    use super::*;
    #[test]
    fn signed_source_ids_are_distinct_and_reusable_after_owner_drop() {
        let registry = SessionRegistry::with_capacity(3).unwrap();
        let leases: Vec<_> = [i32::MIN, 0, i32::MAX]
            .into_iter()
            .map(|id| registry.reserve(id).unwrap())
            .collect();
        assert_eq!(registry.active_count().unwrap(), 3);
        for id in [i32::MIN, 0, i32::MAX] {
            assert!(matches!(
                registry.reserve(id),
                Err(RegistryError::Duplicate)
            ));
        }
        drop(leases);
        for id in [i32::MIN, 0, i32::MAX] {
            drop(registry.reserve(id).unwrap());
        }
        assert_eq!(registry.active_count().unwrap(), 0);
    }
}
