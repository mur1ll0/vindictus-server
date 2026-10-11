use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

async fn request(path: &str, method: &str) -> axum::response::Response {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .expect("pool");
    vindictus_server::http::router(pool)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router")
}

#[tokio::test]
async fn liveness_is_available_when_database_is_unavailable() {
    assert_eq!(
        request("/health/live", "GET").await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn readiness_does_not_claim_ready_when_database_is_unavailable() {
    let response = request("/health/ready", "GET").await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).expect("json"),
        serde_json::json!({"status":"not_ready"})
    );
}

#[tokio::test]
async fn capabilities_explicitly_reports_incomplete_game_migration() {
    let bytes = request("/capabilities", "GET")
        .await
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(body["game_client_compatible"], false);
    assert_eq!(body["legacy_migration_complete"], false);
    assert_eq!(body["game_authentication_available"], false);
}

#[tokio::test]
async fn no_public_registration_endpoint_is_exposed() {
    assert_eq!(
        request("/accounts", "POST").await.status(),
        StatusCode::NOT_FOUND
    );
}
