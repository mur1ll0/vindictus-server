//! Account administration for the panel (B4.2). Every change writes its audit row in the
//! same transaction; a repeated `operation_id` violates the audit's unique key and rolls the
//! whole change back.
use super::admin_store::{self, AuditRecord, SessionAccount};
use crate::domain::admin::{AuditReason, PanelRole};
use sqlx::{PgPool, Postgres, Transaction};
use std::net::IpAddr;
use uuid::Uuid;

#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct AccountSummary {
    pub username: String,
    /// Game account name it enters when not its own login (an existing game user).
    pub game_name: Option<String>,
    /// admin, gm, leitura or null (no panel access).
    pub panel_role: Option<String>,
    /// active or disabled.
    pub status: String,
    /// Panel login locked after consecutive wrong passwords.
    pub locked: bool,
    /// UTC, RFC 3339.
    pub created_at: String,
    /// Game user of its game name, created at the first game login, if any.
    pub game_uid: Option<i32>,
    pub characters: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AccountChange {
    Done,
    NoAccount,
    Exists,
    RepeatedOperation,
}

/// Target state the permission rules need.
pub struct Target {
    pub id: Uuid,
    pub username: String,
    pub role: Option<PanelRole>,
    pub active: bool,
}

/// Who changes, for the audit row.
pub struct Actor<'a> {
    pub session: &'a SessionAccount,
    pub peer: Option<IpAddr>,
    pub operation_id: Uuid,
    pub reason: &'a AuditReason,
}

pub async fn list(
    pool: &PgPool,
    search: Option<&str>,
    after: Option<&str>,
    limit: i64,
) -> Result<Vec<AccountSummary>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.username, a.game_name, a.panel_role, a.status,
                COALESCE(a.locked_until > now(), false) AS locked,
                to_char(a.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                u.id AS game_uid,
                (SELECT count(*) FROM game_character_info c WHERE c.uid = u.id) AS characters
         FROM accounts a
         LEFT JOIN game_users u ON u.name_key = lower(COALESCE(a.game_name, a.username))
         WHERE ($1::text IS NULL OR a.normalized_name LIKE '%' || $1 || '%')
           AND ($2::text IS NULL OR a.normalized_name > $2)
         ORDER BY a.normalized_name
         LIMIT $3",
    )
    .bind(search)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
}

async fn target(
    tx: &mut Transaction<'_, Postgres>,
    normalized: &str,
) -> Result<Option<Target>, sqlx::Error> {
    let row: Option<(Uuid, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, username, panel_role, status FROM accounts WHERE normalized_name = $1 FOR UPDATE",
    )
    .bind(normalized)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(|(id, username, role, status)| Target {
        id,
        username,
        role: role.as_deref().and_then(PanelRole::parse),
        active: status == "active",
    }))
}

/// The target of a change, locked; `None` when the account does not exist.
pub async fn lock_target(
    pool: &PgPool,
    normalized: &str,
) -> Result<(Transaction<'static, Postgres>, Option<Target>), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout = '2s'")
        .execute(&mut *tx)
        .await?;
    // Same lock order as admin_store::set_role (admin rows, then the target): no deadlock.
    sqlx::query("SELECT id FROM accounts WHERE panel_role = 'admin' ORDER BY id FOR UPDATE")
        .execute(&mut *tx)
        .await?;
    let found = target(&mut tx, normalized).await?;
    Ok((tx, found))
}

fn audit<'a>(
    actor: &'a Actor<'a>,
    action: &'a str,
    target_id: &'a str,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
) -> AuditRecord<'a> {
    AuditRecord {
        account_id: Some(actor.session.account_id),
        actor_name: Some(&actor.session.username),
        role: Some(actor.session.role),
        operation_id: Some(actor.operation_id),
        action,
        target_kind: Some("account"),
        target_id: Some(target_id),
        reason: Some(actor.reason),
        before: before.map(|v| v.to_string()),
        after: after.map(|v| v.to_string()),
        refused: false,
        peer: actor.peer,
    }
}

async fn finish(
    mut tx: Transaction<'_, Postgres>,
    record: &AuditRecord<'_>,
) -> Result<AccountChange, sqlx::Error> {
    match admin_store::insert_audit(&mut tx, record).await {
        Ok(()) => {
            tx.commit().await?;
            Ok(AccountChange::Done)
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Ok(AccountChange::RepeatedOperation)
        }
        Err(e) => Err(e),
    }
}

pub async fn create(
    pool: &PgPool,
    username: &crate::domain::Username,
    password_hash: &str,
    role: Option<PanelRole>,
    actor: &Actor<'_>,
) -> Result<AccountChange, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO accounts (id, username, normalized_name, password_hash, panel_role)
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(username.display())
    .bind(username.normalized())
    .bind(password_hash)
    .bind(role.map(PanelRole::as_str))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Ok(AccountChange::Exists);
    }
    let after = serde_json::json!({ "username": username.display(), "panel_role": role, "status": "active" });
    finish(
        tx,
        &audit(
            actor,
            "account.create",
            username.normalized(),
            None,
            Some(after),
        ),
    )
    .await
}

/// New password: the old sessions end (password_changed_at) and the lockout is cleared.
pub async fn set_password(
    tx: Transaction<'static, Postgres>,
    target: &Target,
    password_hash: &str,
    actor: &Actor<'_>,
) -> Result<AccountChange, sqlx::Error> {
    let mut tx = tx;
    sqlx::query(
        "UPDATE accounts SET password_hash = $2, password_changed_at = clock_timestamp(),
                failed_logins = 0, locked_until = NULL
         WHERE id = $1",
    )
    .bind(target.id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await?;
    let key = target.username.to_ascii_lowercase();
    // The audit never holds a hash, only that the password changed.
    finish(
        tx,
        &audit(
            actor,
            "account.password",
            &key,
            None,
            Some(serde_json::json!({ "password": "changed" })),
        ),
    )
    .await
}

pub async fn set_status(
    tx: Transaction<'static, Postgres>,
    target: &Target,
    active: bool,
    actor: &Actor<'_>,
) -> Result<AccountChange, sqlx::Error> {
    let mut tx = tx;
    let status = |a: bool| if a { "active" } else { "disabled" };
    sqlx::query("UPDATE accounts SET status = $2 WHERE id = $1")
        .bind(target.id)
        .bind(status(active))
        .execute(&mut *tx)
        .await?;
    if !active {
        sqlx::query("DELETE FROM admin_sessions WHERE account_id = $1")
            .bind(target.id)
            .execute(&mut *tx)
            .await?;
    }
    let key = target.username.to_ascii_lowercase();
    finish(
        tx,
        &audit(
            actor,
            "account.status",
            &key,
            Some(serde_json::json!({ "status": status(target.active) })),
            Some(serde_json::json!({ "status": status(active) })),
        ),
    )
    .await
}

/// Active admins other than `except`; their rows are already locked by `lock_target`.
pub async fn other_active_admins(
    tx: &mut Transaction<'static, Postgres>,
    except: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM accounts WHERE panel_role = 'admin' AND status = 'active' AND id <> $1",
    )
    .bind(except)
    .fetch_one(&mut **tx)
    .await
}
