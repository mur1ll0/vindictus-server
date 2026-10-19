//! Source journal allocation and CustomizeCharacter journal; no item event delivery implied.
use super::user_export::{PrivateText, integer, nullable};
use crate::domain::{
    character_ledger::{LedgerPartition, partition},
    legacy_user_data::SqlDateTime,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum JournalError {
    #[error("journal input is outside the recovered profile")]
    Input,
    #[error("journal identity preservation conflicts with stored state")]
    Conflict,
    #[error("journal database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("journal operation exceeded its deadline")]
    Timeout,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    format_version: u32,
    source: String,
    #[serde(deserialize_with = "nullable")]
    item_last: Option<PrivateText>,
    #[serde(deserialize_with = "nullable")]
    character_last: Option<PrivateText>,
}
pub struct Snapshot {
    last: [Option<i64>; 2],
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, JournalError> {
    let mut b = Zeroizing::new(Vec::new());
    input
        .take(4097)
        .read_to_end(&mut b)
        .map_err(|_| JournalError::Input)?;
    if b.len() > 4096 {
        return Err(JournalError::Input);
    }
    let r: Raw = serde_json::from_slice(&b).map_err(|_| JournalError::Input)?;
    if r.format_version != 1 || r.source != "study_heroesLog.journal-identities" {
        return Err(JournalError::Input);
    }
    let last = [r.item_last, r.character_last]
        .map(|v| v.map(integer).transpose().map_err(|_| JournalError::Input));
    let [a, b] = last;
    let last = [a?, b?];
    for (v, seed) in last.iter().zip([611635750, 4419625]) {
        if v.is_some_and(|v| !(seed..i64::MAX).contains(&v)) {
            return Err(JournalError::Input);
        }
    }
    Ok(Snapshot { last })
}
#[derive(Serialize)]
pub struct Summary {
    pub initialized: usize,
    pub character_creation_available: bool,
}
const SEQUENCES: [&str; 2] = ["game_item_journal_id", "game_character_journal_id"];
/// Replay checks the source binding and never reseeds an initialized allocator.
pub async fn preserve(
    pool: &PgPool,
    s: &Snapshot,
    verify_only: bool,
) -> Result<Summary, JournalError> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut tx = pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL statement_timeout='5s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("LOCK TABLE game_journal_identity IN EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        for (n, seq) in SEQUENCES.iter().enumerate() {
            let kind = (n + 1) as i16;
            let stored: Option<(Option<i64>,)> =
                sqlx::query_as("SELECT source_last FROM game_journal_identity WHERE kind=$1")
                    .bind(kind)
                    .fetch_optional(&mut *tx)
                    .await?;
            if let Some((v,)) = stored {
                if v != s.last[n] {
                    return Err(JournalError::Conflict);
                }
                let (value, called): (i64, bool) =
                    sqlx::query_as(&format!("SELECT last_value,is_called FROM {seq}"))
                        .fetch_one(&mut *tx)
                        .await?;
                if v.is_some_and(|source| value < source || (!called && value == source)) {
                    return Err(JournalError::Conflict);
                }
            } else {
                if verify_only {
                    return Err(JournalError::Conflict);
                }
                let (value, called): (i64, bool) =
                    sqlx::query_as(&format!("SELECT last_value,is_called FROM {seq}"))
                        .fetch_one(&mut *tx)
                        .await?;
                if called {
                    return Err(JournalError::Conflict);
                }
                sqlx::query("INSERT INTO game_journal_identity(kind,source_last)VALUES($1,$2)")
                    .bind(kind)
                    .bind(s.last[n])
                    .execute(&mut *tx)
                    .await?;
                // No consumer can run before this transaction publishes the initialization row.
                let next = s.last[n].map_or(value, |v| v + 1);
                sqlx::query(&format!("ALTER SEQUENCE {seq} RESTART WITH {next}"))
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(Summary {
            initialized: 2,
            character_creation_available: false,
        })
    })
    .await
    .map_err(|_| JournalError::Timeout)?
}
async fn allocate(pool: &PgPool, kind: i16) -> Result<i64, sqlx::Error> {
    let q = if kind == 1 {
        "SELECT nextval('game_item_journal_id') FROM game_journal_identity WHERE kind=1"
    } else {
        "SELECT nextval('game_character_journal_id') FROM game_journal_identity WHERE kind=2"
    };
    sqlx::query_scalar(q).fetch_one(pool).await
}
/// DisableOperationLogging is distinct from DisableLogging. Failure returns the source sentinel.
pub async fn get_item_journal_id(pool: &PgPool, disable_operation_logging: bool) -> i64 {
    if disable_operation_logging {
        return -1;
    }
    match tokio::time::timeout(Duration::from_secs(5), allocate(pool, 1)).await {
        Ok(Ok(id)) => id,
        _ => {
            tracing::warn!("item journal allocation failed;private details suppressed");
            -1
        }
    }
}
/// Caller must validate the normal creation profile. SQL insertion failure consumes the ID.
pub async fn add_customization_journal(pool: &PgPool, disable_logging: bool) -> i64 {
    add_named_journal(pool, disable_logging, "CustomizeCharacter").await
}
/// Same recovered AddItemJournal procedure and independent counter/context.
pub async fn add_marble_journal(pool: &PgPool, disable_logging: bool) -> i64 {
    add_named_journal(pool, disable_logging, "MarbleLog").await
}
/// Original ManufactureManager constructor journal;independent allocation and insert.
pub async fn add_manufacture_journal(pool: &PgPool, disabled: bool) -> i64 {
    add_named_journal(pool, disabled, "ManufactureReport").await
}
async fn add_named_journal(pool: &PgPool, disable_logging: bool, name: &str) -> i64 {
    if disable_logging {
        return -1;
    }
    let r = tokio::time::timeout(Duration::from_secs(5), async {
        let id = allocate(pool, 2).await?;
        let units: i64 = sqlx::query_scalar(
            "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
        )
        .fetch_one(pool)
        .await?;
        let t = SqlDateTime::from_units300(units)
            .map_err(|_| sqlx::Error::Protocol("journal clock outside recovered range".into()))?;
        let q = match partition(t) {
            LedgerPartition::One => {
                "INSERT INTO game_item_journal01(id,category_utf16)VALUES($1,$2)"
            }
            LedgerPartition::Two => {
                "INSERT INTO game_item_journal02(id,category_utf16)VALUES($1,$2)"
            }
        };
        let category: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        sqlx::query(q).bind(id).bind(category).execute(pool).await?;
        Ok::<_, sqlx::Error>(id)
    })
    .await;
    match r {
        Ok(Ok(id)) => id,
        _ => {
            tracing::warn!("item journal attempt failed;private details suppressed");
            -1
        }
    }
}
pub async fn check_ready(pool: &PgPool) -> Result<(), sqlx::Error> {
    for q in [
        "SELECT kind,source_last FROM game_journal_identity LIMIT 0",
        "SELECT id,category_utf16,time_ticks,legacy_last_update FROM game_item_journal01 LIMIT 0",
        "SELECT id,category_utf16,time_ticks,legacy_last_update FROM game_item_journal02 LIMIT 0",
        "SELECT last_value,is_called FROM game_item_journal_id",
        "SELECT last_value,is_called FROM game_character_journal_id",
    ] {
        sqlx::query(q).execute(pool).await?;
    }
    Ok(())
}
