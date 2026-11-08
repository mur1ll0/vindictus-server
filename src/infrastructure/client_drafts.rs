//! Drafts of a client version and the client builder's jobs (migration 0071, docs/future/game-launcher.md
//! 4.10): files to put or remove at a game path, published as N+1 by the panel.
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// The open draft.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct DraftRow {
    pub id: i64,
    /// The published version it applies to.
    pub base_version: i32,
    /// Seconds since the Unix epoch.
    pub created_unix: i64,
    pub author: Option<String>,
}

/// One change of a draft.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ItemRow {
    pub path: String,
    /// `put` (add or replace) or `remove`.
    pub action: String,
    pub sha256: Option<String>,
    pub size: Option<i64>,
    /// The published file it replaces or removes; none for a new path.
    pub before_sha256: Option<String>,
    /// `upload` (panel) or `content` (the published panel content, applied by the client builder).
    pub source: String,
}

/// A client builder job, for the panel.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct JobRow {
    pub id: i64,
    pub kind: String,
    /// `queued`, `running`, `done` or `failed`.
    pub state: String,
    pub created_unix: i64,
    pub finished_unix: Option<i64>,
    pub message: Option<String>,
}

const DRAFT: &str =
    "SELECT d.id, d.base_version, EXTRACT(EPOCH FROM d.created_at)::BIGINT AS created_unix,
        a.username AS author
   FROM client_drafts d LEFT JOIN accounts a ON a.id = d.account_id WHERE d.state = 'open'";

pub async fn open_draft(pool: &PgPool) -> Result<Option<DraftRow>, sqlx::Error> {
    sqlx::query_as(DRAFT).fetch_optional(pool).await
}

/// The open draft, locked for the change; created on `base_version` when there is none.
pub async fn lock_or_create(
    tx: &mut Transaction<'_, Postgres>,
    base_version: i32,
    account_id: Option<Uuid>,
) -> Result<DraftRow, sqlx::Error> {
    sqlx::query(
        "INSERT INTO client_drafts(base_version, account_id) VALUES ($1, $2)
         ON CONFLICT ((state)) WHERE state = 'open' DO NOTHING",
    )
    .bind(base_version)
    .bind(account_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query_as(&format!("{DRAFT} FOR UPDATE OF d"))
        .fetch_one(&mut **tx)
        .await
}

/// The open draft, locked (none when there is no open draft).
pub async fn lock_open(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Option<DraftRow>, sqlx::Error> {
    sqlx::query_as(&format!("{DRAFT} FOR UPDATE OF d"))
        .fetch_optional(&mut **tx)
        .await
}

const ITEMS: &str =
    "SELECT path, action, sha256, size, before_sha256, source FROM client_draft_items
   WHERE draft_id = $1 ORDER BY lower(path)";

pub async fn items(pool: &PgPool, draft_id: i64) -> Result<Vec<ItemRow>, sqlx::Error> {
    sqlx::query_as(ITEMS).bind(draft_id).fetch_all(pool).await
}

pub async fn items_in(
    tx: &mut Transaction<'_, Postgres>,
    draft_id: i64,
) -> Result<Vec<ItemRow>, sqlx::Error> {
    sqlx::query_as(ITEMS)
        .bind(draft_id)
        .fetch_all(&mut **tx)
        .await
}

/// Puts the change for `item.path` in the draft (replacing any change of the same path, any case).
pub async fn upsert_item(
    tx: &mut Transaction<'_, Postgres>,
    draft_id: i64,
    item: &ItemRow,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM client_draft_items WHERE draft_id = $1 AND lower(path) = lower($2)")
        .bind(draft_id)
        .bind(&item.path)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO client_draft_items(draft_id, path, action, sha256, size, before_sha256, source)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(draft_id)
    .bind(&item.path)
    .bind(&item.action)
    .bind(&item.sha256)
    .bind(item.size)
    .bind(&item.before_sha256)
    .bind(&item.source)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Takes `path` out of the draft; false when it was not there.
pub async fn delete_item(
    tx: &mut Transaction<'_, Postgres>,
    draft_id: i64,
    path: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM client_draft_items WHERE draft_id = $1 AND lower(path) = lower($2)",
    )
    .bind(draft_id)
    .bind(path)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        > 0)
}

/// Replaces the draft's content items with `items`; a path the panel uploaded or removed by hand keeps
/// that change. Returns the paths left to the panel's change.
pub async fn replace_content_items(
    tx: &mut Transaction<'_, Postgres>,
    draft_id: i64,
    items: &[ItemRow],
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query("DELETE FROM client_draft_items WHERE draft_id = $1 AND source = 'content'")
        .bind(draft_id)
        .execute(&mut **tx)
        .await?;
    let manual: std::collections::HashSet<String> = items_in(tx, draft_id)
        .await?
        .into_iter()
        .map(|i| i.path.to_ascii_lowercase())
        .collect();
    let mut kept = Vec::new();
    for item in items {
        if manual.contains(&item.path.to_ascii_lowercase()) {
            kept.push(item.path.clone());
            continue;
        }
        upsert_item(tx, draft_id, item).await?;
    }
    Ok(kept)
}

/// Closes the open draft: `published` (with the version) or `discarded`.
pub async fn close(
    tx: &mut Transaction<'_, Postgres>,
    draft_id: i64,
    published_version: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE client_drafts SET state = CASE WHEN $2::INTEGER IS NULL THEN 'discarded' ELSE 'published' END,
                published_version = $2, closed_at = CURRENT_TIMESTAMP
          WHERE id = $1 AND state = 'open'",
    )
    .bind(draft_id)
    .bind(published_version)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ------------------------------------------------------------------ client builder jobs

const JOB: &str = "SELECT id, kind, state, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_unix,
        EXTRACT(EPOCH FROM finished_at)::BIGINT AS finished_unix, message FROM client_build_jobs";

/// Queues a job; none when one of the same kind is already queued or running.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    kind: &str,
    account_id: Option<Uuid>,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO client_build_jobs(kind, account_id) VALUES ($1, $2)
         ON CONFLICT ((kind)) WHERE state IN ('queued','running') DO NOTHING RETURNING id",
    )
    .bind(kind)
    .bind(account_id)
    .fetch_optional(&mut **tx)
    .await
}

