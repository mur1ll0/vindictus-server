//! Maps and channels route (B4.6): read-only.
use super::{AdminState, Authenticated, Refusal};
use crate::application::world_admin::{self, WorldView};
use axum::{Json, extract::State};

/// The laboratory's channel, its members, the mission relay, the ports and the content
/// version loaded by the game.
#[utoipa::path(get, path = "/api/world",
    responses((status = 200, body = WorldView), (status = 403, body = super::ApiError)))]
pub(super) async fn world(
    State(state): State<AdminState>,
    auth: Authenticated,
) -> Result<Json<WorldView>, Refusal> {
    Ok(Json(
        world_admin::world(state.auth.pool(), &auth.session, state.runtime.as_deref()).await?,
    ))
}
