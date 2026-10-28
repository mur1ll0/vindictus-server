//! Item editor routes (docs/future/item-editor.md, cut 2): list and read items, save a new,
//! cloned or edited item, hide, remove or restore one, upload a model kit and read the state of
//! the server's item exports. Every change is a pending content edit published with the others.
use super::{AdminState, Authenticated, Refusal};
use crate::{
    application::{
        admin_session::AdminError,
        content_admin::Actor,
        item_editor::{
            self, DraftSaved, EditorItemDetail, EditorItemSummary, ItemContentStatus, ItemDraft,
            Sources,
        },
    },
    domain::admin::Permission,
    infrastructure::{item_kits, model_kit},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct ListQuery {
    /// Class or English name (2–64 characters; optional with a filter).
    #[serde(default)]
    search: String,
    /// added, edited, hidden or pending.
    filter: Option<String>,
    /// TradeCategory (WEAPON, PLATEARMOR...); the search is optional with it.
    category: Option<String>,
    /// TradeCategorySub (LONGSWORD, PLATEARMOR_HELM...).
    sub: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct SaveItem {
    operation_id: Uuid,
    draft: ItemDraft,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct HideItem {
    operation_id: Uuid,
    hidden: bool,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct ItemOperation {
    operation_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ItemEditCreated {
    /// The pending content edit.
    edit: i64,
}

fn sources(state: &AdminState) -> Sources<'_> {
    Sources {
        names: &state.names,
        client: state.client.clone(),
        assets: state
            .private_assets
            .as_deref()
            .map(std::path::PathBuf::as_path),
    }
}

/// Items of the content and of the item edits, by name (up to 200).
#[utoipa::path(operation_id = "item_editor_list", get, path = "/api/item-editor/items", params(ListQuery),
    responses((status = 200, body = Vec<EditorItemSummary>), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError)))]
pub(super) async fn list(
    State(state): State<AdminState>,
    auth: Authenticated,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<EditorItemSummary>>, Refusal> {
    Ok(Json(
        item_editor::list(
            state.auth.pool(),
            &auth.session,
            &state.names,
            &query.search,
            query.filter.as_deref(),
            (query.category.as_deref(), query.sub.as_deref()),
        )
        .await?,
    ))
}

/// One item: editable columns, definition of an added item, its edits and who owns it.
#[utoipa::path(operation_id = "item_editor_detail", get, path = "/api/item-editor/items/{item_class}", params(("item_class" = String, Path)),
    responses((status = 200, body = EditorItemDetail), (status = 404, body = super::ApiError)))]
pub(super) async fn detail(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(item_class): Path<String>,
) -> Result<Json<EditorItemDetail>, Refusal> {
    Ok(Json(
        item_editor::detail(state.auth.pool(), &auth.session, &state.names, &item_class).await?,
    ))
}

/// Save the item form: a new item (from a template, or a clone), an added item's change, or a
/// base item's override. The edit stays pending until a version is published.
#[utoipa::path(operation_id = "item_editor_save", post, path = "/api/item-editor/items", request_body = SaveItem,
    responses((status = 200, body = DraftSaved), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError), (status = 409, body = super::ApiError),
              (status = 503, body = super::ApiError)))]
pub(super) async fn save(
    State(state): State<AdminState>,
    auth: Authenticated,
    Json(body): Json<SaveItem>,
) -> Result<Json<DraftSaved>, Refusal> {
    let actor = Actor {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
    };
    Ok(Json(
        item_editor::save(state.auth.pool(), &actor, &sources(&state), &body.draft).await?,
    ))
}

/// Hide an item (or show it again): it leaves the NPC shops and the give list; owned items stay.
#[utoipa::path(operation_id = "item_editor_hide", post, path = "/api/item-editor/items/{item_class}/hide", params(("item_class" = String, Path)),
    request_body = HideItem,
    responses((status = 200, body = ItemEditCreated), (status = 404, body = super::ApiError),
              (status = 409, body = super::ApiError)))]
