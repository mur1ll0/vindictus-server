//! Original creation Skill/Title event attempts. No original rowversion is manufactured.
use crate::domain::{
    character_ledger::{LedgerPartition, partition},
    legacy_user_data::SqlDateTime,
};
use sqlx::PgPool;
async fn routing_time(pool: &PgPool) -> Result<SqlDateTime, sqlx::Error> {
    let units: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
    )
    .fetch_one(pool)
    .await?;
    SqlDateTime::from_units300(units)
        .map_err(|_| sqlx::Error::Protocol("creation log clock outside recovered range".into()))
}
/// Caller holds the owned first-Fiona transaction. Separate source context commits even if Submit fails.
pub(crate) async fn give_skill(
    pool: &PgPool,
    name: &[u8],
    rank: i32,
    deadline: tokio::time::Instant,
) -> bool {
    let result=tokio::time::timeout_at(deadline.min(tokio::time::Instant::now()+std::time::Duration::from_secs(5)),async{
  let q=match partition(routing_time(pool).await?){LedgerPartition::One=>"INSERT INTO game_character_skill_log01(journal_id,event,skill_utf16,skill_level,ap) VALUES(0,1,$1,$2,0)",LedgerPartition::Two=>"INSERT INTO game_character_skill_log02(journal_id,event,skill_utf16,skill_level,ap) VALUES(0,1,$1,$2,0)"};
  sqlx::query(q).bind(name).bind(rank).execute(pool).await?;Ok::<(),sqlx::Error>(())
 }).await;
    let ok = matches!(result, Ok(Ok(())));
    if !ok {
        tracing::warn!("initial skill log attempt failed;private details suppressed");
    }
    ok
}
pub(crate) async fn give_title(pool: &PgPool, id: i32, deadline: tokio::time::Instant) -> bool {
    let result=tokio::time::timeout_at(deadline.min(tokio::time::Instant::now()+std::time::Duration::from_secs(5)),async{
  let q=match partition(routing_time(pool).await?){LedgerPartition::One=>"INSERT INTO game_character_title_log01(journal_id,event,title_id,goal_id,value) VALUES(0,2,$1,0,0)",LedgerPartition::Two=>"INSERT INTO game_character_title_log02(journal_id,event,title_id,goal_id,value) VALUES(0,2,$1,0,0)"};
  sqlx::query(q).bind(id).execute(pool).await?;Ok::<(),sqlx::Error>(())
 }).await;
    let ok = matches!(result, Ok(Ok(())));
    if !ok {
        tracing::warn!("initial title log attempt failed;private details suppressed");
    }
    ok
}
pub async fn check_ready(pool: &PgPool) -> Result<(), sqlx::Error> {
    for q in [
        "SELECT journal_id,event,skill_utf16,skill_level,ap,time_ticks,legacy_last_update FROM game_character_skill_log01 LIMIT 0",
        "SELECT journal_id,event,skill_utf16,skill_level,ap,time_ticks,legacy_last_update FROM game_character_skill_log02 LIMIT 0",
        "SELECT journal_id,event,title_id,goal_id,value,time_ticks,legacy_last_update FROM game_character_title_log01 LIMIT 0",
        "SELECT journal_id,event,title_id,goal_id,value,time_ticks,legacy_last_update FROM game_character_title_log02 LIMIT 0",
    ] {
        sqlx::query(q).execute(pool).await?;
    }
    Ok(())
}
