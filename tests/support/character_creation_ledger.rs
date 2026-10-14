use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use vindictus_server::{
    application::game_users::provision_game_user,
    infrastructure::{
        character_creation_ledger::{self as ledger, CreationLedgerError},
        character_lobby::UserKey,
        character_name_check::{self as names, NameCheckError, NameCheckResult as R},
    },
};
fn key() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
async fn setup(p: &sqlx::PgPool) {
    provision_game_user(p, 31, "Owner", 42, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(p).await.unwrap();
}
async fn count(p: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM game_character_ledger01)+(SELECT COUNT(*) FROM game_character_ledger02)").fetch_one(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_name_check_logs_only_successful_creation_branch_and_preserves_duplicate_journals()
{
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    let proof = common::proof(&common::content());
    assert_eq!(
        names::check(&pool, key(), &proof, Some("Murael"))
            .await
            .unwrap(),
        R::Success
    );
    assert_eq!(count(&pool).await, 0);
    let before: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for _ in 0..2 {
        assert_eq!(
            names::check_for_creation(&pool, key(), &proof, Some("Murael"))
                .await
                .unwrap(),
            R::Success
        );
    }
    ledger::dormant_create(&pool, &key(), false).await.unwrap();
    let after: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    type LedgerRow = (
        i32,
        i64,
        Vec<u8>,
        Option<Vec<u8>>,
        i32,
        i64,
        Option<Vec<u8>>,
    );
    let rows:Vec<LedgerRow>=sqlx::query_as("SELECT 1,journal_id,event_utf16,target_utf16,value,time_units300,legacy_last_update FROM game_character_ledger01 UNION ALL SELECT 2,journal_id,event_utf16,target_utf16,value,time_units300,legacy_last_update FROM game_character_ledger02").fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 3);
    let mut checks = 0;
    let mut dormant = 0;
    use vindictus_server::domain::{
        character_ledger::{LedgerPartition, partition},
        legacy_user_data::SqlDateTime,
    };
    let part = partition(SqlDateTime::from_units300(before).unwrap());
    for (table, journal, event, target, value, time, version) in rows {
        assert_eq!(journal, 0);
        assert_eq!(value, 0);
        assert_eq!(version, None);
        assert!((before..=after).contains(&time));
        if partition(SqlDateTime::from_units300(after).unwrap()) == part {
            assert_eq!(table, if part == LedgerPartition::One { 1 } else { 2 });
        }
        if event == utf16("ChangedNameCheck-Owner") {
            assert_eq!(target, Some(utf16("I:Murael,I:30,O:0")));
            checks += 1;
        } else {
            assert_eq!(event, utf16("Dormant - CreateCharacter"));
            assert_eq!(target, Some(utf16("UsePreset: False")));
            dormant += 1;
        }
    }
    assert_eq!((checks, dormant), (2, 1));
    let keys:i64=sqlx::query_scalar("SELECT COUNT(*) FROM pg_constraint WHERE conrelid IN('game_character_ledger01'::regclass,'game_character_ledger02'::regclass) AND contype IN('p','u','f')").fetch_one(&pool).await.unwrap();
    assert_eq!(keys, 0);
    let identities:i64=sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name IN('game_character_ledger01','game_character_ledger02') AND is_identity='YES'").fetch_one(&pool).await.unwrap();
    assert_eq!(identities, 0);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_ledger_failure_preserves_prior_reserved_consumption_and_refuses_unmarked_owner() {
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    let proof = common::proof(&common::content());
    assert_eq!(
        names::check_for_creation(&pool, key(), &proof, Some("A!"))
            .await
            .unwrap(),
        R::InvalidName
    );
    assert_eq!(count(&pool).await, 0);
    sqlx::query("INSERT INTO game_reserved_names(row_id,nexon_id_utf16,nexon_sn,character_name_utf16,created) VALUES(1,$1,31,$2,false)").bind(utf16("Owner")).bind(utf16("Murael")).execute(&pool).await.unwrap();
    sqlx::query(
        "ALTER TABLE game_character_ledger01 ADD CONSTRAINT synthetic_reject CHECK(value<>0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE game_character_ledger02 ADD CONSTRAINT synthetic_reject CHECK(value<>0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        names::check_for_creation(&pool, key(), &proof, Some("Murael")).await,
        Err(NameCheckError::Ledger(CreationLedgerError::Database(_)))
    ));
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT created FROM game_reserved_names WHERE row_id=1")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    assert_eq!(count(&pool).await, 0);
    sqlx::query("ALTER TABLE game_character_ledger01 DROP CONSTRAINT synthetic_reject")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE game_character_ledger02 DROP CONSTRAINT synthetic_reject")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_lab_account_origins")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        ledger::dormant_create(&pool, &key(), false).await,
        Err(CreationLedgerError::Ownership)
    ));
    assert!(matches!(
        names::check_for_creation(&pool, key(), &proof, Some("Murael")).await,
        Err(NameCheckError::Ledger(CreationLedgerError::Ownership))
    ));
    assert_eq!(count(&pool).await, 0);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_ledger_cancelled_append_rolls_back_and_readiness_requires_every_source_column() {
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    let mut block = pool.begin().await.unwrap();
    sqlx::query(
        "LOCK TABLE game_character_ledger01,game_character_ledger02 IN ACCESS EXCLUSIVE MODE",
    )
    .execute(&mut *block)
    .await
    .unwrap();
    let p = pool.clone();
    let task = tokio::spawn(async move { ledger::dormant_create(&p, &key(), false).await });
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    block.rollback().await.unwrap();
    assert_eq!(count(&pool).await, 0);
    ledger::dormant_create(&pool, &key(), true).await.unwrap();
    assert_eq!(count(&pool).await, 1);
    vindictus_server::infrastructure::postgres::check_ready(&pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE game_character_ledger02 DROP COLUMN target_utf16")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        vindictus_server::infrastructure::postgres::check_ready(&pool)
            .await
            .is_err()
    );
    cleanup_game_pool(admin, pool, schema).await;
}