pub(super) async fn hide(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(item_class): Path<String>,
    Json(body): Json<HideItem>,
) -> Result<Json<ItemEditCreated>, Refusal> {
    let actor = Actor {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
    };
    let edit = item_editor::hide(
        state.auth.pool(),
        &actor,
        &state.names,
        &item_class,
        body.hidden,
    )
    .await?;
    Ok(Json(ItemEditCreated { edit }))
}

/// Remove an added item (refused while a character owns one).
#[utoipa::path(operation_id = "item_editor_remove", post, path = "/api/item-editor/items/{item_class}/remove", params(("item_class" = String, Path)),
    request_body = ItemOperation,
    responses((status = 200, body = ItemEditCreated), (status = 404, body = super::ApiError),
              (status = 409, body = super::ApiError)))]
pub(super) async fn remove(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(item_class): Path<String>,
    Json(body): Json<ItemOperation>,
) -> Result<Json<ItemEditCreated>, Refusal> {
    let actor = Actor {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
    };
    let edit = item_editor::remove(state.auth.pool(), &actor, &state.names, &item_class).await?;
    Ok(Json(ItemEditCreated { edit }))
}

/// Give a base item its original columns and text back.
#[utoipa::path(operation_id = "item_editor_restore", post, path = "/api/item-editor/items/{item_class}/restore", params(("item_class" = String, Path)),
    request_body = ItemOperation,
    responses((status = 200, body = ItemEditCreated), (status = 400, body = super::ApiError),
              (status = 404, body = super::ApiError)))]
pub(super) async fn restore(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(item_class): Path<String>,
    Json(body): Json<ItemOperation>,
) -> Result<Json<ItemEditCreated>, Refusal> {
    let actor = Actor {
        session: &auth.session,
        peer: auth.peer,
        operation_id: body.operation_id,
    };
    let edit = item_editor::restore(state.auth.pool(), &actor, &state.names, &item_class).await?;
    Ok(Json(ItemEditCreated { edit }))
}

/// Upload a model kit (the Blender add-on's zip: kit.json, models/ and materials/ files; up to
/// 64 MiB). The same zip always gets the same asset id.
#[utoipa::path(operation_id = "item_editor_upload_kit", post, path = "/api/item-editor/kits",
    request_body(content = Vec<u8>, content_type = "application/zip"),
    responses((status = 200, body = item_kits::StoredKit), (status = 409, body = super::ApiError),
              (status = 413), (status = 503, body = super::ApiError)))]
pub(super) async fn upload_kit(
    State(state): State<AdminState>,
    auth: Authenticated,
    body: Bytes,
) -> Result<Json<item_kits::StoredKit>, Refusal> {
    Ok(Json(
        item_editor::upload_kit(
            &auth.session,
            state
                .private_assets
                .as_deref()
                .map(std::path::PathBuf::as_path),
            body,
        )
        .await?,
    ))
}

/// A stored kit as a zip, for the form's 3D preview before the client copy has it.
#[utoipa::path(operation_id = "item_editor_kit", get, path = "/api/item-editor/kits/{assets}", params(("assets" = String, Path)),
    responses((status = 200, content_type = "application/zip"), (status = 404, body = super::ApiError)))]
pub(super) async fn kit(
    State(state): State<AdminState>,
    auth: Authenticated,
    Path(assets): Path<String>,
) -> Result<Response, Refusal> {
    auth.require(Permission::Manage)?;
    let root = state.private_assets.clone().ok_or(AdminError::NotFound)?;
    let zipped = tokio::task::spawn_blocking(move || -> Option<Vec<u8>> {
        let kit = item_kits::stored(&root, &assets).ok()?;
        let folder = root.join(&kit.assets);
        let mut files = vec![(
            "kit.json".to_owned(),
            serde_json::json!({ "model": kit.model })
                .to_string()
                .into_bytes(),
        )];
        for path in kit.files {
            let content = std::fs::read(folder.join(&path)).ok()?;
            files.push((path, content));
        }
        Some(model_kit::zip(&files))
    })
    .await
    .map_err(|_| AdminError::NotFound)?
    .ok_or(AdminError::NotFound)?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/zip"),
            (axum::http::header::CACHE_CONTROL, "private, no-store"),
        ],
        zipped,
    )
        .into_response())
}

