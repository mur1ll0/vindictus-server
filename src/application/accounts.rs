use crate::{
    domain::{Username, hash_password},
    error::Error,
    infrastructure::postgres,
};
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Provision an operator-managed preservation account; game login is unavailable.
pub async fn create_account(
    pool: &PgPool,
    username: Username,
    password: Zeroizing<Vec<u8>>,
) -> Result<Uuid, Error> {
    postgres::check_ready(pool).await?;
    let hash = tokio::task::spawn_blocking(move || hash_password(&password)).await??;
    postgres::insert_account(pool, &username, &hash).await
}
