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
    domain::{
        Username,
        admin::{LauncherTicket, PanelRole},
    },
    http::admin::{self, AdminState},
    infrastructure::{admin_store::AuditFilter, game_links, postgres},
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

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn launcher_session_mints_one_time_tickets_until_logout() {
    let (admin_pool, pool, schema) = isolated_pool().await;
    create_account(
        &pool,
        Username::parse("Lancador_A").expect("name"),
        secret(PASSWORD),
    )
    .await
    .expect("account");
    let peer = Some("127.0.0.1".parse().expect("ip"));
    let auth = PanelAuth::new(pool.clone());
    let redeem = |ticket: &LauncherTicket| {
        let pool = pool.clone();
        let digest = ticket.digest();
        async move {
            game_links::redeem_launcher_ticket(&pool, &digest)
                .await
                .expect("redeem")
        }
    };

    // Any active account (no panel role) opens a launcher session; each Play gets a ticket that
    // enters the account's game name once.
    let session = auth
        .launcher_login("lancador_a", secret(PASSWORD), peer)
        .await
        .expect("launcher login");
    assert!(matches!(
        auth.launcher_login("lancador_a", secret("senha errada demais"), peer)
            .await,
        Err(AdminError::InvalidCredentials)
    ));
    let first = auth.launcher_ticket(&session).await.expect("ticket");
    assert_eq!(first.as_str().len(), 24);
    assert_eq!(redeem(&first).await.as_deref(), Some("Lancador_A"));
    assert_eq!(redeem(&first).await, None, "a ticket is spent once");

    // Long after Play (the player stays on the title screen), the ticket is still good: it lives
    // as long as the session.
    let later = auth.launcher_ticket(&session).await.expect("second ticket");
    sqlx::query(
        "UPDATE game_launcher_tickets SET created_at = now() - interval '3 hours'
         WHERE ticket_sha256 = $1",
    )
    .bind(later.digest().as_slice())
    .execute(&pool)
    .await
    .expect("age");
    assert_eq!(redeem(&later).await.as_deref(), Some("Lancador_A"));

    // Session past its longest life: no ticket redeems and no new ticket is minted.
    let stale = auth.launcher_ticket(&session).await.expect("third ticket");
    sqlx::query(
        "UPDATE launcher_sessions SET created_at = now() - interval '13 hours',
                expires_at = now() - interval '1 hour'",
    )
    .execute(&pool)
    .await
    .expect("expire session");
    assert_eq!(redeem(&stale).await, None);
    assert!(matches!(
        auth.launcher_ticket(&session).await,
        Err(AdminError::Unauthenticated)
    ));

    // Logout (launcher closed) ends the session and its unspent tickets.
    let session = auth
        .launcher_login("lancador_a", secret(PASSWORD), peer)
        .await
        .expect("relogin");
    let unspent = auth.launcher_ticket(&session).await.expect("ticket");
    auth.launcher_logout(&session).await.expect("logout");
    assert_eq!(redeem(&unspent).await, None);
    assert!(matches!(
        auth.launcher_ticket(&session).await,
        Err(AdminError::Unauthenticated)
    ));

    // A password change or a disabled account ends the session too.
    let session = auth
        .launcher_login("lancador_a", secret(PASSWORD), peer)
        .await
        .expect("relogin");
    let ticket = auth.launcher_ticket(&session).await.expect("ticket");
    sqlx::query(
        "UPDATE accounts SET password_changed_at = now() + interval '1 second'
         WHERE normalized_name = 'lancador_a'",
    )
    .execute(&pool)
    .await
    .expect("password change");
    assert_eq!(redeem(&ticket).await, None);
    let session = auth
        .launcher_login("lancador_a", secret(PASSWORD), peer)
        .await
        .expect("relogin");
    let ticket = auth.launcher_ticket(&session).await.expect("ticket");
    sqlx::query("UPDATE accounts SET status = 'disabled' WHERE normalized_name = 'lancador_a'")
        .execute(&pool)
        .await
        .expect("disable");
    assert_eq!(redeem(&ticket).await, None);
    sqlx::query("UPDATE accounts SET status = 'active' WHERE normalized_name = 'lancador_a'")
        .execute(&pool)
        .await
        .expect("enable");

    // HTTP: login without Origin and without cookie, Bearer ticket and logout; another origin is
    // refused.
    let app = admin::router(AdminState::new(auth.clone(), false), None);
    let body = format!(r#"{{"username":"Lancador_A","password":"{PASSWORD}"}}"#);
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/launcher/login")
                .header(header::HOST, "127.0.0.1:27090")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.clone()))
                .expect("request"),
        )
        .await
        .expect("launcher login");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    let reply = response_json(response).await;
    assert_eq!(reply["expires_in"], 12 * 60 * 60);
    let bearer = format!("Bearer {}", reply["session"].as_str().expect("session"));
    let response = app
        .clone()
        .oneshot(launcher_post("/api/launcher/ticket", Some(&bearer)))
        .await
        .expect("ticket");
    assert_eq!(response.status(), StatusCode::OK);
    let reply = response_json(response).await;
    let ticket = LauncherTicket::parse(reply["ticket"].as_str().expect("ticket")).expect("form");
    let response = app
        .clone()
        .oneshot(launcher_post("/api/launcher/ticket", None))
        .await
        .expect("no bearer");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .clone()
        .oneshot(launcher_post("/api/launcher/logout", Some(&bearer)))
        .await
        .expect("logout");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        redeem(&ticket).await,
        None,
        "logout revokes the unspent ticket"
    );
    let response = app
        .clone()
        .oneshot(launcher_post("/api/launcher/ticket", Some(&bearer)))
        .await
        .expect("after logout");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(
            Request::post("/api/launcher/login")
                .header(header::HOST, "127.0.0.1:27090")
                .header(header::ORIGIN, "http://evil.example")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("cross origin");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let actions = audit_actions(&pool).await;
    let count = |action: &str, outcome: &str| {
        actions
            .iter()
            .filter(|(a, o)| a == action && o == outcome)
            .count()
    };
    assert_eq!(count("launcher.login", "ok"), 5);
    assert_eq!(count("launcher.login", "refused"), 1);

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .expect("cleanup");
    admin_pool.close().await;
}

fn launcher_post(path: &'static str, authorization: Option<&str>) -> Request<Body> {
    let mut request = Request::post(path).header(header::HOST, "127.0.0.1:27090");
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    request.body(Body::empty()).expect("request")
}

async fn response_json(response: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(
        &response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes(),
    )
    .expect("json")
}
