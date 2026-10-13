use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::{
    application::game_users::provision_game_user,
    domain::{home_cafe_bonus::HomeCafeBonus, legacy_user_data::SqlDateTime},
    infrastructure::home_cafe_bonus::{self as storage, HomeCafeError, UserKey},
};
fn snapshot() -> Vec<HomeCafeBonus> {
    storage::read_snapshot(include_bytes!("../fixtures/synthetic-home-cafe-bonus.json").as_slice())
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn home_cafe_preservation_replay_late_conflict_concurrency_and_cancel_are_atomic() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let mut tasks = tokio::task::JoinSet::new();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    for _ in 0..8 {
        let pool = pool.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            storage::preserve(&pool, &snapshot(), false).await
        });
    }
    let mut inserted = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(summary) => {
                inserted += summary.inserted;
                assert_eq!(summary.verified, 2);
            }
            Err(HomeCafeError::Database(sqlx::Error::Database(e)))
                if e.code().as_deref() == Some("55P03") => {}
            _ => panic!("unexpected preservation result"),
        }
    }
    assert_eq!(inserted, 2);
    assert_eq!(
        storage::preserve(&pool, &snapshot(), false)
            .await
            .unwrap()
            .inserted,
        0
    );
    let mut conflict = snapshot();
    conflict.insert(
        0,
        HomeCafeBonus {
            uid: 99,
            cid: 1,
            bonus_type: 9,
            expired_time: SqlDateTime::from_units300(0).unwrap(),
        },
    );
    conflict[2].expired_time = SqlDateTime::from_units300(0).unwrap();
    assert!(matches!(
        storage::preserve(&pool, &conflict, false).await,
        Err(HomeCafeError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_home_cafe_bonus")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        storage::preserve(&pool, &snapshot(), true)
            .await
            .unwrap()
            .verified,
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_users")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(pool.acquire().await.unwrap());
    }
    let waiting = pool.clone();
    let task = tokio::spawn(async move { storage::preserve(&waiting, &snapshot(), false).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    task.abort();
    assert!(matches!(task.await, Err(e) if e.is_cancelled()));
    drop(held);
    assert_eq!(
        storage::preserve(&pool, &snapshot(), true)
            .await
            .unwrap()
            .verified,
        2
    );
    storage::check_ready(&pool).await.unwrap();
    sqlx::query("ALTER TABLE game_home_cafe_bonus DROP COLUMN expired_time_units300")
        .execute(&pool)
        .await
        .unwrap();
    assert!(storage::check_ready(&pool).await.is_err());
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn home_cafe_query_matches_all_sql_cases_and_requires_exact_owner_revision() {
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 44, "HomeCafeLab", 1, 0)
        .await
        .unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/sql-home-cafe-bonus.json")).unwrap();
    for c in fixture["cases"].as_array().unwrap() {
        sqlx::query("DELETE FROM game_home_cafe_bonus")
            .execute(&pool)
            .await
            .unwrap();
        let mask = c["mask"].as_u64().unwrap();
        let expiry =
            SqlDateTime::from_units300(c["row_expiry_units300"].as_str().unwrap().parse().unwrap())
                .unwrap();
        let mut rows = Vec::new();
        for (bit, bonus_type) in [(1, 3), (2, 4), (4, 2)] {
            if mask & bit != 0 {
                rows.push(HomeCafeBonus {
                    uid: 44,
                    cid: 0,
                    bonus_type,
                    expired_time: expiry,
                });
            }
        }
        for (uid, cid, bonus_type) in [(45, 0, 3), (44, 7, 3), (44, 0, 1)] {
            rows.push(HomeCafeBonus {
                uid,
                cid,
                bonus_type,
                expired_time: expiry,
            });
        }
        storage::preserve(&pool, &rows, false).await.unwrap();
        let initial = c["initial_bonus_type"]
            .as_u64()
            .map(|v| u8::try_from(v).unwrap());
        let expected = c["bonus_type"].as_u64().map(|v| u8::try_from(v).unwrap());
        assert_eq!(
            storage::query_account(
                &pool,
                UserKey {
                    uid: 44,
                    name: "HOMECAFELAB ",
                    revision: 0
                },
                initial
            )
            .await
            .unwrap(),
            expected
        );
        assert_eq!(
            storage::preserve(&pool, &rows, true)
                .await
                .unwrap()
                .verified,
            rows.len()
        );
    }
    for (uid, name, revision) in [
        (43, "HomeCafeLab", 0),
        (44, "Other", 0),
        (44, "HomeCafeLab", 1),
    ] {
        assert!(matches!(
            storage::query_account(
                &pool,
                UserKey {
                    uid,
                    name,
                    revision
                },
                Some(0)
            )
            .await,
            Err(HomeCafeError::Ownership)
        ));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT local_revision FROM game_users WHERE id=44")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn home_cafe_application_binds_admission_and_preserves_source_feature_short_circuit() {
    use vindictus_server::{
        application::{
            frontend_session::FrontendSession,
            home_cafe_bonus::{HomeCafeQueryError, query_admitted},
            lab_login::evaluate_existing_lab_user,
            session_registry::SessionRegistry,
        },
        domain::feature_policy::FeaturePolicy,
    };
    fn policy(home: Option<&str>, cafe: Option<&str>) -> FeaturePolicy {
        FeaturePolicy::new(
            [
                ("CurrentVer", Some("3.09.00")),
                ("HomeCafeBonus", home),
                ("CafeBonus", cafe),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.map(str::to_owned))),
        )
        .unwrap()
    }
    let (admin, pool, schema) = isolated_game_pool().await;
    let registry = SessionRegistry::with_capacity(8).unwrap();
    let active = policy(Some("2.02.05"), None);
    assert!(matches!(
        query_admitted(&FrontendSession::new(&registry), &pool, 0, &active, 0).await,
        Err(HomeCafeQueryError::Session(_))
    ));
    let cfg = super::lab_admission();
    let name = "AdmittedCafe";
    let uid =
        vindictus_server::protocol::identity::derive_lab_identity(Some(name), cfg.identity_options)
            .unwrap()
            .nexon_sn;
    provision_game_user(&pool, uid, name, 1, 0).await.unwrap();
    let (mut f, token) = super::begin_lab(&registry, name, &cfg);
    let accepted = evaluate_existing_lab_user(&mut f, token, &pool, 0, super::lab_clocks(), 10)
        .await
        .unwrap();
    storage::preserve(
        &pool,
        &[HomeCafeBonus {
            uid,
            cid: 0,
            bonus_type: 4,
            expired_time: SqlDateTime::from_units300(SqlDateTime::MIN).unwrap(),
        }],
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        query_admitted(&f, &pool, accepted.revision, &active, 0)
            .await
            .unwrap(),
        4
    );
    assert_eq!(
        query_admitted(&f, &pool, accepted.revision, &active, 1)
            .await
            .unwrap(),
        1
    );
    assert!(matches!(
        query_admitted(&f, &pool, accepted.revision + 1, &active, 0).await,
        Err(HomeCafeQueryError::Storage(HomeCafeError::Ownership))
    ));
    assert!(matches!(
        query_admitted(&f, &pool, accepted.revision, &policy(None, Some("1")), 1).await,
        Err(HomeCafeQueryError::CafeService)
    ));
    let missing = FeaturePolicy::new([("CurrentVer".into(), Some("3.09.00".into()))]).unwrap();
    assert!(matches!(
        query_admitted(&f, &pool, accepted.revision, &missing, 0).await,
        Err(HomeCafeQueryError::Profile)
    ));
    // Disabled HomeCafe and CafeType1 must not access storage (including a stale revision).
    sqlx::query("DROP TABLE game_home_cafe_bonus")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        query_admitted(&f, &pool, 999, &policy(None, None), 4)
            .await
            .unwrap(),
        4
    );
    assert_eq!(query_admitted(&f, &pool, 999, &active, 1).await.unwrap(), 1);
    assert!(
        query_admitted(&f, &pool, accepted.revision, &active, 0)
            .await
            .is_err()
    );
    f.close();
    assert!(matches!(
        query_admitted(&f, &pool, 0, &active, 1).await,
        Err(HomeCafeQueryError::Session(_))
    ));
    cleanup_game_pool(admin, pool, schema).await;
}
