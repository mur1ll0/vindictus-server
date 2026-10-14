//! Original insertion and failed-initialization cleanup, scoped to synthetic lab owners.
use super::{character_lobby::UserKey, character_name_proof::NameProof, legacy_users::SourceBytes};
use crate::domain::{game_user::LegacyNameKey, legacy_user_data::SqlDateTime};
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum CreationEffectsError {
    #[error("creation effects database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("creation effects do not own the synthetic revision or character")]
    Ownership,
    #[error("creation cleanup is outside the measured unambiguous name profile")]
    Profile,
    #[error("creation effects deadline exceeded")]
    Timeout,
}
async fn owned<'p>(
    pool: &'p PgPool,
    key: &UserKey<'_>,
) -> Result<Transaction<'p, Postgres>, CreationEffectsError> {
    let name = LegacyNameKey::parse(key.name).map_err(|_| CreationEffectsError::Ownership)?;
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
    if !actual.is_some_and(|(n, r)| n == name.as_str() && r == key.revision) {
        return Err(CreationEffectsError::Ownership);
    }
    let synthetic: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)",
    )
    .bind(key.uid)
    .bind(name.as_str())
    .fetch_one(&mut *tx)
    .await?;
    if !synthetic {
        return Err(CreationEffectsError::Ownership);
    }
    Ok(tx)
}
async fn character(
    tx: &mut Transaction<'_, Postgres>,
    key: &UserKey<'_>,
    cid: i64,
) -> Result<(), CreationEffectsError> {
    let yes: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_character_info WHERE id=$1 AND uid=$2)",
    )
    .bind(cid)
    .bind(key.uid)
    .fetch_one(&mut **tx)
    .await?;
    if !yes {
        return Err(CreationEffectsError::Ownership);
    }
    Ok(())
}
/// Source INSERT only. Duplicate CID throws; existing connection fields are never updated.
pub async fn insert_dormant_connection(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    date: SqlDateTime,
) -> Result<(), CreationEffectsError> {
    tokio::time::timeout(Duration::from_secs(10),async{
        let mut tx=owned(pool,key).await?;character(&mut tx,key,cid).await?;
        sqlx::query("INSERT INTO game_last_connect_info(id,last_connect_date_units300,last_cafe_bonus_units300,last_cafe_login_units300,last_cafe_logout_units300) VALUES($1,$2,NULL,NULL,NULL)").bind(cid).bind(date.units300()).execute(&mut *tx).await?;
        tx.commit().await?;Ok(())
    }).await.map_err(|_|CreationEffectsError::Timeout)?
}
pub async fn has_connection(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
) -> Result<bool, CreationEffectsError> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut tx = owned(pool, key).await?;
        character(&mut tx, key, cid).await?;
        let yes =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_last_connect_info WHERE id=$1)")
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?;
        tx.commit().await?;
        Ok(yes)
    })
    .await
    .map_err(|_| CreationEffectsError::Timeout)?
}
async fn matching(
    tx: &mut Transaction<'_, Postgres>,
    key: &UserKey<'_>,
    proof: &NameProof,
) -> Result<Vec<i64>, CreationEffectsError> {
    let rows: Vec<(i64, Option<SourceBytes>)> =
        sqlx::query_as("SELECT id,name_utf16 FROM game_character_info WHERE uid=$1 LIMIT 1001")
            .bind(key.uid)
            .fetch_all(&mut **tx)
            .await?;
    if rows.len() > 1000 {
        return Err(CreationEffectsError::Profile);
    }
    let mut ids = Vec::new();
    for (id, name) in rows {
        if let Some(name) = name {
            let bytes = Zeroizing::new(name.into_bytes());
            if proof
                .equals(&bytes)
                .map_err(|_| CreationEffectsError::Profile)?
            {
                ids.push(id)
            }
        }
    }
    Ok(ids)
}
/// Target scope guard: don't create a second matching name requiring an arbitrary TOP 1 cleanup.
pub async fn require_empty_cleanup_candidate(
    pool: &PgPool,
    key: &UserKey<'_>,
    proof: &NameProof,
) -> Result<(), CreationEffectsError> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut tx = owned(pool, key).await?;
        if !matching(&mut tx, key, proof).await?.is_empty() {
            return Err(CreationEffectsError::Profile);
        }
        tx.commit().await?;
        Ok(())
    })
    .await
    .map_err(|_| CreationEffectsError::Timeout)?
}
/// Original UID/name predicate, no status/SN filter; delete the parent and use measured cascades.
/// No match returns zero deleted rows (source @cid=NULL test doesn't enter its rollback branch).
/// Multiple matches are explicitly unsupported: never invent TOP 1 ordering or select a CID.
pub async fn cleanup(
    pool: &PgPool,
    key: &UserKey<'_>,
    proof: &NameProof,
    name: &str,
) -> Result<u64, CreationEffectsError> {
    if !proof
        .valid_request(Some(name))
        .map_err(|_| CreationEffectsError::Profile)?
    {
        return Err(CreationEffectsError::Profile);
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut tx = owned(pool, key).await?;
        sqlx::query("LOCK TABLE game_character_info IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        let ids = matching(&mut tx, key, proof).await?;
        if ids.len() > 1 {
            return Err(CreationEffectsError::Profile);
        }
        let deleted = if let Some(cid) = ids.first() {
            sqlx::query("DELETE FROM game_characters WHERE id=$1")
                .bind(cid)
                .execute(&mut *tx)
                .await?
                .rows_affected()
        } else {
            0
        };
        tx.commit().await?;
        Ok(deleted)
    })
    .await
    .map_err(|_| CreationEffectsError::Timeout)?
}

