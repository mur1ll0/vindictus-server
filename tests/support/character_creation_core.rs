use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use serde_json::{Value, json};
use vindictus_server::{
    application::game_users::provision_game_user,
    infrastructure::{
        character_creation_core::{
            self as core, CreationCoreError as E, CreationCoreRequest, CreationCoreResult as R,
        },
        character_lobby::UserKey,
    },
};
fn key() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
fn request(cid: i64, sn: i32) -> CreationCoreRequest<'static> {
    CreationCoreRequest {
        cid,
        character_sn: sn,
        name: "Murael",
        class: 1,
        use_preset: false,
    }
}
async fn setup(pool: &sqlx::PgPool) {
    provision_game_user(pool, 31, "Owner", 42, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31")
        .bind(vec![0u8;32]).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes) VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(pool)
        .await
        .unwrap();
}
async fn parents(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM game_characters")
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn utc(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_base_retains_all_defaults_and_original_collision_order_without_response_packets()
{
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    let proof = common::proof(&common::content());
    let before = utc(&pool).await;
    assert_eq!(
        core::create_base(&pool, key(), &proof, request(111, -7))
            .await
            .unwrap(),
        R::Created
    );
    let after = utc(&pool).await;
    let actual: Value =
        sqlx::query_scalar("SELECT to_jsonb(c) FROM game_character_info c WHERE id=111")
            .fetch_one(&pool)
            .await
            .unwrap();
    let units = actual["last_ap_update_units300"].as_i64().unwrap();
    assert!((before..=after).contains(&units));
    let ticks = vindictus_server::domain::legacy_user_data::SqlDateTime::from_units300(units)
        .unwrap()
        .framework_date_time()
        .unwrap()
        .ticks();
    let expected = json!({"id":111,"uid":31,"character_sn":-7,"name_utf16":"\\x4d0075007200610065006c00","class":1,"level":1,"exp":0,"str":0,"dex":0,"int":0,"will":0,"luck":0,"hp":0,"stamina":0,"dirty":true,"play_time":0,"selected_title":0,"ap":0,"total_used_ap":0,"last_ap_update_units300":units,"permission":3,"status":0,"quote_utf16":null,"delete_time_units300":null,"region_code":0,"is_connected":false,"legacy_last_update":null,"create_time_ticks":ticks,"free_match_win_count":0,"free_match_lose_count":0,"use_preset":false,"is_premium":false,"free_title_name_utf16":null,"today_play_time":null,"channel":null,"order_num":null,"map_state_info":0});
    assert_eq!(actual, expected);
    let owners:(i32,i32,i32,i32,i16)=sqlx::query_as("SELECT i.permission,q.permission,m.permission,s.permission,s.hint_category FROM game_item_collections i JOIN game_quest_collections q ON q.owner_id=i.owner_id JOIN game_mailboxes m ON m.owner_id=i.owner_id JOIN game_story_entries s ON s.cid=i.owner_id WHERE i.owner_id=111").fetch_one(&pool).await.unwrap();
    assert_eq!(owners, (3, 3, 3, 2, 0));
    assert_eq!(
        core::create_base(&pool, key(), &proof, request(111, 8))
            .await
            .unwrap(),
        R::ExistingCid
    );
    assert_eq!(
        core::create_base(&pool, key(), &proof, request(112, 8))
            .await
            .unwrap(),
        R::DuplicateNameOrSn
    );
    let other = "OtherName"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    sqlx::query("UPDATE game_character_info SET name_utf16=$1 WHERE id=111")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        core::create_base(&pool, key(), &proof, request(112, -7))
            .await
            .unwrap(),
        R::DuplicateNameOrSn
    );
    sqlx::query("UPDATE game_character_info SET status=1 WHERE id=111")
        .execute(&pool)
        .await
        .unwrap();
    // Base SQL excludes status1; outer name-retention logic remains independently required.
    assert_eq!(
        core::create_base(&pool, key(), &proof, request(112, -7))
            .await
            .unwrap(),
        R::Created
    );
    assert_eq!(parents(&pool).await, 2);
    let settings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_my_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(settings, 1);
    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_queued_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_base_refuses_unmarked_owner_revision_and_unmeasured_features_then_serializes_writers()
 {
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    let proof = common::proof(&common::content());
    let wrong = UserKey {
        revision: 1,
        ..key()
    };
    assert!(matches!(
        core::create_base(&pool, wrong, &proof, request(1, 1)).await,
        Err(E::Ownership)
    ));
    sqlx::query("DELETE FROM game_lab_account_origins")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        core::create_base(&pool, key(), &proof, request(1, 1)).await,
        Err(E::Ownership)
    ));
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(&pool).await.unwrap();
    for r in [
        CreationCoreRequest {
            class: 2,
            ..request(1, 1)
        },
        CreationCoreRequest {
            use_preset: true,
            ..request(1, 1)
        },
        CreationCoreRequest {
            name: "OtherName",
            ..request(1, 1)
        },
    ] {
        assert!(matches!(
            core::create_base(&pool, key(), &proof, r).await,
            Err(E::Profile)
        ));
    }
    for raw in [
        b"1".as_slice(),
        b"1suffix".as_slice(),
        b"".as_slice(),
        b" ".as_slice(),
    ] {
        sqlx::query("UPDATE game_my_settings SET value_bytes=$1")
            .bind(raw)
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            core::create_base(&pool, key(), &proof, request(1, 1)).await,
            Err(E::Profile)
        ));
        assert_eq!(parents(&pool).await, 0);
    }
    sqlx::query("UPDATE game_my_settings SET value_bytes=$1")
        .bind(b"0".as_slice())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_my_settings SELECT * FROM game_my_settings")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        core::create_base(&pool, key(), &proof, request(1, 1)).await,
        Err(E::Profile)
    ));
    sqlx::query(
        "DELETE FROM game_my_settings WHERE ctid IN(SELECT ctid FROM game_my_settings LIMIT 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut tasks = tokio::task::JoinSet::new();
    for cid in [11, 12] {
        let p = pool.clone();
        let b = barrier.clone();
        tasks.spawn(async move {
            let proof = common::proof(&common::content());
            b.wait().await;
            core::create_base(&p, key(), &proof, request(cid, cid as i32)).await
        });
    }
    let mut created = 0;
    let mut duplicate = 0;
    while let Some(r) = tasks.join_next().await {
        match r.unwrap().unwrap() {
            R::Created => created += 1,
            R::DuplicateNameOrSn => duplicate += 1,
            R::ExistingCid => panic!("unexpected cid collision"),
        }
    }
    assert_eq!((created, duplicate, parents(&pool).await), (1, 1, 1));
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_base_late_failure_and_cancellation_rollback_every_owner_and_character() {
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    let proof = common::proof(&common::content());
    sqlx::query("ALTER TABLE game_story_entries ADD CONSTRAINT synthetic_reject CHECK(cid<0)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        core::create_base(&pool, key(), &proof, request(100, 100)).await,
        Err(E::Database(_))
    ));
    assert_eq!(parents(&pool).await, 0);
    sqlx::query("ALTER TABLE game_story_entries DROP CONSTRAINT synthetic_reject")
        .execute(&pool)
        .await
        .unwrap();
    let mut block = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE game_story_entries IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *block)
        .await
        .unwrap();
    let p = pool.clone();
    let task = tokio::spawn(async move {
        let proof = common::proof(&common::content());
        core::create_base(&p, key(), &proof, request(100, 100)).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    block.rollback().await.unwrap();
    assert_eq!(parents(&pool).await, 0);
    for table in [
        "game_character_info",
        "game_item_collections",
        "game_quest_collections",
        "game_mailboxes",
        "game_story_entries",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    assert_eq!(
        core::create_base(&pool, key(), &proof, request(100, 100))
            .await
            .unwrap(),
        R::Created
    );
    cleanup_game_pool(admin, pool, schema).await;
}
