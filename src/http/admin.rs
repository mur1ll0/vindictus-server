//! Administration panel API (B4) on its own loopback listener, separate from the game TCP
//! and from the operational endpoints. The panel never implements the game login.
use crate::{
    application::admin_session::{self, AdminError, PanelAuth},
    domain::admin::{Permission, SessionToken, same_secret},
    infrastructure::admin_store::{AuditEntry, AuditFilter, SessionAccount},
};
use axum::{
    Json, Router,
    extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts, Query, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, SocketAddr},
    path::Path,
    time::Duration,
};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    timeout::TimeoutLayer,
};
use utoipa::{IntoParams, OpenApi, ToSchema};
use zeroize::Zeroizing;

mod accounts;

pub const COOKIE: &str = "vindictus_admin";
pub const CSRF_HEADER: &str = "x-csrf-token";
const MAX_BODY: usize = 8 * 1024;

#[derive(Clone)]
pub struct AdminState {
    auth: PanelAuth,
    secure_cookie: bool,
}

impl AdminState {
    /// `secure_cookie` marks the cookie Secure; leave it off for plain http://127.0.0.1.
    pub fn new(auth: PanelAuth, secure_cookie: bool) -> Self {
        Self {
            auth,
            secure_cookie,
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Vindictus admin panel",
        description = "Laboratory administration API (B4)."
    ),
    paths(
        login,
        play_code,
        session,
        logout,
        audit,
        accounts::list,
        accounts::create,
        accounts::set_role,
        accounts::set_password,
        accounts::set_status
    ),
    components(schemas(
        LoginRequest,
        PlayCodeResponse,
        SessionResponse,
        AuditEntry,
        ApiError,
        crate::infrastructure::admin_accounts::AccountSummary,
        accounts::CreateAccount,
        accounts::SetRole,
        accounts::SetPassword,
        accounts::SetStatus
    ))
)]
pub struct ApiDoc;

/// OpenAPI document of the panel API, as committed in web/admin/openapi.json.
pub fn openapi_json() -> String {
    let mut doc = ApiDoc::openapi();
    doc.info.version = env!("CARGO_PKG_VERSION").into();
    doc.info.license = None;
    doc.to_pretty_json().unwrap_or_default() + "\n"
}

/// API router; with `ui_dir` the built single-page app is served for every other path.
pub fn router(state: AdminState, ui_dir: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/api/session/login", post(login))
        .route("/api/play/code", post(play_code))
        .route("/api/session", get(session))
        .route("/api/session/logout", post(logout))
        .route("/api/audit", get(audit))
        .route("/api/accounts", get(accounts::list).post(accounts::create))
        .route("/api/accounts/{username}/role", post(accounts::set_role))
        .route(
            "/api/accounts/{username}/password",
            post(accounts::set_password),
        )
        .route(
            "/api/accounts/{username}/status",
            post(accounts::set_status),
        )
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .with_state(state);
    let app = match ui_dir {
        Some(dir) => api
            .fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")))),
        None => api,
    };
    app.layer(SetResponseHeaderLayer::if_not_present(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; \
             frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    ))
}

/// Error body of every refused request.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiError {
    /// Stable machine-readable code.
    code: &'static str,
    message: &'static str,
}

enum Refusal {
    Admin(AdminError),
    Csrf,
    Origin,
}

