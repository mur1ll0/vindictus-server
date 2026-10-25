//! Administration panel persistence: sessions, lockout and the append-only audit log.
//! Times are computed by PostgreSQL (`now()`); the binary has no timestamp decoder.
use crate::domain::admin::{AuditReason, PanelRole};
use sqlx::{PgPool, Postgres, Transaction};
use std::net::IpAddr;
use uuid::Uuid;

/// Account row the panel login needs. Not Debug: it carries the password hash.
pub struct LoginAccount {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: Option<PanelRole>,
    /// Login locked after failures, or the account is disabled.
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionAccount {
    pub account_id: Uuid,
    pub username: String,
    pub role: PanelRole,
}

/// One audit row to append. `before`/`after` are JSON texts.
pub struct AuditRecord<'a> {
    pub account_id: Option<Uuid>,
    pub actor_name: Option<&'a str>,
    pub role: Option<PanelRole>,
    pub operation_id: Option<Uuid>,
    pub action: &'a str,
    pub target_kind: Option<&'a str>,
    pub target_id: Option<&'a str>,
    pub reason: Option<&'a AuditReason>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub refused: bool,
    pub peer: Option<IpAddr>,
}

#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct AuditEntry {
    pub id: i64,
    /// UTC, RFC 3339 with milliseconds.
    pub at: String,
    pub actor_name: Option<String>,
    pub role: Option<String>,
    pub action: String,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub reason: Option<String>,
    /// JSON text of the state before the change.
    pub before: Option<String>,
    /// JSON text of the state after the change.
    pub after: Option<String>,
    pub outcome: String,
    pub peer: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct AuditFilter {
    pub before_id: Option<i64>,
    pub action: Option<String>,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub limit: i64,
}

fn role(value: Option<String>) -> Result<Option<PanelRole>, sqlx::Error> {
    value
        .map(|v| PanelRole::parse(&v).ok_or_else(|| sqlx::Error::Decode("panel role".into())))
        .transpose()
}

pub async fn login_account(
    pool: &PgPool,
    normalized: &str,
) -> Result<Option<LoginAccount>, sqlx::Error> {
    let row: Option<(Uuid, String, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT id, username, password_hash, panel_role,
                COALESCE(locked_until > now(), false) OR status <> 'active'
         FROM accounts WHERE normalized_name = $1",
    )
    .bind(normalized)
    .fetch_optional(pool)
    .await?;
    row.map(|(id, username, password_hash, r, locked)| {
        Ok(LoginAccount {
            id,
            username,
            password_hash,
            role: role(r)?,
            locked,
        })
    })
    .transpose()
}

pub async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    record: &AuditRecord<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO admin_audit_events
            (account_id, actor_name, role, operation_id, action, target_kind, target_id,
             reason, before, after, outcome, peer)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb, $10::jsonb, $11, $12::inet)",
    )
    .bind(record.account_id)
    .bind(record.actor_name)
    .bind(record.role.map(PanelRole::as_str))
    .bind(record.operation_id)
    .bind(record.action)
    .bind(record.target_kind)
    .bind(record.target_id)
    .bind(record.reason.map(AuditReason::as_str))
    .bind(record.before.as_deref())
    .bind(record.after.as_deref())
    .bind(if record.refused { "refused" } else { "ok" })
    .bind(record.peer.map(|p| p.to_string()))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Failed login: count it and, at `max_failures`, lock the panel login for `lock_seconds`.
pub async fn record_failure(
    pool: &PgPool,
    account: Option<Uuid>,
    audit: &AuditRecord<'_>,
    max_failures: i32,
    lock_seconds: f64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    if let Some(id) = account {
        sqlx::query(
            "UPDATE accounts SET
                locked_until = CASE WHEN failed_logins + 1 >= $2
                    THEN now() + make_interval(secs => $3) ELSE locked_until END,
                failed_logins = CASE WHEN failed_logins + 1 >= $2 THEN 0 ELSE failed_logins + 1 END
             WHERE id = $1",
        )
        .bind(id)
        .bind(max_failures)
        .bind(lock_seconds)
        .execute(&mut *tx)
        .await?;
    }
    insert_audit(&mut tx, audit).await?;
    tx.commit().await
}

/// Successful login: reset the failure count, drop stale sessions and open a new one.
pub async fn open_session(
    pool: &PgPool,
    account: Uuid,
    token_sha256: &[u8; 32],
    session_seconds: f64,
    idle_seconds: f64,
    audit: &AuditRecord<'_>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE accounts SET failed_logins = 0, locked_until = NULL WHERE id = $1")
        .bind(account)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM admin_sessions
         WHERE expires_at <= now() OR last_seen_at <= now() - make_interval(secs => $1)",
    )
    .bind(idle_seconds)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO admin_sessions (token_sha256, account_id, password_changed_at, expires_at)
         SELECT $1, id, password_changed_at, now() + make_interval(secs => $3)
         FROM accounts WHERE id = $2",
    )
    .bind(token_sha256.as_slice())
    .bind(account)
    .bind(session_seconds)
    .execute(&mut *tx)
    .await?;
    insert_audit(&mut tx, audit).await?;
    tx.commit().await
}

