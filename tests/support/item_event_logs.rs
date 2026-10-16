use super::{cleanup_game_pool, isolated_game_pool};
use sqlx::{PgPool, Row};
use std::time::Duration;
use vindictus_server::{
    domain::{
        item_logs::{Counts, Data, Ledger, Manager, Record},
        legacy_user_data::{LegacyBytes, SqlDateTime2},
    },
    infrastructure::item_event_logs::{self as logs, Attempt},
};
fn deadline() -> tokio::time::Instant {
    tokio::time::Instant::now() + Duration::from_secs(15)
}
fn data(id: i64) -> Data {
    Data::new(
        id,
        vec![0, 216, 0, 0],
        i32::MIN,
        [i32::MIN, 0, i32::MAX],
        -7,
        9,
        false,
    )
    .unwrap()
}
fn record(id: i64) -> Record {
    Record {
        journal: -1,
        event: 19,
        detail: 2,
        before_id: Some(i64::MIN),
        data: data(id),
        after: Counts {
            inventory: Some(i32::MAX),
            shared: None,
        },
        memo: Some(LegacyBytes::from_bytes(vec![0, 220]).unwrap()),
        captured: SqlDateTime2::from_ticks(638000000000017001).unwrap(),
    }
}
async fn number(p: &PgPool, family: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT (SELECT count(*) FROM game_item_{family}01)+(SELECT count(*) FROM game_item_{family}02)")).fetch_one(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn item_event_logs_preserve_all_fields_and_allow_original_duplicate_events() {
    let (a, p, s) = isolated_game_pool().await;
    let r = record(i64::MAX);
    assert_eq!(
        logs::write_log(&p, -31, &r, deadline()).await,
        Attempt::Written
    );
    assert_eq!(
        logs::write_log(&p, -31, &r, deadline()).await,
        Attempt::Written
    );
    assert_eq!(number(&p, "log").await, 2);
    let rows = sqlx::query("SELECT * FROM game_item_log01 UNION ALL SELECT * FROM game_item_log02")
        .fetch_all(&p)
        .await
        .unwrap();
    for x in rows {
        assert_eq!(x.get::<i64, _>("journal_id"), -1);
        assert_eq!(x.get::<i64, _>("character_id"), -31);
        assert_eq!(x.get::<i16, _>("event"), 19);
        assert_eq!(x.get::<i16, _>("detail"), 2);
        assert_eq!(x.get::<Option<i64>, _>("before_item_id"), Some(i64::MIN));
        assert_eq!(x.get::<i64, _>("item_id"), i64::MAX);
        assert_eq!(x.get::<Vec<u8>, _>("item_class_utf16"), r.data.class());
        assert_eq!(x.get::<i32, _>("count"), i32::MIN);
        assert_eq!(x.get::<Option<i32>, _>("after_inventory"), Some(i32::MAX));
        assert_eq!(x.get::<Option<i32>, _>("after_shared"), None);
        assert_eq!(x.get::<Option<i32>, _>("color1"), Some(i32::MIN));
        assert_eq!(x.get::<Option<i32>, _>("color2"), Some(0));
        assert_eq!(x.get::<Option<i32>, _>("color3"), Some(i32::MAX));
        assert_eq!(x.get::<Option<i32>, _>("reduced_durability"), Some(-7));
        assert_eq!(x.get::<Option<i32>, _>("max_durability_bonus"), Some(9));
        assert_eq!(
            x.get::<Option<Vec<u8>>, _>("memo_utf16"),
            Some(vec![0, 220])
        );
        assert_eq!(
            x.get::<i64, _>("time_units300"),
            r.captured.sql_parameter().unwrap().units300()
        );
        assert_eq!(x.get::<Option<Vec<u8>>, _>("legacy_last_update"), None);
    }
    let mut invalid = record(1);
    invalid.captured = SqlDateTime2::from_ticks(0).unwrap();
    assert_eq!(
        logs::write_log(&p, 23, &invalid, deadline()).await,
        Attempt::Failed
    );
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn item_event_ledger_matches_source_defaults_suppression_and_simple_failure_fields() {
    let (a, p, s) = isolated_game_pool().await;
    let r = Ledger::creation(17, data(101), Some(11));
    assert_eq!(
        logs::write_ledger(&p, 23, &r, true, deadline()).await,
        Attempt::Suppressed
    );
    let invalid = Ledger::creation(-1, data(101), None);
    assert_eq!(
        logs::write_ledger(&p, 23, &invalid, false, deadline()).await,
        Attempt::Suppressed
    );
    let mut ignored = Ledger::creation(17, data(101), None);
    ignored.event = 18;
    assert_eq!(
        logs::write_ledger(&p, 23, &ignored, false, deadline()).await,
        Attempt::Suppressed
    );
    for _ in 0..2 {
        assert_eq!(
            logs::write_ledger(&p, 23, &r, false, deadline()).await,
            Attempt::Written
        );
    }
    let rows =
        sqlx::query("SELECT * FROM game_item_ledger01 UNION ALL SELECT * FROM game_item_ledger02")
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(rows.len(), 2);
    for x in rows {
        assert_eq!(x.get::<i64, _>("journal_id"), 17);
        assert_eq!(x.get::<i64, _>("item_id"), 101);
        assert_eq!(x.get::<Vec<u8>, _>("item_class_utf16"), r.data.class());
        assert_eq!(x.get::<i64, _>("character_id"), 23);
        assert_eq!(x.get::<i16, _>("event"), 2);
        assert_eq!(x.get::<i32, _>("count"), i32::MIN);
        assert_eq!(x.get::<i16, _>("action"), 23);
        assert_eq!(x.get::<Option<i32>, _>("bundle_item_count"), Some(11));
        assert!(x.get::<i64, _>("time_units300") > 0);
        assert_eq!(x.get::<Option<Vec<u8>>, _>("legacy_last_update"), None);
        for (k, v) in [
            ("color1", i32::MIN),
            ("color2", 0),
            ("color3", i32::MAX),
            ("reduced_durability", -7),
            ("max_durability_bonus", 9),
        ] {
            assert_eq!(x.get::<Option<i32>, _>(k), Some(v));
        }
    }
    for t in ["game_item_ledger01", "game_item_ledger02"] {
        sqlx::query(&format!(
            "ALTER TABLE {t} ADD CONSTRAINT reject_give CHECK(event<>2) NOT VALID"
        ))
        .execute(&p)
        .await
        .unwrap();
    }
    let batch = [
        Ledger::creation(17, data(102), None),
        Ledger::creation_failure(17, vec![0, 220]).unwrap(),
        Ledger::creation(-1, data(103), None),
    ];
    let out = logs::write_ledgers(&p, 23, &batch, false).await.unwrap();
    assert_eq!((out.written, out.failed, out.suppressed), (1, 1, 1));
    let msg = "giveDefaultItemFail(save)"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let f = Ledger::creation_failure(17, msg).unwrap();
    assert_eq!(
        logs::write_ledger(&p, 23, &f, false, deadline()).await,
        Attempt::Written
    );
    let x=sqlx::query("SELECT * FROM game_item_ledger01 WHERE event=0 AND octet_length(item_class_utf16)>2 UNION ALL SELECT * FROM game_item_ledger02 WHERE event=0 AND octet_length(item_class_utf16)>2").fetch_one(&p).await.unwrap();
    assert_eq!(x.get::<i64, _>("item_id"), -1);
    assert_eq!(x.get::<i32, _>("count"), 0);
    assert_eq!(x.get::<Option<i32>, _>("bundle_item_count"), None);
    for k in [
        "color1",
        "color2",
        "color3",
        "reduced_durability",
        "max_durability_bonus",
    ] {
        assert_eq!(x.get::<Option<i32>, _>(k), Some(0));
    }
    cleanup_game_pool(a, p, s).await;
}
fn manager() -> Manager {
    let mut m = Manager::default();
    m.set_journal(-1, 19);
    for id in [1, 2] {
        m.add(data(id), true, || {
            SqlDateTime2::from_ticks(638000000000000000).unwrap()
        })
        .unwrap();
    }
    m
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn item_event_batch_catches_each_failure_clears_after_writes_and_commits_independently() {
    let (a, p, s) = isolated_game_pool().await;
    for table in ["game_item_log01", "game_item_log02"] {
        sqlx::query(&format!(
            "ALTER TABLE {table} ADD CONSTRAINT fail_first CHECK(item_id<>1) NOT VALID"
        ))
        .execute(&p)
        .await
        .unwrap();
    }
    sqlx::query("CREATE TABLE fake_inventory(id INT)")
        .execute(&p)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    sqlx::query("INSERT INTO fake_inventory VALUES(1)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let mut m = manager();
    let out = logs::write_manager(&p, 23, &mut m, true, |_| Counts {
        inventory: Some(3),
        shared: None,
    })
    .await;
    assert_eq!((out.written, out.failed), (1, 1));
    assert_eq!(m.pending(), 0);
    tx.rollback().await.unwrap();
    assert_eq!(number(&p, "log").await, 1);
    let x:Option<i32>=sqlx::query_scalar("SELECT after_inventory FROM game_item_log01 UNION ALL SELECT after_inventory FROM game_item_log02").fetch_one(&p).await.unwrap();
    assert_eq!(x, Some(3));
    let out = logs::write_manager(&p, 23, &mut m, true, |_| {
        panic!("empty queue has no last entry")
    })
    .await;
    assert_eq!((out.written, out.failed), (0, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn item_event_cancelled_flush_retains_queue_and_disabled_flush_does_not_query_database() {
    let (a, p, s) = isolated_game_pool().await;
    let mut m = manager();
    let out = logs::write_manager(&p, 23, &mut m, false, |_| panic!("disabled")).await;
    assert_eq!((out.written, out.failed), (0, 0));
    assert_eq!(m.pending(), 2);
    sqlx::query("CREATE FUNCTION block_item_event()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_advisory_xact_lock(hashtext(current_schema()),74923);RETURN NEW;END$$").execute(&p).await.unwrap();
    for t in ["game_item_log01", "game_item_log02"] {
        sqlx::query(&format!("CREATE TRIGGER block_item_event BEFORE INSERT ON {t} FOR EACH ROW EXECUTE FUNCTION block_item_event()")).execute(&p).await.unwrap();
    }
    let mut blocker = p.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(hashtext(current_schema()),74923)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    {
        let flush = logs::write_manager(&p, 23, &mut m, true, |_| Counts::default());
        tokio::pin!(flush);
        tokio::select! { _=&mut flush=>panic!("flush must block"), observed=tokio::time::timeout(Duration::from_secs(4),async{loop{if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=74923 AND NOT granted)").fetch_one(&p).await.unwrap(){break}tokio::time::sleep(Duration::from_millis(25)).await;}})=>{observed.expect("blocked log attempt observed");} }
    }
    assert_eq!(m.pending(), 2);
    sqlx::query("SELECT pg_advisory_unlock(hashtext(current_schema()),74923)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    drop(blocker);
    cleanup_game_pool(a, p, s).await;
}
