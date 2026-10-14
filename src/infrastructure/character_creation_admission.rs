//! Read-only CharacterMaker admission and its original initial-SN collision probe.
//! No creation success, CID allocation, initialization or client response is produced here.
use super::{character_lobby::UserKey, legacy_users::SourceBytes};
use crate::domain::{
    character_allocator::{CharacterSerial, first_character_serial, next_character_serial},
    character_creation_admission::CreationSlots,
    character_lobby::{ProductKind, classify_product},
    game_user::LegacyNameKey,
    legacy_user_data::{LegacyBytes, SqlDateTime2},
};
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CreationAdmissionError {
    #[error("creation admission database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("creation admission does not own a synthetic admitted revision")]
    Ownership,
    #[error("creation admission is outside the measured product-name profile")]
    Profile,
    #[error("creation admission deadline exceeded")]
    Timeout,
    #[error("original SingleOrDefault found multiple initial-SN rows")]
    MultipleSerialRows,
    #[error("creation allocator clock failed")]
    Clock,
}
async fn owned<'p>(
    pool: &'p PgPool,
    key: &UserKey<'_>,
) -> Result<Transaction<'p, Postgres>, CreationAdmissionError> {
    let name = LegacyNameKey::parse(key.name).map_err(|_| CreationAdmissionError::Ownership)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let owner: Option<(String, i64)> =
        sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE")
            .bind(key.uid)
            .fetch_optional(&mut *tx)
            .await?;
    if !owner.is_some_and(|(n, r)| n == name.as_str() && r == key.revision) {
        return Err(CreationAdmissionError::Ownership);
    }
    let synthetic: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)",
    )
    .bind(key.uid)
    .bind(name.as_str())
    .fetch_one(&mut *tx)
    .await?;
    if !synthetic {
        return Err(CreationAdmissionError::Ownership);
    }
    Ok(tx)
}
/// QueryCharacterCount joins the parent Character; only Status 0 and 2 are visible.
/// Product SQL counts rows, disregards Argument and ExpireTime, and has no status subtraction.
/// This snapshot is a source precheck, not a target atomic reservation of a character slot.
pub async fn slots(
    pool: &PgPool,
    key: UserKey<'_>,
) -> Result<CreationSlots, CreationAdmissionError> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut tx = owned(pool, &key).await?;
        let visible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_characters c JOIN game_character_info i ON i.id=c.id WHERE i.uid=$1 AND i.status IN(0,2)")
            .bind(key.uid).fetch_one(&mut *tx).await?;
        let products: Vec<SourceBytes> = sqlx::query_scalar("SELECT product_name_utf16 FROM game_user_products WHERE nexon_sn=$1 LIMIT 1001")
            .bind(key.uid).fetch_all(&mut *tx).await?;
        if products.len()>1000 { return Err(CreationAdmissionError::Profile); }
        let (mut purchased, mut premium) = (0,0);
        for p in products {
            let name = LegacyBytes::<128,true>::from_bytes(p.into_bytes()).map_err(|_| CreationAdmissionError::Profile)?;
            match classify_product(&name).map_err(|_| CreationAdmissionError::Profile)? {
                ProductKind::CharacterSlot => purchased += 1,
                ProductKind::PremiumCharacterSlot => premium += 1,
                ProductKind::Other => {},
            }
        }
        tx.commit().await?;
        Ok(CreationSlots { visible:i32::try_from(visible).map_err(|_| CreationAdmissionError::Profile)?, purchased, premium })
    }).await.map_err(|_| CreationAdmissionError::Timeout)?
}
/// Preserve the source bug: each query compares against initial_sn, not the latest output SN.
/// No Status filter. Exactly one row causes another allocation; two rows throw immediately.
/// At 256 iterations the short circuit exits WITHOUT querying again.
pub async fn serial_with_clock<F>(
    pool: &PgPool,
    key: UserKey<'_>,
    server_code: i32,
    mut utc: F,
) -> Result<CharacterSerial, CreationAdmissionError>
where
    F: FnMut() -> Result<SqlDateTime2, CreationAdmissionError>,
{
    tokio::time::timeout(Duration::from_secs(10), async {
        // Target-only scope guard precedes sampling; no name/reservation/ledger write occurs.
        let mut tx = owned(pool, &key).await?;
        let mut serial = first_character_serial(utc()?, utc()?, server_code);
        if serial.serial == 0 {
            serial = next_character_serial(utc()?, server_code, 0);
        }
        let initial_sn = serial.serial;
        for _ in 0..256 {
            let rows: Vec<i64> = sqlx::query_scalar(
                "SELECT id FROM game_character_info WHERE uid=$1 AND character_sn=$2 LIMIT 2",
            )
            .bind(key.uid)
            .bind(initial_sn)
            .fetch_all(&mut *tx)
            .await?;
            match rows.len() {
                0 => break,
                1 => serial = next_character_serial(utc()?, server_code, serial.serial),
                _ => return Err(CreationAdmissionError::MultipleSerialRows),
            }
        }
        tx.commit().await?;
        Ok(serial)
    })
    .await
    .map_err(|_| CreationAdmissionError::Timeout)?
}
/// Runtime UTC adapter; no fabricated clock or configured service-ID assumption.
pub async fn serial(
    pool: &PgPool,
    key: UserKey<'_>,
    server_code: i32,
) -> Result<CharacterSerial, CreationAdmissionError> {
    serial_with_clock(pool, key, server_code, || {
        super::lab_clock::utc_now().map_err(|_| CreationAdmissionError::Clock)
    })
    .await
}
