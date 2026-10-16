use super::{cleanup_game_pool, isolated_game_pool};
mod fixture {
    include!("../fixtures/character_state_support.rs");
}
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use vindictus_server::{
    domain::legacy_user_data::SqlDateTime,
    infrastructure::fiona_character_state::{self as db, StateError},
};
fn snapshot(v: &Value) -> db::Snapshot {
    db::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
async fn roots(p: &PgPool) {
    sqlx::query("INSERT INTO game_characters(id) VALUES(1),(2),(3)")
        .execute(p)
        .await
        .unwrap();
}
async fn counts(p: &PgPool) -> [i64; 6] {
    let(a,b,c,d,e,f)=sqlx::query_as::<_,(i64,i64,i64,i64,i64,i64)>("SELECT (SELECT COUNT(*) FROM game_titles),(SELECT COUNT(*) FROM game_title_goal_progress),(SELECT COUNT(*) FROM game_title_presets),(SELECT COUNT(*) FROM game_micro_play_status),(SELECT COUNT(*) FROM game_character_mission_info),(SELECT COUNT(*) FROM game_mission_assignments)").fetch_one(p).await.unwrap();
    [a, b, c, d, e, f]
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn character_state_preserves_all_source_fields_nullable_bytes_composite_keys_and_fk_actions()
{
    let (admin, p, schema) = isolated_game_pool().await;
    roots(&p).await;
    let s = snapshot(&fixture::export());
    assert!(matches!(
        db::preserve(&p, &s, true).await,
        Err(StateError::Uninitialized)
    ));
    let (a, b) = tokio::join!(db::preserve(&p, &s, false), db::preserve(&p, &s, false));
    let a = a.unwrap();
    let b = b.unwrap();
    use sha2::{Digest, Sha256};
    let original_bytes = serde_json::to_vec(&fixture::export()).unwrap();
    let digest: [u8; 32] = Sha256::digest(&original_bytes).into();
    db::verify_source_binding(&p, &digest).await.unwrap();
    assert!(matches!(
        db::verify_source_binding(&p, &[0; 32]).await,
        Err(StateError::Conflict)
    ));
    let mut altered_bytes = original_bytes;
    altered_bytes.push(b' ');
    let altered = db::read_snapshot(altered_bytes.as_slice()).unwrap();
    assert!(matches!(
        db::preserve(&p, &altered, false).await,
        Err(StateError::Conflict)
    ));

    for (i, j) in a.inserted.into_iter().zip(b.inserted) {
        assert_eq!(i + j, 2)
    }
    assert_eq!(counts(&p).await, [2; 6]);
    assert_eq!(db::preserve(&p, &s, false).await.unwrap().inserted, [0; 6]);
    assert_eq!(db::preserve(&p, &s, true).await.unwrap().verified, [2; 6]);
    let title:(i32,bool,Option<i64>,Option<Vec<u8>>,Option<i64>)=sqlx::query_as("SELECT title_id,acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks FROM game_titles WHERE cid=1").fetch_one(&p).await.unwrap();
    assert_eq!(
        title,
        (
            -1,
            true,
            Some(SqlDateTime::MIN),
            Some(vec![0, 216]),
            Some(3_155_378_975_999_999_999)
        )
    );
    let title:(bool,Option<i64>,Option<Vec<u8>>,Option<i64>)=sqlx::query_as("SELECT acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks FROM game_titles WHERE cid=2").fetch_one(&p).await.unwrap();
    assert_eq!(title, (false, None, Some(vec![]), None));
    let micro: Value =
        sqlx::query_scalar("SELECT to_jsonb(m) FROM game_micro_play_status m WHERE cid=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(
        micro,
        json!({"cid":1,"micro_play_count":i32::MIN,"micro_play_time":i64::MIN,"success_micro_play_count":i32::MAX,"success_micro_play_time":i64::MAX,"daily_micro_play_count":-1,"daily_free_play_count":i32::MAX,"last_micro_play_started_units300":SqlDateTime::MIN,"daily_report_count":i32::MIN,"last_reported_time_units300":SqlDateTime::MAX,"current_quest_utf16":null,"report_received_count":null})
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<i32>>(
            "SELECT report_received_count FROM game_micro_play_status WHERE cid=2"
        )
        .fetch_one(&p)
        .await
        .unwrap(),
        Some(i32::MIN)
    );
    let assignment: Value = sqlx::query_scalar(
        "SELECT to_jsonb(a)-'mission_utf16' FROM game_mission_assignments a WHERE cid=1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(
        assignment,
        json!({"cid":1,"id":i64::MIN,"completion_progress":null,"assignment_ticks":0,"removed_ticks":null,"progress0":null,"progress1":i32::MIN,"progress2":0,"progress3":i32::MAX,"progress4":null})
    );
    assert_eq!(
        sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT mission_utf16 FROM game_mission_assignments WHERE cid=1"
        )
        .fetch_one(&p)
        .await
        .unwrap(),
        vec![0, 216]
    );
    assert_eq!(sqlx::query_as::<_,(i32,Option<i64>)>("SELECT completed_mission_count,last_clear_ticks FROM game_character_mission_info WHERE cid=99").fetch_one(&p).await.unwrap(),(i32::MAX,None));
    sqlx::query("UPDATE game_characters SET id=4 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    for table in [
        "game_titles",
        "game_title_goal_progress",
        "game_micro_play_status",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table} WHERE cid=4"))
                .fetch_one(&p)
                .await
                .unwrap(),
            1
        );
    }
    for table in ["game_character_mission_info", "game_mission_assignments"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table} WHERE cid=1"))
                .fetch_one(&p)
                .await
                .unwrap(),
            1
        );
    }
    sqlx::query("DELETE FROM game_characters WHERE id=4")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(counts(&p).await, [1, 1, 2, 1, 2, 2]);
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn character_state_rejects_conflicts_without_repair_and_rolls_back_late_fk_and_state_submit_failure()
 {
    let (admin, p, schema) = isolated_game_pool().await;
    roots(&p).await;
    let mut bad = fixture::export();
    bad["micro"][1]["cid"] = json!("98");
    assert!(db::preserve(&p, &snapshot(&bad), false).await.is_err());
    assert_eq!(counts(&p).await, [0; 6]);
    sqlx::query("CREATE FUNCTION reject_character_state_commit() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic state commit failure';END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_character_state_commit BEFORE UPDATE ON game_character_state_preservation FOR EACH ROW EXECUTE FUNCTION reject_character_state_commit()").execute(&p).await.unwrap();
    let s = snapshot(&fixture::export());
    assert!(db::preserve(&p, &s, false).await.is_err());
    assert_eq!(counts(&p).await, [0; 6]);
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT initialized FROM game_character_state_preservation")
            .fetch_one(&p)
            .await
            .unwrap()
    );
    sqlx::query("DROP TRIGGER reject_character_state_commit ON game_character_state_preservation")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION reject_character_state_commit()")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(db::preserve(&p, &s, false).await.unwrap().inserted, [2; 6]);
    for (table, change, restore) in [
        ("game_titles", "acquired=false", "acquired=true"),
        (
            "game_titles",
            "acquired_time_units300=NULL",
            "acquired_time_units300=-1391644800000",
        ),
        (
            "game_titles",
            "acquired_quest_utf16=''::bytea",
            "acquired_quest_utf16=decode('00d8','hex')",
        ),
        (
            "game_titles",
            "expire_ticks=NULL",
            "expire_ticks=3155378975999999999",
        ),
        (
            "game_title_goal_progress",
            "progress=0",
            "progress=2147483647",
        ),
        (
            "game_micro_play_status",
            "report_received_count=1",
            "report_received_count=NULL",
        ),
        (
            "game_character_mission_info",
            "last_clear_ticks=NULL",
            "last_clear_ticks=0",
        ),
        (
            "game_mission_assignments",
            "progress1=NULL",
            "progress1=-2147483648",
        ),
    ] {
        sqlx::query(&format!("UPDATE {table} SET {change} WHERE cid=1"))
            .execute(&p)
            .await
            .unwrap();
        assert!(matches!(
            db::preserve(&p, &s, true).await,
            Err(StateError::Conflict)
        ));
        assert!(matches!(
            db::preserve(&p, &s, false).await,
            Err(StateError::Conflict)
        ));
        sqlx::query(&format!("UPDATE {table} SET {restore} WHERE cid=1"))
            .execute(&p)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE game_title_presets SET class_restriction=0 WHERE title_id=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &s, false).await,
        Err(StateError::Conflict)
    ));
    sqlx::query("UPDATE game_title_presets SET class_restriction=-1 WHERE title_id=1")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(db::preserve(&p, &s, true).await.unwrap().verified, [2; 6]);
    assert_eq!(
        db::preserve(&p, &snapshot(&fixture::empty()), false)
            .await
            .err()
            .unwrap()
            .to_string(),
        StateError::Conflict.to_string()
    );
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn character_state_cancelled_import_leaves_all_six_tables_and_receipt_untouched_then_retries()
{
    let (admin, p, schema) = isolated_game_pool().await;
    roots(&p).await;
    sqlx::query("CREATE FUNCTION delay_character_state_import() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_advisory_xact_lock(hashtext(current_schema()),74920);PERFORM pg_sleep(20);RETURN NEW;END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER delay_character_state_import BEFORE INSERT ON game_mission_assignments FOR EACH ROW WHEN(NEW.cid=99) EXECUTE FUNCTION delay_character_state_import()").execute(&p).await.unwrap();
    let s = Arc::new(snapshot(&fixture::export()));
    let pp = p.clone();
    let ss = s.clone();
    let task = tokio::spawn(async move { db::preserve(&pp, &ss, false).await });
    tokio::time::timeout(Duration::from_secs(10),async{loop{if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=74920 AND granted)").fetch_one(&p).await.unwrap(){break};tokio::time::sleep(Duration::from_millis(25)).await;}}).await.unwrap();
    task.abort();
    match task.await {
        Err(e) => assert!(e.is_cancelled()),
        Ok(_) => panic!("import completed before cancellation"),
    };
    tokio::time::timeout(Duration::from_secs(10),async{loop{if !sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=74920 AND granted)").fetch_one(&p).await.unwrap(){break};tokio::time::sleep(Duration::from_millis(25)).await;}}).await.unwrap();
    assert_eq!(counts(&p).await, [0; 6]);
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT initialized FROM game_character_state_preservation")
            .fetch_one(&p)
            .await
            .unwrap()
    );
    sqlx::query("DROP TRIGGER delay_character_state_import ON game_mission_assignments")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION delay_character_state_import()")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(db::preserve(&p, &s, false).await.unwrap().inserted, [2; 6]);
    cleanup_game_pool(admin, p, schema).await;
}
fn owner() -> vindictus_server::infrastructure::character_lobby::UserKey<'static> {
    vindictus_server::infrastructure::character_lobby::UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
async fn new_fiona(p: &PgPool) {
    use super::name_check_common as common;
    use vindictus_server::{
        application::game_users::provision_game_user,
        infrastructure::character_creation_core::{self, CreationCoreRequest},
    };
    provision_game_user(p, 31, "Owner", 900, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(p).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes) VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(p)
        .await
        .unwrap();
    let proof = common::proof(&common::content());
    character_creation_core::create_base(
        p,
        owner(),
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
async fn first_fiona_state_writers_require_bootstrap_owner_revision_origin_and_replay_exactly() {
    use vindictus_server::{
        domain::legacy_user_data::SqlDateTime2, infrastructure::character_lobby::UserKey,
    };
    let (admin, p, schema) = isolated_game_pool().await;
    new_fiona(&p).await;
    let clear = SqlDateTime2::from_ticks(0).unwrap();
    let seed = db::MicroSeed {
        started: SqlDateTime::from_units300(10).unwrap(),
        reported: SqlDateTime::from_units300(11).unwrap(),
    };
    assert!(matches!(
        db::initialize_mission(&p, &owner(), 1, clear).await,
        Err(StateError::Uninitialized)
    ));
    assert!(matches!(
        db::initialize_micro(&p, &owner(), 1, seed).await,
        Err(StateError::Uninitialized)
    ));
    assert!(matches!(
        db::seed_title_presets(&p, &owner(), 1, &[1, 3]).await,
        Err(StateError::Uninitialized)
    ));
    let mut input = fixture::empty();
    input["presets"] = json!([{"title_id":1,"class_restriction":2},{"title_id":2,"class_restriction":1},{"title_id":3,"class_restriction":3}]);
    db::preserve(&p, &snapshot(&input), false).await.unwrap();
    for k in [
        UserKey { uid: 32, ..owner() },
        UserKey {
            name: "Other",
            ..owner()
        },
        UserKey {
            revision: 1,
            ..owner()
        },
    ] {
        assert!(matches!(
            db::initialize_mission(&p, &k, 1, clear).await,
            Err(StateError::Ownership)
        ));
        assert!(matches!(
            db::initialize_micro(&p, &k, 1, seed).await,
            Err(StateError::Ownership)
        ));
        assert!(matches!(
            db::seed_title_presets(&p, &k, 1, &[1, 3]).await,
            Err(StateError::Ownership)
        ));
    }
    sqlx::query("DELETE FROM game_lab_account_origins WHERE uid=31")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::initialize_mission(&p, &owner(), 1, clear).await,
        Err(StateError::Ownership)
    ));
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(&p).await.unwrap();
    for change in [
        "class=2",
        "level=2",
        "use_preset=true",
        "is_premium=true",
        "status=2",
    ] {
        sqlx::query(&format!(
            "UPDATE game_character_info SET {change} WHERE id=1"
        ))
        .execute(&p)
        .await
        .unwrap();
        assert!(matches!(
            db::initialize_mission(&p, &owner(), 1, clear).await,
            Err(StateError::Ownership)
        ));
        sqlx::query("UPDATE game_character_info SET class=1,level=1,use_preset=false,is_premium=false,status=0 WHERE id=1").execute(&p).await.unwrap();
    }
    assert!(matches!(
        db::seed_title_presets(&p, &owner(), 1, &[1]).await,
        Err(StateError::Profile)
    ));
    let joined_owner = owner();
    let (a, b, c) = tokio::join!(
        db::initialize_mission(&p, &joined_owner, 1, clear),
        db::initialize_micro(&p, &joined_owner, 1, seed),
        db::seed_title_presets(&p, &joined_owner, 1, &[1, 3])
    );
    assert_eq!(a.unwrap(), 1);
    assert_eq!(b.unwrap(), 1);
    assert_eq!(c.unwrap(), [1, 3]);
    assert_eq!(
        db::initialize_mission(&p, &owner(), 1, clear)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        db::initialize_micro(&p, &owner(), 1, seed).await.unwrap(),
        0
    );
    assert!(
        db::seed_title_presets(&p, &owner(), 1, &[1, 3])
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM game_titles WHERE cid=1 AND NOT acquired AND acquired_time_units300 IS NULL AND acquired_quest_utf16 IS NULL AND expire_ticks IS NULL").fetch_one(&p).await.unwrap(),2);
    assert_eq!(
        sqlx::query_scalar::<_, Option<i32>>(
            "SELECT report_received_count FROM game_micro_play_status WHERE cid=1"
        )
        .fetch_one(&p)
        .await
        .unwrap(),
        None
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT last_clear_ticks FROM game_character_mission_info WHERE cid=1"
        )
        .fetch_one(&p)
        .await
        .unwrap(),
        Some(0)
    );
    sqlx::query("UPDATE game_character_mission_info SET completed_mission_count=1 WHERE cid=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::initialize_mission(&p, &owner(), 1, clear).await,
        Err(StateError::Conflict)
    ));
    sqlx::query("UPDATE game_micro_play_status SET report_received_count=0 WHERE cid=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::initialize_micro(&p, &owner(), 1, seed).await,
        Err(StateError::Conflict)
    ));
    sqlx::query("UPDATE game_titles SET acquired=true WHERE cid=1 AND title_id=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::seed_title_presets(&p, &owner(), 1, &[1, 3]).await,
        Err(StateError::Conflict)
    ));
    assert_eq!(
        db::preserve(&p, &snapshot(&input), true)
            .await
            .unwrap()
            .verified,
        [0, 0, 3, 0, 0, 0]
    );
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn first_fiona_title_seed_rolls_back_entire_batch_on_late_failure_and_rejects_mission_assignments()
 {
    use vindictus_server::domain::legacy_user_data::SqlDateTime2;
    let (admin, p, schema) = isolated_game_pool().await;
    new_fiona(&p).await;
    let mut input = fixture::empty();
    input["presets"] =
        json!([{"title_id":1,"class_restriction":2},{"title_id":3,"class_restriction":2}]);
    db::preserve(&p, &snapshot(&input), false).await.unwrap();
    sqlx::query("CREATE FUNCTION reject_last_fiona_title() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF NEW.title_id=3 THEN RAISE EXCEPTION 'synthetic late title failure';END IF;RETURN NEW;END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_last_fiona_title BEFORE INSERT ON game_titles FOR EACH ROW EXECUTE FUNCTION reject_last_fiona_title()").execute(&p).await.unwrap();
    assert!(matches!(
        db::seed_title_presets(&p, &owner(), 1, &[1, 3]).await,
        Err(StateError::Database(_))
    ));
    assert_eq!(counts(&p).await, [0, 0, 2, 0, 0, 0]);
    sqlx::query("DROP TRIGGER reject_last_fiona_title ON game_titles")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION reject_last_fiona_title()")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        db::seed_title_presets(&p, &owner(), 1, &[1, 3])
            .await
            .unwrap(),
        [1, 3]
    );
    sqlx::query("INSERT INTO game_mission_assignments(cid,id,mission_utf16) VALUES(1,1,''::bytea)")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::initialize_mission(&p, &owner(), 1, SqlDateTime2::from_ticks(0).unwrap()).await,
        Err(StateError::Profile)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM game_character_mission_info WHERE cid=1"
        )
        .fetch_one(&p)
        .await
        .unwrap(),
        0
    );
    cleanup_game_pool(admin, p, schema).await;
}
