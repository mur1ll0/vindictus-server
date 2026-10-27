//! Model kits of the client for the panel's 3D viewer and the Blender add-on
//! (docs/future/item-editor.md).
use super::{AdminState, Authenticated, Refusal};
use crate::{
    application::admin_session::AdminError,
    domain::admin::Permission,
    infrastructure::model_kit::{self, KitError},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, IntoParams)]
pub(super) struct KitQuery {
    /// Game path of the studio model (`models/player/fiona/fiona_inner_fringe.mdl`).
    model: String,
}

/// The model of an item: its entry (by CostumeType) in player_costume.txt, the weapon script
/// of its EquipClass or shields.txt.
#[derive(Serialize, ToSchema)]
pub(super) struct ItemModel {
    item_class: String,
    category: Option<String>,
    costume_type: Option<i64>,
    section: Option<String>,
    /// `None` when the item has no model the panel can resolve (hair, accessories...).
    model: Option<String>,
}

fn refusal(error: KitError) -> Refusal {
    match error {
        KitError::InvalidPath => AdminError::InvalidInput.into(),
        KitError::Missing => AdminError::NotFound.into(),
        KitError::Malformed | KitError::Client(_) => {
            tracing::warn!(error = %error, "model kit unavailable");
            AdminError::NotFound.into()
        }
    }
}

/// A model kit (stored zip: model, vertex and optimized model files, materials, textures and
/// kit.json) for the viewer and the Blender add-on.
#[utoipa::path(get, path = "/api/models/kit", params(KitQuery),
    responses((status = 200, content_type = "application/zip"), (status = 404, body = super::ApiError)))]
pub(super) async fn kit(
    State(state): State<AdminState>,
    auth: Authenticated,
    Query(query): Query<KitQuery>,
) -> Result<Response, Refusal> {
    auth.require(Permission::Manage)?;
    let files = state.client.clone().ok_or(AdminError::NotFound)?;
    let model = model_kit::model_path(&query.model).map_err(refusal)?;
    let name = model
        .rsplit('/')
        .next()
        .unwrap_or("model")
        .trim_end_matches(".mdl")
        .to_owned();
    let zipped = tokio::task::spawn_blocking(move || {
        model_kit::build(&files, &model).map(|f| model_kit::zip(&f))
    })
    .await
    .map_err(|_| AdminError::NotFound)?
    .map_err(refusal)?;
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/zip".to_owned(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}.zip\""),
            ),
            (
                axum::http::header::CACHE_CONTROL,
                "private, no-store".to_owned(),
            ),
        ],
        zipped,
    )
        .into_response())
}

/// The model of an item, resolved through player_costume.txt, the weapon scripts or
/// shields.txt.
#[utoipa::path(get, path = "/api/items/{item_class}/model", params(("item_class" = String, Path)),
    responses((status = 200, body = ItemModel), (status = 404, body = super::ApiError)))]
pub(super) async fn item_model(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(item_class): Path<String>,
) -> Result<Json<ItemModel>, Refusal> {
    auth.require(Permission::Manage)?;
    let valid = !item_class.is_empty()
        && item_class.len() <= 96
        && item_class
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !valid {
        return Err(AdminError::InvalidInput.into());
    }
    let files = state.client.clone().ok_or(AdminError::NotFound)?;
    let costume = state.names.item_costume(&item_class);
    let (category, costume_type) = costume.clone().unzip();
    let equip_class = state.names.item_equip_class(&item_class);
    let script = category
        .as_deref()
        .and_then(|c| model_kit::ModelScript::of(c, equip_class.as_deref()));
    let section = script.as_ref().map(model_kit::ModelScript::section);
    let model = match (script, costume_type) {
        (Some(script), Some(key)) => tokio::task::spawn_blocking(move || {
            files
                .read(&script.file())
                .ok()
                .flatten()
                .and_then(|text| script.model(&text, key))
        })
        .await
        .map_err(|_| AdminError::NotFound)?,
        _ => None,
    };
    Ok(Json(ItemModel {
        item_class: item_class.to_ascii_lowercase(),
        category,
        costume_type,
        section,
        model,
    }))
}
