//! Lossless LimitedAccount column with bounded, fail-closed ASCII equality.
use crate::{
    domain::{game_user::LegacyNameKey, legacy_user_data::LegacyBytes},
    error::Error,
};
use sqlx::PgPool;

/// Snapshot insertion preserves duplicates because the source table has no key.
/// Surrogate local_id is operational only and never pretends to be a source identifier.
pub async fn insert_snapshot(pool: &PgPool, name: &LegacyBytes<24, true>) -> Result<i64, Error> {
    Ok(sqlx::query_scalar(
        "INSERT INTO game_limited_accounts(nexon_id_utf16) VALUES($1) RETURNING local_id",
    )
    .bind(name.as_bytes())
    .fetch_one(pool)
    .await?)
}
/// Matches Limited(...).SingleOrDefault, not an EXISTS boolean on possibly duplicated rows.
/// Unknown Unicode equality denies the policy rather than silently missing a restriction.
pub async fn is_limited(pool: &PgPool, name: &str) -> Result<bool, Error> {
    let key = LegacyNameKey::parse(name)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let keys: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT name_key FROM game_limited_accounts WHERE name_key=$1 OR name_key IS NULL LIMIT 2",
    )
    .bind(key.as_str())
    .fetch_all(&mut *tx)
    .await?;
    if keys.iter().any(Option::is_none) {
        return Err(Error::LimitedAccountUnsupported);
    }
    if keys.len() > 1 {
        return Err(Error::LimitedAccountDuplicate);
    }
    tx.commit().await?;
    Ok(!keys.is_empty())
}
