type StoredRow = (
    i64,
    i64,
    Vec<u8>,
    i32,
    Option<i32>,
    Option<i64>,
    Option<i16>,
);
use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use sqlx::PgPool;
use vindictus_server::{
    error::Error,
    infrastructure::{character_status_effects as db, postgres},
};
fn input() -> Value {
    json!({"format_version":1,"source":"study_heroes.status-effects","source_last_value":"100","effects":[{"row_id":"2","cid":"1","type_utf16":"4100","level":i32::MIN,"remain_time":null,"expire_ticks":null,"remain_micro":null},{"row_id":"9","cid":"1","type_utf16":"4100","level":i32::MAX,"remain_time":i32::MIN,"expire_ticks":"3155378975999999999","remain_micro":255},{"row_id":"10","cid":"1","type_utf16":"00d8","level":0,"remain_time":0,"expire_ticks":"0","remain_micro":0}]})
}
fn snapshot(v: &Value) -> db::Snapshot {
    db::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
async fn roots(p: &PgPool) {
    sqlx::query("INSERT INTO game_characters(id)VALUES(1),(2)")
        .execute(p)
        .await
        .unwrap();
}
async fn seq(p: &PgPool) -> (i64, bool) {
    sqlx::query_as("SELECT last_value,is_called FROM game_status_effect_row_id")
        .fetch_one(p)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn status_preservation_compares_all_fields_freezes_hash_and_concurrent_bootstrap_never_reseeds()
 {
    let (a, p, s) = isolated_game_pool().await;
    roots(&p).await;
    let v = input();
    let snap = snapshot(&v);
    let (x, y) = tokio::join!(
        db::preserve(&p, &snap, false),
        db::preserve(&p, &snap, false)
    );
    let (x, y) = (x.unwrap(), y.unwrap());
    assert_eq!(x.inserted + y.inserted, 3);
    let actual:Vec<StoredRow>=sqlx::query_as("SELECT row_id,cid,type_utf16,level,remain_time,expire_ticks,remain_micro FROM game_status_effects ORDER BY row_id").fetch_all(&p).await.unwrap();
    assert_eq!(
        actual,
        vec![
            (2, 1, vec![65, 0], i32::MIN, None, None, None),
            (
                9,
                1,
                vec![65, 0],
                i32::MAX,
                Some(i32::MIN),
                Some(3155378975999999999),
                Some(255)
            ),
            (10, 1, vec![0, 216], 0, Some(0), Some(0), Some(0))
        ]
    );
    let first: i64 = sqlx::query_scalar("SELECT nextval('game_status_effect_row_id')")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(first, 101);
    assert_eq!(db::preserve(&p, &snap, false).await.unwrap().inserted, 0);
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().verified, 3);
    let next: i64 = sqlx::query_scalar("SELECT nextval('game_status_effect_row_id')")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(next, 102);
    let mut changed = v;
    changed["effects"][0]["level"] = json!(1);
    assert!(matches!(
        db::preserve(&p, &snapshot(&changed), false).await,
        Err(db::StatusError::Conflict)
    ));
    sqlx::query("UPDATE game_status_effects SET remain_time=7 WHERE row_id=9")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, true).await,
        Err(db::StatusError::Conflict)
    ));
    let time: Option<i32> =
        sqlx::query_scalar("SELECT remain_time FROM game_status_effects WHERE row_id=9")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(time, Some(7));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn status_late_conflict_extra_rows_and_fk_orphan_rollback_without_sequence_or_marker_changes()
{
    let (a, p, s) = isolated_game_pool().await;
    roots(&p).await;
    let snap = snapshot(&input());
    sqlx::query("INSERT INTO game_status_effects VALUES(9,1,$1,7,NULL,NULL,NULL)")
        .bind(vec![65u8, 0])
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, false).await,
        Err(db::StatusError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_status_effects")
            .fetch_one(&p)
            .await
            .unwrap(),
        1
    );
    sqlx::raw_sql("DELETE FROM game_status_effects;INSERT INTO game_status_effects VALUES(99,1,decode('','hex'),0,NULL,NULL,NULL)").execute(&p).await.unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, false).await,
        Err(db::StatusError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_status_effects")
            .fetch_one(&p)
            .await
            .unwrap(),
        1
    );
    sqlx::query("DELETE FROM game_status_effects")
        .execute(&p)
        .await
        .unwrap();
    let mut orphan = input();
    orphan["effects"][2]["cid"] = json!("999");
    assert!(matches!(
        db::preserve(&p, &snapshot(&orphan), false).await,
        Err(db::StatusError::Database(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_status_effects")
            .fetch_one(&p)
            .await
            .unwrap(),
        0
    );
    let state:(bool,Option<i64>,Option<Vec<u8>>)=sqlx::query_as("SELECT initialized,source_last_value,source_sha256 FROM game_status_effect_preservation WHERE singleton").fetch_one(&p).await.unwrap();
    assert_eq!(state, (false, None, None));
    assert_eq!(seq(&p).await, (1, false));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn status_absence_guard_requires_initialized_complete_source_and_never_writes_or_consumes_ids()
 {
    let (a, p, s) = isolated_game_pool().await;
    roots(&p).await;
    let mut tx = p.begin().await.unwrap();
    sqlx::query("SELECT id FROM game_characters WHERE id=2 FOR UPDATE")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(matches!(
        db::require_absent(&mut tx, 2).await,
        Err(db::StatusError::Uninitialized)
    ));
    tx.rollback().await.unwrap();
    db::preserve(&p, &snapshot(&input()), false).await.unwrap();
    let before = seq(&p).await;
    sqlx::query("CREATE FUNCTION reject_status_guard_write()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected absence-guard write';END$$").execute(&p).await.unwrap();
    for table in ["game_status_effects", "game_status_effect_preservation"] {
        sqlx::query(&format!("CREATE TRIGGER reject_status_guard_write BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_status_guard_write()")).execute(&p).await.unwrap();
    }
    let mut tx = p.begin().await.unwrap();
    sqlx::query("SELECT id FROM game_characters WHERE id IN(1,2) ORDER BY id FOR UPDATE")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(matches!(
        db::require_absent(&mut tx, 1).await,
        Err(db::StatusError::Existing)
    ));
    db::require_absent(&mut tx, 2).await.unwrap();
    assert!(matches!(
        db::require_absent(&mut tx, 99).await,
        Err(db::StatusError::MissingCharacter)
    ));
    tx.commit().await.unwrap();
    assert_eq!(seq(&p).await, before);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_status_effects")
            .fetch_one(&p)
            .await
            .unwrap(),
        3
    );
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn status_schema_readiness_requires_all_fields_and_sequence_with_preservation_on_migration_replay()
 {
    let (a, p, s) = isolated_game_pool().await;
    roots(&p).await;
    let snap = snapshot(&input());
    db::preserve(&p, &snap, false).await.unwrap();
    let before = seq(&p).await;
    postgres::migrate(&p).await.unwrap();
    postgres::check_ready(&p).await.unwrap();
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().verified, 3);
    assert_eq!(seq(&p).await, before);
    sqlx::query("ALTER TABLE game_status_effects RENAME COLUMN remain_micro TO unexpected_micro")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        postgres::check_ready(&p).await,
        Err(Error::SchemaNotReady)
    ));
    sqlx::query("ALTER TABLE game_status_effects RENAME COLUMN unexpected_micro TO remain_micro")
        .execute(&p)
        .await
        .unwrap();
    db::check_ready(&p).await.unwrap();
    sqlx::query("DROP SEQUENCE game_status_effect_row_id")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        postgres::check_ready(&p).await,
        Err(Error::SchemaNotReady)
    ));
    cleanup_game_pool(a, p, s).await;
}
