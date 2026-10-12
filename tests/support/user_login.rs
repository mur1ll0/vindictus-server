use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use uuid::Uuid;
use vindictus_server::{
    application::user_login::*,
    domain::{
        game_user::NewGameUser,
        legacy_user_data::{SqlDateTime, SqlDateTime2},
        login_effects::EffectClockSamples,
        login_policy::{ApplicationClockSamples, IpRequest},
        passport::LoginRejection,
        user_login_composition::TokenProfile,
        user_login_return::UserLoginReturn,
    },
    error::Error,
    infrastructure::{
        reward_policy::{RewardPolicyPackage, read_policy},
        user_login::prepare_user,
    },
};
fn content(features: &[(&str, &str)]) -> RewardPolicyPackage {
    let mut v: Value =
        serde_json::from_str(include_str!("../fixtures/synthetic-reward-policy.json")).unwrap();
    for (k, s) in [
        ("NGM_disable", Some("1")),
        ("PassportImmigrator", None),
        ("NotifyBanReason", None),
        ("IPLockTime", None),
    ] {
        v["features"]
            .as_array_mut()
            .unwrap()
            .push(json!({"key":k,"value":s}));
    }
    for (k, s) in features {
        v["features"]
            .as_array_mut()
            .unwrap()
            .push(json!({"key":k,"value":s}));
    }
    read_policy(serde_json::to_vec(&v).unwrap().as_slice()).unwrap()
}
fn req<'a>(id: i32, name: &'a str, c: &'a RewardPolicyPackage) -> UserLoginRequest<'a> {
    let n = SqlDateTime2::from_ticks(638_000_000_000_000_000).unwrap();
    UserLoginRequest {
        user: NewGameUser::new(id, name, 17, 255).unwrap(),
        operation_id: Uuid::new_v4(),
        content: c,
        clocks: EffectClockSamples {
            loyalty_now: n,
            dormant_elapsed_now: n,
            latest_elapsed_now: n,
            set_dormant_now: n,
            set_event_now: n,
        },
        loyalty_insert_now: n.sql_parameter().unwrap(),
        ban_now: n.sql_parameter().unwrap(),
        ip: IpRequest {
            connection_ip: "127.0.0.1".parse().unwrap(),
            secured: false,
            lock_minutes_feature: 10,
            has_cheat_permission: false,
            clock: ApplicationClockSamples {
                set_lock_now: n,
                check_lock_now: n,
            },
        },
        token_profile: TokenProfile::SqlLatin1GeneralCp1,
    }
}
fn now() -> SqlDateTime {
    SqlDateTime::from_units300(1_000_000).unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn new_and_existing_user_composition_preserves_identity_ownership_token_ip_and_limited() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let c = content(&[
        ("friend_loyalty_system", "1"),
        ("DormantAccount", "1"),
        ("DormantAccount_ver2", "1"),
    ]);
    let r = req(41, "LoginLab", &c);
    let raw: Vec<u8> = "loginlab  "
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    sqlx::query("INSERT INTO game_user_achievements(id,uid,name_utf16,achievement_utf16) VALUES(1,-1,$1,'\\x'::bytea)")
        .bind(raw).execute(&pool).await.unwrap();
    let o = execute_local_user_login(&pool, &r, || Ok(now()))
        .await
        .unwrap();
    let UserLoginOutcome::Success {
        feedback,
        created,
        revision,
    } = o
    else {
        panic!("new user should pass")
    };
    assert!(created);
    let UserLoginReturn::Success(s) = feedback.feedback() else {
        panic!()
    };
    assert_eq!(s.package_count, 1);
    assert_eq!(s.limited, 0);
    assert_eq!(s.last_login_ip, Some([].as_slice()));
    assert!(!s.is_dormant);
    assert!(!s.is_return);
    assert_eq!(s.facebook_token, Some([].as_slice()));
    let row:(i32,i32,Option<i16>,i64)=sqlx::query_as("SELECT next_character_sn,service_id,channeling_code,local_revision FROM game_users WHERE id=41").fetch_one(&pool).await.unwrap();
    assert_eq!(row, (0, 17, Some(255), revision));
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT uid FROM game_user_achievements WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap(),
        41
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT recommend_flag FROM game_friend_loyalty WHERE recommender_uid=41"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let old: Vec<u8> = "127.0.0.2"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    sqlx::query("UPDATE game_users SET last_login_ip_utf16=$1,facebook_token_bytes=$2 WHERE id=41")
        .bind(old)
        .bind(vec![0u8, 0x80, 0x81, 0xff])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_limited_accounts(nexon_id_utf16) VALUES($1)")
        .bind(
            "LOGINLAB"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .execute(&pool)
        .await
        .unwrap();
    let r = req(41, "loginlab", &c);
    let UserLoginOutcome::Success {
        feedback, created, ..
    } = execute_local_user_login(&pool, &r, || Ok(now()))
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(!created);
    let UserLoginReturn::Success(s) = feedback.feedback() else {
        panic!()
    };
    assert_eq!(s.lock_last_seconds, 600);
    assert_eq!(s.limited, 1);
    assert_eq!(s.facebook_token, Some([0, 0x20ac, 0x81, 0xff].as_slice()));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM game_users WHERE id=41")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "LoginLab"
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn banned_login_retains_prior_effects_without_ip_mutation_and_uses_own_reason() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let c = content(&[("friend_loyalty_system", "1"), ("NotifyBanReason", "1")]);
    for (id, status) in [(42, 1), (43, 2)] {
        let name = format!("BannedLab{id}");
        sqlx::query("INSERT INTO game_user_bans(name,status,reason_utf16) VALUES($1,$2,$3)")
            .bind(&name)
            .bind(status)
            .bind(
                "own reason"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .execute(&pool)
            .await
            .unwrap();
        let UserLoginOutcome::Failure { reason, text } =
            execute_local_user_login(&pool, &req(id, &name, &c), || Ok(now()))
                .await
                .unwrap()
        else {
            panic!()
        };
        assert_eq!(reason, LoginRejection::Banned);
        let expected = if status == 1 {
            "GameUI_Login_Fail_BannedReason_AccountStolen"
        } else {
            "own reason"
        };
        assert_eq!(&*text, &expected.encode_utf16().collect::<Vec<_>>());
        assert_eq!(
            sqlx::query_scalar::<_, i32>(
                "SELECT recommend_flag FROM game_friend_loyalty WHERE recommender_uid=$1"
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert!(
            sqlx::query_scalar::<_, Option<Vec<u8>>>(
                "SELECT last_login_ip_utf16 FROM game_users WHERE id=$1"
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .is_none()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM game_login_reward_runs WHERE uid=$1"
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn new_user_barrier_rolls_back_unknown_achievement_and_concurrent_creation_has_one_winner() {
    let (admin, pool, schema) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_user_achievements(id,uid,name_utf16,achievement_utf16) VALUES(1,-1,'\\xE900'::bytea,'\\x'::bytea)").execute(&pool).await.unwrap();
    let u = NewGameUser::new(44, "BarrierLab", 0, 0).unwrap();
    assert!(matches!(
        prepare_user(&pool, &u, true, now()).await,
        Err(Error::LoginAccountEqualityUnsupported)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_users")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_friend_loyalty")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    sqlx::query("DELETE FROM game_user_achievements")
        .execute(&pool)
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            prepare_user(
                &pool,
                &NewGameUser::new(44, "BarrierLab", 0, 0).unwrap(),
                true,
                now(),
            )
            .await
        }));
    }
    let mut created = 0;
    for t in tasks {
        created += usize::from(t.await.unwrap().unwrap().created);
    }
    assert_eq!(created, 1);
    assert!(matches!(
        prepare_user(
            &pool,
            &NewGameUser::new(44, "other", 0, 0).unwrap(),
            true,
            now()
        )
        .await,
        Err(Error::GameUserIdConflict)
    ));
    assert!(matches!(
        prepare_user(
            &pool,
            &NewGameUser::new(45, "barrierlab", 0, 0).unwrap(),
            true,
            now()
        )
        .await,
        Err(Error::GameUserNameConflict)
    ));
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn missing_external_profile_is_refused_before_any_account_mutation() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let c = content(&[("PassportImmigrator", "1")]);
    assert!(matches!(
        execute_local_user_login(&pool, &req(46, "ExternalLab", &c), || Ok(now())).await,
        Err(UserLoginError::PassportImmigrator)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_users")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    cleanup_game_pool(admin, pool, schema).await;
}

fn config(
    c: &RewardPolicyPackage,
) -> vindictus_server::application::local_login_session::LocalLoginConfig<'_> {
    let r = req(0, "", c);
    vindictus_server::application::local_login_session::LocalLoginConfig {
        content: c,
        service_id: 17,
        channel_code: 0,
        effects_clock: r.clocks,
        loyalty_insert_now: r.loyalty_insert_now,
        ban_now: r.ban_now,
        application_clock: r.ip.clock,
        token_profile: r.token_profile,
    }
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn admitted_session_new_user_holds_reservation_and_stale_completion_does_not_write() {
    use vindictus_server::application::{
        frontend_session::{SessionError, SessionPhase},
        local_login_session::*,
        session_registry::SessionRegistry,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let c = content(&[("friend_loyalty_system", "1"), ("IPLockTime", "3")]);
    let cfg = config(&c);
    let registry = SessionRegistry::with_capacity(8).unwrap();
    let (mut session, token) =
        super::begin_lab(&registry, "SessionNewLab", &super::lab_admission());
    let (mut other, other_token) =
        super::begin_lab(&registry, "SessionOtherLab", &super::lab_admission());
    assert!(matches!(
        evaluate_local_lab_user(&mut session, other_token, &pool, &cfg, || Ok(now())).await,
        Err(LocalLoginError::Session(SessionError::StaleCallback))
    ));
    assert_eq!(session.phase(), SessionPhase::CheckingPolicies);
    let UserLoginOutcome::Success { created, .. } =
        evaluate_local_lab_user(&mut session, token, &pool, &cfg, || Ok(now()))
            .await
            .unwrap()
    else {
        panic!()
    };
    assert!(created);
    assert_eq!(session.phase(), SessionPhase::PoliciesAccepted);
    let mut duplicate =
        vindictus_server::application::frontend_session::FrontendSession::new(&registry);
    let n = std::time::Instant::now();
    let t = duplicate
        .begin(n, std::time::Duration::from_secs(10))
        .unwrap();
    assert!(matches!(
        duplicate.admit_lab(
            t,
            n,
            Some("SessionNewLab"),
            "127.0.0.1".parse().unwrap(),
            &super::lab_admission()
        ),
        Err(SessionError::Rejected(LoginRejection::DuplicateLogin))
    ));
    session.close();
    other.close();
    let (mut reconnect, token) =
        super::begin_lab(&registry, "SessionNewLab", &super::lab_admission());
    let UserLoginOutcome::Success { created, .. } =
        evaluate_local_lab_user(&mut reconnect, token, &pool, &cfg, || Ok(now()))
            .await
            .unwrap()
    else {
        panic!()
    };
    assert!(!created);
    reconnect.close();
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn canceled_local_login_closes_session_and_releases_reservation_without_new_user() {
    use vindictus_server::application::{
        frontend_session::SessionPhase, local_login_session::*, session_registry::SessionRegistry,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let c = content(&[]);
    let cfg = config(&c);
    let registry = SessionRegistry::with_capacity(8).unwrap();
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(pool.acquire().await.unwrap());
    }
    // Pool setup must not consume the attempt deadline before the cancellation is polled.
    let (mut session, token) =
        super::begin_lab(&registry, "CancelledNewLab", &super::lab_admission());
    {
        let future = evaluate_local_lab_user(&mut session, token, &pool, &cfg, || Ok(now()));
        tokio::pin!(future);
        tokio::select! { result=&mut future=>panic!("pool should remain blocked: {}",result.is_ok()),_ = tokio::time::sleep(std::time::Duration::from_millis(50))=>{} }
    }
    assert_eq!(session.phase(), SessionPhase::Closed);
    drop(held);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_users")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let (mut retry, _) = super::begin_lab(&registry, "CancelledNewLab", &super::lab_admission());
    retry.close();
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn partial_or_online_profile_is_refused_without_storage_writes() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let raw = include_str!("../fixtures/synthetic-reward-policy.json");
    let partial = read_policy(raw.as_bytes()).unwrap();
    assert!(matches!(
        execute_local_user_login(&pool, &req(47, "PartialLab", &partial), || Ok(now())).await,
        Err(UserLoginError::LocalProfile)
    ));
    let online = content(&[("NGM_disable", "")]);
    assert!(matches!(
        execute_local_user_login(&pool, &req(47, "PartialLab", &online), || Ok(now())).await,
        Err(UserLoginError::LocalProfile)
    ));
    let offline = content(&[]);
    let mut r = req(47, "PartialLab", &offline);
    r.ip.secured = true;
    assert!(matches!(
        execute_local_user_login(&pool, &r, || Ok(now())).await,
        Err(UserLoginError::LocalProfile)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM game_users")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    cleanup_game_pool(admin, pool, schema).await;
}
