use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::{
    application::{
        game_users::provision_game_user,
        lab_login::{PolicyClock, evaluate_existing_lab_user},
        second_password::*,
        session_registry::SessionRegistry,
    },
    domain::{feature_policy::FeaturePolicy, legacy_user_data::SqlDateTime2, second_password::*},
    error::Error,
    infrastructure::second_password::{self as store, StorageError, UserKey},
    protocol::identity::*,
};
fn identity() -> IdentityProfile {
    IdentityProfile {
        architecture: FrameworkArchitecture::X86,
        culture: AsciiCulture::Invariant,
    }
}
fn clock() -> ClockSamples {
    let n = SqlDateTime2::from_ticks(638000000000000000).unwrap();
    ClockSamples {
        operation_now: n,
        retry_compare_now: n,
        retry_remaining_now: n,
    }
}
fn policy(ip: bool) -> FeaturePolicy {
    policy_with_essential(ip, true)
}
fn policy_with_essential(ip: bool, essential: bool) -> FeaturePolicy {
    FeaturePolicy::new(
        [
            ("CurrentVer", Some("3.01.01")),
            ("SecondPassword", Some("1")),
            (
                "EssentialSecondPassword",
                if essential { Some("1") } else { None },
            ),
            ("SecondPasswordClear", Some("1")),
            ("SecondPassword_UsingSHA", Some("0")),
            ("SecondPassword_IP_Limit", if ip { Some("1") } else { None }),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.map(str::to_owned))),
    )
    .unwrap()
}
async fn row(pool: &sqlx::PgPool, id: i32) -> (Option<Vec<u8>>, Option<i32>, Option<i64>, i64) {
    sqlx::query_as("SELECT second_password_utf16,second_password_fail_count,second_password_last_fail_ticks,local_revision FROM game_users WHERE id=$1").bind(id).fetch_one(pool).await.unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn second_password_first_query_preserves_captured_feedback_and_initializes_once() {
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 61, "PasswordLab", 1, 0)
        .await
        .unwrap();
    sqlx::query("UPDATE game_users SET second_password_fail_count=5,second_password_last_fail_ticks=$1 WHERE id=61").bind(clock().operation_now.ticks()).execute(&pool).await.unwrap();
    let before = row(&pool, 61).await;
    let key = UserKey {
        uid: 61,
        name: "passwordlab",
        revision: before.3,
    };
    let (q, revision) = store::query(&pool, &key, clock()).await.unwrap();
    assert_eq!(
        q,
        QueryResult {
            restricted_until: Some(
                SqlDateTime2::from_ticks(clock().operation_now.ticks() + 6_000_000_000).unwrap()
            ),
            first: true,
            has_password: false,
            fail_count: 5,
            retry_seconds: 600
        }
    );
    assert_eq!(
        row(&pool, 61).await,
        (Some(vec![]), Some(0), None, revision)
    );
    let key = UserKey {
        uid: 61,
        name: "PASSWORDLAB",
        revision,
    };
    let (q, after) = store::query(&pool, &key, clock()).await.unwrap();
    assert!(!q.first);
    assert_eq!(q.fail_count, 0);
    assert_eq!(after, revision);
    assert!(matches!(
        store::query(
            &pool,
            &UserKey {
                uid: 61,
                name: "DifferentLab",
                revision
            },
            clock()
        )
        .await,
        Err(StorageError::Storage(Error::GameUserIdConflict))
    ));
    assert!(matches!(
        store::query(
            &pool,
            &UserKey {
                uid: 61,
                name: "PasswordLab",
                revision: revision - 1
            },
            clock()
        )
        .await,
        Err(StorageError::Storage(Error::GameUserRevisionConflict))
    ));
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn second_password_legacy_registration_five_failures_retry_and_rehash_preserve_source_fields()
{
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 62, "MiXeDLab", 1, 0)
        .await
        .unwrap();
    let (_, mut revision) = store::query(
        &pool,
        &UserKey {
            uid: 62,
            name: "MiXeDLab",
            revision: 0,
        },
        clock(),
    )
    .await
    .unwrap();
    let pw: Vec<u16> = "123456".encode_utf16().collect();
    revision = store::change(
        &pool,
        &UserKey {
            uid: 62,
            name: "MiXeDLab",
            revision,
        },
        &pw,
        HashProfile::Legacy(identity()),
        clock(),
    )
    .await
    .unwrap();
    assert_eq!(
        row(&pool, 62).await.0.unwrap(),
        *hash("MiXeDLab", &pw, HashProfile::Legacy(identity())).unwrap()
    );
    for count in 1..=5 {
        let (r, v) = store::check(
            &pool,
            &UserKey {
                uid: 62,
                name: "MiXeDLab",
                revision,
            },
            &[57],
            HashProfile::Legacy(identity()),
            identity(),
            clock(),
        )
        .await
        .unwrap();
        revision = v;
        assert!(!r.passed);
        assert_eq!(r.fail_count, count);
        assert_eq!(r.retry_seconds, if count == 5 { 600 } else { -1 });
    }
    let before = row(&pool, 62).await;
    assert!(matches!(
        store::check(
            &pool,
            &UserKey {
                uid: 62,
                name: "MiXeDLab",
                revision
            },
            &pw,
            HashProfile::Sha512,
            identity(),
            clock()
        )
        .await,
        Err(StorageError::Domain(SecondPasswordError::Locked))
    ));
    assert_eq!(row(&pool, 62).await, before);
    let mut elapsed = clock();
    let now = SqlDateTime2::from_ticks(elapsed.operation_now.ticks() + 6000000000).unwrap();
    elapsed.operation_now = now;
    elapsed.retry_compare_now = now;
    elapsed.retry_remaining_now = now;
    let (r, v) = store::check(
        &pool,
        &UserKey {
            uid: 62,
            name: "MiXeDLab",
            revision,
        },
        &[57],
        HashProfile::Sha512,
        identity(),
        elapsed,
    )
    .await
    .unwrap();
    assert_eq!(r.fail_count, 1);
    revision = v;
    for target in [
        HashProfile::Sha512,
        HashProfile::AccountSha512,
        HashProfile::Legacy(identity()),
    ] {
        let (r, v) = store::check(
            &pool,
            &UserKey {
                uid: 62,
                name: "MiXeDLab",
                revision,
            },
            &pw,
            target,
            identity(),
            elapsed,
        )
        .await
        .unwrap();
        assert!(r.passed);
        assert_eq!(r.fail_count, 0);
        revision = v;
        assert_eq!(
            row(&pool, 62).await,
            (
                Some((*hash("MiXeDLab", &pw, target).unwrap()).clone()),
                Some(0),
                None,
                revision
            )
        );
    }
    revision = store::change(
        &pool,
        &UserKey {
            uid: 62,
            name: "MiXeDLab",
            revision,
        },
        &[],
        HashProfile::Legacy(identity()),
        elapsed,
    )
    .await
    .unwrap();
    assert_eq!(
        row(&pool, 62).await,
        (Some(vec![]), Some(0), None, revision)
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn second_password_concurrent_writes_serialize_and_invalid_values_do_not_mutate() {
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 63, "ConcurrentPasswordLab", 1, 0)
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..2 {
        let p = pool.clone();
        tasks.spawn(async move {
            store::change(
                &p,
                &UserKey {
                    uid: 63,
                    name: "ConcurrentPasswordLab",
                    revision: 0,
                },
                &[49, 50, 51],
                HashProfile::AccountSha512,
                clock(),
            )
            .await
        });
    }
    let mut success = 0;
    let mut stale = 0;
    while let Some(r) = tasks.join_next().await {
        match r.unwrap() {
            Ok(_) => success += 1,
            Err(StorageError::Storage(Error::GameUserRevisionConflict)) => stale += 1,
            Err(_) => panic!("unexpected storage error"),
        }
    }
    assert_eq!((success, stale), (1, 1));
    let before = row(&pool, 63).await;
    assert!(matches!(
        store::change(
            &pool,
            &UserKey {
                uid: 63,
                name: "ConcurrentPasswordLab",
                revision: before.3
            },
            &[49; 129],
            HashProfile::Legacy(identity()),
            clock()
        )
        .await,
        Err(StorageError::Domain(SecondPasswordError::Input))
    ));
    assert_eq!(row(&pool, 63).await, before);
    sqlx::query("UPDATE game_users SET second_password_fail_count=-1 WHERE id=63")
        .execute(&pool)
        .await
        .unwrap();
    let before = row(&pool, 63).await;
    assert!(matches!(
        store::check(
            &pool,
            &UserKey {
                uid: 63,
                name: "ConcurrentPasswordLab",
                revision: before.3
            },
            &[57],
            HashProfile::Sha512,
            identity(),
            clock()
        )
        .await,
        Err(StorageError::Domain(SecondPasswordError::Counter))
    ));
    assert_eq!(row(&pool, 63).await, before);
    cleanup_game_pool(admin, pool, schema).await;
}
async fn admitted(
    pool: &sqlx::PgPool,
    registry: &SessionRegistry,
) -> (
    vindictus_server::application::frontend_session::FrontendSession,
    i64,
) {
    let name = "PasswordSessionLab";
    let admission = super::lab_admission();
    let id = derive_lab_identity(Some(name), admission.identity_options)
        .unwrap()
        .nexon_sn;
    provision_game_user(pool, id, name, 1, 0).await.unwrap();
    let revision: i64 = sqlx::query_scalar("SELECT local_revision FROM game_users WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    let (mut f, t) = super::begin_lab(registry, name, &admission);
    let n = clock().operation_now;
    let r = evaluate_existing_lab_user(
        &mut f,
        t,
        pool,
        revision,
        PolicyClock {
            application: vindictus_server::domain::login_policy::ApplicationClockSamples {
                set_lock_now: n,
                check_lock_now: n,
            },
            sql_local_now: n.sql_parameter().unwrap(),
        },
        10,
    )
    .await
    .unwrap();
    (f, r.revision)
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn second_password_admitted_owner_essential_gates_closed_session_and_ip_limit_are_enforced() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let registry = SessionRegistry::with_capacity(8).unwrap();
    let (mut frontend, revision) = admitted(&pool, &registry).await;
    let config = PasswordConfig::recovered(&policy(false), identity()).unwrap();
    let mut session = PasswordSession::new(&frontend, revision, config).unwrap();
    assert!(!session.selection_allowed(&frontend));
    let uid = derive_lab_identity(
        Some("PasswordSessionLab"),
        super::lab_admission().identity_options,
    )
    .unwrap()
    .nexon_sn;
    sqlx::query("UPDATE game_users SET second_password_fail_count=5,second_password_last_fail_ticks=$1 WHERE id=$2").bind(clock().operation_now.ticks()).bind(uid).execute(&pool).await.unwrap();
    let revision: i64 = sqlx::query_scalar("SELECT local_revision FROM game_users WHERE id=$1")
        .bind(uid)
        .fetch_one(&pool)
        .await
        .unwrap();
    session = PasswordSession::new(
        &frontend,
        revision,
        PasswordConfig::recovered(&policy(false), identity()).unwrap(),
    )
    .unwrap();
    let first = session.query(&frontend, &pool, clock()).await.unwrap();
    assert!(first.first);
    assert_eq!(first.retry_seconds, 600);
    assert_eq!(row(&pool, uid).await.1, Some(0));
    let repeated = session.query(&frontend, &pool, clock()).await.unwrap();
    assert!(!repeated.first);
    assert!(!repeated.has_password);
    assert_eq!(repeated.retry_seconds, -1);
    assert_eq!(repeated.restricted_until, first.restricted_until);
    assert!(matches!(
        session.change(&frontend, &pool, &[49], clock()).await,
        Err(PasswordSessionError::Domain(SecondPasswordError::Locked))
    ));
    assert!(matches!(
        session.check(&frontend, &pool, &[], clock()).await,
        Err(PasswordSessionError::Domain(SecondPasswordError::Locked))
    ));
    let mut expired = clock();
    expired.operation_now = first.restricted_until.unwrap();
    expired.retry_compare_now = expired.operation_now;
    expired.retry_remaining_now = expired.operation_now;
    assert_eq!(session.state(), ClientState::NoPassword);
    assert!(!session.selection_allowed(&frontend));
    let mut optional = PasswordSession::new(
        &frontend,
        session.revision(),
        PasswordConfig::recovered(&policy_with_essential(false, false), identity()).unwrap(),
    )
    .unwrap();
    assert!(!optional.selection_allowed(&frontend));
    optional.query(&frontend, &pool, clock()).await.unwrap();
    assert!(optional.selection_allowed(&frontend));
    let result = optional
        .check(&frontend, &pool, &[49], expired)
        .await
        .unwrap();
    assert!(!result.passed);
    assert!(!optional.selection_allowed(&frontend));
    // A failed optional check changes storage; synchronize a newly admitted password state.
    let updated_revision = optional.revision();
    session = PasswordSession::new(
        &frontend,
        updated_revision,
        PasswordConfig::recovered(&policy(false), identity()).unwrap(),
    )
    .unwrap();
    session.query(&frontend, &pool, expired).await.unwrap();

    session
        .change(&frontend, &pool, &[49, 50, 51, 52, 53, 54], expired)
        .await
        .unwrap();
    assert_eq!(session.state(), ClientState::Passed);
    assert!(session.selection_allowed(&frontend));
    let before = session.revision();
    assert!(
        !session
            .query(&frontend, &pool, clock())
            .await
            .unwrap()
            .first
    );
    assert_eq!(session.revision(), before);
    session
        .change(&frontend, &pool, &[], clock())
        .await
        .unwrap();
    assert!(!session.selection_allowed(&frontend));
    let config = PasswordConfig::recovered(&policy(true), identity()).unwrap();
    let mut limited = PasswordSession::new(&frontend, session.revision(), config).unwrap();
    limited.query(&frontend, &pool, clock()).await.unwrap();
    let before = limited.revision();
    assert!(matches!(
        limited.change(&frontend, &pool, &[49], clock()).await,
        Err(PasswordSessionError::IpLimit)
    ));
    assert_eq!(limited.revision(), before);
    let (other, _) = super::begin_lab(&registry, "OtherPasswordLab", &super::lab_admission());
    assert!(limited.query(&other, &pool, clock()).await.is_err());
    drop(other);
    let different_profile = IdentityProfile {
        architecture: FrameworkArchitecture::X64,
        culture: AsciiCulture::Invariant,
    };
    assert!(matches!(
        PasswordSession::new(
            &frontend,
            session.revision(),
            PasswordConfig::recovered(&policy(false), different_profile).unwrap()
        ),
        Err(PasswordSessionError::Profile)
    ));
    session
        .change(&frontend, &pool, &[49], clock())
        .await
        .unwrap();
    assert!(session.selection_allowed(&frontend));
    frontend.close();
    assert!(!session.selection_allowed(&frontend));
    assert!(session.query(&frontend, &pool, clock()).await.is_err());
    let (mut reconnected, _) = admitted(&pool, &registry).await;
    assert!(!session.selection_allowed(&reconnected));
    assert!(matches!(
        session.query(&reconnected, &pool, clock()).await,
        Err(PasswordSessionError::State)
    ));
    reconnected.close();
    let incomplete = FeaturePolicy::new([("CurrentVer".into(), Some("3.01.01".into()))]).unwrap();
    assert!(matches!(
        PasswordConfig::recovered(&incomplete, identity()),
        Err(PasswordSessionError::Profile)
    ));
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn canceled_second_password_query_does_not_grant_selection_or_mutate_storage() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let registry = SessionRegistry::with_capacity(8).unwrap();
    let (mut frontend, revision) = admitted(&pool, &registry).await;
    let mut session = PasswordSession::new(
        &frontend,
        revision,
        PasswordConfig::recovered(&policy(false), identity()).unwrap(),
    )
    .unwrap();
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(pool.acquire().await.unwrap());
    }
    {
        let future = session.query(&frontend, &pool, clock());
        tokio::pin!(future);
        tokio::select! {_=&mut future=>panic!("pool must remain occupied"),_=tokio::time::sleep(std::time::Duration::from_millis(50))=>{}}
    }
    assert_eq!(session.state(), ClientState::Unknown);
    assert!(!session.selection_allowed(&frontend));
    drop(held);
    assert_eq!(
        row(
            &pool,
            derive_lab_identity(
                Some("PasswordSessionLab"),
                super::lab_admission().identity_options
            )
            .unwrap()
            .nexon_sn
        )
        .await
        .3,
        revision
    );
    assert!(
        session
            .query(&frontend, &pool, clock())
            .await
            .unwrap()
            .first
    );
    frontend.close();
    assert!(!session.selection_allowed(&frontend));
    cleanup_game_pool(admin, pool, schema).await;
}
