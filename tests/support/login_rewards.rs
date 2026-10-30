use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;
use vindictus_server::{
    application::{game_users::provision_game_user, login_rewards::*},
    domain::{
        legacy_user_data::{SqlDateTime, SqlDateTime2},
        login_effects::EffectClockSamples,
    },
    error::Error,
    infrastructure::{
        character_export, characters, dormant_items, postgres,
        reward_policy::{self, RewardPolicyPackage},
    },
};
const DAY: i64 = 25_920_000;
fn number(value: &Value) -> Option<i64> {
    value.as_str().map(|v| v.parse().unwrap())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn source_cases() -> Value {
    serde_json::from_str(include_str!(
        "../fixtures/framework-login-reward-executor.json"
    ))
    .unwrap()
}
fn active_case() -> Value {
    source_cases()["profiles"][0]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            c["friend"] == true && c["dormant"] == true && c["v2"] == true && c["scenario"] == 3
        })
        .unwrap()
        .clone()
}
fn content(case: &Value) -> RewardPolicyPackage {
    let mut value: Value =
        serde_json::from_str(include_str!("../fixtures/synthetic-reward-policy.json")).unwrap();
    let features = value["features"].as_array_mut().unwrap();
    for (key, enabled) in [
        ("friend_loyalty_system", case["friend"].as_bool().unwrap()),
        ("DormantAccount", case["dormant"].as_bool().unwrap()),
        ("DormantAccount_ver2", case["v2"].as_bool().unwrap()),
    ] {
        features.push(json!({"key":key,"value":if enabled {Some("1.09")}else{None}}));
    }
    for (key, value) in [
        ("friend_loyalty_return_interval", "7"),
        ("DormantAccountInterval", "7"),
        ("DormantNewAccountInterval", "1"),
    ] {
        features.push(json!({"key":key,"value":value}));
    }
    reward_policy::read_policy(serde_json::to_vec(&value).unwrap().as_slice()).unwrap()
}
fn request<'a>(
    case: &Value,
    content: &'a RewardPolicyPackage,
    id: Uuid,
    revision: i64,
) -> LoginRewardRequest<'a> {
    let now = SqlDateTime2::from_ticks(number(&case["now_ticks"]).unwrap()).unwrap();
    LoginRewardRequest {
        operation_id: id,
        uid: 7,
        name: "reward_user",
        expected_revision: revision,
        clocks: EffectClockSamples {
            loyalty_now: now,
            dormant_elapsed_now: now,
            latest_elapsed_now: now,
            set_dormant_now: now,
            set_event_now: now,
        },
        loyalty_insert_now: SqlDateTime::from_units300(100 * DAY).unwrap(),
        content,
    }
}
async fn setup(pool: &PgPool, case: &Value) -> i64 {
    sqlx::query("DELETE FROM game_characters")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_users")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_friend_loyalty")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_queued_items")
        .execute(pool)
        .await
        .unwrap();
    provision_game_user(pool, 7, "reward_user", 1, 0)
        .await
        .unwrap();
    sqlx::query("UPDATE game_users SET is_dormant_user_units300=$1,is_dormant_user_event_units300=$2 WHERE id=7")
        .bind(number(&case["input"]["since"])).bind(number(&case["input"]["event_date"])).execute(pool).await.unwrap();
    if let Some(flag) = case["input"]["friend_flag"].as_i64() {
        sqlx::query("INSERT INTO game_friend_loyalty(recommender_uid,recommend_flag,update_time_units300) VALUES(7,$1,0)").bind(flag as i32).execute(pool).await.unwrap();
    }
    let source: Value =
        serde_json::from_str(include_str!("../fixtures/character-bundle.json")).unwrap();
    let mut infos = Vec::new();
    let mut connections = Vec::new();
    let mut identities = Vec::new();
    for (id, level, name, order) in [
        (22, 20, Some("00d80000"), Some(0)),
        (11, 60, None, None),
        (
            33,
            i32::MIN,
            Some("730079006e00740068006500740069006300"),
            Some(255),
        ),
    ] {
        let mut row = source["character_info"][0].clone();
        row["id"] = json!(id.to_string());
        row["uid"] = json!(7);
        row["level"] = json!(level);
        row["name_utf16"] = json!(name);
        row["order_num"] = json!(order);
        infos.push(row);
        identities.push(json!({"id":id.to_string()}));
        if let Some(latest) = number(&case["input"]["latest"]) {
            let mut row = source["connections"][0].clone();
            row["id"] = json!(id.to_string());
            row["last_connect_date_units300"] =
                json!((if id == 22 { latest - DAY } else { 0 }).to_string());
            row["last_cafe_logout_units300"] = if id == 22 {
                json!(latest.to_string())
            } else {
                Value::Null
            };
            connections.push(row);
        }
    }
    let bundle = json!({"format_version":1,"source":"study_heroes.character-login-bundle","source_counts":{"characters":3,"character_info":3,"connections":connections.len(),"queued_items":0},"characters":identities,"character_info":infos,"connections":connections,"queued_items":[]});
    let bundle =
        character_export::read_export(serde_json::to_vec(&bundle).unwrap().as_slice()).unwrap();
    characters::import_bundle(pool, &bundle, false)
        .await
        .unwrap();
    sqlx::query_scalar("SELECT local_revision FROM game_users WHERE id=7")
        .fetch_one(pool)
        .await
        .unwrap()
}
fn clock() -> impl FnMut() -> Result<SqlDateTime, Error> {
    let mut n = 0;
    move || {
        let value = SqlDateTime::from_units300(100 * DAY + n)?;
        n += 1;
        Ok(value)
    }
}
async fn state(pool: &PgPool) -> (i64, Option<i64>, Option<i64>, Option<i32>, i64, i64) {
    sqlx::query_as("SELECT local_revision,is_dormant_user_units300,is_dormant_user_event_units300,(SELECT recommend_flag FROM game_friend_loyalty WHERE recommender_uid=7),(SELECT count(*) FROM game_queued_items),(SELECT count(*) FROM game_login_reward_runs) FROM game_users WHERE id=7").fetch_one(pool).await.unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_matches_recovered_existing_account_block() {
    let (admin, pool, schema) = isolated_game_pool().await;
    dormant_items::initialize_identity(&pool, None)
        .await
        .unwrap();
    let fixture = source_cases();
    assert_eq!(
        fixture["profiles"][0]["cases"],
        fixture["profiles"][1]["cases"]
    );
    for case in fixture["profiles"][0]["cases"].as_array().unwrap() {
        let revision = setup(&pool, case).await;
        let package = content(case);
        let request = request(case, &package, Uuid::new_v4(), revision);
        let result = apply_existing_login_rewards(&pool, &request, clock())
            .await
            .unwrap();
        let expected = &case["expected"];
        assert!(!result.replayed);
        assert_eq!(
            result.flags.is_dormant,
            expected["flags"]["is_dormant"].as_bool().unwrap()
        );
        assert_eq!(
            result.flags.is_return,
            expected["flags"]["is_return"].as_bool().unwrap()
        );
        let observed = state(&pool).await;
        assert_eq!(observed.1, number(&expected["since"]));
        assert_eq!(observed.2, number(&expected["event_date"]));
        assert_eq!(
            observed.3,
            expected["friend_flag"].as_i64().map(|v| v as i32)
        );
        assert_eq!(observed.0, result.revision);
        let rows = expected["rewards"].as_array().unwrap();
        assert_eq!(result.queued_row_ids.len(), rows.len());
        for (id, expected) in result.queued_row_ids.iter().zip(rows) {
            let item = characters::read_queued_items(&pool, *id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(item.cid, number(&expected["cid"]).unwrap());
            assert_eq!(
                hex(item.item_class_ex.as_bytes()),
                expected["item"].as_str().unwrap()
            );
            assert_eq!(item.count, expected["count"].as_i64().unwrap() as i32);
            assert!(item.is_character_binded);
            assert_eq!(
                hex(item.mail_title.as_ref().unwrap().as_bytes()),
                expected["title"].as_str().unwrap()
            );
            assert_eq!(
                hex(item.mail_content.as_ref().unwrap().as_bytes()),
                expected["content"].as_str().unwrap()
            );
            assert_eq!(
                item.log_date.unwrap().units300(),
                number(&expected["clock"]).unwrap()
            );
            assert_eq!((item.color1, item.color2, item.color3), (-1, -1, -1));
            assert_eq!((item.reduced_durability, item.max_durability_bonus), (0, 0));
            assert!(!item.processed);
            assert!(
                item.item_id.is_none()
                    && item.event_code.is_none()
                    && item.processed_time.is_none()
                    && item.expire_time.is_none()
            );
        }
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_login_reward_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        48
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_dormant_item_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_concurrent_retry_is_historical_after_deletion() {
    let (admin, pool, schema) = isolated_game_pool().await;
    dormant_items::initialize_identity(&pool, None)
        .await
        .unwrap();
    let case = active_case();
    let revision = setup(&pool, &case).await;
    let package = Arc::new(content(&case));
    let id = Uuid::new_v4();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let (pool, case, package, calls) =
            (pool.clone(), case.clone(), package.clone(), calls.clone());
        tasks.push(tokio::spawn(async move {
            let request = request(&case, &package, id, revision);
            let mut n = 0;
            apply_existing_login_rewards(&pool, &request, || {
                calls.fetch_add(1, Ordering::SeqCst);
                let date = SqlDateTime::from_units300(100 * DAY + n)?;
                n += 1;
                Ok(date)
            })
            .await
            .unwrap()
        }));
    }
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert_eq!(results.iter().filter(|r| !r.replayed).count(), 1);
    let first = &results[0];
    for result in &results {
        assert_eq!(result.queued_row_ids, first.queued_row_ids);
        assert_eq!(result.revision, first.revision);
        assert_eq!(result.flags, first.flags);
    }
    assert_eq!(calls.load(Ordering::SeqCst), first.queued_row_ids.len());
    let sequence: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_queued_item_row_id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE game_queued_items SET processed=true")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_queued_items")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_characters")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE game_users SET is_dormant_user_units300=NULL,is_dormant_user_event_units300=NULL WHERE id=7").execute(&pool).await.unwrap();
    let replay =
        apply_existing_login_rewards(&pool, &request(&case, &package, id, revision), || {
            Err(Error::LegacyClockOverflow)
        })
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.queued_row_ids, first.queued_row_ids);
    assert_eq!(replay.revision, first.revision);
    assert_eq!(replay.flags, first.flags);
    assert_eq!(
        sqlx::query_as::<_, (i64, bool)>(
            "SELECT last_value,is_called FROM game_queued_item_row_id"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        sequence
    );
    assert_eq!(state(&pool).await.4, 0);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_replay_binds_request_and_actual_content() {
    let (admin, pool, schema) = isolated_game_pool().await;
    dormant_items::initialize_identity(&pool, None)
        .await
        .unwrap();
    let case = active_case();
    let revision = setup(&pool, &case).await;
    let package = content(&case);
    let id = Uuid::new_v4();
    let original =
        apply_existing_login_rewards(&pool, &request(&case, &package, id, revision), clock())
            .await
            .unwrap();
    let before = state(&pool).await;
    for kind in 0..11 {
        let mut altered = content(&case);
        match kind {
            0 => altered.rewards[0].num += 1,
            1 => altered.rewards.swap(0, 1),
            2 => altered.rewards[0].feature = Some("!b".into()),
            3 => altered.manifest.server_code += 1,
            9 => {
                altered.policy = vindictus_server::domain::feature_policy::FeaturePolicy::new([(
                    "CurrentVer".into(),
                    Some("3.09.00".into()),
                )])
                .unwrap()
            }
            10 => {
                altered.rewards[0].item_class =
                    vindictus_server::domain::legacy_user_data::LegacyBytes::from_bytes(vec![0, 0])
                        .unwrap()
            }
            _ => {}
        }
        let mut r = request(&case, &altered, id, revision);
        match kind {
            4 => r.uid = 8,
            5 => r.name = "other_user",
            6 => r.expected_revision += 1,
            7 => {
                r.clocks.set_event_now =
                    SqlDateTime2::from_ticks(r.clocks.set_event_now.ticks() + 1).unwrap()
            }
            8 => r.loyalty_insert_now = SqlDateTime::from_units300(1).unwrap(),
            _ => {}
        }
        assert!(
            matches!(
                apply_existing_login_rewards(&pool, &r, || panic!("conflict cannot sample clock"))
                    .await,
                Err(Error::LoginEffectReplayConflict)
            ),
            "kind={kind}"
        );
        assert_eq!(state(&pool).await, before);
    }
    let mut invalid = content(&case);
    invalid.manifest.catalog_rows += 1;
    assert!(matches!(
        apply_existing_login_rewards(
            &pool,
            &request(&case, &invalid, Uuid::new_v4(), original.revision),
            clock()
        )
        .await,
        Err(Error::RewardPolicyExport)
    ));
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_revision_has_one_concurrent_winner() {
    let (admin, pool, schema) = isolated_game_pool().await;
    dormant_items::initialize_identity(&pool, None)
        .await
        .unwrap();
    let case = active_case();
    let revision = setup(&pool, &case).await;
    let package = Arc::new(content(&case));
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let (pool, case, package) = (pool.clone(), case.clone(), package.clone());
        tasks.push(tokio::spawn(async move {
            apply_existing_login_rewards(
                &pool,
                &request(&case, &package, Uuid::new_v4(), revision),
                clock(),
            )
            .await
        }));
    }
    let (mut winners, mut stale) = (0, 0);
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => winners += 1,
            Err(Error::GameUserRevisionConflict) => stale += 1,
            other => panic!("unexpected CAS result: {other:?}"),
        }
    }
    assert_eq!((winners, stale), (1, 15));
    let state = state(&pool).await;
    assert_eq!(state.5, 1);
    assert_eq!(
        state.4,
        case["expected"]["rewards"].as_array().unwrap().len() as i64
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_late_failure_rolls_back_batch_but_not_identity() {
    let (admin, pool, schema) = isolated_game_pool().await;
    dormant_items::initialize_identity(&pool, None)
        .await
        .unwrap();
    let case = active_case();
    let revision = setup(&pool, &case).await;
    let package = content(&case);
    let before = state(&pool).await;
    let id = Uuid::new_v4();
    // Fail the final receipt update, after all account, loyalty and queue writes.
    sqlx::query("ALTER TABLE game_login_reward_runs ADD CONSTRAINT injected_failure CHECK(revision_after IS NULL)").execute(&pool).await.unwrap();
    assert!(matches!(
        apply_existing_login_rewards(&pool, &request(&case, &package, id, revision), clock()).await,
        Err(Error::Database(_))
    ));
    assert_eq!(state(&pool).await, before);
    let count = case["expected"]["rewards"].as_array().unwrap().len() as i64;
    assert_eq!(
        sqlx::query_as::<_, (i64, bool)>(
            "SELECT last_value,is_called FROM game_queued_item_row_id"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (count, true)
    );
    sqlx::query("ALTER TABLE game_login_reward_runs DROP CONSTRAINT injected_failure")
        .execute(&pool)
        .await
        .unwrap();
    let result =
        apply_existing_login_rewards(&pool, &request(&case, &package, id, revision), clock())
            .await
            .unwrap();
    assert!(!result.replayed);
    assert_eq!(result.queued_row_ids[0] as i64, count + 1);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_missing_identity_clock_and_overflow_refuse_all_writes() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let case = active_case();
    let revision = setup(&pool, &case).await;
    let package = content(&case);
    let before = state(&pool).await;
    assert!(matches!(
        apply_existing_login_rewards(
            &pool,
            &request(&case, &package, Uuid::new_v4(), revision),
            clock()
        )
        .await,
        Err(Error::QueueIdentityUninitialized)
    ));
    assert_eq!(state(&pool).await, before);
    dormant_items::initialize_identity(&pool, None)
        .await
        .unwrap();
    let mut n = 0;
    assert!(matches!(
        apply_existing_login_rewards(
            &pool,
            &request(&case, &package, Uuid::new_v4(), revision),
            || {
                n += 1;
                if n == 3 {
                    Err(Error::LegacyClockOverflow)
                } else {
                    Ok(SqlDateTime::from_units300(0).unwrap())
                }
            }
        )
        .await,
        Err(Error::LegacyClockOverflow)
    ));
    assert_eq!(n, 3);
    assert_eq!(state(&pool).await, before);
    sqlx::query("SELECT setval('game_queued_item_row_id',2147483647,true)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        apply_existing_login_rewards(
            &pool,
            &request(&case, &package, Uuid::new_v4(), revision),
            clock()
        )
        .await,
        Err(Error::Database(_))
    ));
    assert_eq!(state(&pool).await, before);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_cancelled_wait_releases_claim_and_transaction() {
    let (admin, pool, schema) = isolated_game_pool().await;
    dormant_items::initialize_identity(&pool, None)
        .await
        .unwrap();
    let case = active_case();
    let revision = setup(&pool, &case).await;
    let package = Arc::new(content(&case));
    let before = state(&pool).await;
    let id = Uuid::new_v4();
    let mut blocker = pool.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM game_users WHERE id=7 FOR UPDATE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let marker = format!("reward_cancel_{}", Uuid::new_v4().simple());
    let search_path = format!("SET search_path TO {schema}");
    let connection_marker = marker.clone();
    let waiting_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            let search_path = search_path.clone();
            let marker = connection_marker.clone();
            Box::pin(async move {
                sqlx::query(&search_path).execute(&mut *connection).await?;
                sqlx::query("SELECT set_config('application_name',$1,false)")
                    .bind(&marker)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&std::env::var("TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let (task_pool, task_case, task_package) =
        (waiting_pool.clone(), case.clone(), package.clone());
    let task = tokio::spawn(async move {
        apply_existing_login_rewards(
            &task_pool,
            &request(&task_case, &task_package, id, revision),
            clock(),
        )
        .await
    });
    // Allow transaction setup to finish; cancellation still requires a real wait on this blocker.
    // This observer deadline does not change production lock/statement deadlines (2s/5s).
    let observed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND $2::integer=ANY(pg_blocking_pids(pid)))")
                .bind(&marker).bind(blocker_pid).fetch_one(&admin).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await;
    assert!(observed.is_ok());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    // Acquiring pool and replaying the UUID also proves SQLx's rollback-on-drop completed.
    assert_eq!(state(&pool).await, before);
    let result =
        apply_existing_login_rewards(&pool, &request(&case, &package, id, revision), clock())
            .await
            .unwrap();
    assert!(!result.replayed);
    waiting_pool.close().await;
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_readiness_and_lazy_disabled_features() {
    let (admin, pool, schema) = isolated_game_pool().await;
    postgres::check_ready(&pool).await.unwrap();
    let mut case = active_case();
    case["friend"] = json!(false);
    case["dormant"] = json!(false);
    case["v2"] = json!(false);
    let revision = setup(&pool, &case).await;
    let mut package = content(&case);
    package.policy = vindictus_server::domain::feature_policy::FeaturePolicy::new([
        ("CurrentVer".into(), Some("3.09.00".into())),
        ("DormantAccountInterval".into(), Some("2147483648".into())),
        (
            "DormantNewAccountInterval".into(),
            Some("2147483648".into()),
        ),
        (
            "friend_loyalty_return_interval".into(),
            Some("2147483648".into()),
        ),
    ])
    .unwrap();
    let before = state(&pool).await;
    let result = apply_existing_login_rewards(
        &pool,
        &request(&case, &package, Uuid::new_v4(), revision),
        || panic!("disabled features cannot reward"),
    )
    .await
    .unwrap();
    assert_eq!(result.revision, revision);
    assert!(!result.flags.is_dormant && !result.flags.is_return);
    assert!(result.queued_row_ids.is_empty());
    let after = state(&pool).await;
    assert_eq!(
        (before.0, before.1, before.2, before.3, before.4),
        (after.0, after.1, after.2, after.3, after.4)
    );
    assert_eq!(after.5, before.5 + 1);
    sqlx::query("UPDATE game_users SET name='' WHERE id=7")
        .execute(&pool)
        .await
        .unwrap();
    let revision = state(&pool).await.0;
    let mut empty_name = request(&case, &package, Uuid::new_v4(), revision);
    empty_name.name = "   ";
    let empty_result = apply_existing_login_rewards(&pool, &empty_name, || panic!("no rewards"))
        .await
        .unwrap();
    assert_eq!(empty_result.revision, revision);
    sqlx::query("ALTER TABLE game_login_reward_runs RENAME COLUMN queued_row_ids TO broken")
        .execute(&pool)
        .await
        .unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn postgres_login_rewards_upgrade_nine_to_ten_preserves_queue_and_user() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let case = active_case();
    let revision = setup(&pool, &case).await;
    dormant_items::initialize_identity(&pool, None)
        .await
        .unwrap();
    let package = content(&case);
    let result = apply_existing_login_rewards(
        &pool,
        &request(&case, &package, Uuid::new_v4(), revision),
        clock(),
    )
    .await
    .unwrap();
    let before = state(&pool).await;
    let sequence: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_queued_item_row_id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::raw_sql(
        "DROP TABLE game_pet_skills,game_pets,game_pet_feeds,game_pet_ledger;DELETE FROM _sqlx_migrations WHERE version=50;DROP TABLE content_edits,content_versions;DELETE FROM _sqlx_migrations WHERE version IN (49,48,47,46,45);DROP INDEX accounts_game_identity;ALTER TABLE accounts DROP COLUMN game_name;DELETE FROM _sqlx_migrations WHERE version=44;ALTER TABLE accounts DROP COLUMN status;DELETE FROM _sqlx_migrations WHERE version=43;DROP TABLE admin_sessions,admin_audit_events;DROP FUNCTION admin_audit_events_append_only();ALTER TABLE accounts DROP COLUMN panel_role,DROP COLUMN failed_logins,DROP COLUMN locked_until,DROP COLUMN password_changed_at;DELETE FROM _sqlx_migrations WHERE version=42;DROP TABLE game_channel_buffs;DELETE FROM _sqlx_migrations WHERE version=41;DROP TABLE game_daily_item_process;DELETE FROM _sqlx_migrations WHERE version=40;DROP TABLE game_cash_wishlist,game_cash_orders,game_cash_balances;DELETE FROM _sqlx_migrations WHERE version=39;DROP TABLE game_skill_enhances;DELETE FROM _sqlx_migrations WHERE version=38;DROP TABLE game_character_journal01,game_character_journal02;DELETE FROM _sqlx_migrations WHERE version=37;DROP TABLE game_quests,game_quest_goal_achievements,game_quest_clear_times;DELETE FROM _sqlx_migrations WHERE version=36;DROP TABLE game_story_saves,game_story_variables;DELETE FROM _sqlx_migrations WHERE version=35;DROP TABLE game_story_progress,game_story_guides;DELETE FROM _sqlx_migrations WHERE version=34;DROP TABLE game_minimum_character_initialization;DELETE FROM _sqlx_migrations WHERE version=33;DROP TABLE game_mail,game_mail_preservation;DELETE FROM _sqlx_migrations WHERE version=32;DROP TABLE game_quick_slots,game_expanded_slots,game_manufacture,game_manufacture_recipes,game_new_recipes,game_item_loader_preservation;DELETE FROM _sqlx_migrations WHERE version=31;DROP TABLE game_newbie_attendance_info,game_newbie_attendance_preservation;DELETE FROM _sqlx_migrations WHERE version=30;DROP TABLE game_character_transfer_info,game_character_transfer_guard;DELETE FROM _sqlx_migrations WHERE version=29;DROP TABLE game_user_captcha_native_state;DELETE FROM _sqlx_migrations WHERE version=28;DROP TABLE game_user_captcha,game_user_captcha_preservation;DELETE FROM _sqlx_migrations WHERE version=27;DROP TABLE game_status_effects,game_status_effect_preservation;DELETE FROM _sqlx_migrations WHERE version=26;DROP TABLE game_vocation_skills,game_vocations,game_vocation_preservation;DELETE FROM _sqlx_migrations WHERE version=25;DROP TABLE game_shared_items,game_shared_storage_status,game_shared_item_preservation;DROP SEQUENCE game_shared_storage_id;DELETE FROM _sqlx_migrations WHERE version=24;DROP TABLE game_item_log01,game_item_log02,game_item_ledger01,game_item_ledger02;DELETE FROM _sqlx_migrations WHERE version=23;DROP TABLE game_item_journal01,game_item_journal02,game_journal_identity;DROP SEQUENCE game_item_journal_id,game_character_journal_id;DELETE FROM _sqlx_migrations WHERE version=22;DROP TABLE game_character_skill_log01,game_character_skill_log02,game_character_title_log01,game_character_title_log02;DROP FUNCTION character_event_log_ticks(BIGINT);DELETE FROM _sqlx_migrations WHERE version=21;DROP TABLE game_titles,game_title_goal_progress,game_title_presets,game_micro_play_status,game_character_mission_info,game_mission_assignments,game_character_state_preservation;DELETE FROM _sqlx_migrations WHERE version=20;DROP TABLE game_skills,game_sp_skills,game_skill_identity;DELETE FROM _sqlx_migrations WHERE version=19;DROP TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status,game_item_identity;DROP FUNCTION item_attribute_ascii_key(bytea);DELETE FROM _sqlx_migrations WHERE version=18;DROP TABLE game_costumes,game_costume_identity;DELETE FROM _sqlx_migrations WHERE version=17;DROP TABLE game_character_ledger01,game_character_ledger02;DELETE FROM _sqlx_migrations WHERE version=16;DROP TABLE game_item_collections,game_quest_collections,game_mailboxes,game_story_entries,game_my_settings;DELETE FROM _sqlx_migrations WHERE version=15;DROP TABLE game_reserved_names,game_reserved_titles,game_character_name_logs;DELETE FROM _sqlx_migrations WHERE version=14;DROP TABLE game_lab_account_origins;DELETE FROM _sqlx_migrations WHERE version=13;DROP TABLE game_home_cafe_bonus;DELETE FROM _sqlx_migrations WHERE version=12;DROP TABLE game_user_care_info,game_user_products,game_user_info;DELETE FROM _sqlx_migrations WHERE version=11;DROP TABLE game_login_reward_runs;DELETE FROM _sqlx_migrations WHERE version=10",
    )
    .execute(&pool)
    .await
    .unwrap();
    let old = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            postgres::MIGRATOR
                .iter()
                .filter(|m| m.version <= 9)
                .cloned()
                .collect(),
        ),
        ..postgres::MIGRATOR
    };
    old.run(&pool).await.unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    postgres::migrate(&pool).await.unwrap();
    postgres::check_ready(&pool).await.unwrap();
    postgres::migrate(&pool).await.unwrap();
    let after = state(&pool).await;
    assert_eq!(
        (before.0, before.1, before.2, before.3, before.4),
        (after.0, after.1, after.2, after.3, after.4)
    );
    assert_eq!(after.5, 0);
    assert_eq!(after.4, result.queued_row_ids.len() as i64);
    assert_eq!(
        sqlx::query_as::<_, (i64, bool)>(
            "SELECT last_value,is_called FROM game_queued_item_row_id"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        sequence
    );
    cleanup_game_pool(admin, pool, schema).await;
}
