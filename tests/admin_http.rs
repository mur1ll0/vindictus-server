//! Admin panel HTTP refusals that happen before any database access, and the committed
//! OpenAPI contract. The database flow is in tests/admin_panel.rs.
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use vindictus_server::{
    application::admin_session::PanelAuth,
    http::admin::{self, AdminState},
};

async fn send(request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .expect("pool");
    let response = admin::router(AdminState::new(PanelAuth::new(pool), false), None)
        .oneshot(request)
        .await
        .expect("router");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or_default(),
    )
}

const TOKEN: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

#[tokio::test]
async fn session_without_cookie_is_refused_with_security_headers() {
    let (status, headers, body) = send(
        Request::get("/api/session")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "sessao_encerrada");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY");
    assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));
}

#[tokio::test]
async fn mutation_without_csrf_token_is_refused() {
    let request = Request::post("/api/session/logout")
        .header(header::HOST, "127.0.0.1:27090")
        .header(header::COOKIE, format!("{}={TOKEN}", admin::COOKIE))
        .body(Body::empty())
        .expect("request");
    let (status, _, body) = send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "csrf");
}

#[tokio::test]
async fn cross_origin_login_is_refused() {
    for (name, value) in [
        (header::ORIGIN.as_str(), "http://evil.example"),
        ("sec-fetch-site", "cross-site"),
    ] {
        let request = Request::post("/api/session/login")
            .header(header::HOST, "127.0.0.1:27090")
            .header(header::CONTENT_TYPE, "application/json")
            .header(name, value)
            .body(Body::from(r#"{"username":"lab169_a","password":"x"}"#))
            .expect("request");
        let (status, _, body) = send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{name}");
        assert_eq!(body["code"], "origem_recusada");
    }
}

#[test]
fn committed_openapi_matches_the_code() {
    let committed = std::fs::read_to_string("web/admin/openapi.json")
        .expect("web/admin/openapi.json")
        .replace("\r\n", "\n");
    assert!(
        committed == admin::openapi_json(),
        "regenerate: vindictus-server export-openapi --output web/admin/openapi.json, then npm run gen:api"
    );
}
