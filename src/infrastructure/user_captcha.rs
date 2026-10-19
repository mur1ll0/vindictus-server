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
    pub native_verified: usize,
    pub initialized: bool,
    pub empty_owner_verified: bool,
    pub character_creation_available: bool,
}
fn row_hash(r: &Row) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"UserCaptcha/native/v1");
    h.update(r.uid.to_le_bytes());
    for v in [r.punished_count, r.failure_count, r.condition] {
        match v {
            Some(v) => {
                h.update([1]);
                h.update(v.to_le_bytes());
            }
            None => h.update([0]),
        }
    }
    for v in [r.last_failure_units300, r.last_succeed_units300] {
        match v {
            Some(v) => {
                h.update([1]);
                h.update(v.to_le_bytes());
            }
            None => h.update([0]),
        }
    }
    h.finalize().into()
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, CaptchaError> {
    tokio::time::timeout(Duration::from_secs(20),async{
        let mut tx=pool.begin().await?;timeouts(&mut tx).await?;
        sqlx::query("LOCK TABLE game_user_captcha_preservation,game_user_captcha,game_user_captcha_native_state IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
        let ids:Vec<i32>=s.rows.iter().map(|r|r.uid).collect();
        let marker:Option<(Vec<u8>,Vec<i32>)>=sqlx::query_as("SELECT source_sha256,source_uids FROM game_user_captcha_preservation WHERE singleton").fetch_optional(&mut *tx).await?;
        let mut inserted=0;
        if let Some((hash,stored_ids))=marker {
            if hash!=s.hash||stored_ids!=ids{return Err(CaptchaError::Conflict)}
        }else{
            let occupied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_user_captcha) OR EXISTS(SELECT 1 FROM game_user_captcha_native_state)").fetch_one(&mut *tx).await?;
            if verify||occupied{return Err(CaptchaError::Conflict)}
            for r in &s.rows{inserted+=sqlx::query("INSERT INTO game_user_captcha(uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition)VALUES($1,$2,$3,$4,$5,$6)")
                .bind(r.uid).bind(r.punished_count).bind(r.failure_count).bind(r.last_failure_units300).bind(r.last_succeed_units300).bind(r.condition).execute(&mut *tx).await?.rows_affected()as usize;}
            sqlx::query("INSERT INTO game_user_captcha_preservation(singleton,source_sha256,source_uids)VALUES(true,$1,$2)").bind(s.hash.as_slice()).bind(&ids).execute(&mut *tx).await?;
        }
        let all:Vec<Row>=sqlx::query_as("SELECT uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition FROM game_user_captcha ORDER BY uid LIMIT 100001").fetch_all(&mut *tx).await?;
        let total:i64=sqlx::query_scalar("SELECT count(*) FROM game_user_captcha").fetch_one(&mut *tx).await?;
        if total!=all.len()as i64{return Err(CaptchaError::Conflict)}
        let reference:Vec<&Row>=all.iter().filter(|r|ids.binary_search(&r.uid).is_ok()).collect();
        if reference.len()!=s.rows.len()||reference.iter().zip(&s.rows).any(|(a,b)|*a!=b){return Err(CaptchaError::Conflict)}
        let mut native_verified=0;
        for r in all.iter().filter(|r|ids.binary_search(&r.uid).is_err()){
            if !native_proof(&mut tx,r).await?{return Err(CaptchaError::Conflict)}
            native_verified+=1;
        }
        let receipts:i64=sqlx::query_scalar("SELECT count(*) FROM game_user_captcha_native_state").fetch_one(&mut *tx).await?;
        if receipts!=native_verified as i64{return Err(CaptchaError::Conflict)}
        tx.commit().await?;Ok(Summary{rows:s.rows.len(),inserted,verified:s.rows.len(),native_verified,initialized:true,empty_owner_verified:false,character_creation_available:false})
    }).await.map_err(|_|CaptchaError::Timeout)?
}
async fn native_proof(tx: &mut Transaction<'_, Postgres>, r: &Row) -> Result<bool, CaptchaError> {
    let proof:Option<Vec<u8>>=sqlx::query_scalar("SELECT n.saved_sha256 FROM game_user_captcha_native_state n JOIN game_lab_account_origins o ON o.uid=n.uid AND o.profile_fingerprint=n.origin_fingerprint JOIN game_users u ON u.id=o.uid AND u.name_key=o.name_key WHERE n.uid=$1")
        .bind(r.uid).fetch_optional(&mut **tx).await?;
    Ok(proof.as_deref() == Some(row_hash(r).as_slice()))
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
    sqlx::query("LOCK TABLE game_user_captcha_preservation,game_user_captcha,game_user_captcha_native_state IN SHARE MODE")
        .execute(&mut **tx)
        .await?;
    let ids: Option<Vec<i32>> = sqlx::query_scalar(
        "SELECT source_uids FROM game_user_captcha_preservation WHERE singleton",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let ids = ids.ok_or(CaptchaError::Uninitialized)?;
    if !ids.windows(2).all(|p| p[0] < p[1]) {
        return Err(CaptchaError::Conflict);
    }
    let row:Option<Row>=sqlx::query_as("SELECT uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition FROM game_user_captcha WHERE uid=$1")
        .bind(owner.uid).fetch_optional(&mut **tx).await?;
    if let Some(r) = &row {
        if ids.binary_search(&r.uid).is_err() && !native_proof(tx, r).await? {
            return Err(CaptchaError::Conflict);
        }
    } else {
        let orphan: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM game_user_captcha_native_state WHERE uid=$1)",
        )
        .bind(owner.uid)
        .fetch_one(&mut **tx)
        .await?;
        if ids.binary_search(&owner.uid).is_ok() || orphan {
            return Err(CaptchaError::Conflict);
        }
    }
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
        let ids:Option<Vec<i32>>=sqlx::query_scalar("SELECT source_uids FROM game_user_captcha_preservation WHERE singleton FOR SHARE").fetch_optional(&mut *tx).await?;
        if ids.is_none_or(|ids|ids.contains(&state.uid())){return Err(CaptchaError::Conflict)}
        // No owner lock: the recovered Save commits independently of the owner transaction.
        let origin:Option<Vec<u8>>=sqlx::query_scalar("SELECT o.profile_fingerprint FROM game_lab_account_origins o JOIN game_users u ON u.id=o.uid AND u.name_key=o.name_key WHERE o.uid=$1").bind(state.uid()).fetch_optional(&mut *tx).await?;
        let origin=origin.ok_or(CaptchaError::Owner)?;
        let row=Row{uid:state.uid(),punished_count:Some(i16::from(state.punished())),failure_count:Some(i16::from(state.failure())),last_failure_units300:failure,last_succeed_units300:succeed,condition:Some(i16::from(u8::from(state.force())))};
        sqlx::query("INSERT INTO game_user_captcha(uid,punished_count,failure_count,last_failure_units300,last_succeed_units300,condition)VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(uid) DO UPDATE SET punished_count=excluded.punished_count,failure_count=excluded.failure_count,last_failure_units300=excluded.last_failure_units300,last_succeed_units300=excluded.last_succeed_units300,condition=excluded.condition")
            .bind(row.uid).bind(row.punished_count).bind(row.failure_count).bind(failure).bind(succeed).bind(row.condition).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO game_user_captcha_native_state(uid,last_cid,origin_fingerprint,saved_sha256,revision)VALUES($1,$2,$3,$4,1) ON CONFLICT(uid) DO UPDATE SET last_cid=excluded.last_cid,origin_fingerprint=excluded.origin_fingerprint,saved_sha256=excluded.saved_sha256,revision=game_user_captcha_native_state.revision+1")
            .bind(state.uid()).bind(state.cid()).bind(origin).bind(row_hash(&row).as_slice()).execute(&mut *tx).await?;
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
        "SELECT singleton,source_sha256,source_uids FROM game_user_captcha_preservation LIMIT 0",
        "SELECT uid,last_cid,origin_fingerprint,saved_sha256,revision FROM game_user_captcha_native_state LIMIT 0",
    ] {
        sqlx::query(q).execute(pool).await?;
    }
    Ok(())
}
