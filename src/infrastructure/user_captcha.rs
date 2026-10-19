//! Complete UserCaptcha preservation; constructor/challenges/Save remain separate.
use crate::{
    domain::legacy_user_data::SqlDateTime,
    infrastructure::{
        character_lobby::UserKey,
        user_export::{PrivateText, integer, nullable},
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::{collections::HashSet, io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum CaptchaError {
    #[error("invalid bounded user-captcha snapshot")]
    Export,
    #[error("user-captcha input could not be read")]
    Input(#[from] std::io::Error),
    #[error("user-captcha database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("user-captcha reference preservation is not initialized")]
    Uninitialized,
    #[error("user-captcha preservation conflict;no repair performed")]
    Conflict,
    #[error("user-captcha absence requires current owner")]
    Owner,
    #[error("user-captcha row exists;no constructor or challenge implemented")]
    Existing,
    #[error("user-captcha deadline exceeded;reconcile before retry")]
    Timeout,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    rows: Vec<Raw>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    uid: i32,
    #[serde(deserialize_with = "nullable")]
    punished_count: Option<u8>,
    #[serde(deserialize_with = "nullable")]
    failure_count: Option<u8>,
    #[serde(deserialize_with = "nullable")]
    last_failure_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    last_succeed_units300: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    condition: Option<u8>,
}
#[derive(sqlx::FromRow, PartialEq, Eq)]
struct Row {
    uid: i32,
    punished_count: Option<i16>,
    failure_count: Option<i16>,
    last_failure_units300: Option<i64>,
    last_succeed_units300: Option<i64>,
    condition: Option<i16>,
}
pub struct Snapshot {
    rows: Vec<Row>,
    hash: [u8; 32],
}
fn date(v: Option<PrivateText>) -> Result<Option<i64>, CaptchaError> {
    v.map(|v| {
        let n = integer(v).map_err(|_| CaptchaError::Export)?;
        SqlDateTime::from_units300(n).map_err(|_| CaptchaError::Export)?;
        Ok(n)
    })
    .transpose()
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, CaptchaError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(CaptchaError::Export);
    }
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| CaptchaError::Export)?;
    if e.format_version != 1 || e.source != "study_heroes.user-captcha" || e.rows.len() > 100000 {
        return Err(CaptchaError::Export);
    }
    let mut ids = HashSet::new();
    let mut rows = Vec::with_capacity(e.rows.len());
    for r in e.rows {
        if !ids.insert(r.uid) {
            return Err(CaptchaError::Export);
        }
        rows.push(Row {
            uid: r.uid,
            punished_count: r.punished_count.map(i16::from),
            failure_count: r.failure_count.map(i16::from),
            last_failure_units300: date(r.last_failure_units300)?,
            last_succeed_units300: date(r.last_succeed_units300)?,
            condition: r.condition.map(i16::from),
        });
    }
    rows.sort_by_key(|r| r.uid);
    Ok(Snapshot {
        rows,
        hash: Sha256::digest(&*bytes).into(),
    })
}
#[derive(Serialize)]
pub struct Summary {
    pub rows: usize,
    pub inserted: usize,
    pub verified: usize,
    pub initialized: bool,
    pub empty_owner_verified: bool,
    pub character_creation_available: bool,
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, CaptchaError> {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut tx = pool.begin().await?;
        timeouts(&mut tx).await?;
        sqlx::query("LOCK TABLE game_user_captcha_preservation,game_user_captcha IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx).await?;
        let marker: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT source_sha256 FROM game_user_captcha_preservation WHERE singleton",
        ).fetch_optional(&mut *tx).await?;
        let mut inserted = 0;
        if let Some(hash) = marker {
            if hash != s.hash { return Err(CaptchaError::Conflict); }
        } else {
            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_user_captcha)")
                .fetch_one(&mut *tx).await?;
            if verify || exists { return Err(CaptchaError::Conflict); }
            for r in &s.rows {
                inserted += sqlx::query("INSERT INTO game_user_captcha(uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition)VALUES($1,$2,$3,$4,$5,$6)")
                    .bind(r.uid).bind(r.punished_count).bind(r.failure_count)
                    .bind(r.last_failure_units300).bind(r.last_succeed_units300).bind(r.condition)
                    .execute(&mut *tx).await?.rows_affected() as usize;
            }
            sqlx::query("INSERT INTO game_user_captcha_preservation(singleton,source_sha256)VALUES(true,$1)")
                .bind(s.hash.as_slice()).execute(&mut *tx).await?;
        }
        let actual: Vec<Row> = sqlx::query_as("SELECT uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition FROM game_user_captcha ORDER BY uid LIMIT 100001")
            .fetch_all(&mut *tx).await?;
        if actual != s.rows { return Err(CaptchaError::Conflict); }
        tx.commit().await?;
        Ok(Summary { rows: s.rows.len(), inserted, verified: s.rows.len(), initialized: true,
            empty_owner_verified: false, character_creation_available: false })
    }).await.map_err(|_| CaptchaError::Timeout)?
}
async fn timeouts(tx: &mut Transaction<'_, Postgres>) -> Result<(), CaptchaError> {
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
/// Observation only;caller keeps this transaction until consumption. Table SHARE
/// lock prevents insertion into the missing UID, not just changes to existing rows.
/// This strengthens source NOLOCK for a stable native admission;not dirty-read equivalence.
pub async fn require_absent(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
) -> Result<(), CaptchaError> {
    if read_initial(tx, owner).await?.is_some() {
        return Err(CaptchaError::Existing);
    }
    Ok(())
}
/// Exact zero/one UserCaptchaGet row. Stable native admission is stronger than NOLOCK.
pub async fn read_initial(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
) -> Result<Option<crate::domain::initial_captcha::Source>, CaptchaError> {
    timeouts(tx).await?;
    let current: Option<(String, i64)> =
        sqlx::query_as("SELECT name,local_revision FROM game_users WHERE id=$1 FOR UPDATE")
            .bind(owner.uid)
            .fetch_optional(&mut **tx)
            .await?;
    if current
        .as_ref()
        .is_none_or(|(name, revision)| name != owner.name || *revision != owner.revision)
    {
        return Err(CaptchaError::Owner);
    }
    sqlx::query("LOCK TABLE game_user_captcha_preservation,game_user_captcha IN SHARE MODE")
        .execute(&mut **tx)
        .await?;
    let initialized: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_user_captcha_preservation WHERE singleton)",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !initialized {
        return Err(CaptchaError::Uninitialized);
    }
    let row:Option<Row>=sqlx::query_as("SELECT uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition FROM game_user_captcha WHERE uid=$1")
        .bind(owner.uid).fetch_optional(&mut **tx).await?;
    row.map(|r| {
        let byte = |v: Option<i16>| {
            v.map(|v| u8::try_from(v).map_err(|_| CaptchaError::Export))
                .transpose()
        };
        let date = |v: Option<i64>| {
            v.map(|v| SqlDateTime::from_units300(v).map_err(|_| CaptchaError::Export))
                .transpose()
        };
        Ok(crate::domain::initial_captcha::Source {
            punished: byte(r.punished_count)?,
            failure: byte(r.failure_count)?,
            last_failure: date(r.last_failure_units300)?,
            last_succeed: date(r.last_succeed_units300)?,
            condition: byte(r.condition)?,
        })
    })
    .transpose()
}
#[derive(Debug, PartialEq, Eq)]
pub enum LogoutSave {
    Saved,
    Caught,
}
/// Empty-pool OnLogOut always calls Save. Independent context/upsert,all provider
/// failures caught like the original. No constructor/reward/journal replay.
pub async fn save_empty_logout(
    pool: &PgPool,
    state: &crate::domain::initial_captcha::Loaded,
) -> LogoutSave {
    let attempt=tokio::time::timeout(Duration::from_secs(5),async{
        let date=|v:Option<crate::domain::legacy_user_data::SqlDateTime2>|v.map(|v|v.sql_parameter().map(|v|v.units300())).transpose();
        let failure=date(state.last_failure()).map_err(|_|CaptchaError::Export)?;
        let succeed=date(state.last_succeed()).map_err(|_|CaptchaError::Export)?;
        let mut tx=pool.begin().await?;timeouts(&mut tx).await?;
        sqlx::query("INSERT INTO game_user_captcha(uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition)VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(uid) DO UPDATE SET punished_count=excluded.punished_count,failure_count=excluded.failure_count,last_failure_units300=excluded.last_failure_units300,last_succeed_units300=excluded.last_succeed_units300,condition=excluded.condition")
            .bind(state.uid()).bind(i16::from(state.punished())).bind(i16::from(state.failure())).bind(failure).bind(succeed).bind(i16::from(u8::from(state.force())))
            .execute(&mut *tx).await?;
        tx.commit().await?;Ok::<_,CaptchaError>(())
    }).await;
    if matches!(attempt, Ok(Ok(()))) {
        LogoutSave::Saved
    } else {
        tracing::warn!("captcha save attempt failed;private details suppressed");
        LogoutSave::Caught
    }
}
pub async fn check_ready(pool: &PgPool) -> Result<(), CaptchaError> {
    for q in [
        "SELECT uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition FROM game_user_captcha LIMIT 0",
        "SELECT singleton,source_sha256 FROM game_user_captcha_preservation LIMIT 0",
    ] {
        sqlx::query(q).execute(pool).await?;
    }
    Ok(())
}
