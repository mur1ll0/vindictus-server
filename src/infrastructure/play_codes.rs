//! One-time play codes (B4.2): issued to an account after its password is checked, redeemed
//! once by the laboratory frontend in place of the passport.
use super::admin_store::{self, AuditRecord};
use crate::domain::admin::AuditReason;
use sqlx::PgPool;
use uuid::Uuid;

/// Issue a code. Earlier unredeemed codes of the account stop working: one valid code each.
pub async fn issue(
    pool: &PgPool,
    account: Uuid,
    code_sha256: &[u8; 32],
    ttl_seconds: f64,
    audit: &AuditRecord<'_>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "DELETE FROM game_play_codes
         WHERE account_id = $1 OR expires_at <= now() OR redeemed_at <= now() - interval '1 day'",
    )
    .bind(account)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO game_play_codes (code_sha256, account_id, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))",
    )
    .bind(code_sha256.as_slice())
    .bind(account)
    .bind(ttl_seconds)
    .execute(&mut *tx)
    .await?;
    admin_store::insert_audit(&mut tx, audit).await?;
    tx.commit().await
}

/// Use a code: the game account name (NexonID) of its active account, at most once.
pub async fn redeem(pool: &PgPool, code_sha256: &[u8; 32]) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE game_play_codes c SET redeemed_at = now()
         FROM accounts a
         WHERE c.code_sha256 = $1 AND a.id = c.account_id
           AND c.redeemed_at IS NULL AND c.expires_at > now() AND a.status = 'active'
         RETURNING COALESCE(a.game_name, a.username)",
    )
    .bind(code_sha256.as_slice())
    .fetch_optional(pool)
    .await
}

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
