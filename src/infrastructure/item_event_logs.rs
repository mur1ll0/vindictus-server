//! Independent bounded source log attempts; no successful Create or inventory delivery implied.
use crate::domain::{
    character_ledger::{LedgerPartition, partition},
    item_logs::{Counts, Data, Ledger, LogBound, Manager, Record},
    legacy_user_data::SqlDateTime,
};
use sqlx::PgPool;
use std::time::Duration;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum Attempt {
    Written,
    Failed,
    Suppressed,
}
async fn routing(pool: &PgPool) -> Result<LedgerPartition, sqlx::Error> {
    let units: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
    )
    .fetch_one(pool)
    .await?;
    Ok(partition(SqlDateTime::from_units300(units).map_err(
        |_| sqlx::Error::Protocol("item log clock outside recovered bounds".into()),
    )?))
}
fn result(r: Result<Result<(), sqlx::Error>, tokio::time::error::Elapsed>) -> Attempt {
    if matches!(r, Ok(Ok(()))) {
        Attempt::Written
    } else {
        tracing::warn!("item event log attempt failed;private details suppressed");
        Attempt::Failed
    }
}
/// No DisableLogging check exists in LogInfo.Write; even journal -1 is attempted.
pub async fn write_log(
    pool: &PgPool,
    cid: i64,
    r: &Record,
    deadline: tokio::time::Instant,
) -> Attempt {
    let d = &r.data;
    result(tokio::time::timeout_at(deadline.min(tokio::time::Instant::now()+Duration::from_secs(5)),async{
 let time=r.captured.sql_parameter().map_err(|_|sqlx::Error::Protocol("captured item log time outside recovered bounds".into()))?;
 let table=match routing(pool).await?{LedgerPartition::One=>"game_item_log01",LedgerPartition::Two=>"game_item_log02"};
 let q=format!("INSERT INTO {table}(journal_id,character_id,event,detail,before_item_id,item_id,item_class_utf16,count,after_inventory,after_shared,color1,color2,color3,reduced_durability,max_durability_bonus,memo_utf16,time_units300)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)");
 sqlx::query(&q).bind(r.journal).bind(cid).bind(r.event).bind(r.detail).bind(r.before_id).bind(d.item_id).bind(d.class()).bind(d.count).bind(r.after.inventory).bind(r.after.shared).bind(d.colors[0]).bind(d.colors[1]).bind(d.colors[2]).bind(d.reduced).bind(d.bonus).bind(r.memo.as_ref().map(|m|m.as_bytes())).bind(time.units300()).execute(pool).await?;Ok(())
 }).await)
}
/// Each ledger row has its own source context. Time is sampled by the table default.
pub async fn write_ledger(
    pool: &PgPool,
    cid: i64,
    r: &Ledger,
    disable_logging: bool,
    deadline: tokio::time::Instant,
) -> Attempt {
    if r.suppressed(disable_logging) {
        return Attempt::Suppressed;
    }
    let d = &r.data;
    result(tokio::time::timeout_at(deadline.min(tokio::time::Instant::now()+Duration::from_secs(5)),async{
 let table=match routing(pool).await?{LedgerPartition::One=>"game_item_ledger01",LedgerPartition::Two=>"game_item_ledger02"};
 let q=format!("INSERT INTO {table}(journal_id,item_id,item_class_utf16,character_id,event,count,color1,color2,color3,reduced_durability,max_durability_bonus,action,bundle_item_count)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)");
 sqlx::query(&q).bind(r.journal).bind(d.item_id).bind(d.class()).bind(cid).bind(r.event).bind(d.count).bind(d.colors[0]).bind(d.colors[1]).bind(d.colors[2]).bind(d.reduced).bind(d.bonus).bind(r.action).bind(r.bundle).execute(pool).await?;Ok(())
 }).await)
}
#[derive(Default, Debug, PartialEq, Eq)]
#[must_use]
pub struct BatchOutcome {
    pub written: usize,
    pub failed: usize,
    pub suppressed: usize,
}
/// Clears only after all attempts; cancellation leaves the manager's queued records intact.
pub async fn write_manager(
    pool: &PgPool,
    cid: i64,
    manager: &mut Manager,
    enabled: bool,
    counts: impl FnOnce(&Data) -> Counts,
) -> BatchOutcome {
    let mut out = BatchOutcome::default();
    if let Some(records) = manager.prepare_write(enabled, counts) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        for r in records {
            match write_log(pool, cid, r, deadline).await {
                Attempt::Written => out.written += 1,
                _ => out.failed += 1,
            }
        }
        manager.clear();
    }
    out
}
/// Original AddItemLedgers continues after a caught failure of an individual row.
pub async fn write_ledgers(
    pool: &PgPool,
    cid: i64,
    rows: &[Ledger],
    disable_logging: bool,
) -> Result<BatchOutcome, LogBound> {
    if rows.len() > 4096 {
        return Err(LogBound);
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let mut out = BatchOutcome::default();
    for r in rows {
        match write_ledger(pool, cid, r, disable_logging, deadline).await {
            Attempt::Written => out.written += 1,
            Attempt::Failed => out.failed += 1,
            Attempt::Suppressed => out.suppressed += 1,
        }
    }
    Ok(out)
}
pub async fn check_ready(pool: &PgPool) -> Result<(), sqlx::Error> {
    for table in ["game_item_log01", "game_item_log02"] {
        sqlx::query(&format!("SELECT journal_id,character_id,event,detail,before_item_id,item_id,item_class_utf16,count,after_inventory,after_shared,color1,color2,color3,reduced_durability,max_durability_bonus,memo_utf16,time_units300,legacy_last_update FROM {table} LIMIT 0")).execute(pool).await?;
    }
    for table in ["game_item_ledger01", "game_item_ledger02"] {
        sqlx::query(&format!("SELECT journal_id,item_id,item_class_utf16,character_id,event,count,time_units300,legacy_last_update,color1,color2,color3,reduced_durability,max_durability_bonus,action,bundle_item_count FROM {table} LIMIT 0")).execute(pool).await?;
    }
    Ok(())
}
