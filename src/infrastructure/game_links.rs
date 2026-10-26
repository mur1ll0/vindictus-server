//! Game names (NexonID) claimed by accounts (B4.2). A claimed name no longer logs in to the game
//! by its passport alone; an account can enter an existing game user, keeping its characters.
use super::admin_store::{self, AuditRecord};
use crate::domain::admin::AuditReason;
use sqlx::PgPool;
use uuid::Uuid;

/// Whether an account enters this game name, so its passport alone no longer logs in.
pub async fn claimed(pool: &PgPool, name_key: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE lower(COALESCE(game_name, username)) = $1)",
    )
    .bind(name_key)
    .fetch_one(pool)
    .await
}

#[derive(Debug, PartialEq, Eq)]
pub enum LinkChange {
    Done,
    NoAccount,
    /// Another account already enters that game name.
    Claimed,
}

/// Operator CLI: an account enters an existing game user (for example the laboratory user
/// created from the allowlist), keeping its characters. Audited.
pub async fn link_game(
    pool: &PgPool,
    normalized: &str,
    game_name: &str,
    reason: &AuditReason,
) -> Result<LinkChange, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let current: Option<(Uuid, Option<String>)> =
        sqlx::query_as("SELECT id, game_name FROM accounts WHERE normalized_name = $1 FOR UPDATE")
            .bind(normalized)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((id, before)) = current else {
        return Ok(LinkChange::NoAccount);
    };
    let updated = sqlx::query("UPDATE accounts SET game_name = $2 WHERE id = $1")
        .bind(id)
        .bind(game_name)
        .execute(&mut *tx)
        .await;
    match updated {
        Ok(_) => {}
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => return Ok(LinkChange::Claimed),
        Err(e) => return Err(e),
    }
    admin_store::insert_audit(
        &mut tx,
        &AuditRecord {
            account_id: None,
            actor_name: None,
            role: None,
            operation_id: None,
            action: "account.game_link",
            target_kind: Some("account"),
            target_id: Some(normalized),
            reason: Some(reason),
            before: Some(serde_json::json!({ "game_name": before }).to_string()),
            after: Some(serde_json::json!({ "game_name": game_name }).to_string()),
            refused: false,
            peer: None,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(LinkChange::Done)
}

/// Lower-case game name key an account enters (its game_name, else its login).
pub async fn account_key(pool: &PgPool, account: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT lower(COALESCE(game_name, username)) FROM accounts WHERE id = $1")
        .bind(account)
        .fetch_optional(pool)
        .await
}

/// The same by the account's normalized login.
pub async fn account_key_by_name(
    pool: &PgPool,
    normalized: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT lower(COALESCE(game_name, username)) FROM accounts WHERE normalized_name = $1",
    )
    .bind(normalized)
    .fetch_optional(pool)
    .await
}
