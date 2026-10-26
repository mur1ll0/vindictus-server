use crate::error::Error;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

/// Validated ASCII username retaining display case and a case-insensitive identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Username {
    display: String,
    normalized: String,
}

impl Username {
    pub fn parse(value: &str) -> Result<Self, Error> {
        if !(3..=32).contains(&value.len())
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(Error::InvalidUsername);
        }
        Ok(Self {
            display: value.to_owned(),
            normalized: value.to_ascii_lowercase(),
        })
    }
    pub fn display(&self) -> &str {
        &self.display
    }
    pub fn normalized(&self) -> &str {
        &self.normalized
    }
}

pub const MIN_PASSWORD: usize = 12;
/// Operator decision (2026-10-25): one laboratory account (`admin`) may use a short password,
/// only through the explicit CLI option; the listeners are loopback-only.
pub const MIN_LAB_SHORT_PASSWORD: usize = 5;

pub fn validate_password(password: &[u8]) -> Result<(), Error> {
    validate_password_with(password, MIN_PASSWORD)
}

pub fn validate_password_with(password: &[u8], minimum: usize) -> Result<(), Error> {
    if !(minimum..=128).contains(&password.len())
        || password.iter().any(|b| matches!(b, 0 | b'\r' | b'\n'))
    {
        return Err(Error::InvalidPassword);
    }
    Ok(())
}

/// Hash an independent preservation account password as an Argon2id PHC string.
/// This does not implement the legacy Vindictus credential format.
pub fn hash_password(password: &[u8]) -> Result<String, Error> {
    hash_password_with(password, MIN_PASSWORD)
}

pub fn hash_password_with(password: &[u8], minimum: usize) -> Result<String, Error> {
    validate_password_with(password, minimum)?;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password, &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| Error::PasswordHash)
}

/// Check a password against a stored Argon2id PHC string; an unparsable hash never matches.
pub fn verify_password(password: &[u8], hash: &str) -> bool {
    PasswordHash::new(hash)
        .is_ok_and(|parsed| Argon2::default().verify_password(password, &parsed).is_ok())
}
