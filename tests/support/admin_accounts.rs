//! Panel account administration (B4.2) against PostgreSQL: GM/admin permissions, repeated
//! operations, name rules, disabling, password reset and the audit. Part of the postgres
//! test target; needs TEST_DATABASE_URL (CREATE SCHEMA permission).
use super::admin_panel::isolated_pool;
use uuid::Uuid;
use vindictus_server::{
    application::{
        accounts::create_account,
        admin_accounts::{self, Request},
        admin_session::{AdminError, PanelAuth, audit_page, set_role_from_cli},
    },
    domain::{Username, admin::PanelRole},
    infrastructure::admin_store::AuditFilter,
};
use zeroize::Zeroizing;

const PASSWORD: &str = "senha de teste do painel";

fn secret(value: &str) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(value.as_bytes().to_vec())
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn admin_accounts_permissions_and_audit() {
    let (admin_pool, pool, schema) = isolated_pool().await;
    let peer = Some("127.0.0.1".parse().expect("ip"));
    create_account(
        &pool,
        Username::parse("Chefe").expect("name"),
        secret(PASSWORD),
    )
    .await
    .expect("account");
    set_role_from_cli(&pool, "chefe", Some(PanelRole::Admin), "primeiro admin")
        .await
        .expect("admin");
    let auth = PanelAuth::new(pool.clone());
    let admin = auth
        .login("chefe", secret(PASSWORD), peer)
        .await
        .expect("login")
        .session;
    let ask = |session, reason| Request {
        session,
        peer,
        operation_id: Uuid::new_v4(),
        reason,
    };

    admin_accounts::create(
        &pool,
        &ask(&admin, "gm"),
        "Ajudante",
        secret(PASSWORD),
        Some(PanelRole::Gm),
    )
    .await
    .expect("gm account");
    let gm = auth
        .login("ajudante", secret(PASSWORD), peer)
        .await
        .expect("gm login");
    let gm_session = gm.session.clone();

    // A GM creates game accounts, but cannot give roles or change other panel accounts.
    assert!(matches!(
        admin_accounts::create(
            &pool,
            &ask(&gm_session, "x"),
            "Outro_Gm",
            secret(PASSWORD),
            Some(PanelRole::Gm)
        )
        .await,
        Err(AdminError::Forbidden)
    ));
    let repeated = ask(&gm_session, "jogador");
    admin_accounts::create(&pool, &repeated, "Jogador_1", secret(PASSWORD), None)
        .await
        .expect("player");
    assert!(matches!(
        admin_accounts::create(&pool, &repeated, "Jogador_2", secret(PASSWORD), None).await,
        Err(AdminError::RepeatedOperation)
    ));
    assert!(matches!(
        admin_accounts::create(
            &pool,
            &ask(&gm_session, "x"),
            "JOGADOR_1",
            secret(PASSWORD),
            None
        )
        .await,
        Err(AdminError::Exists)
    ));
    assert!(matches!(
        admin_accounts::create(
            &pool,
            &ask(&gm_session, "x"),
            &"n".repeat(25),
            secret(PASSWORD),
            None
        )
        .await,
        Err(AdminError::InvalidInput)
    ));
    assert!(matches!(
        admin_accounts::create(
            &pool,
            &ask(&gm_session, "  "),
            "Jogador_3",
            secret(PASSWORD),
            None
        )
        .await,
        Err(AdminError::InvalidInput)
    ));
    assert!(matches!(
        admin_accounts::set_role(
            &pool,
            &ask(&gm_session, "x"),
            "jogador_1",
            Some(PanelRole::Leitura)
        )
        .await,
        Err(AdminError::Forbidden)
    ));
    assert!(matches!(
        admin_accounts::set_password(
            &pool,
            &ask(&gm_session, "x"),
            "chefe",
            secret("outra senha qualquer")
        )
        .await,
        Err(AdminError::Forbidden)
    ));
    admin_accounts::set_password(
        &pool,
        &ask(&gm_session, "pedido do jogador"),
        "jogador_1",
        secret("senha nova do jogador"),
    )
    .await
    .expect("player password");
    assert!(matches!(
        admin_accounts::set_status(&pool, &ask(&gm_session, "x"), "chefe", false).await,
        Err(AdminError::Forbidden)
    ));
    assert!(matches!(
        admin_accounts::set_status(&pool, &ask(&admin, "x"), "chefe", false).await,
        Err(AdminError::InvalidInput)
    ));
    assert!(matches!(
        admin_accounts::set_status(&pool, &ask(&admin, "x"), "ninguem", false).await,
        Err(AdminError::NotFound)
    ));

    // An admin resets the GM's password: the GM's session ends. Disabling refuses the login.
    admin_accounts::set_password(
        &pool,
        &ask(&admin, "troca"),
        "ajudante",
        secret("senha nova do ajudante"),
    )
    .await
    .expect("gm password");
    assert!(matches!(
        auth.authorize(&gm.token).await,
        Err(AdminError::Unauthenticated)
    ));
    let again = auth
        .login("ajudante", secret("senha nova do ajudante"), peer)
        .await
        .expect("new password");
    admin_accounts::set_status(&pool, &ask(&admin, "afastado"), "ajudante", false)
        .await
        .expect("disable");
    assert!(matches!(
        auth.authorize(&again.token).await,
        Err(AdminError::Unauthenticated)
    ));
    assert!(matches!(
        auth.login("ajudante", secret("senha nova do ajudante"), peer)
            .await,
        Err(AdminError::InvalidCredentials)
    ));
    admin_accounts::set_status(&pool, &ask(&admin, "voltou"), "ajudante", true)
        .await
        .expect("enable");
    admin_accounts::set_role(
        &pool,
        &ask(&admin, "promovido"),
        "jogador_1",
        Some(PanelRole::Leitura),
    )
    .await
    .expect("role");

    let listed = admin_accounts::list(&pool, Some("JOGADOR"), None, 50)
        .await
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].username, "Jogador_1");
    assert_eq!(listed[0].panel_role.as_deref(), Some("leitura"));
    assert_eq!((listed[0].game_uid, listed[0].characters), (None, 0));
    assert!(
        admin_accounts::list(&pool, Some("a%"), None, 50)
            .await
            .is_err()
    );

    let audit = audit_page(
        &pool,
        AuditFilter {
            limit: 200,
            ..AuditFilter::default()
        },
    )
    .await
    .expect("audit");
    let count = |action: &str| audit.iter().filter(|e| e.action == action).count();
    assert_eq!(count("account.create"), 2);
    assert_eq!(count("account.password"), 2);
    assert_eq!(count("account.status"), 2);
    // The CLI admin role plus the panel role given to Jogador_1.
    assert_eq!(count("account.role"), 2);
    assert!(audit.iter().all(|e| {
        let text = format!("{:?}{:?}", e.before, e.after);
        !text.contains("argon2") && !text.contains(PASSWORD)
    }));
    let created = audit
        .iter()
        .find(|e| e.action == "account.create" && e.target_id.as_deref() == Some("jogador_1"))
        .expect("create row");
    assert_eq!(created.actor_name.as_deref(), Some("Ajudante"));
    assert_eq!(created.reason.as_deref(), Some("jogador"));

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .expect("cleanup");
    admin_pool.close().await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn play_codes_are_single_use_expire_and_follow_the_game_link() {
    use vindictus_server::{
        application::{accounts::create_account_with, admin_session::link_game_from_cli},
        domain::{MIN_LAB_SHORT_PASSWORD, admin::PlayCode},
        infrastructure::play_codes,
    };
    let (admin_pool, pool, schema) = isolated_pool().await;
    // The operator's laboratory short-password option (5 bytes) for this one account.
    create_account_with(
        &pool,
        Username::parse("admin").expect("name"),
        secret("admin"),
        MIN_LAB_SHORT_PASSWORD,
    )
    .await
    .expect("short password account");
    assert!(
        create_account(
            &pool,
            Username::parse("curta").expect("name"),
            secret("admin")
        )
        .await
        .is_err()
    );
    let auth = PanelAuth::new(pool.clone());
    assert!(matches!(
        auth.issue_play_code("admin", secret("errada"), None).await,
        Err(AdminError::InvalidCredentials)
    ));
    let first = auth
        .issue_play_code("admin", secret("admin"), None)
        .await
        .expect("code");
    let second = auth
        .issue_play_code("admin", secret("admin"), None)
        .await
        .expect("code");
    // A new code replaces the earlier one; a code works once.
    assert_eq!(
        play_codes::redeem(&pool, &first.digest())
            .await
            .expect("redeem"),
        None
    );
    assert_eq!(
        play_codes::redeem(&pool, &second.digest())
            .await
            .expect("redeem")
            .as_deref(),
        Some("admin")
    );
    assert_eq!(
        play_codes::redeem(&pool, &second.digest())
            .await
            .expect("redeem"),
        None
    );

    link_game_from_cli(&pool, "admin", "lab169_a", "manter a Fiona do laboratório")
        .await
        .expect("link");
    assert!(
        play_codes::claimed(&pool, "lab169_a")
            .await
            .expect("claimed")
    );
    assert!(!play_codes::claimed(&pool, "admin").await.expect("claimed"));
    let linked = auth
        .issue_play_code("admin", secret("admin"), None)
        .await
        .expect("code");
    assert_eq!(
        play_codes::redeem(&pool, &linked.digest())
            .await
            .expect("redeem")
            .as_deref(),
        Some("lab169_a")
    );
    // One account per game name, including another account's own login.
    create_account(
        &pool,
        Username::parse("Lab169_A").expect("name"),
        secret(PASSWORD),
    )
    .await
    .expect_err("claimed by admin");

    let expired = auth
        .issue_play_code("admin", secret("admin"), None)
        .await
        .expect("code");
    sqlx::query("UPDATE game_play_codes SET expires_at = created_at + interval '1 microsecond', created_at = created_at - interval '1 hour'")
        .execute(&pool)
        .await
        .expect("expire");
    assert_eq!(
        play_codes::redeem(&pool, &expired.digest())
            .await
            .expect("redeem"),
        None
    );
    let disabled = auth
        .issue_play_code("admin", secret("admin"), None)
        .await
        .expect("code");
    sqlx::query("UPDATE accounts SET status = 'disabled'")
        .execute(&pool)
        .await
        .expect("disable");
    assert_eq!(
        play_codes::redeem(&pool, &disabled.digest())
            .await
            .expect("redeem"),
        None
    );
    assert!(PlayCode::parse(disabled.as_str()).is_some());

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .expect("cleanup");
    admin_pool.close().await;
}
