use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::json;
use vindictus_server::infrastructure::{character_transfer_guard as db, postgres};
fn source(schema: &str) -> db::Snapshot {
    let v = json!({"format_version":1,"source":"study_heroes.character-transfer-empty","rows":0,"columns":8,"schema_sha256":schema.repeat(64),"procedure_sha256":"6f47c1f5f6926d7d5bd474fa91547c2e31343d3afc0f8853e3a0ffee0370baaa"});
    db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn transfer_guard_bootstrap_replay_locks_and_nonempty_reference_refusal() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = source("a");
    assert!(matches!(
        db::preserve(&p, &snap, true).await,
        Err(db::TransferError::Reference)
    ));
    let mut tx = p.begin().await.unwrap();
    assert!(db::require_empty(&mut tx).await.is_err());
    tx.rollback().await.unwrap();
    let (x, y) = tokio::join!(
        db::preserve(&p, &snap, false),
        db::preserve(&p, &snap, false)
    );
    assert_ne!(x.unwrap().replay, y.unwrap().replay);
    assert!(db::preserve(&p, &snap, true).await.unwrap().initialized);
    assert!(matches!(
        db::preserve(&p, &source("b"), false).await,
        Err(db::TransferError::Reference)
    ));
    let mut held = p.begin().await.unwrap();
    db::require_empty(&mut held).await.unwrap();
    let mut writer = p.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout='100ms'")
        .execute(&mut *writer)
        .await
        .unwrap();
    let err = sqlx::query(
        "INSERT INTO game_character_transfer_info VALUES(1,''::bytea,1,1,''::bytea,2,2,NULL)",
    )
    .execute(&mut *writer)
    .await
    .unwrap_err();
    assert_eq!(
        err.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("55P03")
    );
    writer.rollback().await.unwrap();
    held.commit().await.unwrap();
    sqlx::query(
        "INSERT INTO game_character_transfer_info VALUES(1,''::bytea,1,1,''::bytea,2,2,NULL)",
    )
    .execute(&p)
    .await
    .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, false).await,
        Err(db::TransferError::Reference)
    ));
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_character_transfer_info")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 1, "never repair unexpected transfer rows");
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn transfer_guard_schema_preserves_pk_nullable_utf16_bounds_and_detects_missing_columns() {
    let (a, p, s) = isolated_game_pool().await;
    sqlx::query(
        "INSERT INTO game_character_transfer_info VALUES(1,''::bytea,1,1,''::bytea,2,2,NULL)",
    )
    .execute(&p)
    .await
    .unwrap();
    assert!(sqlx::query("INSERT INTO game_character_transfer_info VALUES(2,''::bytea,3,3,''::bytea,2,4,''::bytea)").execute(&p).await.is_err());
    for value in [vec![1u8], vec![0u8; 102]] {
        assert!(
            sqlx::query("UPDATE game_character_transfer_info SET old_name_utf16=$1")
                .bind(value)
                .execute(&p)
                .await
                .is_err()
        );
    }
    sqlx::query("UPDATE game_character_transfer_info SET change_name_utf16=''::bytea")
        .execute(&p)
        .await
        .unwrap();
    postgres::check_ready(&p).await.unwrap();
    sqlx::query("ALTER TABLE game_character_transfer_info DROP COLUMN new_sn")
        .execute(&p)
        .await
        .unwrap();
    assert!(postgres::check_ready(&p).await.is_err());
    cleanup_game_pool(a, p, s).await;
}
