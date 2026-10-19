//! Complete Mail preservation and exact negative QueryMailList lookup.
//! Caller must retain its current owner/fresh character transaction through consumption.
use crate::{
    domain::legacy_user_data::SqlDateTime,
    infrastructure::user_export::{PrivateText, hex, integer, nullable},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::{io::Read, time::Duration};
use zeroize::Zeroizing;
pub const QUERY_SHA256: &str = "c17336ad0fb0aca9d1cb6320580dd58538227ee48bd7a4292cf6e06cd33f484c";
#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("invalid bounded mail reference")]
    Input,
    #[error("mail reference database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("mail reference uninitialized or changed;no repair")]
    Reference,
    #[error("nonempty mail requires recovered mail lifecycle")]
    Present,
    #[error("mail reference timeout")]
    Timeout,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    procedure_sha256: String,
    rows: Vec<Raw>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    mail_id: PrivateText,
    from_cid: PrivateText,
    from_name_hex: PrivateText,
    to_cid: PrivateText,
    to_name_hex: PrivateText,
    mail_type: u8,
    posted_binary: PrivateText,
    title_hex: PrivateText,
    content_hex: PrivateText,
    arrive_binary: PrivateText,
    #[serde(deserialize_with = "nullable")]
    expire_binary: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    is_read: Option<bool>,
    charged_gold: i32,
    is_deleted: bool,
}
#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Row {
    mail_id: i64,
    from_cid: i64,
    from_name_utf16: Vec<u8>,
    to_cid: i64,
    to_name_utf16: Vec<u8>,
    mail_type: i16,
    posted_units300: i64,
    title_utf16: Vec<u8>,
    content_utf16: Vec<u8>,
    arrive_units300: i64,
    expire_units300: Option<i64>,
    is_read: Option<bool>,
    charged_gold: i32,
    is_deleted: bool,
}
pub struct Snapshot {
    rows: Vec<Row>,
    hash: [u8; 32],
    return_cids: Vec<i64>,
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, MailError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| MailError::Input)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(MailError::Input);
    }
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| MailError::Input)?;
    if e.format_version != 1
        || e.source != "study_heroes.mail"
        || e.procedure_sha256 != QUERY_SHA256
        || e.rows.len() > 100000
    {
        return Err(MailError::Input);
    }
    let int = |s| integer(s).map_err(|_| MailError::Input);
    let text = |s, b| -> Result<Vec<u8>, MailError> {
        let v = hex(s, b).map_err(|_| MailError::Input)?;
        if !v.len().is_multiple_of(2) {
            return Err(MailError::Input);
        }
        Ok(v)
    };
    let date = |s| -> Result<i64, MailError> {
        let b: [u8; 8] = hex(s, 8)
            .map_err(|_| MailError::Input)?
            .try_into()
            .map_err(|_| MailError::Input)?;
        Ok(SqlDateTime::from_sql_binary(b)
            .map_err(|_| MailError::Input)?
            .units300())
    };
    let mut rows = e
        .rows
        .into_iter()
        .map(|r| {
            Ok(Row {
                mail_id: int(r.mail_id)?,
                from_cid: int(r.from_cid)?,
                from_name_utf16: text(r.from_name_hex, 512)?,
                to_cid: int(r.to_cid)?,
                to_name_utf16: text(r.to_name_hex, 40)?,
                mail_type: i16::from(r.mail_type),
                posted_units300: date(r.posted_binary)?,
                title_utf16: text(r.title_hex, 512)?,
                content_utf16: text(r.content_hex, 2048)?,
                arrive_units300: date(r.arrive_binary)?,
                expire_units300: r.expire_binary.map(date).transpose()?,
                is_read: r.is_read,
                charged_gold: r.charged_gold,
                is_deleted: r.is_deleted,
            })
        })
        .collect::<Result<Vec<_>, MailError>>()?;
    rows.sort_by_key(|r| r.mail_id);
    if !rows.windows(2).all(|r| r[0].mail_id < r[1].mail_id) {
        return Err(MailError::Input);
    }
    // Exactly the CID set for which at least one of the two TOP branches is nonempty.
    let mut return_cids = Vec::new();
    for r in &rows {
        return_cids.push(r.from_cid);
        if r.to_cid != r.from_cid && !r.is_deleted {
            return_cids.push(r.to_cid)
        }
    }
    return_cids.sort_unstable();
    return_cids.dedup();
    Ok(Snapshot {
        rows,
        hash: Sha256::digest(&*bytes).into(),
        return_cids,
    })
}
#[derive(Serialize)]
pub struct Summary {
    pub rows: usize,
    pub fields: usize,
    pub inserted: usize,
    pub verified: usize,
    pub initialized: bool,
    pub character_creation_available: bool,
}
async fn bounds(tx: &mut Transaction<'_, Postgres>) -> Result<(), MailError> {
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, MailError> {
    tokio::time::timeout(Duration::from_secs(30),async{
 let mut tx=pool.begin().await?;bounds(&mut tx).await?;
 sqlx::query("LOCK TABLE game_mail,game_mail_preservation IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
 let marker:Option<(Vec<u8>,String,Vec<i64>)>=sqlx::query_as("SELECT source_sha256,procedure_sha256,source_return_cids FROM game_mail_preservation WHERE singleton").fetch_optional(&mut *tx).await?;
 let mut inserted=0;
 if let Some((hash,proc,cids))=marker{if hash!=s.hash||proc!=QUERY_SHA256||cids!=s.return_cids{return Err(MailError::Reference)}}else{
 let occupied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_mail)").fetch_one(&mut *tx).await?;
 if verify||occupied{return Err(MailError::Reference)}
 for r in &s.rows{inserted+=sqlx::query("INSERT INTO game_mail VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)").bind(r.mail_id).bind(r.from_cid).bind(&r.from_name_utf16).bind(r.to_cid).bind(&r.to_name_utf16).bind(r.mail_type).bind(r.posted_units300).bind(&r.title_utf16).bind(&r.content_utf16).bind(r.arrive_units300).bind(r.expire_units300).bind(r.is_read).bind(r.charged_gold).bind(r.is_deleted).execute(&mut *tx).await?.rows_affected() as usize;}
 sqlx::query("INSERT INTO game_mail_preservation VALUES(true,$1,$2,$3)").bind(s.hash.as_slice()).bind(QUERY_SHA256).bind(&s.return_cids).execute(&mut *tx).await?;
 }
 let count:i64=sqlx::query_scalar("SELECT count(*) FROM game_mail").fetch_one(&mut *tx).await?;
 if usize::try_from(count).ok()!=Some(s.rows.len()){return Err(MailError::Reference)}
 let actual:Vec<Row>=sqlx::query_as("SELECT mail_id,from_cid,from_name_utf16,to_cid,to_name_utf16,mail_type,posted_units300,title_utf16,content_utf16,arrive_units300,expire_units300,is_read,charged_gold,is_deleted FROM game_mail ORDER BY mail_id LIMIT 100001").fetch_all(&mut *tx).await?;
 if actual!=s.rows{return Err(MailError::Reference)}
 tx.commit().await?;Ok(Summary{rows:s.rows.len(),fields:14,inserted,verified:s.rows.len(),initialized:true,character_creation_available:false})
 }).await.map_err(|_|MailError::Timeout)?
}
/// Does not load nonempty mail,allocate IDs,schedule jobs or complete ItemOwner.
/// Caller validates current owner/CID before invoking and holds this transaction.
/// TOP and ORDER are immaterial only for the exact empty result being admitted.
pub async fn require_query_empty(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<(), MailError> {
    bounds(tx).await?;
    sqlx::query("LOCK TABLE game_mail,game_mail_preservation IN SHARE MODE")
        .execute(&mut **tx)
        .await?;
    let marker: Option<(String, Vec<i64>)> = sqlx::query_as(
        "SELECT procedure_sha256,source_return_cids FROM game_mail_preservation WHERE singleton",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let (proc, cids) = marker.ok_or(MailError::Reference)?;
    if proc != QUERY_SHA256 || !cids.windows(2).all(|c| c[0] < c[1]) {
        return Err(MailError::Reference);
    }
    let present:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_mail WHERE (to_cid=$1 AND from_cid<>$1 AND NOT is_deleted) OR from_cid=$1)").bind(cid).fetch_one(&mut **tx).await?;
    if present {
        return Err(MailError::Present);
    }
    if cids.binary_search(&cid).is_ok() {
        return Err(MailError::Reference);
    }
    Ok(())
}
pub async fn check_ready(pool: &PgPool) -> Result<(), MailError> {
    sqlx::query("SELECT mail_id,from_cid,from_name_utf16,to_cid,to_name_utf16,mail_type,posted_units300,title_utf16,content_utf16,arrive_units300,expire_units300,is_read,charged_gold,is_deleted FROM game_mail LIMIT 0").execute(pool).await?;
    sqlx::query("SELECT singleton,source_sha256,procedure_sha256,source_return_cids FROM game_mail_preservation LIMIT 0").execute(pool).await?;
    Ok(())
}
