use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;
use vindictus_server::{
    application::game_users::provision_game_user,
    domain::{
        legacy_user_data::{LegacyBytes, SqlDateTime, SqlDateTime2},
        login_account_data::{AchievementSnapshot, LoyaltySnapshot},
        login_effects::*,
    },
    error::Error,
    infrastructure::{login_account_effects::*, postgres},
};
fn date(units: i64) -> SqlDateTime {
    SqlDateTime::from_units300(units).unwrap()
}
fn bytes(text: &Value) -> Vec<u8> {
    let text = text.as_str().unwrap();
    (0..text.len())
        .step_by(2)
        .map(|pos| u8::from_str_radix(&text[pos..pos + 2], 16).unwrap())
        .collect()
}
fn number(value: &Value) -> Option<i64> {
    value.as_str().map(|value| value.parse().unwrap())
}
fn loyalty(value: &Value) -> LoyaltySnapshot {
    LoyaltySnapshot {
        recommender_uid: value["uid"].as_i64().unwrap() as i32,
        recommender_cid: number(&value["recommender_cid"]),
        friend_cid: number(&value["friend_cid"]),
        recommend_flag: value["flag"].as_i64().unwrap() as i32,
        update_time: date(number(&value["update_units300"]).unwrap()),
    }
}
fn achievement(value: &Value) -> AchievementSnapshot {
    AchievementSnapshot {
        id: value["id"].as_i64().unwrap() as i32,
        uid: value["uid"].as_i64().unwrap() as i32,
        name: LegacyBytes::from_bytes(bytes(&value["name_hex"])).unwrap(),
        achievement: LegacyBytes::from_bytes(bytes(&value["achievement_hex"])).unwrap(),
    }
}
fn plan(
    friend: bool,
    dormant: bool,
    v2: bool,
    since_days: Option<i64>,
    latest_days: Option<i64>,
    new_user: bool,
) -> LoginEffectPlan {
    const DAY: i64 = 864_000_000_000;
    let now = SqlDateTime2::from_ticks(100 * DAY).unwrap();
    plan_login_effects(
        &EffectInputs {
            new_user_required: new_user,
            friend_recommend_flag: if friend { None } else { Some(0) },
            latest_for_loyalty: None,
            dormant_since: since_days
                .map(|days| SqlDateTime2::from_ticks(now.ticks() - days * DAY).unwrap()),
            latest_for_dormancy: latest_days
                .map(|days| SqlDateTime2::from_ticks(now.ticks() - days * DAY).unwrap()),
        },
        EffectFeatures {
            friend_loyalty: friend,
            friend_return_days: 30,
            dormant_account: dormant,
            dormant_new_account_days: 1,
            dormant_account_days: 30,
            dormant_account_v2: v2,
        },
        EffectClockSamples {
            loyalty_now: now,
            dormant_elapsed_now: now,
            latest_elapsed_now: now,
            set_dormant_now: now,
            set_event_now: now,
        },
    )
}
fn request<'a>(
    id: Uuid,
    uid: i32,
    name: &'a str,
    revision: i64,
    plan: &'a LoginEffectPlan,
) -> AccountEffectRequest<'a> {
    AccountEffectRequest {
        operation_id: id,
        uid,
        name,
        expected_revision: revision,
        loyalty_insert_now: date(1),
        plan,
    }
}
async fn state(pool: &PgPool, uid: i32) -> (i64, Option<i64>, Option<i64>) {
    sqlx::query_as("SELECT local_revision,is_dormant_user_units300,is_dormant_user_event_units300 FROM game_users WHERE id=$1")
        .bind(uid).fetch_one(pool).await.unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn account_snapshots_and_sql_primitives_preserve_full_source_columns_and_equality() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../docs/evidence/database/login-account-oracle.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 10);
    assert_eq!(fixture["source_loyalty_rows"].as_u64(), Some(0));
    assert_eq!(fixture["source_achievement_rows"].as_u64(), Some(0));
    for case in fixture["cases"].as_array().unwrap() {
        if case["kind"] == "friend" {
            sqlx::query("DELETE FROM game_friend_loyalty")
                .execute(&pool)
                .await
                .unwrap();
            if !case["initial"].is_null() {
                let initial = loyalty(&case["initial"]);
                assert!(insert_loyalty_snapshot(&pool, &initial).await.unwrap());
                assert!(!insert_loyalty_snapshot(&pool, &initial).await.unwrap());
            }
            let mut tx = pool.begin().await.unwrap();
            set_friend_flag(&mut tx, 1, case["flag"].as_i64().unwrap() as i32, date(1))
                .await
                .unwrap();
            tx.commit().await.unwrap();
            assert!(read_loyalty(&pool, 1).await.unwrap().unwrap() == loyalty(&case["result"]));
            // Even the same flag clears links; another UPDATE must not refresh UpdateTime.
            let before = read_loyalty(&pool, 1).await.unwrap().unwrap();
            let mut tx = pool.begin().await.unwrap();
            set_friend_flag(&mut tx, 1, before.recommend_flag, date(999))
                .await
                .unwrap();
            tx.commit().await.unwrap();
            assert!(read_loyalty(&pool, 1).await.unwrap().unwrap() == before);
        } else if case["kind"] == "achievement" {
            sqlx::query("DELETE FROM game_user_achievements")
                .execute(&pool)
                .await
                .unwrap();
            for value in case["initial"].as_array().unwrap() {
                let initial = achievement(value);
                assert!(insert_achievement_snapshot(&pool, &initial).await.unwrap());
                assert!(!insert_achievement_snapshot(&pool, &initial).await.unwrap());
            }
            let mut tx = pool.begin().await.unwrap();
            let affected =
                replace_achievement_owner(&mut tx, case["argument"].as_str().unwrap(), 7)
                    .await
                    .unwrap();
            tx.commit().await.unwrap();
            assert_eq!(affected, case["affected"].as_u64().unwrap());
            for value in case["result"].as_array().unwrap() {
                let expected = achievement(value);
                assert!(read_achievement(&pool, expected.id).await.unwrap().unwrap() == expected);
            }
        } else {
            let id = case["case_id"].as_i64().unwrap() as i32 + 100;
            provision_game_user(&pool, id, &format!("User{id}"), 42, 0)
                .await
                .unwrap();
            sqlx::query("UPDATE game_users SET is_dormant_user_units300=1,is_dormant_user_event_units300=2 WHERE id=$1")
                .bind(id).execute(&pool).await.unwrap();
            let release = case["result"]["dormant_units300"].is_null();
            let clear = case["result"]["event_units300"].is_null();
            let effect_plan = plan(
                false,
                true,
                !clear,
                if release { Some(2) } else { None },
                None,
                false,
            );
            let result = apply_existing_account_effects(
                &pool,
                &request(Uuid::new_v4(), id, &format!("User{id}"), 1, &effect_plan),
            )
            .await
            .unwrap();
            assert_eq!(result.revision, 2);
            let (_, dormant, event) = state(&pool, id).await;
            assert_eq!(dormant, number(&case["result"]["dormant_units300"]));
            assert_eq!(event, number(&case["result"]["event_units300"]));
        }
    }
    let existing = read_loyalty(&pool, 1).await.unwrap().unwrap();
    let conflict = LoyaltySnapshot {
        recommend_flag: existing.recommend_flag + 1,
        ..existing
    };
    assert!(matches!(
        insert_loyalty_snapshot(&pool, &conflict).await,
        Err(Error::LoginAccountSnapshotConflict)
    ));
    let opaque = AchievementSnapshot {
        id: i32::MIN,
        uid: -1,
        name: LegacyBytes::from_bytes(vec![0, 216, 0, 0]).unwrap(),
        achievement: LegacyBytes::from_bytes([0, 216].repeat(50)).unwrap(),
    };
    assert!(insert_achievement_snapshot(&pool, &opaque).await.unwrap());
    assert!(read_achievement(&pool, opaque.id).await.unwrap().unwrap() == opaque);
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        replace_achievement_owner(&mut tx, "Fixture", 7).await,
        Err(Error::LoginAccountEqualityUnsupported)
    ));
    tx.rollback().await.unwrap();
    let conflict = AchievementSnapshot {
        id: opaque.id,
        uid: 7,
        name: LegacyBytes::from_bytes(opaque.name.as_bytes().to_vec()).unwrap(),
        achievement: LegacyBytes::from_bytes(opaque.achievement.as_bytes().to_vec()).unwrap(),
    };
    assert!(matches!(
        insert_achievement_snapshot(&pool, &conflict).await,
        Err(Error::LoginAccountSnapshotConflict)
    ));
    assert!(read_achievement(&pool, opaque.id).await.unwrap().unwrap() == opaque);
    for (name, value) in [
        (vec![0_u8; 49], vec![]),
        (vec![0_u8; 50], vec![]),
        (vec![], vec![0_u8; 102]),
    ] {
        let error=sqlx::query("INSERT INTO game_user_achievements(id,uid,name_utf16,achievement_utf16) VALUES(999,-1,$1,$2)")
            .bind(name).bind(value).execute(&pool).await.unwrap_err();
        assert!(matches!(error,sqlx::Error::Database(ref error) if error.is_check_violation()));
    }
    for units in [SqlDateTime::MIN - 1, SqlDateTime::MAX + 1] {
        assert!(sqlx::query("INSERT INTO game_friend_loyalty(recommender_uid,recommend_flag,update_time_units300) VALUES(999,1,$1)")
            .bind(units).execute(&pool).await.is_err());
    }
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn account_effects_commit_replay_conflict_and_failure_rollback_as_one_batch() {
    let (admin, pool, schema) = isolated_game_pool().await;
    for uid in 1..=3 {
        provision_game_user(&pool, uid, &format!("Effects{uid}"), 42, 0)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE game_users SET is_dormant_user_units300=1,is_dormant_user_event_units300=2,legacy_last_update=decode('0102030405060708','hex') WHERE id=1")
        .execute(&pool).await.unwrap();
    let effect_plan = plan(true, true, true, Some(2), None, false);
    let operation = Uuid::new_v4();
    let input = request(operation, 1, "Effects1", 1, &effect_plan);
    let result = apply_existing_account_effects(&pool, &input).await.unwrap();
    assert_eq!(
        result.flags,
        EffectFlags {
            is_dormant: false,
            is_return: true
        }
    );
    assert_eq!(result.revision, 2);
    assert!(!result.replayed);
    assert_eq!(state(&pool, 1).await, (2, None, Some(2)));
    let original: Vec<u8> =
        sqlx::query_scalar("SELECT legacy_last_update FROM game_users WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(original, [1, 2, 3, 4, 5, 6, 7, 8]);
    let replay = apply_existing_account_effects(&pool, &input).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.flags, result.flags);
    assert_eq!(replay.revision, 2);
    assert!(
        apply_existing_account_effects(&pool, &request(operation, 1, "EFFECTS1 ", 1, &effect_plan))
            .await
            .unwrap()
            .replayed
    );
    assert!(matches!(
        apply_existing_account_effects(&pool, &request(operation, 1, "Effects1", 2, &effect_plan))
            .await,
        Err(Error::LoginEffectReplayConflict)
    ));
    assert!(matches!(
        apply_existing_account_effects(
            &pool,
            &request(Uuid::new_v4(), 1, "Effects1", 1, &effect_plan)
        )
        .await,
        Err(Error::GameUserRevisionConflict)
    ));
    assert!(matches!(
        apply_existing_account_effects(&pool, &request(operation, 2, "Effects2", 0, &effect_plan))
            .await,
        Err(Error::LoginEffectReplayConflict)
    ));
    assert_eq!(state(&pool, 2).await, (0, None, None));
    let rewards = plan(true, true, true, None, Some(31), false);
    assert!(matches!(
        apply_existing_account_effects(&pool, &request(Uuid::new_v4(), 1, "Effects1", 2, &rewards))
            .await,
        Err(Error::LoginEffectDependencies)
    ));
    let new_user = plan(true, false, false, None, None, true);
    assert!(matches!(
        apply_existing_account_effects(
            &pool,
            &request(Uuid::new_v4(), 3, "Effects3", 0, &new_user)
        )
        .await,
        Err(Error::LoginEffectDependencies)
    ));
    // Reject the final receipt INSERT after preceding writes have run; everything must roll back.
    sqlx::query(
        "ALTER TABLE game_login_account_effect_runs ADD CONSTRAINT reject_test_uid CHECK(uid<>3)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let failed_id = Uuid::new_v4();
    assert!(matches!(
        apply_existing_account_effects(&pool, &request(failed_id, 3, "Effects3", 0, &effect_plan))
            .await,
        Err(Error::Database(_))
    ));
    assert_eq!(state(&pool, 3).await, (0, None, None));
    assert!(read_loyalty(&pool, 3).await.unwrap().is_none());
    let receipts: i64 = sqlx::query_scalar("SELECT count(*) FROM game_login_account_effect_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(receipts, 1);
    assert_eq!(state(&pool, 1).await, (2, None, Some(2)));
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn account_effects_concurrent_revision_and_replay_and_cancelled_acquisition() {
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 9, "ConcurrentEffects", 42, 0)
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let pool = pool.clone();
        tasks.spawn(async move {
            let effect_plan = plan(true, false, false, None, None, false);
            apply_existing_account_effects(
                &pool,
                &request(Uuid::new_v4(), 9, "ConcurrentEffects", 0, &effect_plan),
            )
            .await
        });
    }
    let (mut success, mut refused) = (0, 0);
    let mut bounded_revision_refusals = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(value) => {
                assert_eq!(value.revision, 1);
                success += 1;
            }
            Err(Error::GameUserRevisionConflict) => refused += 1,
            Err(Error::Database(sqlx::Error::Database(error)))
                if error.code().as_deref() == Some("55P03") =>
            {
                // A bounded lock refusal cannot turn a stale writer into a winner.
                bounded_revision_refusals += 1;
                refused += 1;
            }
            Err(error) => panic!("unexpected sanitized error: {error}"),
        }
    }
    assert_eq!((success, refused), (1, 15));
    for _ in 0..bounded_revision_refusals {
        let effect_plan = plan(true, false, false, None, None, false);
        assert!(matches!(
            apply_existing_account_effects(
                &pool,
                &request(Uuid::new_v4(), 9, "ConcurrentEffects", 0, &effect_plan)
            )
            .await,
            Err(Error::GameUserRevisionConflict)
        ));
    }
    let same_operation = Uuid::new_v4();
    for _ in 0..16 {
        let pool = pool.clone();
        tasks.spawn(async move {
            let effect_plan = plan(false, false, false, None, None, false);
            apply_existing_account_effects(
                &pool,
                &request(same_operation, 9, "ConcurrentEffects", 1, &effect_plan),
            )
            .await
        });
    }
    let (mut applied, mut replays, mut bounded_replay_refusals) = (0, 0, 0);
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(result) => {
                assert_eq!(result.revision, 1);
                if result.replayed {
                    replays += 1
                } else {
                    applied += 1
                }
            }
            Err(Error::Database(sqlx::Error::Database(error)))
                if error.code().as_deref() == Some("55P03") =>
            {
                bounded_replay_refusals += 1;
            }
            Err(error) => panic!("unexpected replay failure: {error}"),
        }
    }
    assert_eq!(applied, 1);
    assert_eq!(replays + bounded_replay_refusals, 15);
    for _ in 0..bounded_replay_refusals {
        let effect_plan = plan(false, false, false, None, None, false);
        let retry = apply_existing_account_effects(
            &pool,
            &request(same_operation, 9, "ConcurrentEffects", 1, &effect_plan),
        )
        .await
        .unwrap();
        assert!(retry.replayed);
        assert_eq!(retry.revision, 1);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_login_account_effect_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    let scoped_search_path = format!("SET search_path TO {schema}");
    let single = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            let statement = scoped_search_path.clone();
            Box::pin(async move {
                sqlx::query(&statement).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let held = single.begin().await.unwrap();
    let task_pool = single.clone();
    let cancelled = Uuid::new_v4();
    let (started, polled) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let effect_plan = plan(true, false, false, None, None, false);
        started.send(()).unwrap();
        apply_existing_account_effects(
            &task_pool,
            &request(cancelled, 9, "ConcurrentEffects", 1, &effect_plan),
        )
        .await
    });
    polled.await.unwrap();
    // The task ran to the awaited pool.begin; its only connection is still held.
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    held.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_login_account_effect_runs WHERE operation_id=$1",
    )
    .bind(cancelled)
    .fetch_one(&single)
    .await
    .unwrap();
    assert_eq!(count, 0);
    assert_eq!(state(&pool, 9).await, (1, None, None));
    let readonly = plan(false, false, false, None, None, false);
    assert!(
        !apply_existing_account_effects(
            &single,
            &request(cancelled, 9, "ConcurrentEffects", 1, &readonly)
        )
        .await
        .unwrap()
        .replayed
    );
    single.close().await;
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn account_effect_schema_upgrade_readiness_and_replay_preserve_existing_users() {
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 12, "UpgradeEffects", 42, 0)
        .await
        .unwrap();
    sqlx::query("UPDATE game_users SET legacy_last_update=decode('FFEEDDCCBBAA0099','hex'),is_dormant_user_units300=-1 WHERE id=12")
        .execute(&pool).await.unwrap();
    let before: String =
        sqlx::query_scalar("SELECT md5(row(game_users.*)::text) FROM game_users WHERE id=12")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::raw_sql("DROP TABLE game_skills,game_sp_skills,game_skill_identity;DELETE FROM _sqlx_migrations WHERE version=19;DROP TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status,game_item_identity;DROP FUNCTION item_attribute_ascii_key(bytea);DELETE FROM _sqlx_migrations WHERE version=18;DROP TABLE game_costumes,game_costume_identity;DELETE FROM _sqlx_migrations WHERE version=17;DROP TABLE game_character_ledger01,game_character_ledger02;DELETE FROM _sqlx_migrations WHERE version=16;DROP TABLE game_item_collections,game_quest_collections,game_mailboxes,game_story_entries,game_my_settings;DELETE FROM _sqlx_migrations WHERE version=15;DROP TABLE game_reserved_names,game_reserved_titles,game_character_name_logs;DELETE FROM _sqlx_migrations WHERE version=14;DROP TABLE game_lab_account_origins;DELETE FROM _sqlx_migrations WHERE version=13;DROP TABLE game_home_cafe_bonus;DELETE FROM _sqlx_migrations WHERE version=12;DROP TABLE game_user_care_info,game_user_products,game_user_info;DELETE FROM _sqlx_migrations WHERE version=11;DROP TABLE game_login_reward_runs;DELETE FROM _sqlx_migrations WHERE version=10;DROP TABLE game_dormant_item_runs,game_queue_identity;DELETE FROM _sqlx_migrations WHERE version=9;DROP TABLE game_last_connect_info,game_character_info,game_characters,game_queued_items;DELETE FROM _sqlx_migrations WHERE version=8;DELETE FROM _sqlx_migrations WHERE version=7;DROP TABLE game_login_account_effect_runs,game_friend_loyalty,game_user_achievements;DELETE FROM _sqlx_migrations WHERE version=6")
        .execute(&pool).await.unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    let previous = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            postgres::MIGRATOR
                .iter()
                .filter(|m| m.version <= 5)
                .cloned()
                .collect(),
        ),
        ..postgres::MIGRATOR
    };
    previous.run(&pool).await.unwrap();
    postgres::migrate(&pool).await.unwrap();
    postgres::check_ready(&pool).await.unwrap();
    let after: String =
        sqlx::query_scalar("SELECT md5(row(game_users.*)::text) FROM game_users WHERE id=12")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    let row = LoyaltySnapshot {
        recommender_uid: i32::MIN,
        recommender_cid: Some(i64::MIN),
        friend_cid: Some(i64::MAX),
        recommend_flag: i32::MIN,
        update_time: date(SqlDateTime::MIN),
    };
    assert!(insert_loyalty_snapshot(&pool, &row).await.unwrap());
    let source = AchievementSnapshot {
        id: i32::MAX,
        uid: -1,
        name: LegacyBytes::from_bytes(vec![]).unwrap(),
        achievement: LegacyBytes::from_bytes(vec![0, 216, 0, 0]).unwrap(),
    };
    assert!(insert_achievement_snapshot(&pool, &source).await.unwrap());
    postgres::migrate(&pool).await.unwrap();
    assert!(
        read_loyalty(&pool, row.recommender_uid)
            .await
            .unwrap()
            .unwrap()
            == row
    );
    assert!(read_achievement(&pool, source.id).await.unwrap().unwrap() == source);
    sqlx::query(
        "ALTER TABLE game_login_account_effect_runs RENAME COLUMN request TO broken_request",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    sqlx::query(
        "ALTER TABLE game_login_account_effect_runs RENAME COLUMN broken_request TO request",
    )
    .execute(&pool)
    .await
    .unwrap();
    postgres::check_ready(&pool).await.unwrap();
    cleanup_game_pool(admin, pool, schema).await;
}
