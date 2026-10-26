use crate::{domain::Username, error::Error, infrastructure::postgres};
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Provision an operator-managed preservation account; game login is unavailable.
pub async fn create_account(
    pool: &PgPool,
    username: Username,
    password: Zeroizing<Vec<u8>>,
) -> Result<Uuid, Error> {
    create_account_with(pool, username, password, crate::domain::MIN_PASSWORD).await
}

/// `minimum` below the default only for the operator's laboratory short-password option.
pub async fn create_account_with(
    pool: &PgPool,
    username: Username,
    password: Zeroizing<Vec<u8>>,
    minimum: usize,
) -> Result<Uuid, Error> {
    postgres::check_ready(pool).await?;
    let hash =
        tokio::task::spawn_blocking(move || crate::domain::hash_password_with(&password, minimum))
            .await??;
    postgres::insert_account(pool, &username, &hash).await
}
