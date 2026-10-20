use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::oneshot;
use vindictus_server::{
    application::{
        creation_lifecycle::{
            CreationRunError, CreationRuntime, InitializationFailure as I,
            UninitializedOutcome as O,
        },
        frontend_session::FrontendSession,
        game_users::provision_game_user,
        lab_login::{PolicyClock, evaluate_existing_lab_user},
        session_registry::SessionRegistry,
    },
    domain::{
        character_creation::CreateResult, feature_policy::FeaturePolicy,
        legacy_user_data::SqlDateTime2, login_policy::ApplicationClockSamples,
    },
    infrastructure::{
        character_creation_core::{self as core, CreationCoreRequest},
        character_lobby::UserKey,
        reward_policy::RewardPolicyPackage,
    },
    protocol::identity::derive_lab_identity,
};
pub(super) fn package() -> RewardPolicyPackage {
    let mut p = common::content();
    p.manifest.server_code = 23;
    p.policy = FeaturePolicy::new(
        [
            ("CurrentVer", Some("3.09.00")),
            ("MaxCharacterSlot", Some("5")),
            ("DormantAccount", Some("1")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.map(str::to_owned))),
    )
    .unwrap();
    p
}
pub(super) fn runtime(pool: &sqlx::PgPool, p: &RewardPolicyPackage) -> CreationRuntime {
    let proof = serde_json::to_vec(&common::value(p)).unwrap();
    CreationRuntime::new(pool.clone(), p, proof.as_slice(), 77, 8).unwrap()
}
pub(super) async fn setup(pool: &sqlx::PgPool) -> (FrontendSession, i64, i32) {
    let registry = SessionRegistry::with_capacity(8).unwrap();
    let admission = super::lab_admission();
    let name = "LifecycleOwner";
    let uid = derive_lab_identity(Some(name), admission.identity_options)
        .unwrap()
        .nexon_sn;
    provision_game_user(pool, uid, name, 900, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=$2").bind(vec![0u8;32]).bind(uid).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes) VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(pool)
        .await
        .unwrap();
    let (mut f, token) = super::begin_lab(&registry, name, &admission);
    let now = SqlDateTime2::from_ticks(638000000000000000).unwrap();
    let r = evaluate_existing_lab_user(
        &mut f,
        token,
        pool,
        0,
        PolicyClock {
            application: ApplicationClockSamples {
                set_lock_now: now,
                check_lock_now: now,
            },
            sql_local_now: now.sql_parameter().unwrap(),
        },
        10,
    )
    .await
    .unwrap();
    (f, r.revision, uid)
}
async fn parents(p: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM game_characters")
        .fetch_one(p)
        .await
        .unwrap()
}
async fn logs(p: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM game_character_ledger01)+(SELECT COUNT(*) FROM game_character_ledger02)").fetch_one(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn admitted_creation_binds_identity_manifest_policy_and_own_authority_then_compensates_failure()
 {
    let (admin, p, schema) = isolated_game_pool().await;
    let package = package();
    let r = runtime(&p, &package);
    let registry = SessionRegistry::with_capacity(8).unwrap();
    let unopened = FrontendSession::new(&registry);
    assert!(matches!(
        r.run_uninitialized_admitted(&unopened, 0, "Murael", |_| async {
            Err(I::InitCharacterFailed)
        })
        .await,
        Err(CreationRunError::Session(_))
    ));
    assert_eq!(parents(&p).await, 0);
    let (f, revision, uid) = setup(&p).await;
    r.set_stopped(true).await;
    assert_eq!(
        r.run_uninitialized_admitted(&f, revision, "Murael", |_| async {
            panic!("stopped initializer ran")
        })
        .await
        .unwrap(),
        O::Rejected(CreateResult::CreateCharacterStopped)
    );
    assert_eq!(logs(&p).await, 0);
    r.set_stopped(false).await;
    let cp = p.clone();
    assert_eq!(r.run_uninitialized_admitted(&f,revision,"Murael",move|base|async move{
 assert_eq!(base.cid&65535,77);assert_eq!(base.serial&255,23);
 let actual:(i32,bool)=sqlx::query_as("SELECT uid,EXISTS(SELECT 1 FROM game_last_connect_info WHERE id=$1) FROM game_character_info WHERE id=$1").bind(base.cid).fetch_one(&cp).await.unwrap();assert_eq!(actual,(uid,true));Err(I::InitCharacterFailed)
 }).await.unwrap(),O::InitializationFailed(I::InitCharacterFailed));
    r.drain().await.unwrap();
    assert_eq!(parents(&p).await, 0);
    assert_eq!(logs(&p).await, 2);
    let mut missing = package;
    missing.policy =
        FeaturePolicy::new([("CurrentVer".to_owned(), Some("3.09.00".to_owned()))]).unwrap();
    let proof = serde_json::to_vec(&common::value(&missing)).unwrap();
    assert!(matches!(
        CreationRuntime::new(p.clone(), &missing, proof.as_slice(), 77, 1),
        Err(CreationRunError::Profile)
    ));
    r.shutdown().await.unwrap();
    assert!(matches!(
        r.run_uninitialized_admitted(&f, revision, "Murael", |_| async {
            Err(I::InitCharacterFailed)
        })
        .await,
        Err(CreationRunError::Runtime)
    ));
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn cancelled_initializer_compensates_after_reconnect_revision_without_rewinding_user() {
    let (admin, p, schema) = isolated_game_pool().await;
    let (f, revision, uid) = setup(&p).await;
    let r = Arc::new(runtime(&p, &package()));
    let (rr, entered) = (Arc::clone(&r), oneshot::channel());
    let (tx, rx) = entered;
    let task = tokio::spawn(async move {
        rr.run_uninitialized_admitted(&f, revision, "Murael", move |base| async move {
            tx.send(base).unwrap();
            std::future::pending().await
        })
        .await
    });
    let base = rx.await.unwrap();
    assert_eq!(parents(&p).await, 1);
    sqlx::query("UPDATE game_users SET service_id=service_id WHERE id=$1")
        .bind(uid)
        .execute(&p)
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    r.drain().await.unwrap();
    assert_eq!(parents(&p).await, 0);
    assert_eq!(logs(&p).await, 2);
    let actual: i64 = sqlx::query_scalar("SELECT local_revision FROM game_users WHERE id=$1")
        .bind(uid)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(actual, revision + 1);
    let connections: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM game_last_connect_info WHERE id=$1")
            .bind(base.cid)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(connections, 0);
    r.shutdown().await.unwrap();
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn cancellation_during_base_wait_retains_worker_until_known_commit_and_cleanup() {
    let (admin, p, schema) = isolated_game_pool().await;
    let (f, revision, _) = setup(&p).await;
    let r = Arc::new(runtime(&p, &package()));
    let mut block = p.begin().await.unwrap();
    sqlx::query("LOCK TABLE game_character_info IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *block)
        .await
        .unwrap();
    let rr = Arc::clone(&r);
    let called = Arc::new(AtomicBool::new(false));
    let cc = Arc::clone(&called);
    let task = tokio::spawn(async move {
        rr.run_uninitialized_admitted(&f, revision, "Murael", move |_| async move {
            cc.store(true, Ordering::SeqCst);
            Err(I::InitCharacterFailed)
        })
        .await
    });
    tokio::time::timeout(Duration::from_secs(1),async{loop{let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE relation='game_character_info'::regclass AND NOT granted)").fetch_one(&p).await.unwrap();if waiting{break}tokio::time::sleep(Duration::from_millis(10)).await}}).await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    block.rollback().await.unwrap();
    r.drain().await.unwrap();
    assert!(!called.load(Ordering::SeqCst));
    assert_eq!(parents(&p).await, 0);
    assert_eq!(logs(&p).await, 1);
    r.shutdown().await.unwrap();
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn cancelled_drain_keeps_workers_and_shutdown_cancels_initializer_without_deadlock() {
    let (admin, p, schema) = isolated_game_pool().await;
    let (f, revision, _) = setup(&p).await;
    let r = Arc::new(runtime(&p, &package()));
    let rr = Arc::clone(&r);
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        rr.run_uninitialized_admitted(&f, revision, "Murael", move |_| async move {
            tx.send(()).unwrap();
            std::future::pending().await
        })
        .await
    });
    rx.await.unwrap();
    let rr = Arc::clone(&r);
    let drain = tokio::spawn(async move { rr.drain().await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    drain.abort();
    assert!(drain.await.unwrap_err().is_cancelled());
    let rr = Arc::clone(&r);
    let drain = tokio::spawn(async move { rr.drain().await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    tokio::time::timeout(Duration::from_secs(5), r.shutdown())
        .await
        .unwrap()
        .unwrap();
    drain.await.unwrap().unwrap();
    assert!(task.await.unwrap().is_err());
    assert_eq!(parents(&p).await, 0);
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn initializer_panic_cleans_base_but_replacement_is_never_deleted_or_failure_hidden() {
    let (admin, p, schema) = isolated_game_pool().await;
    let (f, revision, uid) = setup(&p).await;
    let package = package();
    let r = runtime(&p, &package);
    assert!(matches!(
        r.run_uninitialized_admitted(&f, revision, "Murael", |_| async {
            panic!("synthetic initializer panic")
        })
        .await,
        Err(CreationRunError::Initializer)
    ));
    r.drain().await.unwrap();
    assert_eq!(parents(&p).await, 0);
    let cp = p.clone();
    let proof = common::proof(&package);
    assert!(matches!(
        r.run_uninitialized_admitted(&f, revision, "Murael", move |base| async move {
            sqlx::query("DELETE FROM game_characters WHERE id=$1")
                .bind(base.cid)
                .execute(&cp)
                .await
                .unwrap();
            core::create_base(
                &cp,
                UserKey {
                    uid,
                    name: "LifecycleOwner",
                    revision,
                },
                &proof,
                CreationCoreRequest {
                    cid: base.cid + 65536,
                    character_sn: 999,
                    name: "Murael",
                    class: 1,
                    use_preset: false,
                },
            )
            .await
            .unwrap();
            Err(I::InitCharacterFailed)
        })
        .await,
        Err(CreationRunError::Cleanup)
    ));
    assert_eq!(parents(&p).await, 1);
    assert!(matches!(r.drain().await, Err(CreationRunError::Cleanup)));
    assert!(matches!(r.shutdown().await, Err(CreationRunError::Cleanup)));
    cleanup_game_pool(admin, p, schema).await;
}