/// Published item version against the server's exports (re-exported by the host).
#[utoipa::path(operation_id = "item_editor_status", get, path = "/api/item-editor/status",
    responses((status = 200, body = ItemContentStatus), (status = 403, body = super::ApiError)))]
pub(super) async fn status(
    State(state): State<AdminState>,
    auth: Authenticated,
) -> Result<Json<ItemContentStatus>, Refusal> {
    Ok(Json(
        item_editor::status(
            state.auth.pool(),
            &auth.session,
            state.item_stamp.as_deref().map(std::path::PathBuf::as_path),
        )
        .await?,
    ))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct CostumeQuery {
    /// ItemClassInfo.Category of the item (INNERARMOR, TUNIC, WEAPON...).
    category: String,
    /// An item whose EquipItemInfo.EquipClass picks the weapon or shield script (the
    /// item-model or the edited item).
    item: Option<String>,
}

/// One entry of the model script: what a CostumeType number means.
#[derive(Serialize, ToSchema)]
pub(super) struct CostumeOption {
    costume_type: i64,
    /// The model the client loads for it.
    model: Option<String>,
    /// The body (`category` of a player_costume.txt entry: fiona, female...).
    body: Option<String>,
    /// Items of the content using it (up to 5) and how many in all.
    items: Vec<CostumeItem>,
    item_count: usize,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CostumeItem {
    class: String,
    name: Option<String>,
}

/// The CostumeType numbers of an item category: the entries of its model script
/// (player_costume.txt, the weapon script or shields.txt) with their models and the items using
/// them.
#[utoipa::path(operation_id = "item_editor_costumes", get, path = "/api/item-editor/costumes",
    params(CostumeQuery),
    responses((status = 200, body = Vec<CostumeOption>), (status = 400, body = super::ApiError),
              (status = 403, body = super::ApiError)))]
pub(super) async fn costumes(
    State(state): State<AdminState>,
    auth: Authenticated,
    Query(query): Query<CostumeQuery>,
) -> Result<Json<Vec<CostumeOption>>, Refusal> {
    auth.require(Permission::Manage)?;
    let category = query.category.trim().to_ascii_uppercase();
    if category.is_empty()
        || category.len() > 32
        || !category
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b == b'_')
    {
        return Err(AdminError::InvalidInput.into());
    }
    let equip_class = query
        .item
        .as_deref()
        .and_then(|i| state.names.item_equip_class(i));
    let Some(script) = model_kit::ModelScript::of(&category, equip_class.as_deref()) else {
        return Ok(Json(Vec::new()));
    };
    let used = state.names.items_by_costume(&script);
    let files = state.client.clone();
    let lookup = script.clone();
    let entries = tokio::task::spawn_blocking(move || {
        let text = files.and_then(|f| f.read(&lookup.file()).ok().flatten());
        text.map(|t| {
            lookup
                .keys(&t)
                .into_iter()
                .map(|key| {
                    (
                        key,
                        lookup.model(&t, key),
                        lookup.value(&t, key, "category"),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
    })
    .await
    .map_err(|_| AdminError::NotFound)?;
    let mut rows: std::collections::BTreeMap<i64, (Option<String>, Option<String>)> = entries
        .into_iter()
        .map(|(key, model, body)| (key, (model, body)))
        .collect();
    // Numbers items use that the client script lacks (added items before the client update).
    for key in used.keys() {
        rows.entry(*key).or_default();
    }
    let mut out: Vec<CostumeOption> = rows
        .into_iter()
        .map(|(costume_type, (model, body))| {
            let items = used.get(&costume_type);
            CostumeOption {
                costume_type,
                model,
                body,
                item_count: items.map_or(0, Vec::len),
                items: items
                    .map(|v| {
                        v.iter()
                            .take(5)
                            .map(|(class, name)| CostumeItem {
                                class: class.clone(),
                                name: name.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect();
    out.sort_by_key(|o| o.costume_type);
    Ok(Json(out))
}
