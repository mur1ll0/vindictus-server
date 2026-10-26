//! Character routes: an account's characters (staff), the session's own characters, one
//! character's sheet (staff, or the owner) (B4.3a); item, gold and cash changes and the item
//! catalog (staff, B4.3b).
use super::{AdminState, Authenticated, Refusal};
use crate::{
    application::{
        admin_character_edits::{self, Change, ChangeResult},
        admin_characters,
    },
    infrastructure::admin_characters::{CharacterSheet, CharacterSummary},
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

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

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum ItemAction {
    Give,
    Remove,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct ItemChange {
    operation_id: Uuid,
    reason: String,
    /// give: an ItemClassEx (`gold`, `longsword[ENHANCE:6]`); remove: a plain class.
    item: String,
    /// 1 to 1 000 000 000.
    count: i32,
    action: ItemAction,
}

/// Give or remove items (gold included) with the game's item rules; the character must be
/// offline.
#[utoipa::path(post, path = "/api/characters/{cid}/items", params(("cid" = String, Path)),
    request_body = ItemChange,
    responses((status = 200, body = ChangeResult), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError), (status = 409, body = super::ApiError)))]
pub(super) async fn change_item(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(cid): Path<String>,
    Json(body): Json<ItemChange>,
) -> Result<Json<ChangeResult>, Refusal> {
    let change = Change {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
        reason: &body.reason,
    };
    Ok(Json(
        admin_character_edits::change_item(
            state.auth.pool(),
            state
                .runtime
                .as_ref()
                .and_then(|r| r.minimum_assets())
                .as_deref(),
            &change,
            &cid,
            &body.item,
            body.count,
            matches!(body.action, ItemAction::Give),
        )
        .await?,
    ))
}

#[derive(Deserialize, ToSchema)]
pub(super) struct CashChange {
    operation_id: Uuid,
    reason: String,
    /// Positive credits, negative debits; never below zero.
    delta: i32,
}

/// Credit or debit the cash balance of the character's account.
#[utoipa::path(post, path = "/api/characters/{cid}/cash", params(("cid" = String, Path)),
    request_body = CashChange,
    responses((status = 200, body = ChangeResult), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError), (status = 409, body = super::ApiError)))]
pub(super) async fn adjust_cash(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(cid): Path<String>,
    Json(body): Json<CashChange>,
) -> Result<Json<ChangeResult>, Refusal> {
    let change = Change {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
        reason: &body.reason,
    };
    Ok(Json(
        admin_character_edits::adjust_cash(state.auth.pool(), &change, &cid, body.delta).await?,
    ))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct CatalogQuery {
    /// 2–64 characters of the class or the English name.
    search: String,
}

/// Item catalog with the client's English names (staff).
#[utoipa::path(get, path = "/api/items", params(CatalogQuery),
    responses((status = 200, body = Vec<crate::infrastructure::admin_characters::CatalogItem>),
              (status = 400, body = super::ApiError), (status = 403, body = super::ApiError)))]
pub(super) async fn catalog(
    State(state): State<AdminState>,
    auth: Authenticated,
    axum::extract::Query(query): axum::extract::Query<CatalogQuery>,
) -> Result<Json<Vec<crate::infrastructure::admin_characters::CatalogItem>>, Refusal> {
    auth.require(crate::domain::admin::Permission::Read)?;
    let term = query.search.trim();
    if !(2..=64).contains(&term.chars().count()) {
        return Err(crate::application::admin_session::AdminError::InvalidInput.into());
    }
    Ok(Json(state.names.search_items(term, 50)))
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub(super) enum ProgressionKind {
    /// EXP through the game's GiveExp (levels up on the way).
    Exp {
        amount: i64,
    },
    /// Reach this level (the EXP the level table requires).
    Level {
        level: i32,
    },
    /// AP through GiveAP (capped like the game).
    Ap {
        amount: i32,
    },
    /// Acquire a title by its ID.
    Title {
        title_id: i32,
    },
    /// Add a status effect by Type and level; seconds null = the effect's own duration.
    AddEffect {
        effect: String,
        level: i32,
        seconds: Option<i32>,
    },
    RemoveEffect {
        effect: String,
    },
}

#[derive(Deserialize, ToSchema)]
pub(super) struct ProgressionChange {
    operation_id: Uuid,
    reason: String,
    change: ProgressionKind,
}

/// EXP, target level, AP, title or status effect; the character must be offline.
#[utoipa::path(post, path = "/api/characters/{cid}/progression", params(("cid" = String, Path)),
    request_body = ProgressionChange,
    responses((status = 200, body = crate::application::admin_character_edits::ProgressionResult),
              (status = 400, body = super::ApiError), (status = 403, body = super::ApiError),
              (status = 409, body = super::ApiError)))]
pub(super) async fn progression(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(cid): Path<String>,
    Json(body): Json<ProgressionChange>,
) -> Result<Json<crate::application::admin_character_edits::ProgressionResult>, Refusal> {
    use crate::infrastructure::admin_progression::PanelGrowth;
    let change = Change {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
        reason: &body.reason,
    };
    let growth = match &body.change {
        ProgressionKind::Exp { amount } => PanelGrowth::Exp(*amount),
        ProgressionKind::Level { level } => PanelGrowth::Level(*level),
        ProgressionKind::Ap { amount } => PanelGrowth::Ap(*amount),
        ProgressionKind::Title { title_id } => PanelGrowth::Title(*title_id),
        ProgressionKind::AddEffect {
            effect,
            level,
            seconds,
        } => PanelGrowth::Effect(effect, Some((*level, *seconds))),
        ProgressionKind::RemoveEffect { effect } => PanelGrowth::Effect(effect, None),
    };
    Ok(Json(
        admin_character_edits::change_progression(
            state.auth.pool(),
            state.runtime.as_deref(),
            &change,
            &cid,
            growth,
        )
        .await?,
    ))
}

/// Status effects of the content (Type, level, English name), for the panel's picker.
#[utoipa::path(get, path = "/api/status-effects", params(CatalogQuery),
    responses((status = 200, body = Vec<crate::infrastructure::admin_characters::CatalogEffect>),
              (status = 400, body = super::ApiError), (status = 403, body = super::ApiError)))]
pub(super) async fn effects_catalog(
    State(state): State<AdminState>,
    auth: Authenticated,
    axum::extract::Query(query): axum::extract::Query<CatalogQuery>,
) -> Result<Json<Vec<crate::infrastructure::admin_characters::CatalogEffect>>, Refusal> {
    auth.require(crate::domain::admin::Permission::Read)?;
    let term = query.search.trim();
    if !(2..=64).contains(&term.chars().count()) {
        return Err(crate::application::admin_session::AdminError::InvalidInput.into());
    }
    let Some(assets) = state.runtime.as_ref().and_then(|r| r.minimum_assets()) else {
        return Err(crate::application::admin_session::AdminError::ContentUnavailable.into());
    };
    let Some(contents) = assets.status_effects() else {
        return Err(crate::application::admin_session::AdminError::ContentUnavailable.into());
    };
    Ok(Json(state.names.search_effects(contents, term, 50)))
}
