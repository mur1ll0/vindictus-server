//! NPC shop and cash shop routes (B4.4): the loaded content, pending edits, versions and
//! publishing. A published version reaches the game through scripts/content/
//! apply-content-edits.ps1 on the host.
use super::{AdminState, Authenticated, Refusal};
use crate::{
    application::content_admin::{
        self, Actor, CashCategoryView, EditRequest, ShopRowView, ShopSummary,
    },
    infrastructure::content_edits::{ContentEdit, ContentVersion},
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

fn assets(
    state: &AdminState,
) -> Option<std::sync::Arc<crate::infrastructure::minimum_assets::Assets>> {
    state.runtime.as_ref().and_then(|r| r.minimum_assets())
}

/// NPC shops of the loaded content, with their row counts.
#[utoipa::path(get, path = "/api/shops",
    responses((status = 200, body = Vec<ShopSummary>), (status = 403, body = super::ApiError)))]
pub(super) async fn shops(
    State(state): State<AdminState>,
    auth: Authenticated,
) -> Result<Json<Vec<ShopSummary>>, Refusal> {
    Ok(Json(content_admin::shops(
        &auth.session,
        assets(&state).as_deref(),
        &state.names,
    )?))
}

/// The rows of one NPC shop as the game sells them.
#[utoipa::path(get, path = "/api/shops/{shop_id}", params(("shop_id" = String, Path)),
    responses((status = 200, body = Vec<ShopRowView>), (status = 404, body = super::ApiError)))]
pub(super) async fn shop(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(shop_id): Path<String>,
) -> Result<Json<Vec<ShopRowView>>, Refusal> {
    Ok(Json(content_admin::shop(
        &auth.session,
        assets(&state).as_deref(),
        &state.names,
        &shop_id,
    )?))
}

/// Cash shop categories with product counts and prices.
#[utoipa::path(get, path = "/api/cash-shop/categories",
    responses((status = 200, body = Vec<CashCategoryView>), (status = 403, body = super::ApiError)))]
pub(super) async fn cash_categories(
    State(state): State<AdminState>,
    auth: Authenticated,
) -> Result<Json<Vec<CashCategoryView>>, Refusal> {
    Ok(Json(content_admin::cash_categories(
        &auth.session,
        assets(&state).as_deref(),
    )?))
}

#[derive(Serialize, ToSchema)]
pub(super) struct ContentState {
    /// Pending first, then the latest.
    edits: Vec<ContentEdit>,
    versions: Vec<ContentVersion>,
}

#[utoipa::path(get, path = "/api/content",
    responses((status = 200, body = ContentState), (status = 403, body = super::ApiError)))]
pub(super) async fn content_state(
    State(state): State<AdminState>,
    auth: Authenticated,
) -> Result<Json<ContentState>, Refusal> {
    let (edits, versions) = content_admin::edits(state.auth.pool(), &auth.session).await?;
    Ok(Json(ContentState { edits, versions }))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct NewEdit {
    operation_id: Uuid,
    reason: String,
    edit: EditRequest,
}

#[derive(Serialize, ToSchema)]
pub(super) struct Created {
    id: i64,
}

/// A pending edit, checked against the loaded content.
#[utoipa::path(post, path = "/api/content/edits", request_body = NewEdit,
    responses((status = 200, body = Created), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError), (status = 409, body = super::ApiError)))]
pub(super) async fn create_edit(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<NewEdit>,
) -> Result<Json<Created>, Refusal> {
    let actor = Actor {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
        reason: &body.reason,
    };
    let id = content_admin::create_edit(
        state.auth.pool(),
        assets(&state).as_deref(),
        &actor,
        &body.edit,
    )
    .await?;
    Ok(Json(Created { id }))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct Reasoned {
    operation_id: Uuid,
    reason: String,
}

#[utoipa::path(post, path = "/api/content/edits/{id}/withdraw", params(("id" = i64, Path)),
    request_body = Reasoned,
    responses((status = 204), (status = 404, body = super::ApiError)))]
pub(super) async fn withdraw(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(id): Path<i64>,
    Json(body): Json<Reasoned>,
) -> Result<axum::http::StatusCode, Refusal> {
    let actor = Actor {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
        reason: &body.reason,
    };
    content_admin::withdraw(state.auth.pool(), &actor, id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub(super) struct Published {
    version: i64,
}

/// Publish the pending edits as the next version (admin); apply it on the host with
/// scripts/content/apply-content-edits.ps1.
#[utoipa::path(post, path = "/api/content/publish", request_body = Reasoned,
    responses((status = 200, body = Published), (status = 403, body = super::ApiError),
              (status = 409, body = super::ApiError)))]
pub(super) async fn publish(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<Reasoned>,
) -> Result<Json<Published>, Refusal> {
    let actor = Actor {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
        reason: &body.reason,
    };
    let version = content_admin::publish(state.auth.pool(), &actor).await?;
    Ok(Json(Published { version }))
}

/// Item icon of the client (PNG exported by export_panel_catalog.py), for the panel's lists.
#[utoipa::path(get, path = "/api/icons/item/{icon}", params(("icon" = String, Path)),
    responses((status = 200, content_type = "image/png"), (status = 404, body = super::ApiError)))]
pub(super) async fn item_icon(
    State(state): State<AdminState>,
    _auth: Authenticated,
    Path(icon): Path<String>,
) -> Result<axum::response::Response, Refusal> {
    use axum::response::IntoResponse;
    let name = icon.strip_suffix(".png").unwrap_or(&icon);
    let valid = !name.is_empty()
        && name.len() <= 96
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    let Some(dir) = state.icons_dir.as_ref().filter(|_| valid) else {
        return Err(crate::application::admin_session::AdminError::NotFound.into());
    };
    let bytes = tokio::fs::read(dir.join("item").join(format!("{name}.png")))
        .await
        .map_err(|_| Refusal::from(crate::application::admin_session::AdminError::NotFound))?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "image/png"),
            (axum::http::header::CACHE_CONTROL, "private, max-age=86400"),
        ],
        bytes,
    )
        .into_response())
}
