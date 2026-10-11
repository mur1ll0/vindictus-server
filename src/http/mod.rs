//! Operational HTTP interface. No game protocol or public account endpoints.
use crate::infrastructure::postgres;
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;
use sqlx::PgPool;
use std::time::Duration;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/capabilities", get(capabilities))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(5),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(AppState { pool })
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn live() -> Json<Health> {
    Json(Health { status: "alive" })
}

async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    match tokio::time::timeout(Duration::from_secs(2), postgres::check_ready(&state.pool)).await {
        Ok(Ok(())) => (StatusCode::OK, Json(Health { status: "ready" })),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Health {
                status: "not_ready",
            }),
        ),
    }
}

#[derive(Serialize)]
struct Capabilities {
    service: &'static str,
    version: &'static str,
    game_client_compatible: bool,
    legacy_migration_complete: bool,
    game_authentication_available: bool,
    account_provisioning: &'static str,
}

async fn capabilities() -> Json<Capabilities> {
    Json(Capabilities {
        service: "vindictus-preservation-foundation",
        version: env!("CARGO_PKG_VERSION"),
        game_client_compatible: false,
        legacy_migration_complete: false,
        game_authentication_available: false,
        account_provisioning: "operator CLI; independent PostgreSQL schema",
    })
}
