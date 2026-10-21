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
/// CharacterEntity.AddSkillLog(e, skillID, skillLevel, ap) with the entity's LogJournalID;
/// a failure is only logged, as the source catches it.
pub(crate) async fn skill_log(
    pool: &PgPool,
    journal: i64,
    event: i32,
    name: &[u8],
    rank: i32,
    ap: i32,
) -> bool {
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let q = match partition(routing_time(pool).await?) {
            LedgerPartition::One => {
                "INSERT INTO game_character_skill_log01(journal_id,event,skill_utf16,skill_level,ap) VALUES($1,$2,$3,$4,$5)"
            }
            LedgerPartition::Two => {
                "INSERT INTO game_character_skill_log02(journal_id,event,skill_utf16,skill_level,ap) VALUES($1,$2,$3,$4,$5)"
            }
        };
        sqlx::query(q)
            .bind(journal)
            .bind(event)
            .bind(name)
            .bind(rank)
            .bind(ap)
            .execute(pool)
            .await?;
        Ok::<(), sqlx::Error>(())
    })
    .await;
    let ok = matches!(result, Ok(Ok(())));
    if !ok {
        tracing::warn!("character skill log failed;private details suppressed");
    }
    ok
}
/// CharacterEntity.OpenLog: AddCharacterJournal(CID, "Login") — getCharacterJournalID,
/// CharacterJournal and the "Login" CharacterLedger row of the day's partition. A failure
/// leaves LogJournalID at 0, as the source catches it.
pub async fn open_character_journal(pool: &PgPool, cid: i64) -> i64 {
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let id: i64 = sqlx::query_scalar(
            "SELECT nextval('game_character_journal_id') FROM game_journal_identity WHERE kind=2",
        )
        .fetch_one(pool)
        .await?;
        let (journal, ledger) = match partition(routing_time(pool).await?) {
            LedgerPartition::One => (
                "INSERT INTO game_character_journal01(id,character_id) VALUES($1,$2)",
                "INSERT INTO game_character_ledger01(journal_id,event_utf16,target_utf16,value) VALUES($1,$2,'\\x'::bytea,0)",
            ),
            LedgerPartition::Two => (
                "INSERT INTO game_character_journal02(id,character_id) VALUES($1,$2)",
                "INSERT INTO game_character_ledger02(journal_id,event_utf16,target_utf16,value) VALUES($1,$2,'\\x'::bytea,0)",
            ),
        };
        sqlx::query(journal).bind(id).bind(cid).execute(pool).await?;
        let login: Vec<u8> = "Login".encode_utf16().flat_map(u16::to_le_bytes).collect();
        sqlx::query(ledger).bind(id).bind(login).execute(pool).await?;
        Ok::<i64, sqlx::Error>(id)
    })
    .await;
    match result {
        Ok(Ok(id)) => id,
        _ => {
            tracing::warn!("character journal failed;private details suppressed");
            0
        }
    }
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