/// Server-issued cleanup authority for one known base creation; never constructed from a wire CID.
/// A reconnect can advance the session revision without revoking compensation for this attempt.
pub(crate) struct CreationCleanupLease {
    uid: i32,
    owner: Zeroizing<String>,
    cid: i64,
}
impl CreationCleanupLease {
    pub(crate) fn issued_after_base(key: &UserKey<'_>, cid: i64) -> Self {
        Self {
            uid: key.uid,
            owner: Zeroizing::new(key.name.to_owned()),
            cid,
        }
    }
}
pub(crate) async fn cleanup_attempt(
    pool: &PgPool,
    lease: &CreationCleanupLease,
    proof: &NameProof,
) -> Result<u64, CreationEffectsError> {
    let expected =
        LegacyNameKey::parse(&lease.owner).map_err(|_| CreationEffectsError::Ownership)?;
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut tx = pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL statement_timeout='5s'")
            .execute(&mut *tx)
            .await?;
        let actual: Option<String> =
            sqlx::query_scalar("SELECT name_key FROM game_users WHERE id=$1 FOR SHARE")
                .bind(lease.uid)
                .fetch_optional(&mut *tx)
                .await?;
        if actual.as_deref() != Some(expected.as_str()) {
            return Err(CreationEffectsError::Ownership);
        }
        let origin: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)",
        )
        .bind(lease.uid)
        .bind(expected.as_str())
        .fetch_one(&mut *tx)
        .await?;
        if !origin {
            return Err(CreationEffectsError::Ownership);
        }
        sqlx::query("LOCK TABLE game_character_info IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        let key = UserKey {
            uid: lease.uid,
            name: &lease.owner,
            revision: 0,
        };
        let ids = matching(&mut tx, &key, proof).await?;
        if ids.len() > 1 || ids.first().is_some_and(|id| *id != lease.cid) {
            return Err(CreationEffectsError::Profile);
        }
        let count = if let Some(cid) = ids.first() {
            sqlx::query("DELETE FROM game_characters WHERE id=$1")
                .bind(cid)
                .execute(&mut *tx)
                .await?
                .rows_affected()
        } else {
            0
        };
        tx.commit().await?;
        Ok(count)
    })
    .await
    .map_err(|_| CreationEffectsError::Timeout)?
}
