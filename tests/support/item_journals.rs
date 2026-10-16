use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::infrastructure::item_journals::{self as j, JournalError};
fn snapshot(item: &str, character: &str) -> j::Snapshot {
    j::read_snapshot(format!(r#"{{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":{item},"character_last":{character}}}"#).as_bytes()).unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn journal_counters_independent_concurrent_replay_and_source_routing() {
    let (admin, p, s) = isolated_game_pool().await;
    assert_eq!(j::get_item_journal_id(&p, false).await, -1);
    let source = snapshot("\"611635799\"", "\"4419699\"");
    assert!(j::preserve(&p, &source, true).await.is_err());
    j::preserve(&p, &source, false).await.unwrap();
    assert_eq!(j::get_item_journal_id(&p, true).await, -1);
    assert_eq!(j::add_customization_journal(&p, true).await, -1);
    let mut jobs = Vec::new();
    for _ in 0..12 {
        let p = p.clone();
        jobs.push(tokio::spawn(async move {
            j::get_item_journal_id(&p, false).await
        }));
    }
    let mut ids = Vec::new();
    for task in jobs {
        ids.push(task.await.unwrap());
    }
    ids.sort_unstable();
    assert_eq!(ids, (611635800..611635812).collect::<Vec<_>>());
    let id = j::add_customization_journal(&p, false).await;
    assert_eq!(id, 4419700);
    type JournalRow = (i64, Vec<u8>, i64, Option<Vec<u8>>);
    let rows:Vec<JournalRow>=sqlx::query_as("SELECT id,category_utf16,time_ticks,legacy_last_update FROM game_item_journal01 UNION ALL SELECT id,category_utf16,time_ticks,legacy_last_update FROM game_item_journal02").fetch_all(&p).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, id);
    assert_eq!(
        rows[0].1,
        "CustomizeCharacter"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert!(rows[0].2 > 0);
    assert_eq!(rows[0].3, None);
    let partition:i32=sqlx::query_scalar("SELECT CASE WHEN ((floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT/25920000)-40542)%2=0 THEN 1 ELSE 2 END").fetch_one(&p).await.unwrap();
    let table = if partition == 1 {
        "game_item_journal01"
    } else {
        "game_item_journal02"
    };
    assert_eq!(
        sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&p)
            .await
            .unwrap(),
        1
    );
    j::preserve(&p, &source, false).await.unwrap();
    j::preserve(&p, &source, true).await.unwrap();
    assert_eq!(j::get_item_journal_id(&p, false).await, 611635812);
    assert!(matches!(
        j::preserve(&p, &snapshot("null", "null"), false).await,
        Err(JournalError::Conflict)
    ));
    cleanup_game_pool(admin, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn journal_failure_and_rollback_never_reuse_consumed_ids() {
    let (admin, p, s) = isolated_game_pool().await;
    j::preserve(&p, &snapshot("null", "null"), false)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    let id: i64 = sqlx::query_scalar("SELECT nextval('game_item_journal_id')")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(id, 611635750);
    tx.rollback().await.unwrap();
    assert_eq!(j::get_item_journal_id(&p, false).await, 611635751);
    for t in ["game_item_journal01", "game_item_journal02"] {
        sqlx::query(&format!(
            "ALTER TABLE {t} ADD CONSTRAINT refuse_insert CHECK(false) NOT VALID"
        ))
        .execute(&p)
        .await
        .unwrap();
    }
    assert_eq!(j::add_customization_journal(&p, false).await, -1);
    for t in ["game_item_journal01", "game_item_journal02"] {
        sqlx::query(&format!("ALTER TABLE {t} DROP CONSTRAINT refuse_insert"))
            .execute(&p)
            .await
            .unwrap();
    }
    assert_eq!(j::add_customization_journal(&p, false).await, 4419626);
    // Startup/connection recycling must not reset either source identity.
    j::preserve(&p, &snapshot("null", "null"), false)
        .await
        .unwrap();
    assert_eq!(j::get_item_journal_id(&p, false).await, 611635752);
    sqlx::query("SELECT setval('game_item_journal_id',9223372036854775807,true)")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(j::get_item_journal_id(&p, false).await, -1);
    cleanup_game_pool(admin, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn journal_bootstrap_is_atomic_and_does_not_repair_divergent_sequences() {
    let (admin, p, s) = isolated_game_pool().await;
    sqlx::query(
        "ALTER TABLE game_journal_identity ADD CONSTRAINT reject_second CHECK(kind=1) NOT VALID",
    )
    .execute(&p)
    .await
    .unwrap();
    let source = snapshot("\"611635799\"", "\"4419699\"");
    assert!(j::preserve(&p, &source, false).await.is_err());
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_journal_identity")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 0);
    let v: (i64, bool) = sqlx::query_as("SELECT last_value,is_called FROM game_item_journal_id")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(v, (611635750, false));
    sqlx::query("ALTER TABLE game_journal_identity DROP CONSTRAINT reject_second")
        .execute(&p)
        .await
        .unwrap();
    j::preserve(&p, &source, false).await.unwrap();
    sqlx::query("SELECT setval('game_item_journal_id',611635750,true)")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        j::preserve(&p, &source, false).await,
        Err(JournalError::Conflict)
    ));
    cleanup_game_pool(admin, p, s).await;
}
