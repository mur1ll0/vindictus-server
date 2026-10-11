use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
use vindictus_server::{
    domain::{
        character_data::QueuedItemSnapshot,
        dormant_item::DormantItemRequest,
        legacy_user_data::{LegacyBytes, SqlDateTime},
    },
    error::Error,
    infrastructure::{
        character_export::read_export,
        characters::{import_bundle, read_queued_items},
        dormant_items::*,
        postgres,
    },
};
fn text<const N: usize>(value: &str) -> LegacyBytes<N, true> {
    LegacyBytes::from_bytes(value.encode_utf16().flat_map(u16::to_le_bytes).collect()).unwrap()
}
fn request() -> DormantItemRequest {
    DormantItemRequest {
        cid: -1,
        item_class_ex: text("synthetic"),
        count: -5,
        is_binded: true,
        mail_title: None,
        mail_content: Some(text("")),
        log_date: SqlDateTime::from_units300(-1).unwrap(),
    }
}
fn source_bundle() -> vindictus_server::domain::character_data::CharacterBundle {
    read_export(include_bytes!("../fixtures/character-bundle.json").as_slice()).unwrap()
}
fn expected(row: &Value) -> QueuedItemSnapshot {
    let data = json!({"format_version":1,"source":"study_heroes.character-login-bundle","source_counts":{"characters":0,"character_info":0,"connections":0,"queued_items":1},"characters":[],"character_info":[],"connections":[],"queued_items":[row]});
    read_export(serde_json::to_vec(&data).unwrap().as_slice())
        .unwrap()
        .queued_items
        .remove(0)
}
fn from_snapshot(row: &QueuedItemSnapshot) -> DormantItemRequest {
    DormantItemRequest {
        cid: row.cid,
        item_class_ex: LegacyBytes::from_bytes(row.item_class_ex.as_bytes().to_vec()).unwrap(),
        count: row.count,
        is_binded: row.is_character_binded,
        mail_title: row
            .mail_title
            .as_ref()
            .map(|v| LegacyBytes::from_bytes(v.as_bytes().to_vec()).unwrap()),
        mail_content: row
            .mail_content
            .as_ref()
            .map(|v| LegacyBytes::from_bytes(v.as_bytes().to_vec()).unwrap()),
        log_date: row.log_date.unwrap(),
    }
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn dormant_item_matches_recovered_sql_all_18_fields_and_rollback_gap() {
    let (admin, pool, schema) = isolated_game_pool().await;
    initialize_identity(&pool, Some(140)).await.unwrap();
    let oracle: Value =
        serde_json::from_str(include_str!("../fixtures/sql-dormant-items.json")).unwrap();
    for row in oracle["rows"].as_array().unwrap() {
        let target = expected(row);
        let req = from_snapshot(&target);
        if target.row_id == 145 {
            let mut tx = pool.begin().await.unwrap();
            assert_eq!(send_dormant_item(&mut tx, &req).await.unwrap(), 144);
            tx.rollback().await.unwrap();
            assert!(read_queued_items(&pool, 144).await.unwrap().is_none());
        }
        let operation = Uuid::new_v4();
        let result = enqueue_once(&pool, operation, &req).await.unwrap();
        assert_eq!(result.row_id, target.row_id);
        assert!(!result.replayed);
        assert!(
            read_queued_items(&pool, target.row_id)
                .await
                .unwrap()
                .as_ref()
                == Some(&target)
        );
        assert!(enqueue_once(&pool, operation, &req).await.unwrap().replayed);
    }
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn queue_bootstrap_is_explicit_one_time_concurrent_and_never_uses_max_id() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let req = request();
    assert!(matches!(
        enqueue_once(&pool, Uuid::new_v4(), &req).await,
        Err(Error::QueueIdentityUninitialized)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_dormant_item_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let source = source_bundle();
    import_bundle(&pool, &source, false).await.unwrap();
    for invalid in [None, Some(1), Some(-1), Some(i32::MAX)] {
        assert!(matches!(
            initialize_identity(&pool, invalid).await,
            Err(Error::QueueIdentityProfile)
        ));
    }
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            initialize_identity(&pool, Some(200)).await.unwrap()
        }));
    }
    let mut initialized = 0;
    for task in tasks {
        initialized += usize::from(task.await.unwrap());
    }
    assert_eq!(initialized, 1);
    assert_eq!(
        enqueue_once(&pool, Uuid::new_v4(), &req)
            .await
            .unwrap()
            .row_id,
        201
    );
    assert!(!initialize_identity(&pool, Some(200)).await.unwrap());
    assert!(matches!(
        initialize_identity(&pool, Some(201)).await,
        Err(Error::QueueIdentityProfile)
    ));
    assert_eq!(
        enqueue_once(&pool, Uuid::new_v4(), &req)
            .await
            .unwrap()
            .row_id,
        202
    );
    assert_eq!(
        import_bundle(&pool, &source, false)
            .await
            .unwrap()
            .inserted
            .total(),
        0
    );
    assert_eq!(
        import_bundle(&pool, &source, true)
            .await
            .unwrap()
            .verified
            .total(),
        13
    );
    let mut new_source = source_bundle();
    new_source.queued_items[0].row_id = 203;
    assert!(matches!(
        import_bundle(&pool, &new_source, false).await,
        Err(Error::QueueImportAfterInitialization)
    ));
    assert!(read_queued_items(&pool, 203).await.unwrap().is_none());
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn queue_concurrency_has_one_receipt_winner_and_unique_allocations() {
    let (admin, pool, schema) = isolated_game_pool().await;
    initialize_identity(&pool, Some(200)).await.unwrap();
    let req = Arc::new(request());
    let operation = Uuid::new_v4();
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let pool = pool.clone();
        let req = Arc::clone(&req);
        tasks.push(tokio::spawn(async move {
            enqueue_once(&pool, operation, &req).await.unwrap()
        }));
    }
    let mut winners = 0;
    for task in tasks {
        let result = task.await.unwrap();
        assert_eq!(result.row_id, 201);
        winners += usize::from(!result.replayed);
    }
    assert_eq!(winners, 1);
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let pool = pool.clone();
        let req = Arc::clone(&req);
        tasks.push(tokio::spawn(async move {
            enqueue_once(&pool, Uuid::new_v4(), &req)
                .await
                .unwrap()
                .row_id
        }));
    }
    let mut rows = Vec::new();
    for task in tasks {
        rows.push(task.await.unwrap());
    }
    rows.sort_unstable();
    assert_eq!(rows, (202..=217).collect::<Vec<_>>());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_dormant_item_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        17
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn queue_replay_compares_every_parameter_and_survives_processing_and_removal() {
    let (admin, pool, schema) = isolated_game_pool().await;
    initialize_identity(&pool, None).await.unwrap();
    let req = request();
    let operation = Uuid::new_v4();
    assert_eq!(
        enqueue_once(&pool, operation, &req).await.unwrap().row_id,
        1
    );
    for field in 0..7 {
        let mut changed = request();
        match field {
            0 => changed.cid += 1,
            1 => changed.item_class_ex = text("different"),
            2 => changed.count += 1,
            3 => changed.is_binded = false,
            4 => changed.mail_title = Some(text("")),
            5 => changed.mail_content = None,
            _ => changed.log_date = SqlDateTime::from_units300(0).unwrap(),
        }
        assert!(matches!(
            enqueue_once(&pool, operation, &changed).await,
            Err(Error::QueueReplayConflict)
        ));
    }
    sqlx::query(
        "UPDATE game_queued_items SET processed=TRUE,processed_time_units300=0 WHERE row_id=1",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(enqueue_once(&pool, operation, &req).await.unwrap().replayed);
    sqlx::query("DELETE FROM game_queued_items WHERE row_id=1")
        .execute(&pool)
        .await
        .unwrap();
    assert!(enqueue_once(&pool, operation, &req).await.unwrap().replayed);
    assert!(read_queued_items(&pool, 1).await.unwrap().is_none());
    assert_eq!(
        enqueue_once(&pool, Uuid::new_v4(), &req)
            .await
            .unwrap()
            .row_id,
        2
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn queue_late_failure_rolls_back_row_and_receipt_but_consumes_identity() {
    let (admin, pool, schema) = isolated_game_pool().await;
    initialize_identity(&pool, Some(140)).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_receipt() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic failure'; END$$;CREATE TRIGGER fail_receipt BEFORE UPDATE ON game_dormant_item_runs FOR EACH ROW EXECUTE FUNCTION fail_receipt()").execute(&pool).await.unwrap();
    let req = request();
    let operation = Uuid::new_v4();
    assert!(enqueue_once(&pool, operation, &req).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_dormant_item_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(read_queued_items(&pool, 141).await.unwrap().is_none());
    sqlx::query("DROP TRIGGER fail_receipt ON game_dormant_item_runs")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        enqueue_once(&pool, operation, &req).await.unwrap().row_id,
        142
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn queue_identity_exhaustion_cannot_wrap_or_leave_an_empty_receipt() {
    let (admin, pool, schema) = isolated_game_pool().await;
    initialize_identity(&pool, Some(i32::MAX - 1))
        .await
        .unwrap();
    let req = request();
    let operation = Uuid::new_v4();
    assert_eq!(
        enqueue_once(&pool, operation, &req).await.unwrap().row_id,
        i32::MAX
    );
    assert!(enqueue_once(&pool, Uuid::new_v4(), &req).await.is_err());
    assert!(enqueue_once(&pool, operation, &req).await.unwrap().replayed);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_dormant_item_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn queue_readiness_checks_schema_counter_policy_and_singleton() {
    let (admin, pool, schema) = isolated_game_pool().await;
    postgres::check_ready(&pool).await.unwrap();
    sqlx::query("ALTER SEQUENCE game_queued_item_row_id CYCLE")
        .execute(&pool)
        .await
        .unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    sqlx::query("ALTER SEQUENCE game_queued_item_row_id NO CYCLE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE game_dormant_item_runs RENAME COLUMN request TO absent")
        .execute(&pool)
        .await
        .unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    sqlx::query("ALTER TABLE game_dormant_item_runs RENAME COLUMN absent TO request")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_queue_identity")
        .execute(&pool)
        .await
        .unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    cleanup_game_pool(admin, pool, schema).await;
}
