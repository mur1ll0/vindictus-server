use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use vindictus_server::infrastructure::item_state::{self as db, ItemStateError};
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/item-state-synthetic.json")).unwrap()
}
fn snapshot(v: &Value) -> db::Snapshot {
    db::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
async fn setup(p: &PgPool) {
    sqlx::raw_sql("INSERT INTO game_characters VALUES(1);INSERT INTO game_item_collections(owner_id) VALUES(1)").execute(p).await.unwrap();
}
async fn counts(p: &PgPool) -> (i64, i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT COUNT(*) FROM game_items),(SELECT COUNT(*) FROM game_item_attributes),(SELECT COUNT(*) FROM game_equippables),(SELECT COUNT(*) FROM game_item_storage_status)").fetch_one(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_state_preserves_every_field_orphans_concurrent_import_and_exact_identity() {
    let (a, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    let s = Arc::new(snapshot(&fixture()));
    assert!(matches!(
        db::preserve(&p, &s, true).await,
        Err(ItemStateError::Uninitialized)
    ));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..2 {
        let p = p.clone();
        let s = s.clone();
        tasks.spawn(async move { db::preserve(&p, &s, false).await.unwrap().inserted });
    }
    let mut total = [0; 4];
    while let Some(r) = tasks.join_next().await {
        for (i, v) in r.unwrap().into_iter().enumerate() {
            total[i] += v;
        }
    }
    assert_eq!(total, [2, 2, 2, 2]);
    assert_eq!(counts(&p).await, (2, 2, 2, 2));
    assert_eq!(db::preserve(&p, &s, true).await.unwrap().verified, [2; 4]);
    let next: (i64, i64) =
        sqlx::query_as("SELECT nextval('game_item_attribute_id'),nextval('game_item_storage_id')")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(next, (11, 21));
    assert_eq!(db::preserve(&p, &s, false).await.unwrap().inserted, [0; 4]);
    let next: (i64, i64) =
        sqlx::query_as("SELECT nextval('game_item_attribute_id'),nextval('game_item_storage_id')")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(next, (12, 22));
    let mut bad = fixture();
    bad["attribute_last_value"] = json!("11");
    assert!(matches!(
        db::preserve(&p, &snapshot(&bad), false).await,
        Err(ItemStateError::Conflict)
    ));
    for table in ["items", "attributes", "equipment", "storage"] {
        let mut subset = fixture();
        subset[table].as_array_mut().unwrap().pop();
        assert!(
            matches!(
                db::preserve(&p, &snapshot(&subset), true).await,
                Err(ItemStateError::Conflict)
            ),
            "truncated {table}"
        );
    }
    // Source has no FK from Attribute/Equippable: preserve orphans and do not invent cascades.
    sqlx::query("DELETE FROM game_item_collections WHERE owner_id=1")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(counts(&p).await, (1, 2, 2, 0));
    cleanup_game_pool(a, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_state_replay_refuses_each_mutated_column_without_repair_or_inserting() {
    let (a, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    let v = fixture();
    db::preserve(&p, &snapshot(&v), false).await.unwrap();
    for table in ["items", "attributes", "equipment", "storage"] {
        let keys: Vec<_> = v[table][0].as_object().unwrap().keys().cloned().collect();
        for field in keys {
            let mut bad = v.clone();
            let value = &v[table][0][&field];
            bad[table][0][&field] = if field == "id" {
                json!("4")
            } else if field == "owner_id" || field == "item_id" {
                json!("2")
            } else if field == "expire_ticks" || field == "expire_units300" {
                json!("0")
            } else if value.is_null() {
                json!(0)
            } else if value.is_boolean() {
                json!(!value.as_bool().unwrap())
            } else if value.is_number() {
                json!(if value.as_i64() == Some(0) { 1 } else { 0 })
            } else {
                json!(if field == "attribute_bytes" {
                    "58"
                } else {
                    "4100"
                })
            };
            assert!(
                db::preserve(&p, &snapshot(&bad), false).await.is_err(),
                "changed {table}.{field}"
            );
            assert_eq!(counts(&p).await, (2, 2, 2, 2));
        }
    }
    assert_eq!(
        db::preserve(&p, &snapshot(&v), true)
            .await
            .unwrap()
            .verified,
        [2; 4]
    );
    cleanup_game_pool(a, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_state_late_errors_and_cancelled_import_roll_back_rows_and_sequence_restart() {
    let (a, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    let v = fixture();
    let mut bad = v.clone();
    bad["storage"][1]["owner_id"] = json!("99");
    assert!(db::preserve(&p, &snapshot(&bad), false).await.is_err());
    assert_eq!(counts(&p).await, (0, 0, 0, 0));
    sqlx::raw_sql("CREATE FUNCTION reject_item_bootstrap() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic late failure';END$$;CREATE TRIGGER reject_item_bootstrap BEFORE UPDATE ON game_item_identity FOR EACH ROW EXECUTE FUNCTION reject_item_bootstrap()").execute(&p).await.unwrap();
    assert!(db::preserve(&p, &snapshot(&v), false).await.is_err());
    assert_eq!(counts(&p).await, (0, 0, 0, 0));
    let seq: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_item_attribute_id")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(seq, (1, false));
    let seq: (i64, bool) = sqlx::query_as("SELECT last_value,is_called FROM game_item_storage_id")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(seq, (1, false));
    sqlx::raw_sql("DROP TRIGGER reject_item_bootstrap ON game_item_identity;CREATE FUNCTION block_item_storage() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_advisory_xact_lock(183669);RETURN NEW;END$$;CREATE TRIGGER block_item_storage BEFORE INSERT ON game_item_storage_status FOR EACH ROW EXECUTE FUNCTION block_item_storage()").execute(&p).await.unwrap();
    let mut block = p.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(183669)")
        .execute(&mut *block)
        .await
        .unwrap();
    let cp = p.clone();
    let task = tokio::spawn(async move { db::preserve(&cp, &snapshot(&v), false).await });
    tokio::time::timeout(Duration::from_secs(2),async{loop{let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname=current_database()))").fetch_one(&p).await.unwrap();if waiting{break}tokio::time::sleep(Duration::from_millis(10)).await}}).await.unwrap();
    task.abort();
    assert!(matches!(task.await, Err(error) if error.is_cancelled()));
    block.rollback().await.unwrap();
    assert_eq!(counts(&p).await, (0, 0, 0, 0));
    let initialized: bool = sqlx::query_scalar("SELECT initialized FROM game_item_identity")
        .fetch_one(&p)
        .await
        .unwrap();
    assert!(!initialized);
    sqlx::raw_sql("DROP TRIGGER block_item_storage ON game_item_storage_status")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        db::preserve(&p, &snapshot(&fixture()), false)
            .await
            .unwrap()
            .inserted,
        [2; 4]
    );
    cleanup_game_pool(a, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_state_attribute_unique_index_matches_all_10201_source_collation_pairs() {
    let (a, p, schema) = isolated_game_pool().await;
    let f: Value = serde_json::from_str(include_str!(
        "../fixtures/sql-item-attribute-collation.json"
    ))
    .unwrap();
    let keys: Vec<Vec<u8>> = f["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            v["hex"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect()
        })
        .collect();
    let codes: Vec<i32> = f["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["code"].as_i64().unwrap() as i32)
        .collect();
    let actual:Vec<(i32,i32)>=sqlx::query_as("WITH keys AS (SELECT code,item_attribute_ascii_key(value) AS key FROM unnest($1::bytea[],$2::integer[]) AS v(value,code)) SELECT a.code,b.code FROM keys a CROSS JOIN keys b WHERE a.key=b.key").bind(keys).bind(codes).fetch_all(&p).await.unwrap();
    let expected: BTreeSet<(i32, i32)> = f["equal_pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["l"].as_i64().unwrap() as i32,
                v["r"].as_i64().unwrap() as i32,
            )
        })
        .collect();
    assert_eq!(actual.into_iter().collect::<BTreeSet<_>>(), expected);
    sqlx::query(
        "INSERT INTO game_item_attributes VALUES(1,99,decode('44555220','hex'),''::bytea,1,2)",
    )
    .execute(&p)
    .await
    .unwrap();
    assert!(sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) VALUES(2,99,decode('647572','hex'),''::bytea,3,4)").execute(&p).await.is_err());
    assert!(sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) VALUES(2,99,decode('ff','hex'),''::bytea,3,4)").execute(&p).await.is_err());
    cleanup_game_pool(a, p, schema).await;
}
