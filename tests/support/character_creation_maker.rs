use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use vindictus_server::{
    application::{
        character_creation::{
            self as maker, BaseOutcome as B, DormantOutcome as D, EntityAuthority, MakerSettings,
            NormalFionaRequest,
        },
        game_users::provision_game_user,
    },
    domain::{
        character_allocator::make_entity_id,
        character_creation::{CreateResult as R, dormant_parameter},
    },
    infrastructure::{
        character_creation_admission::CreationAdmissionError,
        character_creation_core::{self as core, CreationCoreRequest},
        character_creation_effects::{self as effects, CreationEffectsError as E},
        character_lobby::UserKey,
        lab_clock,
    },
};
fn key() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
fn clock() -> vindictus_server::domain::legacy_user_data::SqlDateTime2 {
    lab_clock::unix_ticks(1767225600, 1666700).unwrap()
}
fn settings() -> MakerSettings {
    MakerSettings {
        stopped: false,
        max_free: 5,
        server_code: -1,
    }
}
async fn setup(p: &sqlx::PgPool) {
    provision_game_user(p, 31, "Owner", 42, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(p).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes) VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(p)
        .await
        .unwrap();
}
async fn logs(p: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM game_character_ledger01)+(SELECT COUNT(*) FROM game_character_ledger02)").fetch_one(p).await.unwrap()
}
async fn base(p: &sqlx::PgPool) -> i64 {
    let proof = common::proof(&common::content());
    let mut a = EntityAuthority::new(77);
    match maker::make_normal_fiona_base(
        p,
        NormalFionaRequest {
            owner: key(),
            proof: &proof,
            name: "Murael",
        },
        &settings(),
        &mut a,
        || Ok(clock()),
    )
    .await
    .unwrap()
    {
        B::Created { cid, .. } => cid,
        _ => panic!("synthetic base refused"),
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn normal_maker_persists_base_dormant_then_uid_name_cleanup_with_cascades() {
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    let proof = common::proof(&common::content());
    let mut authority = EntityAuthority::new(77);
    let mut samples = 0;
    let result = maker::make_normal_fiona_base(
        &p,
        NormalFionaRequest {
            owner: key(),
            proof: &proof,
            name: "Murael",
        },
        &settings(),
        &mut authority,
        || {
            samples += 1;
            Ok(clock())
        },
    )
    .await
    .unwrap();
    let B::Created { cid, serial } = result else {
        panic!("base not created")
    };
    assert_eq!(samples, 3);
    assert_eq!(authority.counter(), cid);
    assert_ne!(serial, -1);
    assert_eq!(logs(&p).await, 1);
    assert_eq!(
        maker::prepare_dormant(&p, &key(), cid, clock())
            .await
            .unwrap(),
        D::Ready {
            ledger_written: true
        }
    );
    assert_eq!(logs(&p).await, 2);
    let row: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(l) FROM game_last_connect_info l WHERE id=$1")
            .bind(cid)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(
        row,
        serde_json::json!({"id":cid,"last_connect_date_units300":dormant_parameter(clock()).unwrap().units300(),"last_cafe_bonus_units300":null,"last_cafe_login_units300":null,"last_cafe_logout_units300":null,"last_home_cafe_login_units300":null,"last_home_cafe_logout_units300":null,"last_logout_units300":null})
    );
    // No Status/SN predicate is introduced into cleanup.
    sqlx::query("UPDATE game_character_info SET status=1,character_sn=-99 WHERE id=$1")
        .bind(cid)
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        effects::cleanup(&p, &key(), &proof, "Murael")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        effects::cleanup(&p, &key(), &proof, "Murael")
            .await
            .unwrap(),
        0
    );
    for table in [
        "game_characters",
        "game_character_info",
        "game_item_collections",
        "game_quest_collections",
        "game_mailboxes",
        "game_story_entries",
        "game_last_connect_info",
    ] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(n, 0)
    }
    assert_eq!(logs(&p).await, 2);
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn maker_rejection_order_and_negative_sql_result_keep_original_outputs_and_audit() {
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    let proof = common::proof(&common::content());
    let mut authority = EntityAuthority::new(77);
    let mut cfg = settings();
    cfg.stopped = true;
    let mut samples = 0;
    for (stopped, max_free, wanted) in [
        (true, 0, R::CreateCharacterStopped),
        (false, 0, R::CharacterLimit),
        (false, 5, R::InvalidName),
    ] {
        cfg.stopped = stopped;
        cfg.max_free = max_free;
        assert_eq!(
            maker::make_normal_fiona_base(
                &p,
                NormalFionaRequest {
                    owner: key(),
                    proof: &proof,
                    name: "!"
                },
                &cfg,
                &mut authority,
                || {
                    samples += 1;
                    Ok(clock())
                }
            )
            .await
            .unwrap(),
            B::Rejected {
                result: wanted,
                cid: -1,
                serial: -1
            }
        );
    }
    assert_eq!(samples, 0);
    assert_eq!(authority.counter(), 0);
    assert_eq!(logs(&p).await, 0);
    // A source CID collision is also returned as DuplicatedName, after advancing the counter.
    let expected = make_entity_id(&mut 0, 77, clock())
        .unwrap()
        .entity_time
        .ticks();
    core::create_base(
        &p,
        key(),
        &proof,
        CreationCoreRequest {
            cid: expected,
            character_sn: 123,
            name: "Murael",
            class: 1,
            use_preset: false,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE game_character_info SET name_utf16=NULL,status=1 WHERE id=$1")
        .bind(expected)
        .execute(&p)
        .await
        .unwrap();
    let result = maker::make_normal_fiona_base(
        &p,
        NormalFionaRequest {
            owner: key(),
            proof: &proof,
            name: "Murael",
        },
        &settings(),
        &mut authority,
        || Ok(clock()),
    )
    .await
    .unwrap();
    assert!(matches!(result,B::Rejected{result:R::DuplicatedName,cid:-1,serial}if serial!=-1));
    assert_eq!(authority.counter(), expected);
    assert_eq!(logs(&p).await, 1);
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn dormant_tolerates_existing_connection_after_logger_failure_but_missing_insert_fails() {
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    let cid = base(&p).await;
    for table in ["game_character_ledger01", "game_character_ledger02"] {
        sqlx::query(&format!("ALTER TABLE {table} ADD CONSTRAINT reject_dormant CHECK(event_utf16<>decode($$44006f0072006d0061006e00740020002d002000430072006500610074006500430068006100720061006300740065007200$$,'hex')) NOT VALID")).execute(&p).await.unwrap();
    }
    assert_eq!(
        maker::prepare_dormant(&p, &key(), cid, clock())
            .await
            .unwrap(),
        D::Ready {
            ledger_written: false
        }
    );
    assert_eq!(logs(&p).await, 1);
    let before: i64 = sqlx::query_scalar(
        "SELECT last_connect_date_units300 FROM game_last_connect_info WHERE id=$1",
    )
    .bind(cid)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(
        maker::prepare_dormant(
            &p,
            &key(),
            cid,
            lab_clock::unix_ticks(1768000000, 0).unwrap()
        )
        .await
        .unwrap(),
        D::Ready {
            ledger_written: false
        }
    );
    let after: i64 = sqlx::query_scalar(
        "SELECT last_connect_date_units300 FROM game_last_connect_info WHERE id=$1",
    )
    .bind(cid)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(before, after);
    sqlx::query("DELETE FROM game_last_connect_info")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query(
        "ALTER TABLE game_last_connect_info ADD CONSTRAINT reject_insert CHECK(id<0) NOT VALID",
    )
    .execute(&p)
    .await
    .unwrap();
    assert_eq!(
        maker::prepare_dormant(&p, &key(), cid, clock())
            .await
            .unwrap(),
        D::InitCharacterFailed
    );
    assert!(!effects::has_connection(&p, &key(), cid).await.unwrap());
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_effects_scope_ambiguous_cleanup_and_cancellation_never_delete_or_overwrite() {
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    let proof = common::proof(&common::content());
    let cid = base(&p).await;
    assert!(matches!(
        effects::cleanup(
            &p,
            &UserKey {
                revision: 1,
                ..key()
            },
            &proof,
            "Murael"
        )
        .await,
        Err(E::Ownership)
    ));
    sqlx::query("UPDATE game_character_info SET status=1,delete_time_units300=NULL WHERE id=$1")
        .bind(cid)
        .execute(&p)
        .await
        .unwrap();
    let mut authority = EntityAuthority::new(78);
    let mut samples = 0;
    assert!(matches!(
        maker::make_normal_fiona_base(
            &p,
            NormalFionaRequest {
                owner: key(),
                proof: &proof,
                name: "Murael"
            },
            &settings(),
            &mut authority,
            || {
                samples += 1;
                Ok(clock())
            }
        )
        .await,
        Err(maker::MakerError::Effects(E::Profile))
    ));
    assert_eq!(samples, 0);
    core::create_base(
        &p,
        key(),
        &proof,
        CreationCoreRequest {
            cid: cid + 1,
            character_sn: 999,
            name: "Murael",
            class: 1,
            use_preset: false,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        effects::cleanup(&p, &key(), &proof, "Murael").await,
        Err(E::Profile)
    ));
    let mut block = p.begin().await.unwrap();
    sqlx::query("LOCK TABLE game_last_connect_info IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *block)
        .await
        .unwrap();
    let cp = p.clone();
    let task = tokio::spawn(async move {
        effects::insert_dormant_connection(&cp, &key(), cid, dormant_parameter(clock()).unwrap())
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    block.rollback().await.unwrap();
    assert!(!effects::has_connection(&p, &key(), cid).await.unwrap());
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_characters")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 2);
    assert!(matches!(
        maker::make_normal_fiona_base(
            &p,
            NormalFionaRequest {
                owner: UserKey {
                    revision: 1,
                    ..key()
                },
                proof: &proof,
                name: "Murael"
            },
            &settings(),
            &mut authority,
            || Err(CreationAdmissionError::Clock)
        )
        .await,
        Err(maker::MakerError::Admission(
            CreationAdmissionError::Ownership
        ))
    ));
    cleanup_game_pool(admin, p, schema).await;
}