/// The oldest queued job, now running (FOR UPDATE SKIP LOCKED: one builder takes each job).
pub async fn claim(pool: &PgPool) -> Result<Option<JobRow>, sqlx::Error> {
    sqlx::query_as(
        "WITH next AS (SELECT id FROM client_build_jobs WHERE state = 'queued' ORDER BY id
                         LIMIT 1 FOR UPDATE SKIP LOCKED)
         UPDATE client_build_jobs j SET state = 'running', started_at = CURRENT_TIMESTAMP
           FROM next WHERE j.id = next.id
         RETURNING j.id, j.kind, j.state, EXTRACT(EPOCH FROM j.created_at)::BIGINT AS created_unix,
                   NULL::BIGINT AS finished_unix, j.message",
    )
    .fetch_optional(pool)
    .await
}

/// Ends a job with its outcome (bounded to the column's 2000 characters).
pub async fn finish(pool: &PgPool, id: i64, ok: bool, message: &str) -> Result<(), sqlx::Error> {
    let message: String = message.chars().take(2000).collect();
    sqlx::query(
        "UPDATE client_build_jobs SET state = $2, finished_at = CURRENT_TIMESTAMP, message = $3
          WHERE id = $1 AND state = 'running'",
    )
    .bind(id)
    .bind(if ok { "done" } else { "failed" })
    .bind(message)
    .execute(pool)
    .await?;
    Ok(())
}

/// Jobs a builder left running when it stopped (at its start, nothing else runs them).
pub async fn fail_interrupted(pool: &PgPool) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE client_build_jobs SET state = 'failed', finished_at = CURRENT_TIMESTAMP,
                message = 'interrompido: o construtor do cliente reiniciou'
          WHERE state = 'running'",
    )
    .execute(pool)
    .await?
    .rows_affected())
}

pub async fn recent_jobs(pool: &PgPool, limit: i64) -> Result<Vec<JobRow>, sqlx::Error> {
    sqlx::query_as(&format!("{JOB} ORDER BY id DESC LIMIT $1"))
        .bind(limit)
        .fetch_all(pool)
        .await
}
