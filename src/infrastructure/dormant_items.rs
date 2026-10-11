//! Recovered queue INSERT plus explicit identity bootstrap and durable local retry protection.
use crate::{domain::dormant_item::DormantItemRequest, error::Error};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroizing;

async fn transaction(pool: &PgPool) -> Result<Transaction<'_, Postgres>, Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
/// Install seed=1/increment=1 once, after importing the source snapshot.
/// The explicit identity last_value can exceed all surviving RowIDs; it is never MAX+1.
/// None represents a source identity that has never been allocated, on an empty queue.
pub async fn initialize_identity(
    pool: &PgPool,
    source_last_value: Option<i32>,
) -> Result<bool, Error> {
    if source_last_value.is_some_and(|value| !(0..i32::MAX).contains(&value)) {
        return Err(Error::QueueIdentityProfile);
    }
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut tx = transaction(pool).await?;
        let state = sqlx::query("SELECT initialized,source_last_value FROM game_queue_identity WHERE singleton FOR UPDATE")
            .fetch_one(&mut *tx).await?;
        if state.try_get::<bool,_>("initialized")? {
            if state.try_get::<Option<i32>,_>("source_last_value")? != source_last_value {
                return Err(Error::QueueIdentityProfile);
            }
            tx.commit().await?;
            return Ok(false);
        }
        sqlx::query("LOCK TABLE game_queued_items IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
        let invalid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_queued_items WHERE $1::integer IS NULL OR row_id>$1)")
            .bind(source_last_value).fetch_one(&mut *tx).await?;
        if invalid { return Err(Error::QueueIdentityProfile); }
        // ALTER RESTART is transactional, unlike setval. The value is a validated integer.
        let next = source_last_value.map_or(1, |value| value + 1);
        sqlx::query(&format!("ALTER SEQUENCE game_queued_item_row_id RESTART WITH {next}"))
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE game_queue_identity SET initialized=TRUE,source_last_value=$1 WHERE singleton")
            .bind(source_last_value).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }).await.map_err(|_| Error::QueueDeadline)?
}
/// One source call, inside the caller's transaction. A rollback consumes its identity,
/// matching SQL Server allocation gaps; only the receipt wrapper supplies replay protection.
pub async fn send_dormant_item(
    tx: &mut Transaction<'_, Postgres>,
    request: &DormantItemRequest,
) -> Result<i32, Error> {
    let initialized: bool =
        sqlx::query_scalar("SELECT initialized FROM game_queue_identity WHERE singleton FOR SHARE")
            .fetch_one(&mut **tx)
            .await?;
    if !initialized {
        return Err(Error::QueueIdentityUninitialized);
    }
    let row_id: i32 = sqlx::query_scalar("SELECT nextval('game_queued_item_row_id')::integer")
        .fetch_one(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO game_queued_items(row_id,cid,item_class_ex_utf16,count,is_character_binded,mail_title_utf16,mail_content_utf16,color1,color2,color3,reduced_durability,max_durability_bonus,item_id,processed,event_code_utf16,processed_time_units300,expire_time_units300,log_date_units300) VALUES($1,$2,$3,$4,$5,$6,$7,-1,-1,-1,0,0,NULL,FALSE,NULL,NULL,NULL,$8)")
        .bind(row_id).bind(request.cid).bind(request.item_class_ex.as_bytes()).bind(request.count)
        .bind(request.is_binded).bind(request.mail_title.as_ref().map(|v| v.as_bytes()))
        .bind(request.mail_content.as_ref().map(|v| v.as_bytes())).bind(request.log_date.units300())
        .execute(&mut **tx).await?;
    Ok(row_id)
}
#[derive(Debug, PartialEq, Eq)]
pub struct QueueResult {
    pub row_id: i32,
    pub replayed: bool,
}
/// A single operation UUID and its complete original parameters must be reused on retry.
/// Replay stays valid after delivery removes the row, and never recreates that reward.
pub async fn enqueue_once(
    pool: &PgPool,
    operation_id: Uuid,
    request: &DormantItemRequest,
) -> Result<QueueResult, Error> {
    let canonical = request.replay_key();
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut tx = transaction(pool).await?;
        let claimed = sqlx::query("INSERT INTO game_dormant_item_runs(operation_id,request) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(operation_id).bind(canonical.as_slice()).execute(&mut *tx).await?.rows_affected()==1;
        if !claimed {
            let receipt = sqlx::query("SELECT request,row_id FROM game_dormant_item_runs WHERE operation_id=$1 FOR SHARE")
                .bind(operation_id).fetch_one(&mut *tx).await?;
            let original = Zeroizing::new(receipt.try_get::<Vec<u8>,_>("request")?);
            if original.as_slice()!=canonical.as_slice() { return Err(Error::QueueReplayConflict); }
            let row_id = receipt.try_get::<Option<i32>,_>("row_id")?.ok_or(Error::QueueReplayConflict)?;
            tx.commit().await?;
            return Ok(QueueResult{row_id,replayed:true});
        }
        let row_id=send_dormant_item(&mut tx,request).await?;
        sqlx::query("UPDATE game_dormant_item_runs SET row_id=$2 WHERE operation_id=$1")
            .bind(operation_id).bind(row_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(QueueResult{row_id,replayed:false})
    }).await.map_err(|_| Error::QueueDeadline)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), Error> {
    sqlx::query(
        "SELECT singleton,initialized,source_last_value FROM game_queue_identity WHERE FALSE",
    )
    .execute(pool)
    .await
    .map_err(|_| Error::SchemaNotReady)?;
    sqlx::query("SELECT operation_id,request,row_id FROM game_dormant_item_runs WHERE FALSE")
        .execute(pool)
        .await
        .map_err(|_| Error::SchemaNotReady)?;
    let valid:bool=sqlx::query_scalar("SELECT seqincrement=1 AND seqmin=1 AND seqmax=2147483647 AND seqcache=1 AND NOT seqcycle FROM pg_sequence WHERE seqrelid='game_queued_item_row_id'::regclass")
        .fetch_one(pool).await.map_err(|_|Error::SchemaNotReady)?;
    if !valid {
        return Err(Error::SchemaNotReady);
    }
    let singleton: bool =
        sqlx::query_scalar("SELECT COUNT(*)=1 FROM game_queue_identity WHERE singleton")
            .fetch_one(pool)
            .await
            .map_err(|_| Error::SchemaNotReady)?;
    if !singleton {
        return Err(Error::SchemaNotReady);
    }
    Ok(())
}
