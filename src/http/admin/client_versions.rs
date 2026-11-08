//! "Versões do cliente" (L5): the update store's published versions, the minimum version, withdrawing
//! the current one, and the launcher news. Changes carry an `operation_id` and are audited.
use super::{AdminState, Authenticated, Refusal};
use crate::application::client_versions::{self, ClientVersionsView};
use crate::application::content_admin::Actor;
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

fn actor(auth: &Authenticated, operation_id: Uuid) -> Actor<'_> {
    Actor {
        session: &auth.session,
        peer: auth.peer,
        operation_id,
    }
}

/// Published client versions, minimum, launcher and news.
#[utoipa::path(get, path = "/api/client-versions", operation_id = "client_versions",
    responses((status = 200, body = ClientVersionsView), (status = 403, body = super::ApiError)))]
pub(super) async fn view(
    State(state): State<AdminState>,
    auth: Authenticated,
) -> Result<Json<ClientVersionsView>, Refusal> {
    Ok(Json(
        client_versions::view(
            state.auth.pool(),
            &auth.session,
            state.client_dist.as_deref(),
        )
        .await?,
    ))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct SetMinimum {
    operation_id: Uuid,
    /// 1 to the current version.
    minimum: u32,
}

/// Oldest client version allowed to play (admin): the launcher asks the player to update below it.
#[utoipa::path(post, path = "/api/client-versions/minimum", operation_id = "client_versions_minimum", request_body = SetMinimum,
    responses((status = 204), (status = 400, body = super::ApiError), (status = 403, body = super::ApiError),
              (status = 409, body = super::ApiError)))]
pub(super) async fn set_minimum(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<SetMinimum>,
) -> Result<axum::http::StatusCode, Refusal> {
    client_versions::set_minimum(
        state.auth.pool(),
        &actor(&auth, body.operation_id),
        state.client_dist.as_deref(),
        body.minimum,
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct WithdrawVersion {
    operation_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub(super) struct Withdrawn {
    /// The new current version (with the files of the one before the withdrawn).
    version: u32,
}

/// Withdraw the current version (admin): publishes N+1 with the files of N-1; nothing is deleted.
#[utoipa::path(post, path = "/api/client-versions/withdraw", operation_id = "client_versions_withdraw", request_body = WithdrawVersion,
    responses((status = 200, body = Withdrawn), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError), (status = 409, body = super::ApiError)))]
pub(super) async fn withdraw(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<WithdrawVersion>,
) -> Result<Json<Withdrawn>, Refusal> {
    let version = client_versions::withdraw(
        state.auth.pool(),
        &actor(&auth, body.operation_id),
        state.client_dist.as_deref(),
    )
    .await?;
    Ok(Json(Withdrawn { version }))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct NewNews {
    operation_id: Uuid,
    /// 1-120 characters.
    title: String,
    /// 1-2000 characters; line breaks kept.
    body: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NewsCreated {
    id: i64,
}

/// A launcher news item (admin or GM); the signed feed is rewritten at once.
#[utoipa::path(post, path = "/api/client-versions/news", operation_id = "client_versions_add_news", request_body = NewNews,
    responses((status = 200, body = NewsCreated), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError)))]
pub(super) async fn add_news(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<NewNews>,
) -> Result<Json<NewsCreated>, Refusal> {
    let id = client_versions::add_news(
        state.auth.pool(),
        &actor(&auth, body.operation_id),
        state.client_dist.as_deref(),
        &body.title,
        &body.body,
    )
    .await?;
    Ok(Json(NewsCreated { id }))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct NewsVisibility {
    operation_id: Uuid,
    hidden: bool,
}

/// Hide a news item from the launcher, or show it again (admin or GM).
#[utoipa::path(post, path = "/api/client-versions/news/{id}/hidden", operation_id = "client_versions_news_hidden", request_body = NewsVisibility,
    params(("id" = i64, Path, description = "News id")),
    responses((status = 204), (status = 403, body = super::ApiError), (status = 404, body = super::ApiError)))]
pub(super) async fn set_news_hidden(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(id): Path<i64>,
    Json(body): Json<NewsVisibility>,
) -> Result<axum::http::StatusCode, Refusal> {
    client_versions::set_news_hidden(
        state.auth.pool(),
        &actor(&auth, body.operation_id),
        state.client_dist.as_deref(),
        id,
        body.hidden,
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
