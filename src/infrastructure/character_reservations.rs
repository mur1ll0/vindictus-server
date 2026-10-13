//! Real bounded reservation queries. No writes or original SQL Server connection.
use super::{character_lobby::UserKey, legacy_users::SourceBytes};
use crate::domain::{character_reservations::account_key, game_user::LegacyNameKey};
use sqlx::PgPool;
use std::time::Duration;
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum ReservationError {
    #[error("reservation database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("reservation query does not own the admitted revision")]
    Ownership,
    #[error("reservation query exceeds the recovered bounded profile")]
    Profile,
    #[error("reservation query deadline exceeded")]
    Timeout,
}
/// Private names deliberately have no Debug/Serialize and are zeroized after sending.
pub struct ReservedInfo {
    pub names: Zeroizing<Vec<String>>,
    pub titles: Vec<i32>,
}
const MAX_ROWS: i64 = 1000;
pub async fn check_ready(pool: &PgPool) -> Result<(), sqlx::Error> {
    for query in [
        "SELECT row_id,nexon_id_utf16,nexon_sn,character_name_utf16,created FROM game_reserved_names LIMIT 0",
        "SELECT row_id,nexon_id_utf16,nexon_sn,title_id,title_given FROM game_reserved_titles LIMIT 0",
        "SELECT id,old_character_name_utf16,new_character_name_utf16,log_date_units300 FROM game_character_name_logs LIMIT 0",
    ] {
        sqlx::query(query).execute(pool).await?;
    }
    Ok(())
}
pub async fn query(pool: &PgPool, key: UserKey<'_>) -> Result<ReservedInfo, ReservationError> {
    let owner = LegacyNameKey::parse(key.name).map_err(|_| ReservationError::Ownership)?;
    tokio::time::timeout(
        Duration::from_secs(10),
        query_owned(pool, key, owner.as_str()),
    )
    .await
    .map_err(|_| ReservationError::Timeout)?
}
async fn query_owned(
    pool: &PgPool,
    key: UserKey<'_>,
    owner: &str,
) -> Result<ReservedInfo, ReservationError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let actual: Option<(String, i64)> =
        sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE")
            .bind(key.uid)
            .fetch_optional(&mut *tx)
            .await?;
    if !actual.is_some_and(|(n, rev)| n == owner && rev == key.revision) {
        return Err(ReservationError::Ownership);
    }
    // Source filters by NexonID, not NexonSN. Unproved Unicode account equality is refused.
    let rows:Vec<(SourceBytes,SourceBytes)>=sqlx::query_as("SELECT nexon_id_utf16,character_name_utf16 FROM game_reserved_names WHERE NOT created LIMIT $1").bind(MAX_ROWS+1).fetch_all(&mut *tx).await?;
    if rows.len() > MAX_ROWS as usize {
        return Err(ReservationError::Profile);
    }
    let mut names = Zeroizing::new(Vec::new());
    for (account, name) in rows {
        let account = Zeroizing::new(account.into_bytes());
        if account_key(&account)
            .map_err(|_| ReservationError::Profile)?
            .as_slice()
            == owner.as_bytes()
        {
            let name = Zeroizing::new(name.into_bytes());
            if name.len() > 100 || !name.len().is_multiple_of(2) {
                return Err(ReservationError::Profile);
            }
            let units = Zeroizing::new(
                name.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u16::from_le_bytes(*b))
                    .collect::<Vec<_>>(),
            );
            let text = String::from_utf16(&units).map_err(|_| ReservationError::Profile)?;
            names.push(text);
        }
    }
    let rows: Vec<(SourceBytes, i32)> = sqlx::query_as(
        "SELECT nexon_id_utf16,title_id FROM game_reserved_titles WHERE NOT title_given LIMIT $1",
    )
    .bind(MAX_ROWS + 1)
    .fetch_all(&mut *tx)
    .await?;
    if rows.len() > MAX_ROWS as usize {
        return Err(ReservationError::Profile);
    }
    let mut titles = Vec::new();
    for (account, title) in rows {
        let account = Zeroizing::new(account.into_bytes());
        if account_key(&account)
            .map_err(|_| ReservationError::Profile)?
            .as_slice()
            == owner.as_bytes()
        {
            titles.push(title);
        }
    }
    tx.commit().await?;
    Ok(ReservedInfo { names, titles })
}
