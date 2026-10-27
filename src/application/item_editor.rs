//! Item editor of the admin panel (docs/future/item-editor.md, cut 2): list, create from a
//! template, clone, edit, hide and remove items. The server content database is evidence and is
//! never changed: every change is a content edit (migration 0049) that stays pending until an
//! admin publishes a version (B4.4). The host then re-exports the server's items and updates the
//! client copy (scripts/content/refresh_item_content.py, run by start-lab.bat):
//! - added items (item_additions.json or the panel) are edited directly (`item`);
//! - base items take an override of their editable columns and text (`item_override`);
//! - any item can be hidden (`item_hide`): it leaves the NPC shops and the give list, and the
//!   items characters own keep working;
//! - only added items are removed (`item_remove`); when the removal loads, the game takes every
//!   item of the class from its owners (unequipped first) and drops queued deliveries.
use super::{
    admin_session::AdminError,
    content_admin::{Actor, audit, outcome, require},
};
use crate::{
    domain::{
        admin::Permission,
        item_editor::{
            self, BODIES, DraftError, EQUIP_TABLE, ITEM_TABLE, MAX_DESCRIPTION, MAX_NAME,
        },
    },
    infrastructure::{
        admin_characters::{self, EditorEntry, EnglishNames, ItemUsage},
        admin_store::SessionAccount,
        client_files::ClientFiles,
        content_edits::{self, ContentEdit},
        item_kits::{self, StoredKit},
        model_kit::ModelScript,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::PgPool;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};

const MAX_LISTED: usize = 200;
const MAX_ITEM_EDITS: i64 = 5000;

/// Where the panel finds what it needs besides the database.
pub struct Sources<'a> {
    pub names: &'a EnglishNames,
    /// The client copy, for the model entries and the kit path check.
    pub client: Option<Arc<ClientFiles>>,
    /// Private assets folder (.cache/private-assets) where kits are kept.
    pub assets: Option<&'a Path>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EditorItemSummary {
    pub item_class: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    /// base, added or edited (a base item with a panel override).
    pub origin: String,
    pub hidden: bool,
    /// An item edit of this class waits to be published.
    pub pending: bool,
    /// The server's exports have the item (false for an added item not re-exported yet).
    pub exported: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EditorItemDetail {
    pub item_class: String,
    pub origin: String,
    pub hidden: bool,
    pub exported: bool,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    /// Editable columns (ItemClassInfo and EquipItemInfo) as exported; missing = 0 or empty.
    #[schema(value_type = Object)]
    pub fields: Map<String, Value>,
    /// Added items: the definition the content applies (template, set, text, costumes, assets,
    /// set_id), the newest one, pending or published.
    #[schema(value_type = Option<Object>)]
    pub definition: Option<Value>,
    pub set_id: Option<String>,
    /// Item edits of this class not withdrawn (pending and published), oldest first.
    pub edits: Vec<ContentEdit>,
    pub usage: ItemUsage,
}

/// The item form (new, clone or edit).
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ItemDraft {
    pub item_class: String,
    /// New items: the base item whose rows it copies (a clone of an added item uses that
    /// item's template).
    pub template: Option<String>,
    pub name: String,
    pub description: Option<String>,
    /// Editable columns (domain::item_editor::EDITABLE) → value.
    #[schema(value_type = Object)]
    pub fields: Map<String, Value>,
    /// Equipment set (SetItemInfo SetID) of an added item.
    pub set_id: Option<String>,
    /// A kit uploaded with POST /api/item-editor/kits: the item gets this model under a new
    /// CostumeType.
    pub kit: Option<String>,
    /// Body of the model's player_costume.txt entry (male, female, giant, tall); default: the
    /// template's.
    pub body: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DraftSaved {
    /// The pending content edit.
    pub edit: i64,
    /// item (added item) or item_override (base item).
    pub kind: String,
    /// CostumeType given to the kit's model.
    pub costume_type: Option<i64>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ItemContentStatus {
    /// Newest published version with an item edit (0 = none).
    pub published_version: i32,
    /// Version the host last re-exported the server's items for (refresh_item_content.py).
    pub exported_version: Option<i32>,
    /// The server runs the published items; when false, run start-lab.bat (or
    /// scripts/content/refresh_item_content.py) on the host.
    pub up_to_date: bool,
}

/// What a class is for the editor.
enum Known {
    Base(EditorEntry),
    /// Added item and its current definition.
    Added(Value),
    Unknown,
}

fn origin(entry: Option<&EditorEntry>, added: bool) -> &'static str {
    match entry.map(|e| e.origin.as_str()) {
        _ if added => "added",
        Some("a") => "added",
        Some("e") => "edited",
        _ => "base",
    }
}

/// The newest definition of an added item among the item edits (None once removed).
fn edited_definition(edits: &[ContentEdit], class: &str) -> Option<Option<Value>> {
    edits
        .iter()
        .rev()
        .filter(|e| e.target == class)
        .find_map(|e| match e.kind.as_str() {
            "item" => Some(
                e.value
                    .as_deref()
                    .and_then(|v| serde_json::from_str(v).ok()),
            ),
            "item_remove" => Some(None),
            _ => None,
        })
}

fn known(names: &EnglishNames, edits: &[ContentEdit], class: &str) -> Known {
    let entry = names.editor_item(class);
    match (edited_definition(edits, class), entry) {
        (Some(Some(definition)), _) => Known::Added(definition),
        (Some(None), _) => Known::Unknown,
        (None, Some(e)) if e.origin == "a" => {
            Known::Added(e.definition.clone().unwrap_or(Value::Null))
        }
        (None, Some(e)) => Known::Base(e.clone()),
        (None, None) => Known::Unknown,
    }
}

fn hidden(names: &EnglishNames, edits: &[ContentEdit], class: &str) -> bool {
    edits
        .iter()
        .rev()
        .filter(|e| e.target == class && e.kind == "item_hide")
        .find_map(|e| {
            let v: Value = serde_json::from_str(e.value.as_deref()?).ok()?;
            v["hidden"].as_bool()
        })
        .unwrap_or_else(|| names.editor_item(class).is_some_and(|e| e.hidden == 1))
}

fn same_value(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn invalid(_: DraftError) -> AdminError {
    AdminError::InvalidInput
}

pub async fn list(
    pool: &PgPool,
    session: &SessionAccount,
    names: &EnglishNames,
    term: &str,
    filter: Option<&str>,
    trade: (Option<&str>, Option<&str>),
) -> Result<Vec<EditorItemSummary>, AdminError> {
    require(session, Permission::Manage)?;
    let term = term.trim().to_lowercase();
    if term.chars().count() > 64
        || (filter.is_none() && trade.0.is_none() && term.chars().count() < 2)
    {
        return Err(AdminError::InvalidInput);
    }
    if !matches!(
        filter,
        None | Some("added" | "edited" | "hidden" | "pending")
    ) {
        return Err(AdminError::InvalidInput);
    }
    let edits = content_edits::item_edits(pool, MAX_ITEM_EDITS).await?;
    let pending: BTreeSet<&str> = edits
        .iter()
        .filter(|e| e.state == "pending")
        .map(|e| e.target.as_str())
        .collect();
    // Exported items, then added items that only exist as edits so far.
    let mut classes: BTreeSet<String> = names
        .editor_items()
        .map(|(c, _)| c.to_ascii_lowercase())
        .collect();
    classes.extend(
        edits
            .iter()
            .filter(|e| e.kind == "item")
            .map(|e| e.target.clone()),
    );
    let mut found = Vec::new();
    for class in classes {
        let entry = names.editor_item(&class);
        let state = known(names, &edits, &class);
        if matches!(state, Known::Unknown) {
            continue;
        }
        let added = matches!(state, Known::Added(_));
        let item = EditorItemSummary {
            name: names.item_name(&class).or_else(|| match &state {
                Known::Added(d) => d["text"]["name"].as_str().map(str::to_owned),
                _ => None,
            }),
            icon: names.item_icon(&class),
            category: entry.and_then(|e| e.fields.get("Category")?.as_str().map(str::to_owned)),
            origin: origin(entry, added).to_owned(),
            hidden: hidden(names, &edits, &class),
            pending: pending.contains(class.as_str()),
            exported: entry.is_some(),
            item_class: class,
        };
        let wanted = match filter {
            Some("added") => item.origin == "added",
            Some("edited") => item.origin == "edited",
            Some("hidden") => item.hidden,
            Some("pending") => item.pending,
            _ => true,
        };
        let matches = term.is_empty()
            || item.item_class.contains(&term)
            || item
                .name
                .as_ref()
                .is_some_and(|n| n.to_lowercase().contains(&term));
        // Marketplace category (TradeCategory/TradeCategorySub of the exported row).
        let field = |key: &str| entry.and_then(|e| e.fields.get(key)?.as_str());
        let in_trade = trade.0.is_none_or(|c| field("TradeCategory") == Some(c))
            && trade.1.is_none_or(|s| field("TradeCategorySub") == Some(s));
        if wanted && matches && in_trade {
            found.push(item);
        }
    }
    found.sort_by(|a, b| {
        (a.name.is_none(), &a.name, &a.item_class).cmp(&(b.name.is_none(), &b.name, &b.item_class))
    });
    found.truncate(MAX_LISTED);
    Ok(found)
}

pub async fn detail(
    pool: &PgPool,
    session: &SessionAccount,
    names: &EnglishNames,
    class: &str,
) -> Result<EditorItemDetail, AdminError> {
    require(session, Permission::Manage)?;
    item_editor::item_class(class).map_err(|_| AdminError::NotFound)?;
    let edits = content_edits::item_edits(pool, MAX_ITEM_EDITS).await?;
    let state = known(names, &edits, class);
    let entry = names.editor_item(class);
    let definition = match &state {
        Known::Unknown => return Err(AdminError::NotFound),
        Known::Added(d) => Some(d.clone()),
        Known::Base(_) => None,
    };
    let usage = admin_characters::item_usage(pool, class).await?;
    Ok(EditorItemDetail {
        item_class: class.to_owned(),
        origin: origin(entry, definition.is_some()).to_owned(),
        hidden: hidden(names, &edits, class),
        exported: entry.is_some(),
        name: names.item_name(class),
        description: names.item_description(class),
        icon: names.item_icon(class),
        fields: entry.map(|e| e.fields.clone()).unwrap_or_default(),
        set_id: definition
            .as_ref()
            .and_then(|d| d["set_id"].as_str().map(str::to_owned))
            .or_else(|| names.item_set(class)),
        definition,
        edits: edits.into_iter().filter(|e| e.target == class).collect(),
        usage,
    })
}

/// The checked columns of the form by table ({"ItemClassInfo": {...}, "EquipItemInfo": {...}}).
fn columns(
    names: &EnglishNames,
    fields: &Map<String, Value>,
) -> Result<BTreeMap<&'static str, Map<String, Value>>, AdminError> {
    if fields.len() > item_editor::EDITABLE.len() {
        return Err(AdminError::InvalidInput);
    }
    let mut out: BTreeMap<&'static str, Map<String, Value>> = BTreeMap::new();
    for (column, raw) in fields {
        let (table, _) = item_editor::rule(column).ok_or(AdminError::InvalidInput)?;
        let value = item_editor::value(column, raw).map_err(invalid)?;
        if column == "Icon" && !value.as_str().is_some_and(|i| names.known_icon(i)) {
            return Err(AdminError::InvalidInput);
        }
        out.entry(table).or_default().insert(column.clone(), value);
    }
    Ok(out)
}

fn text_of(draft: &ItemDraft) -> Result<Map<String, Value>, AdminError> {
    item_editor::text(&draft.name, MAX_NAME).map_err(invalid)?;
    let mut text = Map::new();
    text.insert("name".into(), json!(draft.name.trim()));
    if let Some(desc) = draft
        .description
        .as_deref()
        .filter(|d| !d.trim().is_empty())
    {
        item_editor::text(desc, MAX_DESCRIPTION).map_err(invalid)?;
        text.insert("desc".into(), json!(desc.trim()));
    }
    Ok(text)
}

/// CostumeTypes in use in a model script: the client's entries, every exported item whose model
/// the script holds, and every added item's entries for it.
fn used_costume_types(
    names: &EnglishNames,
    edits: &[ContentEdit],
    text: Option<&[u8]>,
    script: &ModelScript,
) -> BTreeSet<i64> {
    let section = script.section();
    let mut used = text.map(|t| script.keys(t)).unwrap_or_default();
    for (class, entry) in names.editor_items() {
        let category = entry.fields.get("Category").and_then(Value::as_str);
        let same =
            category.and_then(|c| ModelScript::of(c, names.item_equip_class(class).as_deref()));
        if same.as_ref() == Some(script)
            && let Some(key) = entry.fields.get("CostumeType").and_then(Value::as_i64)
        {
            used.insert(key);
        }
        if let Some(definition) = &entry.definition {
            used.extend(costume_keys_of(definition, &section));
        }
    }
    for edit in edits.iter().filter(|e| e.kind == "item") {
        if let Some(definition) = edit
            .value
            .as_deref()
            .and_then(|v| serde_json::from_str(v).ok())
        {
            used.extend(costume_keys_of(&definition, &section));
        }
    }
    used
}

fn costume_keys_of(definition: &Value, section: &str) -> Vec<i64> {
    definition["costumes"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|c| c["section"].as_str() == Some(section))
                .filter_map(|c| c["key"].as_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// A free CostumeType: clothes and armor from the editor's range (from 9100, as the first lab
/// items proved in the client), weapons and shields right after their script's last number.
fn allocate(script: &ModelScript, used: &BTreeSet<i64>) -> Option<i64> {
    if script.costume_range() {
        item_editor::allocate_costume_type(used)
    } else {
        let next = used.iter().next_back().map_or(0, |k| k + 1);
        (next <= item_editor::LAST_COSTUME_TYPE).then_some(next)
    }
}

async fn read_script(
    client: Arc<ClientFiles>,
    file: String,
) -> Result<Option<Vec<u8>>, AdminError> {
    tokio::task::spawn_blocking(move || client.read(&file).ok().flatten())
        .await
        .map_err(|_| AdminError::ContentUnavailable)
}

/// The new model's script entry, assets and CostumeType for a kit.
#[allow(clippy::too_many_arguments)]
async fn kit_costume(
    sources: &Sources<'_>,
    edits: &[ContentEdit],
    kit: &str,
    script: Option<ModelScript>,
    template_costume: Option<i64>,
    previous: Option<&Value>,
    body: Option<&str>,
) -> Result<(Value, String, i64), AdminError> {
    let root = sources.assets.ok_or(AdminError::ContentUnavailable)?;
    let client = sources
        .client
        .clone()
        .ok_or(AdminError::ContentUnavailable)?;
    let stored: StoredKit = item_kits::stored(root, kit).map_err(|_| AdminError::InvalidInput)?;
    let script = script.ok_or(AdminError::Unchanged("modelo_sem_secao"))?;
    let section = script.section();
    // The item's earlier kit may already be in the client copy under the same paths.
    let earlier: BTreeSet<String> = previous
        .and_then(|p| p["assets"].as_str())
        .and_then(|a| item_kits::stored(root, a).ok())
        .map(|k| k.files.into_iter().collect())
        .unwrap_or_default();
    let files = stored.files.clone();
    let check = client.clone();
    let collides = tokio::task::spawn_blocking(move || {
        files
            .iter()
            .any(|f| !earlier.contains(f) && check.read(f).ok().flatten().is_some())
    })
    .await
    .map_err(|_| AdminError::ContentUnavailable)?;
    if collides {
        return Err(AdminError::Unchanged("kit_colide"));
    }
    let text = read_script(client, script.file()).await?;
    let earlier_entry = previous
        .and_then(|p| p["costumes"].as_array())
        .and_then(|list| {
            list.iter()
                .find(|c| c["section"].as_str() == Some(section.as_str()))
        });
    let key = match earlier_entry.and_then(|c| c["key"].as_str()?.parse::<i64>().ok()) {
        Some(key) => key,
        None => allocate(
            &script,
            &used_costume_types(sources.names, edits, text.as_deref(), &script),
        )
        .ok_or(AdminError::Unchanged("sem_costume_type"))?,
    };
    let mut entry = json!({ "section": section, "key": key.to_string(), "normal": stored.model });
    // Only player_costume.txt entries have a body.
    if script.costume_range() {
        let body = match body {
            Some(b) if BODIES.contains(&b) => b.to_owned(),
            Some(_) => return Err(AdminError::InvalidInput),
            None => earlier_entry
                .and_then(|c| c["category"].as_str().map(str::to_owned))
                .or_else(|| {
                    let body = script.value(text.as_deref()?, template_costume?, "category")?;
                    let body = body.to_ascii_lowercase();
                    BODIES.contains(&body.as_str()).then_some(body)
                })
                .ok_or(AdminError::InvalidInput)?,
        };
        entry["category"] = json!(body);
    }
    Ok((json!([entry]), stored.assets, key))
}

pub async fn save(
    pool: &PgPool,
    actor: &Actor<'_>,
    sources: &Sources<'_>,
    draft: &ItemDraft,
) -> Result<DraftSaved, AdminError> {
    require(actor.session, Permission::Manage)?;
    let names = sources.names;
    item_editor::item_class(&draft.item_class).map_err(invalid)?;
    let class = draft.item_class.as_str();
    let mut tables = columns(names, &draft.fields)?;
    let text = text_of(draft)?;
    let edits = content_edits::item_edits(pool, MAX_ITEM_EDITS).await?;
    let set_id = draft.set_id.as_deref().filter(|s| !s.is_empty());
    if set_id.is_some_and(|s| !names.set_ids().contains(s)) {
        return Err(AdminError::InvalidInput);
    }
    let (kind, value, costume_type) = match known(names, &edits, class) {
        Known::Base(entry) => {
            // Base items: the columns and text that differ from the exported item, plus what
            // an earlier override already changed (left out, it would go back to the original).
            if draft.template.is_some()
                || draft.kit.is_some()
                || set_id != names.item_set(class).as_deref()
            {
                return Err(AdminError::InvalidInput);
            }
            let earlier: Value = edits
                .iter()
                .rev()
                .find(|e| e.target == class && e.kind == "item_override")
                .and_then(|e| serde_json::from_str(e.value.as_deref()?).ok())
                .unwrap_or(Value::Null);
            let mut set = Map::new();
            for (table, values) in &tables {
                let changed: Map<String, Value> = values
                    .iter()
                    .filter(|(column, value)| {
                        // Defaults (0, empty) are left out of the export.
                        let exported = entry.fields.get(*column).cloned().unwrap_or_else(|| {
                            if value.is_string() {
                                json!("")
                            } else {
                                json!(0)
                            }
                        });
                        !same_value(value, &exported) || !earlier["set"][*table][*column].is_null()
                    })
                    .map(|(c, v)| (c.clone(), v.clone()))
                    .collect();
                if !changed.is_empty() {
                    set.insert((*table).to_owned(), Value::Object(changed));
                }
            }
            let mut text_changes = Map::new();
            if names.item_name(class).as_deref() != text["name"].as_str()
                || !earlier["text"]["name"].is_null()
            {
                text_changes.insert("name".into(), text["name"].clone());
            }
            if let Some(desc) = text.get("desc")
                && (names.item_description(class).as_deref() != desc.as_str()
                    || !earlier["text"]["desc"].is_null())
            {
                text_changes.insert("desc".into(), desc.clone());
            }
            if set.is_empty() && text_changes.is_empty() {
                return Err(AdminError::Unchanged("item_igual"));
            }
            (
                "item_override",
                json!({ "set": set, "text": text_changes }),
                None,
            )
        }
        state => {
            let previous = match &state {
                Known::Added(d) => Some(d.clone()),
                _ => None,
            };
            let template = match &previous {
                Some(d) => d["template"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or(AdminError::InvalidInput)?,
                None => {
                    let template = draft.template.as_deref().ok_or(AdminError::InvalidInput)?;
                    item_editor::item_class(template).map_err(invalid)?;
                    if !matches!(known(names, &edits, template), Known::Base(_)) {
                        return Err(AdminError::InvalidInput);
                    }
                    template.to_owned()
                }
            };
            // A template means a new item: the class must be free.
            if (previous.is_some() && draft.template.is_some())
                || (previous.is_none() && names.editor_item(class).is_some())
            {
                return Err(AdminError::Unchanged("item_existente"));
            }
            let template_entry = names.editor_item(&template).cloned().unwrap_or_default();
            let category = tables
                .get(ITEM_TABLE)
                .and_then(|t| t.get("Category"))
                .or_else(|| template_entry.fields.get("Category"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let mut definition = json!({ "template": template, "text": text });
            let mut costume_type = None;
            if let Some(kit) = draft.kit.as_deref() {
                let template_costume = template_entry
                    .fields
                    .get("CostumeType")
                    .and_then(Value::as_i64);
                let script =
                    ModelScript::of(&category, names.item_equip_class(&template).as_deref());
                let (costumes, assets, key) = kit_costume(
                    sources,
                    &edits,
                    kit,
                    script,
                    template_costume,
                    previous.as_ref(),
                    draft.body.as_deref(),
                )
                .await?;
                definition["costumes"] = costumes;
                definition["assets"] = json!(assets);
                tables
                    .entry(EQUIP_TABLE)
                    .or_default()
                    .insert("CostumeType".into(), json!(key));
                costume_type = Some(key);
            } else if let Some(p) = &previous {
                for key in ["costumes", "assets"] {
                    if !p[key].is_null() {
                        definition[key] = p[key].clone();
                    }
                }
            }
            definition["set"] = json!(tables);
            if let Some(set_id) = set_id {
                definition["set_id"] = json!(set_id);
            }
            ("item", definition, costume_type)
        }
    };
    let id = outcome(
        content_edits::create(
            pool,
            kind,
            class,
            Some(&value),
            actor.session.account_id,
            audit(actor, "content.item_edit", "item", class),
        )
        .await?,
    )?;
    Ok(DraftSaved {
        edit: id,
        kind: kind.to_owned(),
        costume_type,
    })
}

/// Hide an item (or show it again): it leaves the NPC shops and the panel's give list; the items
/// characters own stay and keep working.
pub async fn hide(
    pool: &PgPool,
    actor: &Actor<'_>,
    names: &EnglishNames,
    class: &str,
    hidden: bool,
) -> Result<i64, AdminError> {
    require(actor.session, Permission::Manage)?;
    item_editor::item_class(class).map_err(|_| AdminError::NotFound)?;
    let edits = content_edits::item_edits(pool, MAX_ITEM_EDITS).await?;
    if matches!(known(names, &edits, class), Known::Unknown) {
        return Err(AdminError::NotFound);
    }
    outcome(
        content_edits::create(
            pool,
            "item_hide",
            class,
            Some(&json!({ "hidden": hidden })),
            actor.session.account_id,
            audit(actor, "content.item_hide", "item", class),
        )
        .await?,
    )
}

/// Remove an added item. When the published removal loads (game start), every item of the class
/// leaves its owners: an equipped one is unequipped first; queued deliveries go too.
pub async fn remove(
    pool: &PgPool,
    actor: &Actor<'_>,
    names: &EnglishNames,
    class: &str,
) -> Result<i64, AdminError> {
    require(actor.session, Permission::Manage)?;
    item_editor::item_class(class).map_err(|_| AdminError::NotFound)?;
    let edits = content_edits::item_edits(pool, MAX_ITEM_EDITS).await?;
    match known(names, &edits, class) {
        Known::Added(_) => {}
        Known::Base(_) => return Err(AdminError::Unchanged("item_base")),
        Known::Unknown => return Err(AdminError::NotFound),
    }
    outcome(
        content_edits::create(
            pool,
            "item_remove",
            class,
            None,
            actor.session.account_id,
            audit(actor, "content.item_remove", "item", class),
        )
        .await?,
    )
}

/// Give a base item its original columns and text back.
pub async fn restore(
    pool: &PgPool,
    actor: &Actor<'_>,
    names: &EnglishNames,
    class: &str,
) -> Result<i64, AdminError> {
    require(actor.session, Permission::Manage)?;
    item_editor::item_class(class).map_err(|_| AdminError::NotFound)?;
    let edits = content_edits::item_edits(pool, MAX_ITEM_EDITS).await?;
    match known(names, &edits, class) {
        Known::Base(_) => {}
        Known::Added(_) => return Err(AdminError::InvalidInput),
        Known::Unknown => return Err(AdminError::NotFound),
    }
    outcome(
        content_edits::create(
            pool,
            "item_override",
            class,
            Some(&json!({})),
            actor.session.account_id,
            audit(actor, "content.item_restore", "item", class),
        )
        .await?,
    )
}

/// Keep an uploaded kit in the private assets folder.
pub async fn upload_kit(
    session: &SessionAccount,
    assets: Option<&Path>,
    bytes: axum::body::Bytes,
) -> Result<StoredKit, AdminError> {
    require(session, Permission::Manage)?;
    let root = assets.ok_or(AdminError::ContentUnavailable)?.to_path_buf();
    tokio::task::spawn_blocking(move || item_kits::store(&root, &bytes))
        .await
        .map_err(|_| AdminError::ContentUnavailable)?
        .map_err(|e| match e {
            item_kits::KitError::Io(error) => {
                tracing::warn!(error = %error, "kit not stored");
                AdminError::ContentUnavailable
            }
            _ => AdminError::Unchanged("kit_invalido"),
        })
}

/// The published item version against the one the server's exports carry (the stamp the host
/// writes, .cache/private-content/item-content.json).
pub async fn status(
    pool: &PgPool,
    session: &SessionAccount,
    stamp: Option<&Path>,
) -> Result<ItemContentStatus, AdminError> {
    require(session, Permission::Manage)?;
    let published = content_edits::latest_item_version(pool).await?;
    let exported = exported_version(stamp).await;
    Ok(ItemContentStatus {
        published_version: published,
        exported_version: exported,
        up_to_date: exported.unwrap_or(0) >= published,
    })
}

pub async fn exported_version(stamp: Option<&Path>) -> Option<i32> {
    let bytes = tokio::fs::read(stamp?).await.ok()?;
    if bytes.len() > 64 * 1024 {
        return None;
    }
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    i32::try_from(value["version"].as_i64()?).ok()
}
