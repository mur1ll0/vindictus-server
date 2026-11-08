//! Read-only HTTP of the update service (`client-dist serve`, docs/future/game-launcher.md 4.2), on its own
//! listener, apart from the operational HTTP and the panel. It serves only the published layout: the signed
//! pointer, the manifests, the objects and deltas (with `Range`, for resumable downloads), the public key and
//! a dynamic `status.json`. Nothing else under the store is reachable, and the private key is not there.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde::Serialize;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use vindictus_client_dist::{layout, manifest::VersionPointer};

/// A service the status reports (`name=host:port`, probed by a TCP connect).
#[derive(Debug, Clone)]
pub struct Probe {
    pub name: String,
    pub address: String,
}

impl std::str::FromStr for Probe {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (name, address) = value.split_once('=').ok_or("expected name=host:port")?;
        let valid = !name.is_empty()
            && name.len() <= 32
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !valid
            || address
                .rsplit_once(':')
                .and_then(|(_, port)| port.parse::<u16>().ok())
                .is_none()
        {
            return Err("expected name=host:port".into());
        }
        Ok(Self {
            name: name.into(),
            address: address.into(),
        })
    }
}

#[derive(Clone)]
struct UpdatesState {
    store: Arc<PathBuf>,
    probes: Arc<Vec<Probe>>,
}

#[derive(Serialize)]
struct Status {
    updates: &'static str,
    current: Option<u32>,
    minimum: Option<u32>,
    services: std::collections::BTreeMap<String, &'static str>,
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

async fn status(State(state): State<UpdatesState>) -> impl IntoResponse {
    let pointer = tokio::fs::read(state.store.join(layout::POINTER))
        .await
        .ok()
        .and_then(|b| VersionPointer::parse(&b).ok());
    let mut checks = tokio::task::JoinSet::new();
    for probe in state.probes.iter().cloned() {
        checks.spawn(async move {
            let up = tokio::time::timeout(
                PROBE_TIMEOUT,
                tokio::net::TcpStream::connect(probe.address.as_str()),
            )
            .await
            .is_ok_and(|r| r.is_ok());
            (probe.name, if up { "online" } else { "offline" })
        });
    }
    let services = checks.join_all().await.into_iter().collect();
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(Status {
            updates: "online",
            current: pointer.as_ref().map(|p| p.current),
            minimum: pointer.map(|p| p.minimum),
            services,
        }),
    )
}

fn cached(service: ServeDir, value: &'static str) -> Router {
    Router::new()
        .fallback_service(service)
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static(value),
        ))
}

/// The router of the update service over a store folder.
pub fn router(store: PathBuf, probes: Vec<Probe>) -> Router {
    const IMMUTABLE: &str = "public, max-age=31536000, immutable";
    let file = |name: &str| ServeFile::new(store.join(name));
    let pointer = Router::new()
        .route_service(&format!("/{}", layout::POINTER), file(layout::POINTER))
        .route_service(
            &format!("/{}", layout::signature(layout::POINTER)),
            file(&layout::signature(layout::POINTER)),
        )
        .route_service(
            &format!("/{}", layout::PUBLIC_KEY),
            file(layout::PUBLIC_KEY),
        )
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ));
    let objects = cached(ServeDir::new(store.join("objects")), IMMUTABLE);
    let deltas = cached(ServeDir::new(store.join("deltas")), IMMUTABLE);
    let manifests = cached(ServeDir::new(store.join("manifests")), IMMUTABLE);
    Router::new()
        .route(&format!("/{}", layout::STATUS), get(status))
        .with_state(UpdatesState {
            store: Arc::new(store),
            probes: Arc::new(probes),
        })
        .merge(pointer)
        .nest("/objects", objects)
        .nest("/deltas", deltas)
        .nest("/manifests", manifests)
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(TraceLayer::new_for_http())
}

/// Serves until `shutdown` resolves.
pub async fn serve(
    listener: tokio::net::TcpListener,
    store: PathBuf,
    probes: Vec<Probe>,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        router(store, probes).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn store() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("updates-http-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("objects/ab"));
        let _ = std::fs::create_dir_all(dir.join("keys"));
        let _ = std::fs::write(dir.join("objects/ab/ab01.zst"), b"0123456789");
        let _ = std::fs::write(dir.join("keys/updates.ed25519"), b"secret");
        let _ = std::fs::write(dir.join("other.txt"), b"not served");
        dir
    }

    async fn get(path: &str, range: Option<&str>) -> (StatusCode, Vec<u8>) {
        let mut request = Request::get(path);
        if let Some(range) = range {
            request = request.header(header::RANGE, range);
        }
        let response = router(store(), Vec::new())
            .oneshot(request.body(Body::empty()).unwrap_or_default())
            .await;
        match response {
            Ok(response) => {
                let status = response.status();
                let body = axum::body::to_bytes(response.into_body(), 1 << 20)
                    .await
                    .map(|b| b.to_vec())
                    .unwrap_or_default();
                (status, body)
            }
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Vec::new()),
        }
    }

    #[tokio::test]
    async fn objects_answer_ranges() {
        assert_eq!(
            get("/objects/ab/ab01.zst", Some("bytes=4-")).await,
            (StatusCode::PARTIAL_CONTENT, b"456789".to_vec())
        );
    }

    #[tokio::test]
    async fn only_the_published_layout_is_served() {
        assert_eq!(
            get("/keys/updates.ed25519", None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(get("/other.txt", None).await.0, StatusCode::NOT_FOUND);
        assert_eq!(
            get("/objects/../keys/updates.ed25519", None).await.0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn status_reports_probes_without_a_pointer() {
        let (code, body) = get("/status.json", None).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(
            body,
            br#"{"updates":"online","current":null,"minimum":null,"services":{}}"#.to_vec()
        );
    }

    #[test]
    fn probes_parse_name_and_address() {
        assert!("server=server:8080".parse::<Probe>().is_ok());
        assert!("bad name=server:8080".parse::<Probe>().is_err());
        assert!("server=server".parse::<Probe>().is_err());
    }
}
