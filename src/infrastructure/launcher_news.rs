//! Launcher news (L5, migration 0070): the panel writes them; the visible ones become the update
//! store's signed news.json.
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// One news item as the panel lists it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct NewsRow {
    pub id: i64,
    pub title: String,
    pub body: String,
    /// Seconds since the Unix epoch.
    pub published_unix: i64,
    pub author: Option<String>,
    pub hidden: bool,
}

const SELECT: &str =
    "SELECT n.id, n.title, n.body, EXTRACT(EPOCH FROM n.published_at)::BIGINT AS published_unix,
        a.username AS author, n.hidden
   FROM launcher_news n LEFT JOIN accounts a ON a.id = n.account_id";

/// The newest items, hidden ones included (the panel's list).
pub async fn list(pool: &PgPool, limit: i64) -> Result<Vec<NewsRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{SELECT} ORDER BY n.published_at DESC, n.id DESC LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// The newest visible items (the feed), inside the change's transaction.
pub async fn visible(
    tx: &mut Transaction<'_, Postgres>,
    limit: i64,
) -> Result<Vec<NewsRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE NOT n.hidden ORDER BY n.published_at DESC, n.id DESC LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(&mut **tx)
    .await
}

pub async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    title: &str,
    body: &str,
    account_id: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO launcher_news(title, body, account_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(title)
    .bind(body)
    .bind(account_id)
    .fetch_one(&mut **tx)
    .await
}

/// Hides or shows an item; false when there is none with this id.
pub async fn set_hidden(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    hidden: bool,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("UPDATE launcher_news SET hidden = $2 WHERE id = $1")
            .bind(id)
            .bind(hidden)
            .execute(&mut **tx)
            .await?
            .rows_affected()
            == 1,
    )
}
