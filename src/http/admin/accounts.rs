//! Account administration routes (B4.2). Every change carries an `operation_id` (a UUID the
//! page creates per submission) and a mandatory reason, both recorded in the audit.
use super::{AdminState, Authenticated, Refusal};
use crate::{
    application::admin_accounts::{self, Request},
    domain::admin::{PanelRole, Permission},
    infrastructure::admin_accounts::AccountSummary,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct AccountQuery {
    /// Part of the login (letters, digits, underscore).
    search: Option<String>,
    /// Next page: logins after this one.
    after: Option<String>,
    /// 1–100, default 50.
    limit: Option<i64>,
}

/// Accounts in login order.
#[utoipa::path(get, path = "/api/accounts", params(AccountQuery),
    responses((status = 200, body = Vec<AccountSummary>), (status = 400, body = super::ApiError),
              (status = 401, body = super::ApiError)))]
pub(super) async fn list(
    State(state): State<AdminState>,
    auth: Authenticated,
    Query(query): Query<AccountQuery>,
) -> Result<Json<Vec<AccountSummary>>, Refusal> {
    auth.require(Permission::Read)?;
    Ok(Json(
        admin_accounts::list(
            state.auth.pool(),
            query.search.as_deref(),
            query.after.as_deref(),
            query.limit.unwrap_or(50),
        )
        .await?,
    ))
}

fn request<'a>(auth: &'a Authenticated, operation_id: Uuid, reason: &'a str) -> Request<'a> {
    Request {
        session: &auth.session,
        peer: auth.peer,
        operation_id,
        reason,
    }
}

#[derive(Deserialize, ToSchema)]
pub(super) struct CreateAccount {
    operation_id: Uuid,
    reason: String,
    /// 3–24 letters, digits or underscore (fits the game's account name).
    username: String,
    /// 12–128 bytes.
    #[schema(value_type = String, format = Password)]
    password: Zeroizing<String>,
    /// Panel role; only an admin can give one.
    #[schema(value_type = Option<String>, example = "gm")]
    panel_role: Option<PanelRole>,
}

#[utoipa::path(post, path = "/api/accounts", request_body = CreateAccount,
    responses((status = 204), (status = 400, body = super::ApiError), (status = 403, body = super::ApiError),
              (status = 409, body = super::ApiError)))]
pub(super) async fn create(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<CreateAccount>,
) -> Result<StatusCode, Refusal> {
    let password = Zeroizing::new(body.password.as_bytes().to_vec());
    admin_accounts::create(
        state.auth.pool(),
        &request(&auth, body.operation_id, &body.reason),
        &body.username,
        password,
        body.panel_role,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct SetRole {
    operation_id: Uuid,
    reason: String,
    /// admin, gm, leitura or null to remove panel access.
    #[schema(value_type = Option<String>, example = "leitura")]
    panel_role: Option<PanelRole>,
}

#[utoipa::path(post, path = "/api/accounts/{username}/role", request_body = SetRole,
    params(("username" = String, Path)),
    responses((status = 204), (status = 403, body = super::ApiError), (status = 404, body = super::ApiError),
              (status = 409, body = super::ApiError)))]
pub(super) async fn set_role(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(username): Path<String>,
    Json(body): Json<SetRole>,
) -> Result<StatusCode, Refusal> {
    admin_accounts::set_role(
        state.auth.pool(),
        &request(&auth, body.operation_id, &body.reason),
        &username,
        body.panel_role,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct SetPassword {
    operation_id: Uuid,
    reason: String,
    #[schema(value_type = String, format = Password)]
    password: Zeroizing<String>,
}

/// New password; the account's panel sessions end.
#[utoipa::path(post, path = "/api/accounts/{username}/password", request_body = SetPassword,
    params(("username" = String, Path)),
    responses((status = 204), (status = 400, body = super::ApiError), (status = 403, body = super::ApiError),
              (status = 404, body = super::ApiError)))]
pub(super) async fn set_password(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(username): Path<String>,
    Json(body): Json<SetPassword>,
) -> Result<StatusCode, Refusal> {
    let password = Zeroizing::new(body.password.as_bytes().to_vec());
    admin_accounts::set_password(
        state.auth.pool(),
        &request(&auth, body.operation_id, &body.reason),
        &username,
        password,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct SetStatus {
    operation_id: Uuid,
    reason: String,
    active: bool,
}

/// Enable or disable an account; disabling ends its panel sessions.
#[utoipa::path(post, path = "/api/accounts/{username}/status", request_body = SetStatus,
    params(("username" = String, Path)),
    responses((status = 204), (status = 400, body = super::ApiError), (status = 403, body = super::ApiError),
              (status = 404, body = super::ApiError), (status = 409, body = super::ApiError)))]
pub(super) async fn set_status(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(username): Path<String>,
    Json(body): Json<SetStatus>,
) -> Result<StatusCode, Refusal> {
    admin_accounts::set_status(
        state.auth.pool(),
        &request(&auth, body.operation_id, &body.reason),
        &username,
        body.active,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
