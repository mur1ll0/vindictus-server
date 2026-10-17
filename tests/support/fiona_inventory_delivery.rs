type StoredLog = (i64, i16, i16, Option<i32>, Option<i32>);
type StoredLedger = (
    i64,
    i64,
    i32,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);
use super::{cleanup_game_pool, initial_inventory as fixture, isolated_game_pool};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use vindictus_server::{
    application::{
        character_creation::EntityAuthority,
        inventory_delivery::{self as prep, DeliveryClock, Inputs, Journals, Prepared},
    },
    domain::{
        initial_inventory::{InitialRequest, InventoryError},
        item_colors::{ColorRandom, Materials},
        item_stats::ItemStat,
        legacy_user_data::SqlDateTime2,
    },
    infrastructure::{
        fiona_character_state as state,
        fiona_initializer_profile::Profile,
        fiona_inventory_delivery::{self as delivery, DeliveryError},
        item_journals,
    },
};
mod profile_fixture {
    include!("../fixtures/initializer_profile_support.rs");
}
fn snapshot() -> Value {
    json!({"format_version":1,"source":"study_heroes.character-initialization","titles":[],"goals":[],"presets":[{"title_id":17,"class_restriction":2},{"title_id":18,"class_restriction":0}],"micro":[],"missions":[],"assignments":[]})
}
fn profile(shared: bool) -> Profile {
    let mut v = profile_fixture::synthetic();
    v["state_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&snapshot()).unwrap())
    ));
    if shared {
        for f in v["features"].as_array_mut().unwrap() {
            if f["key"] == "SharedInventory" {
                f["value"] = json!("synthetic-on");
            }
        }
    }
    profile_fixture::read(&v).unwrap()
}
async fn setup(p: &PgPool) {
    fixture::setup(p, true).await;
    state::preserve(
        p,
        &state::read_snapshot(serde_json::to_vec(&snapshot()).unwrap().as_slice()).unwrap(),
        false,
    )
    .await
    .unwrap();
    item_journals::preserve(p,&item_journals::read_snapshot(br#"{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":null,"character_last":null}"#.as_slice()).unwrap(),false).await.unwrap();
}
fn prepared(j: Journals) -> Result<Prepared, DeliveryError> {
    let keys = ["synthetic_eq", "synthetic_stack", "synthetic_cash"].map(|s| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    });
    let stat = ItemStat::from_raw(&[], &[], [50, 0, 0, 0]).unwrap();
    let requests = keys
        .iter()
        .enumerate()
        .map(|(i, k)| InitialRequest {
            class: k,
            count: [1, 5, 2][i],
            max_stack: [1, 3, 99][i],
            unique: 0,
            storage: if i == 2 { 2 } else { 1 },
            expire_in: None,
            equipment: i == 0,
            base: &stat,
            materials: [None; 3],
            gold: false,
        })
        .collect::<Vec<_>>();
    Ok(prep::prepare(
        Inputs {
            requests: &requests,
            materials: &Materials::default(),
            maximum_durability: 100,
            bundle_exceed: 0,
        },
        1,
        j,
        &mut ColorRandom::from_seed(1),
        &mut DeliveryClock {
            authority: &mut EntityAuthority::new(17),
            utc: &mut || Ok(SqlDateTime2::from_ticks(638448480000000000).unwrap()),
            civil: &mut || Ok(SqlDateTime2::from_ticks(638000000000017001).unwrap()),
        },
    )?)
}
async fn count(p: &PgPool, f: &str) -> i64 {
    let q = match f {
        "items" => "SELECT count(*) FROM game_items",
        "logs" => {
            "SELECT (SELECT count(*)FROM game_item_log01)+(SELECT count(*)FROM game_item_log02)"
        }
        "ledgers" => {
            "SELECT (SELECT count(*)FROM game_item_ledger01)+(SELECT count(*)FROM game_item_ledger02)"
        }
        _ => {
            "SELECT (SELECT count(*)FROM game_item_journal01)+(SELECT count(*)FROM game_item_journal02)"
        }
    };
    sqlx::query_scalar(q).fetch_one(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_commits_before_logs_and_captures_original_requests_and_last_counts() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::query("CREATE FUNCTION delivery_saved_first()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF (SELECT count(*) FROM game_items)<>4 THEN RAISE EXCEPTION 'inventory not committed';END IF;RETURN NEW;END$$").execute(&p).await.unwrap();
    for t in [
        "game_item_log01",
        "game_item_log02",
        "game_item_ledger01",
        "game_item_ledger02",
    ] {
        sqlx::query(&format!("CREATE TRIGGER delivery_saved_first BEFORE INSERT ON {t} FOR EACH ROW EXECUTE FUNCTION delivery_saved_first()")).execute(&p).await.unwrap();
    }
    let o = delivery::deliver(&p, &fixture::key(), 1, &profile(false), prepared)
        .await
        .unwrap();
    assert_eq!(o.prepared.inventory().items().len(), 4);
    assert_eq!(
        (o.item_logs.written, o.item_logs.failed, o.ledgers.written),
        (4, 0, 3)
    );
    assert_eq!(count(&p, "items").await, 4);
    let rows:Vec<StoredLog>=sqlx::query_as("SELECT journal_id,event,detail,after_inventory,after_shared FROM game_item_log01 UNION ALL SELECT journal_id,event,detail,after_inventory,after_shared FROM game_item_log02").fetch_all(&p).await.unwrap();
    assert_eq!(rows.iter().filter(|r| r.3 == Some(5)).count(), 1);
    assert!(
        rows.iter()
            .all(|r| r.0 == 611635750 && r.1 == 19 && r.2 == 1 && r.4.is_none())
    );
    let rows:Vec<StoredLedger>=sqlx::query_as("SELECT journal_id,item_id,count,color1,color2,color3,bundle_item_count FROM game_item_ledger01 UNION ALL SELECT journal_id,item_id,count,color1,color2,color3,bundle_item_count FROM game_item_ledger02").fetch_all(&p).await.unwrap();
    assert!(rows.iter().all(|r| r.0 == 4419625
        && r.1 == -1
        && r.3 == Some(-1)
        && r.4 == Some(-1)
        && r.5 == Some(-1)));
    assert!(rows.iter().any(|r| r.2 == 1 && r.6.is_none()));
    assert!(rows.iter().any(|r| r.2 == 5 && r.6 == Some(5)));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_save_and_prepare_failures_do_not_flush_item_events() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(false), |_| Err(
            InventoryError::Clock.into()
        ))
        .await,
        Err(DeliveryError::Preparation(_))
    ));
    assert_eq!(count(&p, "items").await, 0);
    assert_eq!(count(&p, "logs").await, 0);
    assert_eq!(count(&p, "ledgers").await, 1);
    sqlx::query("ALTER TABLE game_items ADD CONSTRAINT reject_delivery CHECK(false) NOT VALID")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(false), prepared).await,
        Err(DeliveryError::Storage(_))
    ));
    assert_eq!(count(&p, "items").await, 0);
    assert_eq!(count(&p, "logs").await, 0);
    assert_eq!(count(&p, "ledgers").await, 2);
    let rows:Vec<Vec<u8>>=sqlx::query_scalar("SELECT item_class_utf16 FROM game_item_ledger01 UNION ALL SELECT item_class_utf16 FROM game_item_ledger02").fetch_all(&p).await.unwrap();
    for msg in ["giveDefaultItemFail", "giveDefaultItemFail(save)"] {
        assert!(rows.contains(&msg.encode_utf16().flat_map(u16::to_le_bytes).collect()));
    }
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_logger_errors_do_not_undo_inventory_and_replay_requires_reconciliation() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    for t in [
        "game_item_log01",
        "game_item_log02",
        "game_item_ledger01",
        "game_item_ledger02",
    ] {
        sqlx::query(&format!(
            "ALTER TABLE {t} ADD CONSTRAINT reject_event CHECK(false) NOT VALID"
        ))
        .execute(&p)
        .await
        .unwrap();
    }
    let o = delivery::deliver(&p, &fixture::key(), 1, &profile(false), prepared)
        .await
        .unwrap();
    assert_eq!((o.item_logs.failed, o.ledgers.failed), (4, 3));
    assert_eq!(count(&p, "items").await, 4);
    assert_eq!(count(&p, "logs").await, 0);
    assert_eq!(count(&p, "ledgers").await, 0);
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(false), prepared).await,
        Err(DeliveryError::Reconcile)
    ));
    assert_eq!(count(&p, "items").await, 4);
    assert_eq!(count(&p, "logs").await, 0);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_rejects_unloaded_shared_state_and_stale_owner_before_side_effects() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(true), |_| panic!(
            "shared state not loaded"
        ))
        .await,
        Err(DeliveryError::SharedState)
    ));
    assert_eq!(count(&p, "journals").await, 0);
    sqlx::query("UPDATE game_users SET service_id=service_id WHERE id=31")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(false), |_| panic!(
            "stale owner"
        ))
        .await,
        Err(DeliveryError::Admission(_))
    ));
    assert_eq!(count(&p, "journals").await, 0);
    assert_eq!(count(&p, "items").await, 0);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_concurrent_attempts_never_duplicate_inventory_or_event_rows() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let profile = profile(false);
    let key = fixture::key();
    let (x, y) = tokio::join!(
        delivery::deliver(&p, &key, 1, &profile, prepared),
        delivery::deliver(&p, &key, 1, &profile, prepared)
    );
    assert!(matches!(
        (&x, &y),
        (Ok(_), Err(DeliveryError::Reconcile)) | (Err(DeliveryError::Reconcile), Ok(_))
    ));
    assert_eq!(count(&p, "items").await, 4);
    assert_eq!(count(&p, "logs").await, 4);
    assert_eq!(count(&p, "ledgers").await, 3);
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_loaded_empty_shared_inventory_records_zero_for_the_last_stack() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    use vindictus_server::infrastructure::shared_item_state as shared;
    let raw=br#"{"format_version":1,"source":"study_heroes.shared-items","last_value":null,"items":[],"storage":[]}"#;
    shared::preserve(&p, &shared::read_snapshot(raw.as_slice()).unwrap(), false)
        .await
        .unwrap();
    let o = delivery::deliver(&p, &fixture::key(), 1, &profile(true), prepared)
        .await
        .unwrap();
    assert_eq!((o.item_logs.written, o.ledgers.written), (4, 3));
    let shared:Vec<Option<i32>>=sqlx::query_scalar("SELECT after_shared FROM game_item_log01 UNION ALL SELECT after_shared FROM game_item_log02").fetch_all(&p).await.unwrap();
    assert_eq!(shared.iter().filter(|v| **v == Some(0)).count(), 1);
    assert_eq!(shared.iter().filter(|v| v.is_none()).count(), 3);
    cleanup_game_pool(a, p, s).await;
}
