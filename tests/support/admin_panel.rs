//! Admin panel login, sessions, lockout, roles and append-only audit against PostgreSQL.
//! Part of the postgres test target; needs TEST_DATABASE_URL (CREATE SCHEMA permission).
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;
use vindictus_server::{
    application::{
        accounts::create_account,
        admin_session::{AdminError, MAX_FAILURES, PanelAuth, audit_page, set_role_from_cli},
    },
    domain::{Username, admin::PanelRole},
    http::admin::{self, AdminState},
    infrastructure::{admin_store::AuditFilter, postgres},
};
use zeroize::Zeroizing;

pub(super) async fn isolated_pool() -> (sqlx::PgPool, sqlx::PgPool, String) {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect");
    let schema = format!("test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .expect("schema");
    let search_path = format!("SET search_path TO {schema}");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |connection, _| {
            let statement = search_path.clone();
            Box::pin(async move {
                sqlx::query(&statement).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .expect("isolated pool");
    postgres::migrate(&pool).await.expect("migrate");
    (admin, pool, schema)
}

const PASSWORD: &str = "senha de teste do painel";

fn secret(value: &str) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(value.as_bytes().to_vec())
}

async fn audit_actions(pool: &sqlx::PgPool) -> Vec<(String, String)> {
    audit_page(
        pool,
        AuditFilter {
            limit: 200,
            ..AuditFilter::default()
        },
    )
    .await
    .expect("audit")
    .into_iter()
    .map(|e| (e.action, e.outcome))
    .collect()
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn admin_panel_login_session_roles_and_audit() {
    let (admin_pool, pool, schema) = isolated_pool().await;
    for name in ["Painel_Admin", "Painel_Gm"] {
        create_account(
            &pool,
            Username::parse(name).expect("name"),
            secret(PASSWORD),
        )
        .await
        .expect("account");
    }
    let peer = Some("127.0.0.1".parse().expect("ip"));
    let auth = PanelAuth::new(pool.clone());

    // No role yet: the account logs in, but only sees its own data.
    let player = auth
        .login("painel_admin", secret(PASSWORD), peer)
        .await
        .expect("login without role");
    assert_eq!(player.session.role, None);
    set_role_from_cli(&pool, "Painel_Admin", Some(PanelRole::Admin))
        .await
        .expect("role");
    assert!(matches!(
        auth.login("painel_admin", secret("senha errada demais"), peer)
            .await,
        Err(AdminError::InvalidCredentials)
    ));
    let granted = auth
        .login("PAINEL_ADMIN", secret(PASSWORD), peer)
        .await
        .expect("login");
    assert_eq!(granted.session.role, Some(PanelRole::Admin));
    assert_eq!(
        auth.authorize(&granted.token).await.expect("session"),
        granted.session
    );

    // The last admin cannot be demoted; with a second admin it can.
    assert!(matches!(
        set_role_from_cli(&pool, "painel_admin", Some(PanelRole::Gm)).await,
        Err(AdminError::LastAdmin)
    ));
    set_role_from_cli(&pool, "painel_gm", Some(PanelRole::Gm))
        .await
        .expect("gm");
    // Removing the role keeps the session, without panel permissions from the next request.
    set_role_from_cli(&pool, "painel_gm", Some(PanelRole::Admin))
        .await
        .expect("admin");
    set_role_from_cli(&pool, "painel_admin", None)
        .await
        .expect("removed");
    assert_eq!(
        auth.authorize(&granted.token).await.expect("session").role,
        None
    );
    // A password change ends the sessions opened before it.
    let second = auth
        .login("painel_gm", secret(PASSWORD), peer)
        .await
        .expect("login");
    sqlx::query("UPDATE accounts SET password_changed_at = password_changed_at + interval '1 second' WHERE normalized_name = 'painel_gm'")
        .execute(&pool)
        .await
        .expect("password change");
    assert!(auth.authorize(&second.token).await.is_err());

    // Consecutive failures lock the panel login even for the right password.
    // A separate instance, so the in-memory per-name throttle (10 per minute) admits all ten.
    let lockout = PanelAuth::new(pool.clone());
    for _ in 0..MAX_FAILURES {
        assert!(matches!(
            lockout
                .login("painel_gm", secret("tentativa errada"), peer)
                .await,
            Err(AdminError::InvalidCredentials)
        ));
    }
    let fresh = PanelAuth::new(pool.clone());
    assert!(matches!(
        fresh.login("painel_gm", secret(PASSWORD), peer).await,
        Err(AdminError::InvalidCredentials)
    ));
    // The in-memory throttle refuses the next attempt of the same name in the window.
    assert!(matches!(
        lockout.login("painel_gm", secret(PASSWORD), peer).await,
        Err(AdminError::RateLimited)
    ));
    sqlx::query("UPDATE accounts SET locked_until = NULL WHERE normalized_name = 'painel_gm'")
        .execute(&pool)
        .await
        .expect("unlock");

    // HTTP: cookie session, CSRF-protected logout.
    let app = admin::router(AdminState::new(fresh.clone(), false), None);
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/session/login")
                .header(header::HOST, "127.0.0.1:27090")
                .header(header::ORIGIN, "http://127.0.0.1:27090")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"username":"painel_gm","password":"{PASSWORD}"}}"#
                )))
                .expect("request"),
        )
        .await
        .expect("login");
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .expect("cookie")
        .to_owned();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    let pair = cookie.split(';').next().expect("pair").to_owned();
    let body: serde_json::Value = serde_json::from_slice(
        &response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes(),
    )
    .expect("json");
    let csrf = body["csrf_token"].as_str().expect("csrf").to_owned();
    let get = |path: &str| {
        Request::get(path)
            .header(header::COOKIE, pair.clone())
            .body(Body::empty())
            .expect("request")
    };
    assert_eq!(
        app.clone()
            .oneshot(get("/api/session"))
            .await
            .expect("session")
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(get("/api/audit?limit=500"))
            .await
            .expect("audit")
            .status(),
        StatusCode::BAD_REQUEST
    );
    let logout = app
        .clone()
        .oneshot(
            Request::post("/api/session/logout")
                .header(header::HOST, "127.0.0.1:27090")
                .header(header::COOKIE, pair.clone())
                .header(admin::CSRF_HEADER, csrf)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("logout");
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        app.oneshot(get("/api/session"))
            .await
            .expect("session")
            .status(),
        StatusCode::UNAUTHORIZED
    );

    // Audit: every attempt and change recorded, newest first; rows cannot change.
    let actions = audit_actions(&pool).await;
    assert_eq!(
        actions.first().map(|a| a.0.as_str()),
        Some("session.logout")
    );
    let count = |action: &str, outcome: &str| {
        actions
            .iter()
            .filter(|(a, o)| a == action && o == outcome)
            .count()
    };
    assert_eq!(count("session.login", "ok"), 4);
    assert_eq!(count("account.role", "ok"), 4);
    assert_eq!(count("session.login", "refused"), 2 + MAX_FAILURES as usize);
    for statement in [
        "UPDATE admin_audit_events SET action = 'x'",
        "DELETE FROM admin_audit_events",
        "TRUNCATE admin_audit_events",
    ] {
        assert!(
            sqlx::query(statement).execute(&pool).await.is_err(),
            "{statement}"
        );
    }

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .expect("cleanup");
    admin_pool.close().await;
}
