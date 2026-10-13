//! Identity-bound internal lobby query. No client messages are emitted here.
use super::frontend_session::{FrontendSession, SessionError};
use crate::{
    domain::{
        character_lobby::EmptyLobby,
        feature_policy::FeaturePolicy,
        legacy_user_data::{LegacyUserDataError, SqlDateTime2},
    },
    error::Error,
    infrastructure::character_lobby::{self as storage, LobbyError, QueryConfig, UserKey},
};
use sqlx::PgPool;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum LobbyQueryError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Storage(#[from] LobbyError),
    #[error(transparent)]
    Policy(#[from] Error),
    #[error(transparent)]
    Clock(#[from] LegacyUserDataError),
    #[error(transparent)]
    Wire(#[from] crate::protocol::empty_character_list::CharacterListWireError),
    #[error("character-lobby policy lacks recovered required declarations")]
    Profile,
}
/// No caller-provided UID and no invented wire acknowledgement.
pub async fn mark_prologue_played_admitted(
    frontend: &FrontendSession,
    pool: &PgPool,
    revision: i64,
) -> Result<(), LobbyQueryError> {
    let id = frontend.accepted_identity()?;
    storage::mark_prologue_played(
        pool,
        UserKey {
            uid: id.nexon_sn,
            name: &id.nexon_id,
            revision,
        },
    )
    .await?;
    Ok(())
}
pub async fn query_empty_admitted(
    frontend: &FrontendSession,
    pool: &PgPool,
    revision: i64,
    policy: &FeaturePolicy,
    local_now: SqlDateTime2,
) -> Result<EmptyLobby, LobbyQueryError> {
    let id = frontend.accepted_identity()?;
    for key in [
        "MaxCharacterSlot",
        "UserCareSystem",
        "DisableInGameCheat",
        "DisableCharacterSlotCheat",
    ] {
        if !policy.entries().any(|(k, _)| k == key) {
            return Err(LobbyQueryError::Profile);
        }
    }
    let max_free = if frontend.accepted_permissions()?.has_cheat()
        && !policy.is_enabled(Some("DisableInGameCheat"))?
        && !policy.is_enabled(Some("DisableCharacterSlotCheat"))?
    {
        10
    } else {
        policy.get_integer("MaxCharacterSlot")?
    };
    let config = QueryConfig {
        max_free,
        care_clock: if policy.is_enabled(Some("UserCareSystem"))? {
            Some(local_now.sql_parameter()?)
        } else {
            None
        },
    };
    Ok(storage::query_empty(
        pool,
        UserKey {
            uid: id.nexon_sn,
            name: &id.nexon_id,
            revision,
        },
        config,
    )
    .await?)
}

/// Query and encode the recovered response; the TCP dispatcher is a separate dependency.
pub async fn prepare_empty_character_list(
    frontend: &FrontendSession,
    pool: &PgPool,
    revision: i64,
    policy: &FeaturePolicy,
    local_now: SqlDateTime2,
) -> Result<Vec<u8>, LobbyQueryError> {
    let lobby = query_empty_admitted(frontend, pool, revision, policy, local_now).await?;
    let message = crate::protocol::empty_character_list::Message {
        characters_present: true,
        max_free: lobby.max_free,
        max_purchased: lobby.max_purchased,
        max_premium: lobby.max_premium,
        prologue_played: u8::from(lobby.prologue_played),
        preset_used: lobby.preset_used,
        party_states: Some(Vec::new()),
    };
    Ok(crate::protocol::empty_character_list::encode(
        Some(&message),
        crate::protocol::DEFAULT_MAX_BODY,
    )?)
}
