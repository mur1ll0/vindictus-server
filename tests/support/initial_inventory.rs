use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use serde_json::json;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use vindictus_server::{
    application::game_users::provision_game_user,
    domain::{
        initial_inventory::{
            self, InitialRequest, InventoryClock, InventoryError, PreparedInventory,
        },
        item_colors::{ColorRandom, Materials},
        item_stats::ItemStat,
        item_time::ItemTimeError,
        legacy_user_data::SqlDateTime2,
    },
    infrastructure::{
        character_creation_core::{self as core, CreationCoreRequest},
        character_lobby::UserKey,
        initial_inventory::{self as db, InventoryWriteError as E},
        item_state,
    },
};
fn key() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
struct Clock {
    i: i64,
}
impl InventoryClock for Clock {
    fn allocate(&mut self) -> Result<i64, InventoryError> {
        self.i += 65536;
        Ok(self.i)
    }
    fn utc(&mut self) -> Result<SqlDateTime2, ItemTimeError> {
        SqlDateTime2::from_ticks(638448480000000000).map_err(|_| ItemTimeError::Clock)
    }
}
fn batch(owner: i64) -> PreparedInventory {
    let stat = ItemStat::from_raw(&[], &[], [200, 0, 0, 0]).unwrap();
    let keys = ["synthetic_bag_a", "synthetic_bag_b", "synthetic_cash"].map(|s| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    });
    let requests = keys
        .iter()
        .enumerate()
        .map(|(i, k)| InitialRequest {
            class: k,
            count: if i == 2 { 3 } else { 1 },
            max_stack: if i == 2 { 99 } else { 1 },
            unique: 0,
            storage: if i == 2 { 2 } else { 1 },
            expire_in: Some(3600),
            equipment: i < 2,
            base: &stat,
            materials: [None; 3],
            gold: i == 2,
        })
        .collect::<Vec<_>>();
    initial_inventory::compose(
        &requests,
        owner,
        &Materials::default(),
        &mut ColorRandom::from_seed(17),
        100,
        0,
        &mut Clock {
            i: 638448480000000000,
        },
    )
    .unwrap()
}
async fn setup(p: &PgPool, initialize: bool) {
    provision_game_user(p, 31, "Owner", 900, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint)SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(p).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes)VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(p)
        .await
        .unwrap();
    core::create_base(
        p,
        key(),
        &common::proof(&common::content()),
        CreationCoreRequest {
            cid: 1,
            character_sn: 12,
            name: "Murael",
            class: 1,
            use_preset: false,
        },
    )
    .await
    .unwrap();
    if initialize {
        ready(p).await;
    }
}
async fn ready(p: &PgPool) {
    let v = json!({"format_version":1,"source":"study_heroes.item-state","attribute_last_value":null,"storage_last_value":null,"items":[],"attributes":[],"equipment":[],"storage":[]});
    let s = item_state::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).unwrap();
    item_state::preserve(p, &s, false).await.unwrap();
}
async fn counts(p: &PgPool) -> (i64, i64) {
    sqlx::query_as("SELECT(SELECT count(*)FROM game_items),(SELECT count(*)FROM game_equippables)")
        .fetch_one(p)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn initial_inventory_concurrent_commit_replay_preserves_all_fields_and_no_identity_advance() {
    let (a, p, schema) = isolated_game_pool().await;
    setup(&p, true).await;
    let b = Arc::new(batch(1));
    let before: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_item_attribute_id")
            .fetch_one(&p)
            .await
            .unwrap();
    let mut jobs = tokio::task::JoinSet::new();
    for _ in 0..2 {
        let p = p.clone();
        let b = Arc::clone(&b);
        jobs.spawn(async move { db::save(&p, &key(), &b).await.unwrap() });
    }
    let mut inserted = 0;
    while let Some(r) = jobs.join_next().await {
        inserted += r.unwrap();
    }
    assert_eq!(inserted, 3);
    assert_eq!(counts(&p).await, (3, 2));
    for r in b.items() {
        let actual:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('id',i.id,'class',encode(i.item_class_utf16,'hex'),'owner',i.owner_id,'count',i.count,'collection',i.collection,'slot',i.slot,'expire',i.expire_ticks,'tradable',i.tradable,'checksum',i.checksum,'part',e.part,'color1',e.color1,'color2',e.color2,'color3',e.color3,'reduced',e.reduced_durability,'bonus',e.max_durability_bonus)FROM game_items i LEFT JOIN game_equippables e ON e.id=i.id WHERE i.id=$1").bind(r.id()).fetch_one(&p).await.unwrap();
        let hex = r
            .class()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let e = r.equipment();
        assert_eq!(
            actual,
            json!({"id":r.id(),"class":hex,"owner":1,"count":r.count(),"collection":r.location().collection,"slot":r.location().slot,"expire":r.expiry(),"tradable":true,"checksum":r.checksum(),"part":null,"color1":e.map(|e|e.colors[0]),"color2":e.map(|e|e.colors[1]),"color3":e.map(|e|e.colors[2]),"reduced":e.map(|e|e.reduced),"bonus":e.map(|e|e.bonus)})
        );
    }
    let after: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_item_attribute_id")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(before, after);
    sqlx::query("UPDATE game_items SET count=count+1 WHERE id=$1")
        .bind(b.items()[0].id())
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(db::save(&p, &key(), &b).await, Err(E::Conflict)));
    assert_eq!(counts(&p).await, (3, 2));
    cleanup_game_pool(a, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn initial_inventory_refuses_uninitialized_wrong_owner_revision_origin_class_and_orphan_children()
 {
    let (a, p, schema) = isolated_game_pool().await;
    setup(&p, false).await;
    let b = batch(1);
    assert!(matches!(
        db::save(&p, &key(), &b).await,
        Err(E::Uninitialized)
    ));
    ready(&p).await;
    for k in [
        UserKey {
            uid: 31,
            name: "Wrong",
            revision: 0,
        },
        UserKey {
            uid: 31,
            name: "Owner",
            revision: 1,
        },
        UserKey {
            uid: 32,
            name: "Owner",
            revision: 0,
        },
    ] {
        assert!(matches!(db::save(&p, &k, &b).await, Err(E::Ownership)));
    }
    assert!(matches!(
        db::save(&p, &key(), &batch(2)).await,
        Err(E::Ownership)
    ));
    sqlx::query("UPDATE game_character_info SET class=2 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(db::save(&p, &key(), &b).await, Err(E::Ownership)));
    sqlx::query("UPDATE game_character_info SET class=1 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_lab_account_origins WHERE uid=31")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(db::save(&p, &key(), &b).await, Err(E::Ownership)));
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint)SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(&p).await.unwrap();
    sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2)VALUES(17,$1,$2,$3,0,0)").bind(b.items()[0].id()).bind(b"SYNTHETIC".as_slice()).bind(b"value".as_slice()).execute(&p).await.unwrap();
    assert!(matches!(db::save(&p, &key(), &b).await, Err(E::Conflict)));
    assert_eq!(counts(&p).await, (0, 0));
    cleanup_game_pool(a, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn initial_inventory_late_failure_and_cancellation_rollback_complete_item_equipment_batch() {
    let (a, p, schema) = isolated_game_pool().await;
    setup(&p, true).await;
    let b = Arc::new(batch(1));
    sqlx::raw_sql("CREATE FUNCTION fail_initial_inventory()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF NEW.collection=0 AND NEW.slot=1 THEN RAISE EXCEPTION 'synthetic late failure';END IF;RETURN NEW;END$$;CREATE TRIGGER fail_initial_inventory BEFORE INSERT ON game_items FOR EACH ROW EXECUTE FUNCTION fail_initial_inventory()").execute(&p).await.unwrap();
    assert!(matches!(
        db::save(&p, &key(), &b).await,
        Err(E::Database(_))
    ));
    assert_eq!(counts(&p).await, (0, 0));
    sqlx::raw_sql("DROP TRIGGER fail_initial_inventory ON game_items;DROP FUNCTION fail_initial_inventory();CREATE FUNCTION wait_initial_inventory()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF NEW.collection=0 AND NEW.slot=1 THEN PERFORM pg_advisory_xact_lock(hashtextextended(current_schema(),71923));PERFORM pg_sleep(10);END IF;RETURN NEW;END$$;CREATE TRIGGER wait_initial_inventory BEFORE INSERT ON game_items FOR EACH ROW EXECUTE FUNCTION wait_initial_inventory()").execute(&p).await.unwrap();
    let target = p.clone();
    let copy = Arc::clone(&b);
    let task = tokio::spawn(async move { db::save(&target, &key(), &copy).await });
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let busy: bool = sqlx::query_scalar(
                "SELECT NOT pg_try_advisory_xact_lock(hashtextextended(current_schema(),71923))",
            )
            .fetch_one(&p)
            .await
            .unwrap();
            if busy {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let free: bool = sqlx::query_scalar(
                "SELECT pg_try_advisory_xact_lock(hashtextextended(current_schema(),71923))",
            )
            .fetch_one(&p)
            .await
            .unwrap();
            if free {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(counts(&p).await, (0, 0));
    sqlx::raw_sql(
        "DROP TRIGGER wait_initial_inventory ON game_items;DROP FUNCTION wait_initial_inventory()",
    )
    .execute(&p)
    .await
    .unwrap();
    assert_eq!(db::save(&p, &key(), &b).await.unwrap(), 3);
    cleanup_game_pool(a, p, schema).await;
}
