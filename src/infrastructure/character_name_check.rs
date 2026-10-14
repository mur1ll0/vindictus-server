//! Name check for a measured lab candidate, using real target state in recovered order.
use super::{
    character_lobby::UserKey,
    character_name_proof::{NameProof, NameProofError},
    lab_clock,
    legacy_users::SourceBytes,
};
use crate::domain::{
    character_reservations::account_key,
    game_user::LegacyNameKey,
    legacy_user_data::SqlDateTime,
    name_retention::{
        MatchedCharacter, duplicate_name_available, history_name_blocked, history_threshold,
    },
};
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameCheckResult {
    Success,
    InvalidName,
    DuplicatedName,
    ReservedName,
    ForbiddenName,
    ChangedCharacterName,
}
impl NameCheckResult {
    pub fn error_key(self) -> Option<&'static str> {
        match self {
            Self::Success => None,
            Self::InvalidName => Some("GameUI_Heroes_Error_Create_Character_InvalidName"),
            Self::DuplicatedName => Some("GameUI_Heroes_Error_Create_Character_DuplicatedName"),
            Self::ReservedName => Some("GameUI_Heroes_Error_Create_Character_ReservedName"),
            Self::ForbiddenName => Some("GameUI_Heroes_Error_Create_Character_ForbiddenName"),
            Self::ChangedCharacterName => {
                Some("GameUI_Heroes_Error_Create_Character_ChangedCharacterName")
            }
        }
    }
}
#[derive(Debug, Error)]
pub enum NameCheckError {
    #[error("name-check database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("name check no longer owns the admitted revision")]
    Ownership,
    #[error(transparent)]
    Ledger(#[from] super::character_creation_ledger::CreationLedgerError),
    #[error("name check is outside its measured private profile")]
    Profile,
    #[error("name-check deadline exceeded")]
    Timeout,
}
impl From<NameProofError> for NameCheckError {
    fn from(_: NameProofError) -> Self {
        Self::Profile
    }
}
async fn owned<'p>(
    pool: &'p PgPool,
    key: &UserKey<'_>,
) -> Result<Transaction<'p, Postgres>, NameCheckError> {
    let owner = LegacyNameKey::parse(key.name).map_err(|_| NameCheckError::Ownership)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let actual: Option<(String, i64)> =
        sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE")
            .bind(key.uid)
            .fetch_optional(&mut *tx)
            .await?;
    if !actual.is_some_and(|(n, r)| n == owner.as_str() && r == key.revision) {
        return Err(NameCheckError::Ownership);
    }
    Ok(tx)
}
pub async fn check(
    pool: &PgPool,
    key: UserKey<'_>,
    proof: &NameProof,
    name: Option<&str>,
) -> Result<NameCheckResult, NameCheckError> {
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        check_owned(pool, key, proof, name, false),
    )
    .await
    .map_err(|_| NameCheckError::Timeout)?
}
/// CharacterMaker calls this branch with needChangedNameLogging=true during creation.
/// The UI Check remains on the nonlogging branch.
pub async fn check_for_creation(
    pool: &PgPool,
    key: UserKey<'_>,
    proof: &NameProof,
    name: Option<&str>,
) -> Result<NameCheckResult, NameCheckError> {
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        check_owned(pool, key, proof, name, true),
    )
    .await
    .map_err(|_| NameCheckError::Timeout)?
}
async fn check_owned(
    pool: &PgPool,
    key: UserKey<'_>,
    proof: &NameProof,
    name: Option<&str>,
    logging: bool,
) -> Result<NameCheckResult, NameCheckError> {
    if !proof.valid_request(name)? {
        return Ok(NameCheckResult::InvalidName);
    }
    if proof.forbidden {
        return Ok(NameCheckResult::ForbiddenName);
    }
    let owner = LegacyNameKey::parse(key.name).map_err(|_| NameCheckError::Ownership)?;
    let mut tx = owned(pool, &key).await?;
    // Source FirstOrDefault has no ordering or owner filter. Do not invent either.
    let rows:Vec<(i32,SourceBytes,SourceBytes)>=sqlx::query_as("SELECT row_id,nexon_id_utf16,character_name_utf16 FROM game_reserved_names WHERE NOT created LIMIT 1001 FOR UPDATE").fetch_all(&mut *tx).await?;
    if rows.len() > 1000 {
        return Err(NameCheckError::Profile);
    }
    let mut first = None;
    let mut own_ordinal = false;
    for (id, account, stored) in rows {
        let account = Zeroizing::new(account.into_bytes());
        let stored = Zeroizing::new(stored.into_bytes());
        if first.is_none() && proof.equals(&stored)? {
            first = Some(id)
        }
        if account_key(&account)
            .map_err(|_| NameCheckError::Profile)?
            .as_slice()
            == owner.as_str().as_bytes()
            && proof.ordinal_candidate(&stored)
        {
            own_ordinal = true
        }
    }
    if let Some(id) = first {
        if !own_ordinal {
            tx.commit().await?;
            return Ok(NameCheckResult::ReservedName);
        }
        sqlx::query("UPDATE game_reserved_names SET created=true WHERE row_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    // Original SubmitChanges commits the reservation BEFORE duplicate/history checks.
    tx.commit().await?;
    let mut tx = owned(pool, &key).await?;
    let rows:Vec<(SourceBytes,i32,Option<i64>)>=sqlx::query_as("SELECT name_utf16,status,delete_time_units300 FROM game_character_info WHERE name_utf16 IS NOT NULL LIMIT 1001").fetch_all(&mut *tx).await?;
    if rows.len() > 1000 {
        return Err(NameCheckError::Profile);
    }
    let mut recent = false;
    for (stored, status, deleted) in rows {
        let stored = Zeroizing::new(stored.into_bytes());
        if !proof.equals(&stored)? {
            continue;
        }
        if status != 1 {
            tx.commit().await?;
            return Ok(NameCheckResult::DuplicatedName);
        }
        if let Some(deleted) = deleted {
            let deleted_at = SqlDateTime::from_units300(deleted)
                .and_then(SqlDateTime::framework_date_time)
                .map_err(|_| NameCheckError::Profile)?;
            let now = lab_clock::utc_now().map_err(|_| NameCheckError::Profile)?;
            if !duplicate_name_available(
                false,
                [MatchedCharacter {
                    status,
                    deleted_at: Some(deleted_at),
                }],
                || now,
            ) {
                recent = true
            }
        }
    }
    if recent && !proof.japanese {
        tx.commit().await?;
        return Ok(NameCheckResult::DuplicatedName);
    }
    let rows:Vec<(SourceBytes,i64)>=sqlx::query_as("SELECT old_character_name_utf16,log_date_units300 FROM game_character_name_logs LIMIT 1001").fetch_all(&mut *tx).await?;
    if rows.len() > 1000 {
        return Err(NameCheckError::Profile);
    }
    let mut dates = Vec::new();
    for (stored, date) in rows {
        let stored = Zeroizing::new(stored.into_bytes());
        if proof.equals(&stored)? {
            dates.push(SqlDateTime::from_units300(date).map_err(|_| NameCheckError::Profile)?);
        }
    }
    let utc_units: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
    )
    .fetch_one(&mut *tx)
    .await?;
    let threshold = history_threshold(
        SqlDateTime::from_units300(utc_units).map_err(|_| NameCheckError::Profile)?,
        proof.protect_period,
    )
    .map_err(|_| NameCheckError::Profile)?;
    let blocked = history_name_blocked(dates, threshold);
    tx.commit().await?;
    if !blocked && logging {
        super::character_creation_ledger::changed_name_check(
            pool,
            &key,
            name.ok_or(NameCheckError::Profile)?,
            proof.protect_period,
        )
        .await?;
    }
    Ok(if blocked {
        NameCheckResult::ChangedCharacterName
    } else {
        NameCheckResult::Success
    })
}