impl From<AdminError> for Refusal {
    fn from(error: AdminError) -> Self {
        Self::Admin(error)
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Csrf => (
                StatusCode::FORBIDDEN,
                "csrf",
                "Recarregue a página e tente de novo.",
            ),
            Self::Origin => (
                StatusCode::FORBIDDEN,
                "origem_recusada",
                "Origem da requisição recusada.",
            ),
            Self::Admin(error) => match error {
                AdminError::InvalidCredentials => (
                    StatusCode::UNAUTHORIZED,
                    "credenciais_invalidas",
                    "Login ou senha inválidos, ou conta sem acesso ao painel.",
                ),
                AdminError::RateLimited => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "muitas_tentativas",
                    "Muitas tentativas. Aguarde um minuto.",
                ),
                AdminError::Busy => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "autenticacao_ocupada",
                    "Autenticação ocupada. Tente de novo.",
                ),
                AdminError::Unauthenticated => (
                    StatusCode::UNAUTHORIZED,
                    "sessao_encerrada",
                    "Sessão encerrada. Entre de novo.",
                ),
                AdminError::Forbidden => (
                    StatusCode::FORBIDDEN,
                    "papel_insuficiente",
                    "Seu papel não permite esta ação.",
                ),
                AdminError::InvalidInput => (
                    StatusCode::BAD_REQUEST,
                    "entrada_invalida",
                    "Dados inválidos.",
                ),
                AdminError::NotFound => {
                    (StatusCode::NOT_FOUND, "nao_encontrado", "Não encontrado.")
                }
                AdminError::Exists => (
                    StatusCode::CONFLICT,
                    "conta_existente",
                    "Já existe uma conta com esse login.",
                ),
                AdminError::RepeatedOperation => (
                    StatusCode::CONFLICT,
                    "operacao_repetida",
                    "Esta operação já foi aplicada.",
                ),
                AdminError::LastAdmin => (
                    StatusCode::CONFLICT,
                    "ultimo_admin",
                    "A última conta admin não pode perder o papel.",
                ),
                AdminError::Unavailable(_) => {
                    tracing::warn!("admin panel storage unavailable");
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        "indisponivel",
                        "Banco de dados indisponível.",
                    )
                }
            },
        };
        (status, Json(ApiError { code, message })).into_response()
    }
}

fn peer(parts_extensions: &axum::http::Extensions) -> Option<IpAddr> {
    parts_extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip())
}

/// Same-origin check for state-changing requests (login included, against login CSRF).
fn check_origin(headers: &HeaderMap) -> Result<(), Refusal> {
    if headers
        .get("sec-fetch-site")
        .is_some_and(|v| v.as_bytes() != b"same-origin" && v.as_bytes() != b"none")
    {
        return Err(Refusal::Origin);
    }
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let host = headers.get(header::HOST).ok_or(Refusal::Origin)?;
    let origin = origin.to_str().map_err(|_| Refusal::Origin)?;
    let host = host.to_str().map_err(|_| Refusal::Origin)?;
    match origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    {
        Some(rest) if rest == host => Ok(()),
        _ => Err(Refusal::Origin),
    }
}

fn cookie_token(headers: &HeaderMap) -> Option<SessionToken> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .and_then(|(_, value)| SessionToken::parse(value))
}

/// An authenticated panel request. Methods other than GET/HEAD also pass the origin and
/// CSRF checks.
pub struct Authenticated {
    token: SessionToken,
    session: SessionAccount,
    peer: Option<IpAddr>,
}

impl Authenticated {
    fn require(&self, permission: Permission) -> Result<(), Refusal> {
        if self.session.role.allows(permission) {
            Ok(())
        } else {
            Err(AdminError::Forbidden.into())
        }
    }
}

impl FromRequestParts<AdminState> for Authenticated {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AdminState) -> Result<Self, Response> {
        let mutating = !matches!(parts.method, Method::GET | Method::HEAD);
        if mutating {
            check_origin(&parts.headers).map_err(IntoResponse::into_response)?;
        }
        let token = cookie_token(&parts.headers)
            .ok_or_else(|| Refusal::from(AdminError::Unauthenticated).into_response())?;
        if mutating {
            let header = parts
                .headers
                .get(CSRF_HEADER)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !same_secret(header, &token.csrf()) {
                return Err(Refusal::Csrf.into_response());
            }
        }
        let session = state
            .auth
            .authorize(&token)
            .await
            .map_err(|e| Refusal::from(e).into_response())?;
        Ok(Self {
            token,
            session,
            peer: peer(&parts.extensions),
        })
    }
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    username: String,
    #[schema(value_type = String, format = Password)]
    password: Zeroizing<String>,
}

#[derive(Serialize, ToSchema)]
pub struct SessionResponse {
    username: String,
    #[schema(value_type = String, example = "admin")]
    role: crate::domain::admin::PanelRole,
    /// Send back in the X-CSRF-Token header of every POST/PUT/DELETE.
    csrf_token: String,
}

fn session_cookie(state: &AdminState, value: &str, max_age: u64) -> HeaderValue {
    let secure = if state.secure_cookie { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}{secure}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("vindictus_admin=; Max-Age=0; Path=/"))
}

