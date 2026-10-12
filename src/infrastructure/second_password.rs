//! Short row-locked second-password writes. Only the new PostgreSQL database is mutated.
use crate::{
    domain::{
        game_user::LegacyNameKey,
        legacy_user_data::SqlDateTime2,
        second_password::{
            self as rules, CheckResult, ClockSamples, HashProfile, QueryResult, SecondPasswordError,
        },
    },
    error::Error,
    infrastructure::legacy_users::SourceBytes,
};
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Domain(#[from] SecondPasswordError),
    #[error(transparent)]
    Storage(#[from] Error),
    #[error("second-password operation deadline exceeded; reread the revision before retry")]
    Deadline,
}
impl From<sqlx::Error> for StorageError {
    fn from(e: sqlx::Error) -> Self {
        Self::Storage(Error::from(e))
    }
}
impl From<crate::domain::legacy_user_data::LegacyUserDataError> for StorageError {
    fn from(e: crate::domain::legacy_user_data::LegacyUserDataError) -> Self {
        Self::Domain(e.into())
    }
}
/// Trusted admitted identity and revision, never taken from a password network message.
pub struct UserKey<'a> {
    pub uid: i32,
    pub name: &'a str,
    pub revision: i64,
}
type LockedRow = (String, i64, Option<SourceBytes>, Option<i32>, Option<i64>);
struct Row {
    password: Option<Zeroizing<Vec<u8>>>,
    count: Option<i32>,
    last: Option<SqlDateTime2>,
}
async fn lock<'a>(
    pool: &PgPool,
    key: &UserKey<'a>,
) -> Result<(Transaction<'static, Postgres>, Row), StorageError> {
    let name = LegacyNameKey::parse(key.name).map_err(Error::from)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let row: Option<LockedRow>=sqlx::query_as("SELECT name_key,local_revision,second_password_utf16,second_password_fail_count,second_password_last_fail_ticks FROM game_users WHERE id=$1 FOR UPDATE").bind(key.uid).fetch_optional(&mut *tx).await?;
    let (own, revision, password, count, last) = row.ok_or(Error::GameUserMissing)?;
    if own != name.as_str() {
        return Err(Error::GameUserIdConflict.into());
    }
    if revision != key.revision {
        return Err(Error::GameUserRevisionConflict.into());
    }
    let last = last.map(SqlDateTime2::from_ticks).transpose()?;
    Ok((
        tx,
        Row {
            password: password.map(|b| Zeroizing::new(b.into_bytes())),
            count,
            last,
        },
    ))
}
async fn save(
    tx: &mut Transaction<'_, Postgres>,
    key: &UserKey<'_>,
    password: Option<&[u8]>,
    count: Option<i32>,
    last: Option<SqlDateTime2>,
) -> Result<i64, StorageError> {
    let revision=sqlx::query_scalar("UPDATE game_users SET second_password_utf16=$3,second_password_fail_count=$4,second_password_last_fail_ticks=$5 WHERE id=$1 AND local_revision=$2 RETURNING local_revision").bind(key.uid).bind(key.revision).bind(password).bind(count).bind(last.map(SqlDateTime2::ticks)).fetch_optional(&mut **tx).await?.ok_or(Error::GameUserRevisionConflict)?;
    Ok(revision)
}
fn allowed(row: &Row, clock: ClockSamples) -> Result<(), StorageError> {
    let last = row
        .last
        .unwrap_or(SqlDateTime2::from_ticks(552_877_920_000_000_000)?);
    if rules::retry_until(row.count.unwrap_or_default(), last)?
        .is_some_and(|t| t.ticks() > clock.operation_now.ticks())
    {
        return Err(SecondPasswordError::Locked.into());
    }
    Ok(())
}
pub async fn query(
    pool: &PgPool,
    key: &UserKey<'_>,
    clock: ClockSamples,
) -> Result<(QueryResult, i64), StorageError> {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let (mut tx, row) = lock(pool, key).await?;
        let result = rules::query(
            row.password.as_deref().map(|b| b.as_slice()),
            row.count,
            row.last,
            clock,
        )?;
        // Feedback uses the captured pre-initialization count/time, exactly as UserEntity.
        let revision = if result.first {
            save(&mut tx, key, Some(&[]), Some(0), None).await?
        } else {
            key.revision
        };
        tx.commit().await?;
        Ok((result, revision))
    })
    .await
    .map_err(|_| StorageError::Deadline)?
}
pub async fn check(
    pool: &PgPool,
    key: &UserKey<'_>,
    password: &[u16],
    profile: HashProfile,
    identity: crate::protocol::identity::IdentityProfile,
    clock: ClockSamples,
) -> Result<(CheckResult, i64), StorageError> {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let (mut tx, row) = lock(pool, key).await?;
        allowed(&row, clock)?;
        let stored = row
            .password
            .as_ref()
            .ok_or(SecondPasswordError::Uninitialized)?;
        let matched = rules::matches(key.name, password, stored, identity)?;
        let count = rules::checked_count(row.count, matched)?;
        let rewritten = if matched
            && rules::stored_version(stored.len() / 2)
                != match profile {
                    HashProfile::Legacy(_) => 0,
                    HashProfile::Sha512 => 1,
                    HashProfile::AccountSha512 => 2,
                } {
            Some(rules::hash(key.name, password, profile)?)
        } else {
            None
        };
        let encoded = rewritten.as_deref().map(|b| b.as_slice()).unwrap_or(stored);
        if encoded.len() > 512 {
            return Err(SecondPasswordError::Input.into());
        }
        let last = if matched {
            None
        } else {
            Some(clock.operation_now)
        };
        let result = CheckResult {
            passed: matched,
            fail_count: count,
            retry_seconds: rules::retry_seconds(
                rules::retry_until(count, clock.operation_now)?,
                clock,
            )?,
        };
        let revision = save(&mut tx, key, Some(encoded), Some(count), last).await?;
        tx.commit().await?;
        Ok((result, revision))
    })
    .await
    .map_err(|_| StorageError::Deadline)?
}
pub async fn change(
    pool: &PgPool,
    key: &UserKey<'_>,
    password: &[u16],
    profile: HashProfile,
    clock: ClockSamples,
) -> Result<i64, StorageError> {
    // Source profile is explicit; empty means clear, not hash(empty).
    let encoded = if password.is_empty() {
        Zeroizing::new(Vec::new())
    } else {
        rules::hash(key.name, password, profile)?
    };
    if encoded.len() > 512 {
        return Err(SecondPasswordError::Input.into());
    }
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let (mut tx, row) = lock(pool, key).await?;
        allowed(&row, clock)?;
        let revision = save(&mut tx, key, Some(&encoded), Some(0), None).await?;
        tx.commit().await?;
        Ok(revision)
    })
    .await
    .map_err(|_| StorageError::Deadline)?
}
