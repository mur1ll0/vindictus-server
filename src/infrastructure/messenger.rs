//! The laboratory messenger service (B5 MS3b, decision D-S1; docs/messenger-design.md): the
//! NMCO calls the lab `nmcogame.dll` tunnels to the server. This first part keeps the passports
//! of accepted game logins, so a messenger LoginAuth (0x110f, the same passport the engine got
//! on its command line) authenticates the same account; a launcher ticket is spent by the game
//! login, so the messenger can only find it here. Only the SHA-256 of a passport is kept, for
//! ten minutes, and nothing is logged.
use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};
use sha2::{Digest, Sha256};

/// How long an accepted login's passport authenticates the messenger.
const PASSPORT_LIFETIME: Duration = Duration::from_secs(600);
/// The registry's bound (a lab server; older entries are dropped first).
const MAX_PASSPORTS: usize = 4096;

fn digest(passport: &str) -> [u8; 32] {
    Sha256::digest(passport.as_bytes()).into()
}

/// The account a passport belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub nexon_sn: i32,
    pub nexon_id: String,
}

struct Entry {
    account: Account,
    at: Instant,
}

/// Passport → account of the recent game logins.
#[derive(Default)]
pub struct Passports {
    entries: Mutex<HashMap<[u8; 32], Entry>>,
}
impl Passports {
    /// After an accepted game login.
    pub fn remember(&self, passport: &str, account: Account, now: Instant) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.retain(|_, e| now.duration_since(e.at) < PASSPORT_LIFETIME);
        if entries.len() >= MAX_PASSPORTS
            && let Some(oldest) = entries.iter().min_by_key(|(_, e)| e.at).map(|(k, _)| *k)
        {
            entries.remove(&oldest);
        }
        entries.insert(digest(passport), Entry { account, at: now });
    }
    /// The account of a messenger LoginAuth's passport, while it is fresh.
    pub fn account(&self, passport: &str, now: Instant) -> Option<Account> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries
            .get(&digest(passport))
            .filter(|e| now.duration_since(e.at) < PASSPORT_LIFETIME)
            .map(|e| e.account.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passports_authenticate_for_ten_minutes() {
        let p = Passports::default();
        let now = Instant::now();
        let account = Account {
            nexon_sn: 31,
            nexon_id: "owner".to_owned(),
        };
        p.remember("ticket-1", account.clone(), now);
        assert_eq!(p.account("ticket-1", now + Duration::from_secs(599)), Some(account));
        assert_eq!(p.account("ticket-1", now + Duration::from_secs(600)), None);
        assert_eq!(p.account("other", now), None);
    }
}