/// Log in with the game account's login and password; the account needs a panel role.
#[utoipa::path(post, path = "/api/session/login", request_body = LoginRequest,
    responses((status = 200, body = SessionResponse), (status = 401, body = ApiError),
              (status = 429, body = ApiError)))]
async fn login(
    State(state): State<AdminState>,
    ConnectInfoOpt(peer): ConnectInfoOpt,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, Refusal> {
    check_origin(&headers)?;
    let password = Zeroizing::new(request.password.as_bytes().to_vec());
    let granted = state.auth.login(&request.username, password, peer).await?;
    let cookie = session_cookie(
        &state,
        granted.token.as_str(),
        admin_session::SESSION_SECONDS,
    );
    let body = SessionResponse {
        username: granted.session.username,
        role: granted.session.role,
        csrf_token: granted.token.csrf(),
    };
    Ok(([(header::SET_COOKIE, cookie)], Json(body)).into_response())
}

#[derive(Serialize, ToSchema)]
pub struct PlayCodeResponse {
    /// Type it in the ID field of the game's login screen (password field empty).
    code: String,
    expires_in_seconds: u64,
}

/// Game login, step one: any active account (no panel role needed) trades its login and
/// password for a one-time code. Same throttle and lockout as the panel login.
#[utoipa::path(post, path = "/api/play/code", request_body = LoginRequest,
    responses((status = 200, body = PlayCodeResponse), (status = 401, body = ApiError),
              (status = 429, body = ApiError)))]
async fn play_code(
    State(state): State<AdminState>,
    ConnectInfoOpt(peer): ConnectInfoOpt,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<PlayCodeResponse>, Refusal> {
    check_origin(&headers)?;
    let password = Zeroizing::new(request.password.as_bytes().to_vec());
    let code = state
        .auth
        .issue_play_code(&request.username, password, peer)
        .await?;
    Ok(Json(PlayCodeResponse {
        code: code.as_str().to_owned(),
        expires_in_seconds: admin_session::PLAY_CODE_SECONDS,
    }))
}

/// The current session, with its CSRF token.
#[utoipa::path(get, path = "/api/session",
    responses((status = 200, body = SessionResponse), (status = 401, body = ApiError)))]
async fn session(auth: Authenticated) -> Json<SessionResponse> {
    Json(SessionResponse {
        csrf_token: auth.token.csrf(),
        username: auth.session.username,
        role: auth.session.role,
    })
}

#[utoipa::path(post, path = "/api/session/logout",
    responses((status = 204), (status = 401, body = ApiError), (status = 403, body = ApiError)))]
async fn logout(State(state): State<AdminState>, auth: Authenticated) -> Result<Response, Refusal> {
    state
        .auth
        .logout(&auth.token, &auth.session, auth.peer)
        .await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, session_cookie(&state, "", 0))],
    )
        .into_response())
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct AuditQuery {
    /// Only events older than this id (next page).
    before_id: Option<i64>,
    action: Option<String>,
    target_kind: Option<String>,
    target_id: Option<String>,
    /// 1–200, default 50.
    limit: Option<i64>,
}

/// Audit events, newest first.
#[utoipa::path(get, path = "/api/audit", params(AuditQuery),
    responses((status = 200, body = Vec<AuditEntry>), (status = 400, body = ApiError),
              (status = 401, body = ApiError)))]
async fn audit(
    State(state): State<AdminState>,
    auth: Authenticated,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<AuditEntry>>, Refusal> {
    auth.require(Permission::Read)?;
    let filter = AuditFilter {
        before_id: query.before_id,
        action: query.action,
        target_kind: query.target_kind,
        target_id: query.target_id,
        limit: query.limit.unwrap_or(50),
    };
    Ok(Json(
        admin_session::audit_page(state.auth.pool(), filter).await?,
    ))
}

/// Peer address when the listener provides it (absent in in-process tests).
struct ConnectInfoOpt(Option<IpAddr>);

impl<S: Send + Sync> FromRequestParts<S> for ConnectInfoOpt {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(peer(&parts.extensions)))
    }
}

/// Serve the panel until the listener fails or the task is aborted.
pub async fn serve(listener: tokio::net::TcpListener, app: Router) {
    let service = app.into_make_service_with_connect_info::<SocketAddr>();
    if axum::serve(listener, service).await.is_err() {
        tracing::warn!("admin panel listener stopped");
    }
}
