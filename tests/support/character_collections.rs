use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use vindictus_server::infrastructure::character_collections::{self as storage, CollectionError};
fn data() -> Value {
    serde_json::from_str(include_str!(
        "../fixtures/synthetic-character-collections.json"
    ))
    .unwrap()
}
fn snapshot(v: &Value) -> storage::Snapshot {
    storage::read_snapshot(v.to_string().as_bytes()).unwrap()
}
async fn count(pool: &sqlx::PgPool, table: &str) -> i64 {
    // Test-only identifiers selected by the test, never input to a production query.
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_collections_preserve_replay_concurrency_defaults_and_source_cascade() {
    let (admin, pool, schema) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_characters VALUES(1),(2),(3)")
        .execute(&pool)
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    for _ in 0..8 {
        let p = pool.clone();
        let b = barrier.clone();
        tasks.spawn(async move {
            b.wait().await;
            storage::preserve(&p, &snapshot(&data()), false).await
        });
    }
    let mut inserted = [0; 5];
    while let Some(r) = tasks.join_next().await {
        match r.unwrap() {
            Ok(s) => {
                for (i, n) in s.inserted.into_iter().enumerate() {
                    inserted[i] += n;
                }
                assert_eq!(s.verified, [2, 2, 2, 2, 3]);
            }
            Err(CollectionError::Database(sqlx::Error::Database(e)))
                if e.code().as_deref() == Some("55P03") => {}
            _ => panic!("unexpected preservation result"),
        }
    }
    assert_eq!(inserted, [2, 2, 2, 2, 3]);
    assert_eq!(
        storage::preserve(&pool, &snapshot(&data()), false)
            .await
            .unwrap()
            .inserted,
        [0; 5]
    );
    storage::preserve(&pool, &snapshot(&data()), true)
        .await
        .unwrap();
    for (table, key) in [
        ("game_item_collections", "owner_id"),
        ("game_quest_collections", "owner_id"),
        ("game_mailboxes", "owner_id"),
    ] {
        let value: i32 = sqlx::query_scalar(&format!(
            "INSERT INTO {table}({key}) VALUES(3) RETURNING permission"
        ))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(value, 3);
    }
    let story: (i32, i16) = sqlx::query_as(
        "INSERT INTO game_story_entries(cid) VALUES(3) RETURNING permission,hint_category",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(story, (2, 0));
    sqlx::query("UPDATE game_characters SET id=11 WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();
    for (table, key) in [
        ("game_item_collections", "owner_id"),
        ("game_quest_collections", "owner_id"),
        ("game_mailboxes", "owner_id"),
        ("game_story_entries", "cid"),
    ] {
        let updated: i64 =
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE {key}=11"))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(updated, 1);
    }
    sqlx::query("DELETE FROM game_characters WHERE id=11")
        .execute(&pool)
        .await
        .unwrap();
    for table in [
        "game_item_collections",
        "game_quest_collections",
        "game_mailboxes",
        "game_story_entries",
    ] {
        assert_eq!(count(&pool, table).await, 2);
    }
    let fks:i64=sqlx::query_scalar("SELECT COUNT(*) FROM pg_constraint WHERE contype='f' AND confdeltype='c' AND confupdtype='c' AND conrelid IN ('game_item_collections'::regclass,'game_quest_collections'::regclass,'game_mailboxes'::regclass,'game_story_entries'::regclass)").fetch_one(&pool).await.unwrap();
    assert_eq!(fks, 4);
    vindictus_server::infrastructure::postgres::check_ready(&pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE game_story_entries DROP COLUMN hint_category")
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
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_settings_preserve_multiset_bytes_and_late_conflict_rolls_back_all_owners() {
    let (admin, pool, schema) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_characters VALUES(1),(2),(3)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        storage::preserve(&pool, &snapshot(&data()), true).await,
        Err(CollectionError::Conflict)
    ));
    storage::preserve(&pool, &snapshot(&data()), false)
        .await
        .unwrap();
    let mut v = data();
    v["settings"].as_array_mut().unwrap().reverse();
    storage::preserve(&pool, &snapshot(&v), true).await.unwrap();
    let duplicates:i64=sqlx::query_scalar("SELECT COUNT(*) FROM game_my_settings WHERE code_bytes='\\x'::bytea AND value_bytes='\\x80ff20'::bytea").fetch_one(&pool).await.unwrap();
    assert_eq!(duplicates, 2);
    for key in ["item_collections", "quest_collections", "mailboxes"] {
        v[key]
            .as_array_mut()
            .unwrap()
            .push(json!({"owner_id":"3","permission":3}));
    }
    v["story_entries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"cid":"3","permission":2,"hint_category":0}));
    v["settings"][0]["value_hex"] = json!("01");
    assert!(matches!(
        storage::preserve(&pool, &snapshot(&v), false).await,
        Err(CollectionError::Conflict)
    ));
    for table in [
        "game_item_collections",
        "game_quest_collections",
        "game_mailboxes",
        "game_story_entries",
    ] {
        assert_eq!(count(&pool, table).await, 2);
    }
    assert_eq!(count(&pool, "game_my_settings").await, 3);
    let constraints:i64=sqlx::query_scalar("SELECT COUNT(*) FROM pg_constraint WHERE conrelid='game_my_settings'::regclass AND contype IN('p','u','f')").fetch_one(&pool).await.unwrap();
    assert_eq!(constraints, 0);
    assert!(
        sqlx::query("INSERT INTO game_my_settings VALUES(NULL,'\\x'::bytea)")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query(
            "INSERT INTO game_my_settings VALUES(decode(repeat('00',256),'hex'),'\\x'::bytea)"
        )
        .execute(&pool)
        .await
        .is_err()
    );
    sqlx::query("DELETE FROM game_item_collections WHERE owner_id=2")
        .execute(&pool)
        .await
        .unwrap();
    let mut v = data();
    v["item_collections"][1]["owner_id"] = json!("99");
    assert!(
        storage::preserve(&pool, &snapshot(&v), false)
            .await
            .is_err()
    );
    assert_eq!(count(&pool, "game_item_collections").await, 1);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_collection_cancellation_rolls_back_inserts_before_blocked_story() {
    let (admin, pool, schema) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_characters VALUES(1),(2)")
        .execute(&pool)
        .await
        .unwrap();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO game_story_entries(cid,permission,hint_category) VALUES(1,2,0)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let p = pool.clone();
    let task = tokio::spawn(async move { storage::preserve(&p, &snapshot(&data()), false).await });
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    assert!(!task.is_finished());
    task.abort();
    assert!(matches!(task.await, Err(e) if e.is_cancelled()));
    blocker.commit().await.unwrap();
    for table in [
        "game_item_collections",
        "game_quest_collections",
        "game_mailboxes",
        "game_my_settings",
    ] {
        assert_eq!(count(&pool, table).await, 0);
    }
    storage::preserve(&pool, &snapshot(&data()), false)
        .await
        .unwrap();
    cleanup_game_pool(admin, pool, schema).await;
}
