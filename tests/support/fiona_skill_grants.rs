type SkillLog = (i64, i32, Vec<u8>, i32, i32, i64, Option<Vec<u8>>);
use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use vindictus_server::{
    application::{character_creation::EntityAuthority, game_users::provision_game_user},
    domain::legacy_user_data::{SqlDateTime, SqlDateTime2},
    infrastructure::{
        character_creation_core::{self, CreationCoreRequest},
        character_lobby::UserKey,
        character_skills,
        fiona_character_state::{self as state, StateError},
        fiona_initializer_profile::Profile,
        fiona_skill_grants::{self as grants, Prepared},
    },
};
mod profile_fixture {
    include!("../fixtures/initializer_profile_support.rs");
}
fn owner() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
fn state_export() -> Value {
    json!({"format_version":1,"source":"study_heroes.character-initialization","titles":[],"goals":[],"presets":[{"title_id":17,"class_restriction":2},{"title_id":18,"class_restriction":0}],"micro":[],"missions":[],"assignments":[]})
}
fn profile() -> Profile {
    let mut raw = profile_fixture::synthetic();
    raw["state_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&state_export()).unwrap())
    ));
    let mut second = raw["skills"][0].clone();
    second["id_utf16"] = json!("530079006e003200");
    second["rank"] = json!(3);
    second["item_callback"] = json!(false);
    raw["skills"].as_array_mut().unwrap().push(second);
    profile_fixture::read(&raw).unwrap()
}
fn prepared() -> Prepared {
    grants::prepare(&profile(), &mut EntityAuthority::new(17), || {
        SqlDateTime2::from_ticks(638000000000000000).unwrap()
    })
    .unwrap()
}
async fn setup(p: &PgPool) {
    provision_game_user(p, 31, "Owner", 900, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(p).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes)VALUES($1,$2)")
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
    let bytes = serde_json::to_vec(&state_export()).unwrap();
    state::preserve(p, &state::read_snapshot(bytes.as_slice()).unwrap(), false)
        .await
        .unwrap();
    let bytes=serde_json::to_vec(&json!({"format_version":1,"source":"study_heroes.skill-state","sp_source_last_value":null,"skills":[],"sp_slots":[]})).unwrap();
    character_skills::preserve(
        p,
        &character_skills::read_snapshot(bytes.as_slice()).unwrap(),
        false,
    )
    .await
    .unwrap();
    character_skills::initialize_sp_slot(p, &owner(), 1)
        .await
        .unwrap();
}
async fn counts(p: &PgPool) -> (i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT COUNT(*) FROM game_skills),(SELECT COUNT(*) FROM game_character_skill_log01)+(SELECT COUNT(*) FROM game_character_skill_log02),(SELECT COUNT(*) FROM game_character_title_log01)+(SELECT COUNT(*) FROM game_character_title_log02)").fetch_one(p).await.unwrap()
}
async fn reject_logs(p: &PgPool) {
    sqlx::query("CREATE FUNCTION reject_init_log()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic log failure';END$$").execute(p).await.unwrap();
    for t in [
        "game_character_skill_log01",
        "game_character_skill_log02",
        "game_character_title_log01",
        "game_character_title_log02",
    ] {
        sqlx::query(&format!("CREATE TRIGGER reject_init_log BEFORE INSERT ON {t} FOR EACH ROW EXECUTE FUNCTION reject_init_log()" )).execute(p).await.unwrap();
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn default_grants_preserve_all_fields_concurrent_replay_sp_and_original_log_clock() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let batch = prepared();
    let key = owner();
    let (x, y) = tokio::join!(
        grants::save(&p, &key, 1, &batch),
        grants::save(&p, &key, 1, &batch)
    );
    let x = x.unwrap();
    let y = y.unwrap();
    assert_eq!(x.inserted + y.inserted, 2);
    assert_eq!(x.logs_written + y.logs_written, 2);
    assert_eq!(x.logs_failed + y.logs_failed, 0);
    assert!(x.runtime_callbacks_required != y.runtime_callbacks_required);
    assert_eq!(counts(&p).await, (2, 2, 0));
    for r in batch.rows() {
        let row: (i64, Vec<u8>, i32, i16, i32) = sqlx::query_as(
            "SELECT cid,skill_utf16,rank,locked,current_ap FROM game_skills WHERE row_id=$1",
        )
        .bind(r.id())
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(row, (1, r.key().to_vec(), r.rank(), 0, 0));
    }
    let log:Vec<SkillLog>=sqlx::query_as("SELECT journal_id,event,skill_utf16,skill_level,ap,time_ticks,legacy_last_update FROM game_character_skill_log01 UNION ALL SELECT journal_id,event,skill_utf16,skill_level,ap,time_ticks,legacy_last_update FROM game_character_skill_log02").fetch_all(&p).await.unwrap();
    for (j, e, k, r, ap, t, version) in log {
        assert_eq!((j, e, ap, version), (0, 1, 0, None));
        assert!(batch.rows().iter().any(|x| x.key() == k && x.rank() == r));
        assert!(SqlDateTime2::from_ticks(t).is_ok());
    }
    let slot: (i32, Option<Vec<u8>>) =
        sqlx::query_as("SELECT slot_id,skill_utf16 FROM game_sp_skills WHERE cid=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(slot, (0, None));
    let f: Value =
        serde_json::from_str(include_str!("../fixtures/sql-character-log-clock.json")).unwrap();
    for c in f["cases"].as_array().unwrap() {
        let h = c["source_binary"].as_str().unwrap();
        let mut bytes = [0; 8];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap();
        }
        let units = SqlDateTime::from_sql_binary(bytes).unwrap().units300();
        let actual: i64 = sqlx::query_scalar("SELECT character_event_log_ticks($1)")
            .bind(units)
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(actual.to_string(), c["converted_ticks"].as_str().unwrap());
    }
    let title = grants::seed_titles(&p, &owner(), 1, &profile())
        .await
        .unwrap();
    assert_eq!(
        (
            title.inserted,
            title.logs_written,
            title.logs_failed,
            title.source_operation_failed
        ),
        (1, 1, 0, false)
    );
    let again = grants::seed_titles(&p, &owner(), 1, &profile())
        .await
        .unwrap();
    assert_eq!(again.inserted, 0);
    assert_eq!(counts(&p).await, (2, 2, 1));
    let log:(i64,i32,i32,i32,i32)=sqlx::query_as("SELECT journal_id,event,title_id,goal_id,value FROM game_character_title_log01 UNION ALL SELECT journal_id,event,title_id,goal_id,value FROM game_character_title_log02").fetch_one(&p).await.unwrap();
    assert_eq!(log, (0, 2, 17, 0, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn failed_event_logs_do_not_abort_skills_or_titles_and_require_runtime_callbacks() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    reject_logs(&p).await;
    let result = grants::save(&p, &owner(), 1, &prepared()).await.unwrap();
    assert_eq!(
        (
            result.inserted,
            result.logs_written,
            result.logs_failed,
            result.source_operation_failed
        ),
        (2, 0, 2, false)
    );
    assert!(result.runtime_callbacks_required);
    let result = grants::seed_titles(&p, &owner(), 1, &profile())
        .await
        .unwrap();
    assert_eq!(
        (
            result.inserted,
            result.logs_written,
            result.logs_failed,
            result.source_operation_failed
        ),
        (1, 0, 1, false)
    );
    assert!(result.runtime_callbacks_required);
    assert_eq!(counts(&p).await, (2, 0, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn failed_skill_submit_rolls_back_skills_but_keeps_independently_committed_source_logs() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let batch = prepared();
    sqlx::query("CREATE FUNCTION reject_skill_submit()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic submit failure';END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_skill_submit BEFORE INSERT ON game_skills FOR EACH ROW WHEN(NEW.rank=3)EXECUTE FUNCTION reject_skill_submit()").execute(&p).await.unwrap();
    assert!(grants::save(&p, &owner(), 1, &batch).await.is_err());
    assert_eq!(counts(&p).await, (0, 2, 0));
    sqlx::query("DROP TRIGGER reject_skill_submit ON game_skills")
        .execute(&p)
        .await
        .unwrap();
    let done = grants::save(&p, &owner(), 1, &batch).await.unwrap();
    assert_eq!(done.inserted, 2);
    assert_eq!(counts(&p).await, (2, 4, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn grant_guards_reject_stale_or_foreign_state_without_logs_or_repairs() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let batch = prepared();
    for (change, restore) in [
        (
            "UPDATE game_character_state_preservation SET initialized=false",
            "UPDATE game_character_state_preservation SET initialized=true",
        ),
        (
            "UPDATE game_skill_identity SET initialized=false",
            "UPDATE game_skill_identity SET initialized=true",
        ),
        (
            "UPDATE game_character_state_preservation SET source_sha256=NULL",
            "UPDATE game_character_state_preservation SET source_sha256=decode('PLACEHOLDER','hex')",
        ),
        (
            "UPDATE game_character_info SET is_premium=true WHERE id=1",
            "UPDATE game_character_info SET is_premium=false WHERE id=1",
        ),
        (
            "UPDATE game_sp_skills SET slot_id=1",
            "UPDATE game_sp_skills SET slot_id=0",
        ),
    ] {
        sqlx::query(change).execute(&p).await.unwrap();
        assert!(grants::save(&p, &owner(), 1, &batch).await.is_err());
        assert_eq!(counts(&p).await, (0, 0, 0));
        let hash = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&state_export()).unwrap())
        );
        sqlx::query(&restore.replace("PLACEHOLDER", &hash))
            .execute(&p)
            .await
            .unwrap();
    }
    let wrong = UserKey {
        revision: 1,
        ..owner()
    };
    assert!(grants::save(&p, &wrong, 1, &batch).await.is_err());
    let done = grants::save(&p, &owner(), 1, &batch).await.unwrap();
    assert_eq!(done.inserted, 2);
    sqlx::query("UPDATE game_skills SET current_ap=1 WHERE rank=3")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        grants::save(&p, &owner(), 1, &batch).await,
        Err(StateError::Conflict)
    ));
    assert_eq!(counts(&p).await, (2, 2, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn cancelled_skill_submit_rolls_back_grants_without_erasing_already_written_event_logs() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::query("CREATE FUNCTION delay_skill_submit()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_advisory_xact_lock(hashtext(current_schema()),74921);PERFORM pg_sleep(20);RETURN NEW;END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER delay_skill_submit BEFORE INSERT ON game_skills FOR EACH ROW WHEN(NEW.rank=3)EXECUTE FUNCTION delay_skill_submit()").execute(&p).await.unwrap();
    let pp = p.clone();
    let batch = Arc::new(prepared());
    let bb = batch.clone();
    let task = tokio::spawn(async move { grants::save(&pp, &owner(), 1, &bb).await });
    tokio::time::timeout(Duration::from_secs(10),async{loop{if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=74921 AND granted)").fetch_one(&p).await.unwrap(){break;}tokio::time::sleep(Duration::from_millis(25)).await;}}).await.unwrap();
    task.abort();
    match task.await {
        Err(e) => assert!(e.is_cancelled()),
        Ok(_) => panic!("grant completed before cancellation"),
    };
    tokio::time::timeout(Duration::from_secs(10),async{loop{if !sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=74921 AND granted)").fetch_one(&p).await.unwrap(){break;}tokio::time::sleep(Duration::from_millis(25)).await;}}).await.unwrap();
    assert_eq!(counts(&p).await, (0, 2, 0));
    sqlx::query("DROP TRIGGER delay_skill_submit ON game_skills")
        .execute(&p)
        .await
        .unwrap();
    let done = grants::save(&p, &owner(), 1, &batch).await.unwrap();
    assert_eq!(done.inserted, 2);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn original_title_preset_storage_failure_is_caught_without_events_or_observable_callback() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::query("CREATE FUNCTION reject_title_seed()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic title failure';END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_title_seed BEFORE INSERT ON game_titles FOR EACH ROW EXECUTE FUNCTION reject_title_seed()").execute(&p).await.unwrap();
    let result = grants::seed_titles(&p, &owner(), 1, &profile())
        .await
        .unwrap();
    assert!(result.source_operation_failed);
    assert!(!result.runtime_callbacks_required);
    assert_eq!(counts(&p).await, (0, 0, 0));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*)FROM game_titles")
            .fetch_one(&p)
            .await
            .unwrap(),
        0
    );
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn default_grant_callbacks_run_per_log_before_submit_and_replay_never_repeats_them() {
    use vindictus_server::domain::skill_runtime::Cache;
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let batch = prepared();
    let mut cache = Cache::default();
    let mut seen = Vec::new();
    let o = grants::save_with_callbacks(&p, &owner(), 1, &batch, |row| {
        seen.push(row.id());
        cache
            .after_grant(row.key(), row.rank(), row.required_ap(), row.arguments())
            .map_err(|_| StateError::Profile)
    })
    .await
    .unwrap();
    assert_eq!(
        seen,
        batch.rows().iter().map(|r| r.id()).collect::<Vec<_>>()
    );
    assert_eq!((o.inserted, o.logs_written, cache.rank_sum()), (2, 2, 10));
    assert!(o.runtime_callbacks_required); // A hook is not a full Stat/AP/observable implementation.
    let o = grants::save_with_callbacks(&p, &owner(), 1, &batch, |_| {
        panic!("replay repeated callbacks")
    })
    .await
    .unwrap();
    assert_eq!(o.inserted, 0);
    assert_eq!(counts(&p).await, (2, 2, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn default_grant_callbacks_failure_stops_before_next_log_and_keeps_prior_cache_and_logs() {
    use vindictus_server::domain::skill_runtime::Cache;
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let batch = prepared();
    let mut cache = Cache::default();
    let mut seen = 0;
    let result = grants::save_with_callbacks(&p, &owner(), 1, &batch, |row| {
        seen += 1;
        cache
            .after_grant(row.key(), row.rank(), row.required_ap(), row.arguments())
            .map_err(|_| StateError::Profile)?;
        Err(StateError::Profile)
    })
    .await;
    assert!(matches!(result, Err(StateError::Profile)));
    assert_eq!((seen, cache.rank_sum()), (1, 4));
    assert_eq!(counts(&p).await, (0, 1, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn default_grant_callbacks_survive_logger_failure_and_precede_late_submit_failure() {
    use vindictus_server::domain::skill_runtime::Cache;
    for reject_submit in [false, true] {
        let (a, p, s) = isolated_game_pool().await;
        setup(&p).await;
        if reject_submit {
            sqlx::query("ALTER TABLE game_skills ADD CONSTRAINT reject_after_callbacks CHECK(false) NOT VALID").execute(&p).await.unwrap();
        } else {
            reject_logs(&p).await;
        }
        let batch = prepared();
        let mut cache = Cache::default();
        let mut seen = 0;
        let result = grants::save_with_callbacks(&p, &owner(), 1, &batch, |row| {
            seen += 1;
            cache
                .after_grant(row.key(), row.rank(), row.required_ap(), row.arguments())
                .map_err(|_| StateError::Profile)
        })
        .await;
        assert_eq!((seen, cache.rank_sum()), (2, 10));
        if reject_submit {
            assert!(matches!(result, Err(StateError::Database(_))));
            assert_eq!(counts(&p).await, (0, 2, 0));
        } else {
            let outcome = result.unwrap();
            assert_eq!((outcome.inserted, outcome.logs_failed), (2, 2));
            assert_eq!(counts(&p).await, (2, 0, 0));
        }
        cleanup_game_pool(a, p, s).await;
    }
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn ordered_multicast_preserves_prefix_on_runtime_failure_and_never_replays_success() {
    use vindictus_server::domain::{
        skill_multicast::{Call, Runtime},
        skill_runtime::Cache,
    };
    struct Hooks {
        fail: bool,
        skills: usize,
        micro: usize,
    }
    impl Runtime for Hooks {
        type Error = StateError;
        fn call(&mut self, call: Call<'_>, cache: &Cache) -> Result<(), StateError> {
            assert!(cache.rank_sum() > 0);
            match call {
                Call::GuildSkillUp { key, rank } => assert_eq!(cache.base_rank(key), rank),
                Call::SkillUpdated => {
                    self.skills += 1;
                    if self.fail {
                        return Err(StateError::Profile);
                    }
                }
                Call::MicroPlayUpdated => self.micro += 1,
                _ => {}
            }
            Ok(())
        }
    }
    for fail in [true, false] {
        let (a, p, s) = isolated_game_pool().await;
        setup(&p).await;
        let batch = prepared();
        let mut cache = Cache::default();
        let mut hooks = Hooks {
            fail,
            skills: 0,
            micro: 0,
        };
        let outcome =
            grants::save_with_ordered_runtime(&p, &owner(), 1, &batch, &mut cache, &mut hooks)
                .await;
        if fail {
            assert!(matches!(outcome, Err(StateError::Profile)));
            assert_eq!((cache.rank_sum(), hooks.skills, hooks.micro), (4, 1, 0));
            assert_eq!(counts(&p).await, (0, 1, 0));
        } else {
            let done = outcome.unwrap();
            assert!(done.runtime_callbacks_required);
            assert_eq!((cache.rank_sum(), hooks.skills, hooks.micro), (10, 2, 2));
            assert_eq!(counts(&p).await, (2, 2, 0));
            hooks.fail = true;
            let replay =
                grants::save_with_ordered_runtime(&p, &owner(), 1, &batch, &mut cache, &mut hooks)
                    .await
                    .unwrap();
            assert_eq!(
                (replay.inserted, cache.rank_sum(), hooks.skills, hooks.micro),
                (0, 10, 2, 2)
            );
        }
        cleanup_game_pool(a, p, s).await;
    }
}
