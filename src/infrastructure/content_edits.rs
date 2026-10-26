//! Content edits of the admin panel (B4.4): pending edits, published versions, the cumulative
//! overlay the host script exports, and the applied mark. Kept apart from character state.
use super::admin_store::{self, AuditRecord};
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ContentEdit {
    pub id: i64,
    /// shop_row, shop_row_remove, cash_price or cash_default.
    pub kind: String,
    /// "<ShopID>#<Order>", the category, or "default".
    pub target: String,
    /// JSON text of the new values (none for a removal).
    pub value: Option<String>,
    /// pending, published or withdrawn.
    pub state: String,
    pub version: Option<i32>,
    pub created_by: Option<String>,
    /// UTC, RFC 3339.
    pub created_at: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ContentVersion {
    pub version: i32,
    pub published_by: Option<String>,
    pub published_at: String,
    pub reason: String,
    /// When the host script applied it (exporters, client copy, game restart).
    pub applied_at: Option<String>,
    pub edits: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditOutcome {
    Done(i64),
    NotFound,
    Nothing,
    Repeated,
}

async fn finish(
    mut tx: Transaction<'_, Postgres>,
    audit: &AuditRecord<'_>,
    value: i64,
) -> Result<EditOutcome, sqlx::Error> {
    match admin_store::insert_audit(&mut tx, audit).await {
        Ok(()) => {
            tx.commit().await?;
            Ok(EditOutcome::Done(value))
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(EditOutcome::Repeated),
        Err(e) => Err(e),
    }
}

pub async fn create(
    pool: &PgPool,
    kind: &str,
    target: &str,
    value: Option<&serde_json::Value>,
    author: Uuid,
    reason: &str,
    mut audit: AuditRecord<'_>,
) -> Result<EditOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO content_edits (kind, target, value, created_by, reason)
         VALUES ($1, $2, $3::jsonb, $4, $5) RETURNING id",
    )
    .bind(kind)
    .bind(target)
    .bind(value.map(|v| v.to_string()))
    .bind(author)
    .bind(reason)
    .fetch_one(&mut *tx)
    .await?;
    audit.after = Some(
        serde_json::json!({ "edit": id, "kind": kind, "target": target, "value": value })
            .to_string(),
    );
    finish(tx, &audit, id).await
}

/// Withdraw a pending edit (it stays, marked withdrawn).
pub async fn withdraw(
    pool: &PgPool,
    id: i64,
    mut audit: AuditRecord<'_>,
) -> Result<EditOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let done = sqlx::query(
        "UPDATE content_edits SET state = 'withdrawn' WHERE id = $1 AND state = 'pending'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if done == 0 {
        return Ok(EditOutcome::NotFound);
    }
    audit.before = Some(serde_json::json!({ "edit": id, "state": "pending" }).to_string());
    audit.after = Some(serde_json::json!({ "edit": id, "state": "withdrawn" }).to_string());
    finish(tx, &audit, id).await
}

/// Publish every pending edit as the next version.
pub async fn publish(
    pool: &PgPool,
    author: Uuid,
    reason: &str,
    mut audit: AuditRecord<'_>,
) -> Result<EditOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("LOCK TABLE content_versions IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM content_edits WHERE state = 'pending'")
            .fetch_one(&mut *tx)
            .await?;
    if pending == 0 {
        return Ok(EditOutcome::Nothing);
    }
    let version: i32 =
        sqlx::query_scalar("SELECT COALESCE(max(version), 0) + 1 FROM content_versions")
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("INSERT INTO content_versions (version, published_by, reason) VALUES ($1, $2, $3)")
        .bind(version)
        .bind(author)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE content_edits SET state = 'published', version = $1 WHERE state = 'pending'",
    )
    .bind(version)
    .execute(&mut *tx)
    .await?;
    audit.after = Some(serde_json::json!({ "version": version, "edits": pending }).to_string());
    finish(tx, &audit, i64::from(version)).await
}

pub async fn list(pool: &PgPool, limit: i64) -> Result<Vec<ContentEdit>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.id, e.kind, e.target, e.value::text AS value, e.state, e.version,
                a.username AS created_by,
                to_char(e.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                e.reason
         FROM content_edits e LEFT JOIN accounts a ON a.id = e.created_by
         ORDER BY (e.state = 'pending') DESC, e.id DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn versions(pool: &PgPool, limit: i64) -> Result<Vec<ContentVersion>, sqlx::Error> {
    sqlx::query_as(
        "SELECT v.version, a.username AS published_by,
                to_char(v.published_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS published_at,
                v.reason,
                to_char(v.applied_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS applied_at,
                (SELECT count(*) FROM content_edits e WHERE e.version = v.version) AS edits
         FROM content_versions v LEFT JOIN accounts a ON a.id = v.published_by
         ORDER BY v.version DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// The cumulative overlay of every published edit (a later edit of a target wins), as the
/// content exporters read it (`.cache/private-content/content-edits.json`).
pub async fn overlay(pool: &PgPool) -> Result<serde_json::Value, sqlx::Error> {
    let rows: Vec<(String, String, Option<String>, i32)> = sqlx::query_as(
        "SELECT kind, target, value::text, version FROM content_edits
         WHERE state = 'published' ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    let mut shops = std::collections::BTreeMap::new();
    let mut prices = std::collections::BTreeMap::new();
    let mut default = serde_json::Value::Null;
    let mut version = 0;
    for (kind, target, value, v) in rows {
        version = version.max(v);
        let value: serde_json::Value = value
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?
            .unwrap_or(serde_json::Value::Null);
        match kind.as_str() {
            "shop_row" => {
                shops.insert(target, value);
            }
            "shop_row_remove" => {
                shops.insert(target, serde_json::Value::Null);
            }
            "cash_price" => {
                prices.insert(target, value["price"].clone());
            }
            "cash_default" => default = value["price"].clone(),
            _ => {}
        }
    }
    // A removed row is {"ShopID","Order","Remove":true}; the target carries both keys.
    let shop_rows: Vec<serde_json::Value> = shops
        .into_iter()
        .filter_map(|(target, value)| {
            let (shop, order) = target.rsplit_once('#')?;
            let order: i64 = order.parse().ok()?;
            Some(if value.is_null() {
                serde_json::json!({ "ShopID": shop, "Order": order, "Remove": true })
            } else {
                let mut row = value;
                row["ShopID"] = serde_json::json!(shop);
                row["Order"] = serde_json::json!(order);
                row
            })
        })
        .collect();
    Ok(serde_json::json!({
        "format_version": 1,
        "version": version,
        "shop_rows": shop_rows,
        "cash_prices": prices,
        "cash_default": default,
    }))
}

/// Host script: the version it exported is now applied.
pub async fn mark_applied(pool: &PgPool, version: i32) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE content_versions SET applied_at = now() WHERE version <= $1 AND applied_at IS NULL",
    )
    .bind(version)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}
