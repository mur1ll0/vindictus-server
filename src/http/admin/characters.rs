//! Read-only character routes (B4.3a): an account's characters (staff), the session's own
//! characters, and one character's sheet (staff, or the owner).
use super::{AdminState, Authenticated, Refusal};
use crate::{
    application::admin_characters,
    infrastructure::admin_characters::{CharacterSheet, CharacterSummary},
};
use axum::{
    Json,
    extract::{Path, State},
};

/// Characters of an account's game user (staff).
#[utoipa::path(get, path = "/api/accounts/{username}/characters",
    params(("username" = String, Path)),
    responses((status = 200, body = Vec<CharacterSummary>), (status = 403, body = super::ApiError),
              (status = 404, body = super::ApiError)))]
pub(super) async fn of_account(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(username): Path<String>,
) -> Result<Json<Vec<CharacterSummary>>, Refusal> {
    Ok(Json(
        admin_characters::of_account(state.auth.pool(), &auth.session, &username).await?,
    ))
}

/// The session's own characters (any account).
#[utoipa::path(get, path = "/api/me/characters",
    responses((status = 200, body = Vec<CharacterSummary>), (status = 401, body = super::ApiError)))]
pub(super) async fn mine(
    State(state): State<AdminState>,
    auth: Authenticated,
) -> Result<Json<Vec<CharacterSummary>>, Refusal> {
    Ok(Json(
        admin_characters::mine(state.auth.pool(), &auth.session).await?,
    ))
}

/// One character: sheet, inventory with attributes, skills, status effects and titles.
#[utoipa::path(get, path = "/api/characters/{cid}", params(("cid" = String, Path)),
    responses((status = 200, body = CharacterSheet), (status = 404, body = super::ApiError)))]
pub(super) async fn sheet(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(cid): Path<String>,
) -> Result<Json<CharacterSheet>, Refusal> {
    Ok(Json(
        admin_characters::sheet(state.auth.pool(), &state.names, &auth.session, &cid).await?,
    ))
}
