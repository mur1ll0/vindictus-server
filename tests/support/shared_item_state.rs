use super::{cleanup_game_pool, initial_inventory as owner, isolated_game_pool};
use serde_json::{Value, json};
use sqlx::Row;
use vindictus_server::{
    domain::legacy_user_data::SqlDateTime,
    infrastructure::shared_item_state::{self as shared, SharedError},
};
fn input() -> Value {
    let binary = SqlDateTime::from_units300(1)
        .unwrap()
        .to_sql_binary()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    json!({"format_version":1,"source":"study_heroes.shared-items","last_value":"7","items":[{"id":"9007199254740993","owner_uid":-17,"collection":255,"slot":-2147483648i64},{"id":"-1","owner_uid":null,"collection":null,"slot":null}],"storage":[{"id":"1","owner_uid":-17,"storage_type":255,"storage":20,"expire_binary":binary},{"id":"7","owner_uid":99,"storage_type":0,"storage":24,"expire_binary":null}]})
}
fn snapshot(v: &Value) -> shared::Snapshot {
    shared::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn shared_state_preserves_all_fields_nulls_identity_and_exact_replay() {
    let (a, p, s) = isolated_game_pool().await;
    let source = input();
    let snap = snapshot(&source);
    assert!(shared::preserve(&p, &snap, true).await.is_err());
    let out = shared::preserve(&p, &snap, false).await.unwrap();
    assert_eq!((out.items, out.storage), (2, 2));
    let row = sqlx::query("SELECT * FROM game_shared_items WHERE id=9007199254740993")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(row.get::<i64, _>("id"), 9007199254740993);
    assert_eq!(row.get::<Option<i32>, _>("owner_uid"), Some(-17));
    assert_eq!(row.get::<Option<i16>, _>("collection"), Some(255));
    assert_eq!(row.get::<Option<i32>, _>("slot"), Some(i32::MIN));
    let nulls: (Option<i32>, Option<i16>, Option<i32>) =
        sqlx::query_as("SELECT owner_uid,collection,slot FROM game_shared_items WHERE id=-1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(nulls, (None, None, None));
    let row:(i64,i32,i16,i16,Option<i64>)=sqlx::query_as("SELECT id,owner_uid,storage_type,storage,expire_units300 FROM game_shared_storage_status WHERE id=1").fetch_one(&p).await.unwrap();
    assert_eq!(row, (1, -17, 255, 20, Some(1)));
    let expiry: Option<i64> =
        sqlx::query_scalar("SELECT expire_units300 FROM game_shared_storage_status WHERE id=7")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(expiry, None);
    let n: i64 = sqlx::query_scalar("SELECT nextval('game_shared_storage_id')")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 8);
    shared::preserve(&p, &snap, false).await.unwrap();
    shared::preserve(&p, &snap, true).await.unwrap();
    let next: i64 = sqlx::query_scalar("SELECT nextval('game_shared_storage_id')")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(next, 9);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn shared_state_bootstrap_rolls_back_and_rejects_changed_source_or_target() {
    let (a, p, s) = isolated_game_pool().await;
    let source = input();
    let snap = snapshot(&source);
    sqlx::query(
        "ALTER TABLE game_shared_storage_status ADD CONSTRAINT fail_last CHECK(id<>7) NOT VALID",
    )
    .execute(&p)
    .await
    .unwrap();
    assert!(shared::preserve(&p, &snap, false).await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM game_shared_items")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("ALTER TABLE game_shared_storage_status DROP CONSTRAINT fail_last")
        .execute(&p)
        .await
        .unwrap();
    shared::preserve(&p, &snap, false).await.unwrap();
    let mut changed = source.clone();
    changed["items"][0]["slot"] = json!(0);
    assert!(matches!(
        shared::preserve(&p, &snapshot(&changed), false).await,
        Err(SharedError::Conflict)
    ));
    sqlx::query("UPDATE game_shared_items SET slot=0 WHERE id=-1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        shared::preserve(&p, &snap, true).await,
        Err(SharedError::Conflict)
    ));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn shared_empty_loader_requires_bootstrap_owned_revision_and_absent_items_and_tabs() {
    let (a, p, s) = isolated_game_pool().await;
    owner::setup(&p, true).await;
    assert!(shared::require_empty(&p, &owner::key()).await.is_err());
    shared::preserve(&p, &snapshot(&input()), false)
        .await
        .unwrap();
    shared::require_empty(&p, &owner::key()).await.unwrap();
    sqlx::query("INSERT INTO game_shared_items VALUES(1,31,NULL,NULL)")
        .execute(&p)
        .await
        .unwrap();
    assert!(shared::require_empty(&p, &owner::key()).await.is_err());
    sqlx::query("DELETE FROM game_shared_items WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_shared_storage_status VALUES(100,31,0,20,NULL)")
        .execute(&p)
        .await
        .unwrap();
    assert!(shared::require_empty(&p, &owner::key()).await.is_err());
    sqlx::query("DELETE FROM game_shared_storage_status WHERE id=100")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("UPDATE game_users SET service_id=service_id WHERE id=31")
        .execute(&p)
        .await
        .unwrap();
    assert!(shared::require_empty(&p, &owner::key()).await.is_err());
    cleanup_game_pool(a, p, s).await;
}
