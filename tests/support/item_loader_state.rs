use super::{cleanup_game_pool, initial_inventory as initial, isolated_game_pool};
use serde_json::json;
use vindictus_server::infrastructure::item_loader_state::{self as loader, LoaderError};
mod fixture {
    include!("../fixtures/item_loader_support.rs");
}
fn snapshot(v: &serde_json::Value) -> loader::Snapshot {
    loader::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
async fn sequences(p: &sqlx::PgPool) -> Vec<(i64, bool)> {
    let mut out = vec![];
    for n in [
        "quick_slot",
        "expanded_slot",
        "manufacture",
        "manufacture_recipe",
        "new_recipe",
    ] {
        out.push(
            sqlx::query_as(&format!("SELECT last_value,is_called FROM game_{n}_id"))
                .fetch_one(p)
                .await
                .unwrap(),
        )
    }
    out
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_loader_preserves_all_twenty_fields_raw_utf16_identities_replay_and_no_repair() {
    let (a, p, s) = isolated_game_pool().await;
    initial::setup(&p, true).await;
    let v = fixture::snapshot();
    let snap = snapshot(&v);
    let out = loader::preserve(&p, &snap, false).await.unwrap();
    assert_eq!(out.rows, [1; 5]);
    assert_eq!(out.fields, [4, 3, 5, 4, 4]);
    assert_eq!(out.inserted, 5);
    assert_eq!(
        sequences(&p).await,
        vec![
            (12, false),
            (13, false),
            (14, false),
            (15, false),
            (16, false)
        ]
    );
    for mode in [false, true] {
        let out = loader::preserve(&p, &snap, mode).await.unwrap();
        assert_eq!(out.inserted, 0);
        assert_eq!(out.verified, 5)
    }
    let bytes: Vec<u8> = sqlx::query_scalar("SELECT item_class_utf16 FROM game_quick_slots")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(bytes, vec![65, 0, 0, 216, 32, 0]);
    let values:(i32,i32,i64,i64)=sqlx::query_as("SELECT (SELECT grade FROM game_manufacture),(SELECT experience_point FROM game_manufacture),(SELECT acquired_ticks FROM game_manufacture_recipes),(SELECT new_until_ticks FROM game_new_recipes)").fetch_one(&p).await.unwrap();
    assert_eq!(values, (-1, i32::MAX, 3155378975999999999, 0));
    let before = sequences(&p).await;
    sqlx::query("UPDATE game_manufacture SET experience_point=0")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        loader::preserve(&p, &snap, false).await,
        Err(LoaderError::Conflict)
    ));
    assert_eq!(sequences(&p).await, before);
    let xp: i32 = sqlx::query_scalar("SELECT experience_point FROM game_manufacture")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(xp, 0);
    loader::check_ready(&p).await.unwrap();
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_loader_late_foreign_key_failure_rolls_back_all_rows_markers_and_identity_restarts() {
    let (a, p, s) = isolated_game_pool().await;
    initial::setup(&p, true).await;
    let mut v = fixture::snapshot();
    v["new_recipes"][0]["cid"] = json!("999");
    let before = sequences(&p).await;
    assert!(loader::preserve(&p, &snapshot(&v), false).await.is_err());
    assert_eq!(sequences(&p).await, before);
    let n:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM game_quick_slots)+(SELECT count(*) FROM game_expanded_slots)+(SELECT count(*) FROM game_manufacture)+(SELECT count(*) FROM game_manufacture_recipes)+(SELECT count(*) FROM game_new_recipes)+(SELECT count(*) FROM game_item_loader_preservation)").fetch_one(&p).await.unwrap();
    assert_eq!(n, 0);
    sqlx::query("ALTER SEQUENCE game_quick_slot_id RESTART WITH 99")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        loader::preserve(&p, &snapshot(&fixture::snapshot()), false).await,
        Err(LoaderError::Conflict)
    ));
    let seq = sequences(&p).await;
    assert_eq!(seq[0], (99, false));
    sqlx::query("ALTER SEQUENCE game_quick_slot_id RESTART WITH 1")
        .execute(&p)
        .await
        .unwrap();
    loader::preserve(&p, &snapshot(&fixture::snapshot()), false)
        .await
        .unwrap();
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_loader_absence_requires_initialized_providers_and_retained_share_lock() {
    let (a, p, s) = isolated_game_pool().await;
    initial::setup(&p, true).await;
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        loader::require_slots_absent(&mut tx, 1).await,
        Err(LoaderError::Uninitialized)
    ));
    tx.rollback().await.unwrap();
    loader::preserve(&p, &snapshot(&fixture::empty()), false)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    loader::require_slots_absent(&mut tx, 1).await.unwrap();
    loader::require_manufacture_absent(&mut tx, 1)
        .await
        .unwrap();
    let mut other = p.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout='100ms'")
        .execute(&mut *other)
        .await
        .unwrap();
    assert!(sqlx::query("INSERT INTO game_manufacture(cid,manufacture_utf16,grade,experience_point)VALUES(1,'\\x4100',0,0)").execute(&mut *other).await.is_err());
    other.rollback().await.unwrap();
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO game_expanded_slots(cid,slot)VALUES(1,99)")
        .execute(&p)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        loader::require_slots_absent(&mut tx, 1).await,
        Err(LoaderError::Nonempty)
    ));
    loader::require_manufacture_absent(&mut tx, 1)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(
        loader::preserve(&p, &snapshot(&fixture::empty()), true).await,
        Err(LoaderError::Conflict)
    ));
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_loader_schema_retains_source_foreign_keys_and_utf16_datetime2_bounds() {
    let (a, p, s) = isolated_game_pool().await;
    initial::setup(&p, true).await;
    let snap = snapshot(&fixture::snapshot());
    loader::preserve(&p, &snap, false).await.unwrap();
    for q in [
        "INSERT INTO game_quick_slots(owner_id,item_class_utf16,slot)VALUES(999,'',0)",
        "INSERT INTO game_quick_slots(owner_id,item_class_utf16,slot)VALUES(1,decode('41','hex'),0)",
        "UPDATE game_manufacture_recipes SET acquired_ticks=-1",
        "UPDATE game_new_recipes SET new_until_ticks=3155378976000000000",
    ] {
        assert!(sqlx::query(q).execute(&p).await.is_err())
    }
    // Manufacture/recipes/expanded slots have no source FK and may contain orphan CID.
    sqlx::query("UPDATE game_manufacture SET cid=999")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("UPDATE game_expanded_slots SET cid=999")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_item_collections WHERE owner_id=1")
        .execute(&p)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_new_recipes")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 0);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_quick_slots")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 1);
    sqlx::query("DELETE FROM game_characters WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_quick_slots")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 0);
    cleanup_game_pool(a, p, s).await;
}
