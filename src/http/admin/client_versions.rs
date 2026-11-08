//! "Versões do cliente" (L5): the update store's published versions, the minimum version, withdrawing
//! the current one, and the launcher news. Changes carry an `operation_id` and are audited.
use super::{AdminState, Authenticated, Refusal};
use crate::application::client_drafts;
use crate::application::client_versions::{self, ClientVersionsView};
use crate::application::content_admin::Actor;
use crate::infrastructure::client_drafts::ItemRow;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
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

// ------------------------------------------------------------------ drafts (game-launcher.md 4.10)

fn staging(state: &AdminState) -> Option<&std::path::Path> {
    state
        .client_staging
        .as_deref()
        .map(std::path::PathBuf::as_path)
}

/// The open draft of the next client version, its changes and the client builder's jobs.
#[utoipa::path(get, path = "/api/client-versions/draft", operation_id = "client_versions_draft",
    responses((status = 200, body = client_drafts::DraftView), (status = 403, body = super::ApiError)))]
pub(super) async fn draft(
    State(state): State<AdminState>,
    auth: Authenticated,
) -> Result<Json<client_drafts::DraftView>, Refusal> {
    Ok(Json(
        client_drafts::view(state.auth.pool(), &auth.session, staging(&state)).await?,
    ))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct FileSearch {
    /// Part of the path (case-insensitive); empty lists the first files.
    #[serde(default)]
    search: String,
}

/// Published files of the current version whose path contains `search` (at most 200).
#[utoipa::path(get, path = "/api/client-versions/files", operation_id = "client_versions_files",
    params(FileSearch),
    responses((status = 200, body = Vec<client_drafts::PublishedFile>), (status = 403, body = super::ApiError)))]
pub(super) async fn files(
    State(state): State<AdminState>,
    auth: Authenticated,
    Query(query): Query<FileSearch>,
) -> Result<Json<Vec<client_drafts::PublishedFile>>, Refusal> {
    let search: String = query.search.chars().take(200).collect();
    Ok(Json(
        client_drafts::published_files(&auth.session, state.client_dist.as_deref(), search).await?,
    ))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct DraftFile {
    operation_id: Uuid,
    /// Game path, relative to the game folder ('/' or '\').
    path: String,
}

/// Add or replace the file at `path` in the draft (admin; body = the file, up to 256 MiB).
#[utoipa::path(put, path = "/api/client-versions/draft/file", operation_id = "client_versions_draft_file",
    params(DraftFile), request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, body = ItemRow), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError), (status = 409, body = super::ApiError), (status = 413)))]
pub(super) async fn upload_file(
    State(state): State<AdminState>,
    auth: Authenticated,
    Query(query): Query<DraftFile>,
    body: Bytes,
) -> Result<Json<ItemRow>, Refusal> {
    Ok(Json(
        client_drafts::upload_file(
            state.auth.pool(),
            &actor(&auth, query.operation_id),
            state.client_dist.as_deref(),
            staging(&state),
            query.path,
            body,
        )
        .await?,
    ))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct DraftPath {
    operation_id: Uuid,
    path: String,
}

/// Mark a published file for removal in the draft (admin): the launchers delete it.
#[utoipa::path(post, path = "/api/client-versions/draft/remove", operation_id = "client_versions_draft_remove",
    request_body = DraftPath,
    responses((status = 200, body = ItemRow), (status = 403, body = super::ApiError), (status = 409, body = super::ApiError)))]
pub(super) async fn remove_file(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<DraftPath>,
) -> Result<Json<ItemRow>, Refusal> {
    Ok(Json(
        client_drafts::remove_file(
            state.auth.pool(),
            &actor(&auth, body.operation_id),
            state.client_dist.as_deref(),
            body.path,
        )
        .await?,
    ))
}

/// Take a path out of the draft (admin).
#[utoipa::path(post, path = "/api/client-versions/draft/drop", operation_id = "client_versions_draft_drop",
    request_body = DraftPath,
    responses((status = 204), (status = 403, body = super::ApiError), (status = 404, body = super::ApiError)))]
pub(super) async fn drop_change(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<DraftPath>,
) -> Result<axum::http::StatusCode, Refusal> {
    client_drafts::drop_change(
        state.auth.pool(),
        &actor(&auth, body.operation_id),
        &body.path,
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct DraftOperation {
    operation_id: Uuid,
}

/// Discard the open draft (admin).
#[utoipa::path(post, path = "/api/client-versions/draft/discard", operation_id = "client_versions_draft_discard",
    request_body = DraftOperation,
    responses((status = 204), (status = 403, body = super::ApiError), (status = 404, body = super::ApiError)))]
pub(super) async fn discard(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<DraftOperation>,
) -> Result<axum::http::StatusCode, Refusal> {
    client_drafts::discard(state.auth.pool(), &actor(&auth, body.operation_id)).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub(super) struct JobQueued {
    job: i64,
}

/// Apply the published panel content to the server's client and put what changed in the draft (admin;
/// the client builder service does it in the background).
#[utoipa::path(post, path = "/api/client-versions/draft/content", operation_id = "client_versions_draft_content",
    request_body = DraftOperation,
    responses((status = 200, body = JobQueued), (status = 403, body = super::ApiError), (status = 409, body = super::ApiError)))]
pub(super) async fn request_content(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<DraftOperation>,
) -> Result<Json<JobQueued>, Refusal> {
    let job =
        client_drafts::request_content(state.auth.pool(), &actor(&auth, body.operation_id)).await?;
    Ok(Json(JobQueued { job }))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct PublishDraft {
    operation_id: Uuid,
    /// 1-200 characters, shown by the launcher.
    title: String,
    /// Up to 4000 characters.
    #[serde(default)]
    notes: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DraftPublished {
    version: u32,
}

/// Publish the draft as the next version (admin): the previous version plus its changes; launchers
/// must update before playing.
#[utoipa::path(post, path = "/api/client-versions/draft/publish", operation_id = "client_versions_draft_publish",
    request_body = PublishDraft,
    responses((status = 200, body = DraftPublished), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError), (status = 404, body = super::ApiError), (status = 409, body = super::ApiError)))]
pub(super) async fn publish_draft(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<PublishDraft>,
) -> Result<Json<DraftPublished>, Refusal> {
    let version = client_drafts::publish(
        state.auth.pool(),
        &actor(&auth, body.operation_id),
        state.client_dist.as_deref(),
        staging(&state),
        &body.title,
        &body.notes,
    )
    .await?;
    Ok(Json(DraftPublished { version }))
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
