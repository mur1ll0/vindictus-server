use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
mod fixtures {
    include!("../fixtures/skills_support.rs");
}
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use vindictus_server::{
    application::game_users::provision_game_user,
    infrastructure::{
        character_creation_core::{self as core, CreationCoreRequest},
        character_lobby::UserKey,
        character_skills::{self as db, SkillError as E},
    },
};
fn snapshot(v: &Value) -> db::Snapshot {
    db::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
fn key() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
async fn counts(p: &PgPool) -> (i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM game_skills),(SELECT COUNT(*) FROM game_sp_skills)",
    )
    .fetch_one(p)
    .await
    .unwrap()
}
async fn setup(p: &PgPool) {
    provision_game_user(p, 31, "Owner", 900, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(p).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes) VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(p)
        .await
        .unwrap();
    let proof = common::proof(&common::content());
    core::create_base(
        p,
        key(),
        &proof,
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
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn skill_state_preserves_every_field_duplicates_counter_replay_and_source_cascades() {
    let (admin, p, schema) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_characters VALUES(1)")
        .execute(&p)
        .await
        .unwrap();
    let s = snapshot(&fixtures::export());
    let (a, b) = tokio::join!(db::preserve(&p, &s, false), db::preserve(&p, &s, false));
    assert_eq!(a.unwrap().inserted + b.unwrap().inserted, 4);
    assert_eq!(counts(&p).await, (2, 2));
    let rows: Vec<(i64, i64, Vec<u8>, i32, i16, i32)> = sqlx::query_as(
        "SELECT row_id,cid,skill_utf16,rank,locked,current_ap FROM game_skills ORDER BY row_id",
    )
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (i64::MIN, 1, vec![0, 216], i32::MIN, 255, i32::MAX),
            (9007199254740993, 1, vec![0, 216], i32::MAX, 0, i32::MIN)
        ]
    );
    let slots: Vec<(i32, i64, i32, Option<Vec<u8>>)> =
        sqlx::query_as("SELECT row_id,cid,slot_id,skill_utf16 FROM game_sp_skills ORDER BY row_id")
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(slots, vec![(-1, 1, 0, None), (7, 1, 0, Some(vec![]))]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT nextval('game_sp_skill_row_id')")
            .fetch_one(&p)
            .await
            .unwrap(),
        11
    );
    assert_eq!(db::preserve(&p, &s, false).await.unwrap().inserted, 0);
    assert_eq!(db::preserve(&p, &s, true).await.unwrap().verified, 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT nextval('game_sp_skill_row_id')")
            .fetch_one(&p)
            .await
            .unwrap(),
        12
    );
    let absent: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('game_skill_row_id')::text")
            .fetch_one(&p)
            .await
            .unwrap();
    assert!(absent.is_none());
    sqlx::query("UPDATE game_characters SET id=2 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_skills WHERE cid=2")
            .fetch_one(&p)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_sp_skills WHERE cid=2")
            .fetch_one(&p)
            .await
            .unwrap(),
        2
    );
    sqlx::query("DELETE FROM game_characters WHERE id=2")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(counts(&p).await, (0, 0));
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn skill_state_late_foreign_key_failure_rolls_back_bootstrap_and_never_repairs_conflicts() {
    let (admin, p, schema) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_characters VALUES(1)")
        .execute(&p)
        .await
        .unwrap();
    let mut bad = fixtures::export();
    bad["sp_slots"][1]["cid"] = json!("99");
    assert!(db::preserve(&p, &snapshot(&bad), false).await.is_err());
    assert_eq!(counts(&p).await, (0, 0));
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT initialized FROM game_skill_identity")
            .fetch_one(&p)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, bool)>("SELECT last_value,is_called FROM game_sp_skill_row_id")
            .fetch_one(&p)
            .await
            .unwrap(),
        (1, false)
    );
    let good = snapshot(&fixtures::export());
    assert_eq!(db::preserve(&p, &good, false).await.unwrap().inserted, 4);
    let mut changed = fixtures::export();
    changed["skills"][0]["current_ap"] = json!(42);
    assert!(matches!(
        db::preserve(&p, &snapshot(&changed), false).await,
        Err(E::Conflict)
    ));
    assert_eq!(db::preserve(&p, &good, true).await.unwrap().verified, 4);
    let mut changed = fixtures::export();
    changed["sp_source_last_value"] = json!("11");
    assert!(matches!(
        db::preserve(&p, &snapshot(&changed), false).await,
        Err(E::Conflict)
    ));
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn skill_state_cancellation_rolls_back_both_tables_and_identity_before_retry() {
    let (admin, p, schema) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_characters VALUES(1)")
        .execute(&p)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION delay_skill_import() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_advisory_xact_lock(hashtext(current_schema()),74919);PERFORM pg_sleep(20);RETURN NEW;END$$;CREATE TRIGGER delay_skill_import BEFORE INSERT ON game_sp_skills FOR EACH ROW WHEN(NEW.row_id=7) EXECUTE FUNCTION delay_skill_import()").execute(&p).await.unwrap();
    let s = Arc::new(snapshot(&fixtures::export()));
    let pp = p.clone();
    let ss = s.clone();
    let task = tokio::spawn(async move { db::preserve(&pp, &ss, false).await });
    tokio::time::timeout(Duration::from_secs(10),async{loop{if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=74919 AND granted)").fetch_one(&p).await.unwrap(){break}tokio::time::sleep(Duration::from_millis(25)).await}}).await.unwrap();
    task.abort();
    match task.await {
        Err(e) => assert!(e.is_cancelled()),
        Ok(_) => panic!("import completed before cancellation"),
    };
    assert_eq!(counts(&p).await, (0, 0));
    tokio::time::timeout(Duration::from_secs(10),async{loop{if !sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=74919 AND granted)").fetch_one(&p).await.unwrap(){break}tokio::time::sleep(Duration::from_millis(25)).await}}).await.unwrap();
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT initialized FROM game_skill_identity")
            .fetch_one(&p)
            .await
            .unwrap()
    );
    sqlx::raw_sql(
        "DROP TRIGGER delay_skill_import ON game_sp_skills;DROP FUNCTION delay_skill_import()",
    )
    .execute(&p)
    .await
    .unwrap();
    assert_eq!(db::preserve(&p, &s, false).await.unwrap().inserted, 4);
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn initial_sp_slot_requires_bootstrap_owner_revision_origin_and_normal_fiona_then_replays_without_ids()
 {
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    assert!(matches!(
        db::initialize_sp_slot(&p, &key(), 1).await,
        Err(E::Uninitialized)
    ));
    db::preserve(&p, &snapshot(&fixtures::empty()), false)
        .await
        .unwrap();
    for k in [
        UserKey { uid: 32, ..key() },
        UserKey {
            name: "Other",
            ..key()
        },
        UserKey {
            revision: 1,
            ..key()
        },
    ] {
        assert!(matches!(
            db::initialize_sp_slot(&p, &k, 1).await,
            Err(E::Ownership)
        ));
    }
    assert!(matches!(
        db::initialize_sp_slot(&p, &key(), 99).await,
        Err(E::Ownership)
    ));
    sqlx::query("DELETE FROM game_lab_account_origins WHERE uid=31")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::initialize_sp_slot(&p, &key(), 1).await,
        Err(E::Ownership)
    ));
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(&p).await.unwrap();
    for change in ["class=10", "level=2", "use_preset=true"] {
        sqlx::query(&format!(
            "UPDATE game_character_info SET {change} WHERE id=1"
        ))
        .execute(&p)
        .await
        .unwrap();
        assert!(matches!(
            db::initialize_sp_slot(&p, &key(), 1).await,
            Err(E::Ownership)
        ));
        sqlx::query("UPDATE game_character_info SET class=1,level=1,use_preset=false WHERE id=1")
            .execute(&p)
            .await
            .unwrap();
    }
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let pp = p.clone();
        tasks.push(tokio::spawn(async move {
            db::initialize_sp_slot(&pp, &key(), 1).await.unwrap()
        }));
    }
    let mut inserted = 0;
    for task in tasks {
        inserted += task.await.unwrap();
    }
    assert_eq!(inserted, 1);
    assert_eq!(counts(&p).await, (0, 1));
    assert_eq!(
        sqlx::query_as::<_, (i32, Option<Vec<u8>>)>(
            "SELECT slot_id,skill_utf16 FROM game_sp_skills WHERE cid=1"
        )
        .fetch_one(&p)
        .await
        .unwrap(),
        (0, None)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT nextval('game_sp_skill_row_id')")
            .fetch_one(&p)
            .await
            .unwrap(),
        2
    );
    assert_eq!(db::initialize_sp_slot(&p, &key(), 1).await.unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT nextval('game_sp_skill_row_id')")
            .fetch_one(&p)
            .await
            .unwrap(),
        3
    );
    sqlx::query("UPDATE game_sp_skills SET skill_utf16=''::bytea WHERE cid=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::initialize_sp_slot(&p, &key(), 1).await,
        Err(E::Conflict)
    ));
    assert_eq!(counts(&p).await, (0, 1));
    cleanup_game_pool(admin, p, schema).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn base_stats_write_all_seven_fields_guard_ownership_and_replay_without_repair() {
    use vindictus_server::{
        domain::character_base_stats::BaseStats,
        infrastructure::character_base_stats::{self as stats, BaseStatError},
    };
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    let expected = BaseStats::compose([i32::MAX, i32::MIN, 3, 4, 5, 6], [1, -1, 7, 8, 9, 10, 11]);
    for bad in [
        UserKey { uid: 32, ..key() },
        UserKey {
            name: "Other",
            ..key()
        },
        UserKey {
            revision: 1,
            ..key()
        },
    ] {
        assert!(matches!(
            stats::initialize(&p, &bad, 1, &expected).await,
            Err(BaseStatError::Ownership)
        ));
    }
    assert!(matches!(
        stats::initialize(&p, &key(), 99, &expected).await,
        Err(BaseStatError::Ownership)
    ));
    sqlx::query("DELETE FROM game_lab_account_origins WHERE uid=31")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        stats::initialize(&p, &key(), 1, &expected).await,
        Err(BaseStatError::Ownership)
    ));
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(&p).await.unwrap();
    for change in ["class=10", "level=2", "use_preset=true"] {
        sqlx::query(&format!(
            "UPDATE game_character_info SET {change} WHERE id=1"
        ))
        .execute(&p)
        .await
        .unwrap();
        assert!(matches!(
            stats::initialize(&p, &key(), 1, &expected).await,
            Err(BaseStatError::Ownership)
        ));
        sqlx::query("UPDATE game_character_info SET class=1,level=1,use_preset=false WHERE id=1")
            .execute(&p)
            .await
            .unwrap();
    }
    let before:serde_json::Value=sqlx::query_scalar("SELECT to_jsonb(c)-ARRAY['dirty','str','dex','int','will','luck','hp','stamina'] FROM game_character_info c WHERE id=1").fetch_one(&p).await.unwrap();
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let pp = p.clone();
        tasks.push(tokio::spawn(async move {
            stats::initialize(&pp, &key(), 1, &expected).await.unwrap()
        }));
    }
    let mut writes = 0;
    for task in tasks {
        writes += task.await.unwrap();
    }
    assert_eq!(writes, 1);
    let got: (bool, i32, i32, i32, i32, i32, i32, i32) = sqlx::query_as(
        "SELECT dirty,str,dex,int,will,luck,hp,stamina FROM game_character_info WHERE id=1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(got, (false, i32::MIN, i32::MAX, 10, 12, 109, 15, 17));
    let after:serde_json::Value=sqlx::query_scalar("SELECT to_jsonb(c)-ARRAY['dirty','str','dex','int','will','luck','hp','stamina'] FROM game_character_info c WHERE id=1").fetch_one(&p).await.unwrap();
    assert_eq!(before, after);
    assert_eq!(
        stats::initialize(&p, &key(), 1, &expected).await.unwrap(),
        0
    );
    sqlx::query("UPDATE game_character_info SET hp=99 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        stats::initialize(&p, &key(), 1, &expected).await,
        Err(BaseStatError::Conflict)
    ));
    sqlx::query("UPDATE game_character_info SET dirty=true,str=0,dex=0,int=0,will=0,luck=0,hp=99,stamina=0 WHERE id=1").execute(&p).await.unwrap();
    assert!(matches!(
        stats::initialize(&p, &key(), 1, &expected).await,
        Err(BaseStatError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT hp FROM game_character_info WHERE id=1")
            .fetch_one(&p)
            .await
            .unwrap(),
        99
    );
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn base_stat_submit_failure_rolls_back_dirty_and_every_stat_then_allows_retry() {
    use vindictus_server::{
        domain::character_base_stats::BaseStats,
        infrastructure::character_base_stats::{self as stats, BaseStatError},
    };
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::query("CREATE FUNCTION reject_base_stats() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic submit failure'; END $$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_base_stats BEFORE UPDATE ON game_character_info FOR EACH ROW EXECUTE FUNCTION reject_base_stats()").execute(&p).await.unwrap();
    let expected = BaseStats::compose([1, 2, 3, 4, 5, 6], [0; 7]);
    assert!(matches!(
        stats::initialize(&p, &key(), 1, &expected).await,
        Err(BaseStatError::Database(_))
    ));
    let got: (bool, i32, i32, i32, i32, i32, i32, i32) = sqlx::query_as(
        "SELECT dirty,str,dex,int,will,luck,hp,stamina FROM game_character_info WHERE id=1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(got, (true, 0, 0, 0, 0, 0, 0, 0));
    sqlx::query("DROP TRIGGER reject_base_stats ON game_character_info")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION reject_base_stats()")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        stats::initialize(&p, &key(), 1, &expected).await.unwrap(),
        1
    );
    cleanup_game_pool(admin, p, schema).await;
}