/// A live session of an account that still has a role and the same password; touches it.
pub async fn touch_session(
    pool: &PgPool,
    token_sha256: &[u8; 32],
    idle_seconds: f64,
) -> Result<Option<SessionAccount>, sqlx::Error> {
    let row: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "UPDATE admin_sessions s SET last_seen_at = now()
         FROM accounts a
         WHERE s.token_sha256 = $1 AND a.id = s.account_id
           AND s.expires_at > now()
           AND s.last_seen_at > now() - make_interval(secs => $2)
           AND a.panel_role IS NOT NULL AND a.status = 'active'
           AND a.password_changed_at = s.password_changed_at
         RETURNING a.id, a.username, a.panel_role",
    )
    .bind(token_sha256.as_slice())
    .bind(idle_seconds)
    .fetch_optional(pool)
    .await?;
    row.map(|(account_id, username, r)| {
        Ok(SessionAccount {
            account_id,
            username,
            role: role(r)?.ok_or_else(|| sqlx::Error::Decode("panel role".into()))?,
        })
    })
    .transpose()
}

pub async fn close_session(
    pool: &PgPool,
    token_sha256: &[u8; 32],
    audit: &AuditRecord<'_>,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let removed = sqlx::query("DELETE FROM admin_sessions WHERE token_sha256 = $1")
        .bind(token_sha256.as_slice())
        .execute(&mut *tx)
        .await?
        .rows_affected()
        == 1;
    if removed {
        insert_audit(&mut tx, audit).await?;
    }
    tx.commit().await?;
    Ok(removed)
}

pub async fn audit_page(
    pool: &PgPool,
    filter: &AuditFilter,
) -> Result<Vec<AuditEntry>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id,
                to_char(at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS at,
                actor_name, role, action, target_kind, target_id, reason,
                before::text AS before, after::text AS after, outcome, host(peer) AS peer
         FROM admin_audit_events
         WHERE ($1::bigint IS NULL OR id < $1)
           AND ($2::text IS NULL OR action = $2)
           AND ($3::text IS NULL OR target_kind = $3)
           AND ($4::text IS NULL OR target_id = $4)
         ORDER BY id DESC
         LIMIT $5",
    )
    .bind(filter.before_id)
    .bind(filter.action.as_deref())
    .bind(filter.target_kind.as_deref())
    .bind(filter.target_id.as_deref())
    .bind(filter.limit)
    .fetch_all(pool)
    .await
}

#[derive(Debug, PartialEq, Eq)]
pub enum RoleChange {
    Changed {
        before: Option<PanelRole>,
        after: Option<PanelRole>,
    },
    NoAccount,
    LastAdmin,
    RepeatedOperation,
}

/// Set or remove a panel role. The last admin cannot lose the role; admin rows are locked so
/// two concurrent demotions cannot both pass the check.
pub async fn set_role(
    pool: &PgPool,
    normalized: &str,
    new_role: Option<PanelRole>,
    reason: &AuditReason,
    actor: Option<(&SessionAccount, Option<IpAddr>, Uuid)>,
) -> Result<RoleChange, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout = '2s'")
        .execute(&mut *tx)
        .await?;
    let admins: Vec<(Uuid,)> =
        sqlx::query_as("SELECT id FROM accounts WHERE panel_role = 'admin' ORDER BY id FOR UPDATE")
            .fetch_all(&mut *tx)
            .await?;
    let target: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, username, panel_role FROM accounts WHERE normalized_name = $1 FOR UPDATE",
    )
    .bind(normalized)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id, username, current)) = target else {
        return Ok(RoleChange::NoAccount);
    };
    let before = role(current)?;
    if before == Some(PanelRole::Admin) && new_role != Some(PanelRole::Admin) && admins.len() <= 1 {
        return Ok(RoleChange::LastAdmin);
    }
    sqlx::query("UPDATE accounts SET panel_role = $2 WHERE id = $1")
        .bind(id)
        .bind(new_role.map(PanelRole::as_str))
        .execute(&mut *tx)
        .await?;
    let json = |r: Option<PanelRole>| serde_json::json!({ "panel_role": r }).to_string();
    let target_id = username.to_ascii_lowercase();
    match insert_audit(
        &mut tx,
        &AuditRecord {
            account_id: actor.map(|(a, _, _)| a.account_id),
            actor_name: actor.map(|(a, _, _)| a.username.as_str()),
            role: actor.map(|(a, _, _)| a.role),
            operation_id: actor.map(|(_, _, o)| o),
            action: "account.role",
            target_kind: Some("account"),
            target_id: Some(&target_id),
            reason: Some(reason),
            before: Some(json(before)),
            after: Some(json(new_role)),
            refused: false,
            peer: actor.and_then(|(_, p, _)| p),
        },
    )
    .await
    {
        Ok(()) => {}
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Ok(RoleChange::RepeatedOperation);
        }
        Err(e) => return Err(e),
    }
    tx.commit().await?;
    Ok(RoleChange::Changed {
        before,
        after: new_role,
    })
}
