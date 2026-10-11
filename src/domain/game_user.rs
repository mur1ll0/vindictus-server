//! Restricted game-user persistence contract. This is not passport authentication.
use crate::domain::legacy_user::{LegacyUserError, LegacyUserInsert, prepare_legacy_user};
use thiserror::Error;

/// Name equality for the printable ASCII subset measured against SQL Server CI_AS.
/// Empty and space-only values are retained here because SQL accepts them; authentication
/// must enforce its own passport policy. Unicode names require a separate proven adapter.
#[derive(PartialEq, Eq)]
pub struct LegacyNameKey(String);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GameUserNameError {
    #[error(transparent)]
    Bound(#[from] LegacyUserError),
    #[error("game user name is outside the verified printable ASCII contract")]
    UnsupportedCharacters,
}

impl LegacyNameKey {
    pub fn parse(name: &str) -> Result<Self, GameUserNameError> {
        // Bound first, without allocating; the recovered DTO and SQL use UTF-16 units.
        if name.encode_utf16().take(25).count() > 24 {
            return Err(LegacyUserError::NameTooLong.into());
        }
        if !name.bytes().all(|byte| (32..=126).contains(&byte)) {
            return Err(GameUserNameError::UnsupportedCharacters);
        }
        Ok(Self(name.trim_end_matches(' ').to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated initial user subset. Private fields prevent accidentally treating this
/// operation as an importer for non-null secondary credentials or legacy timestamps.
pub struct NewGameUser<'a> {
    candidate: LegacyUserInsert<'a>,
    name_key: LegacyNameKey,
}

impl<'a> NewGameUser<'a> {
    pub fn new(
        id: i32,
        name: &'a str,
        service_id: i32,
        channel_code: u8,
    ) -> Result<Self, GameUserNameError> {
        let name_key = LegacyNameKey::parse(name)?;
        let candidate = prepare_legacy_user(id, name, service_id, channel_code)?;
        Ok(Self {
            candidate,
            name_key,
        })
    }

    pub fn id(&self) -> i32 {
        self.candidate.id
    }

    pub fn name(&self) -> &str {
        self.candidate.name
    }

    pub fn name_key(&self) -> &str {
        self.name_key.as_str()
    }

    pub fn next_character_sn(&self) -> i32 {
        self.candidate.next_character_sn
    }

    pub fn service_id(&self) -> i32 {
        self.candidate.service_id
    }

    pub fn channeling_code(&self) -> Option<u8> {
        self.candidate.channeling_code
    }
}

/// Persisted subset, preserving the values of an existing row on repeated provisioning.
/// Deliberately no Debug/Serialize: names must not leak into automatic diagnostics.
#[derive(PartialEq, Eq)]
pub struct GameUser {
    pub id: i32,
    pub name: String,
    pub next_character_sn: i32,
    pub service_id: i32,
    pub channeling_code: Option<i16>,
}

pub struct ProvisionedGameUser {
    pub user: GameUser,
    pub created: bool,
}
