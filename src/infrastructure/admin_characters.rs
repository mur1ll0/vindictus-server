//! Read-only character views for the admin panel (B4.3a): the characters of a game user and
//! one character's sheet, inventory (with ItemClassEx attributes and equipped parts), skills,
//! status effects and titles. English names come from the client's localization
//! (scripts/content/export_item_names.py). Nothing here writes game state.
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::HashMap, io::Read};

pub const MAX_NAMES_BYTES: u64 = 32 * 1024 * 1024;
/// .NET DateTime ticks of the Unix epoch.
const EPOCH_TICKS: i64 = 621_355_968_000_000_000;
/// Bounds of one character's rows read for the sheet.
const MAX_ITEMS: i64 = 4096;
const MAX_ROWS: i64 = 2048;

/// English names keyed by the upper-case class/skill/status key. Not loaded: every name is None.
#[derive(Default, Deserialize)]
pub struct EnglishNames {
    #[serde(default)]
    items: HashMap<String, (String, Option<String>)>,
    #[serde(default)]
    skills: HashMap<String, String>,
    #[serde(default)]
    status_effects: HashMap<String, String>,
    /// Panel catalog (scripts/content/export_panel_catalog.py): NPCs of each shop, NPC names,
    /// item icons. Loaded with `with_catalog`.
    #[serde(skip)]
    catalog: PanelCatalog,
    /// The catalog's items by marketplace category, built by `with_catalog`.
    #[serde(skip)]
    picker: super::admin_item_search::ItemIndex,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ShopNpc {
    pub npc: String,
    pub location: String,
    pub place: String,
}

#[derive(Default, Deserialize)]
struct PanelCatalog {
    #[serde(default)]
    shops: HashMap<String, Vec<ShopNpc>>,
    /// NPC ID (upper case) → English name and description.
    #[serde(default)]
    npcs: HashMap<String, (Option<String>, Option<String>)>,
    /// Item class (upper case) → icon file name (icons/item/<icon>.png).
    #[serde(default)]
    item_icons: HashMap<String, String>,
    /// Item class (upper case) → the tooltip's static fields (short keys, B4.4e).
    #[serde(default)]
    item_info: HashMap<String, HashMap<String, serde_json::Value>>,
    /// Item class without English text → a class with the same icon that has it.
    #[serde(default)]
    item_aliases: HashMap<String, String>,
    /// The client's tooltip texts (upper-case keys).
    #[serde(default)]
    tooltip_text: HashMap<String, String>,
    /// QuestID (upper case) → English title (HEROES_QUEST_TITLE_<ID>).
    #[serde(default)]
    quest_titles: HashMap<String, String>,
    /// Item class (upper case) → the item editor's view (export_panel_catalog.item_edit).
    #[serde(default)]
    item_edit: HashMap<String, EditorEntry>,
    /// Marketplace categories (MainCategoryInfo/SubCategoryInfo order, client names).
    #[serde(default)]
    trade_categories: Vec<super::admin_item_search::TradeCategory>,
}

/// An item as the item editor sees it in the exported content.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct EditorEntry {
    /// b = base, a = added (item_additions.json or the panel), e = base edited by the panel.
    #[serde(rename = "o", default)]
    pub origin: String,
    /// Editable columns of the item's first rows (missing = 0 or empty).
    #[serde(rename = "f", default)]
    pub fields: serde_json::Map<String, serde_json::Value>,
    #[serde(rename = "h", default)]
    pub hidden: u8,
    /// Definition of an added item (template, set, text, costumes, assets, set_id).
    #[serde(rename = "d", default)]
    pub definition: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid or oversized English names file")]
pub struct NamesError;

impl EnglishNames {
    pub fn read(reader: impl Read) -> Result<Self, NamesError> {
        let mut bytes = Vec::new();
        reader
            .take(MAX_NAMES_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| NamesError)?;
        if bytes.len() as u64 > MAX_NAMES_BYTES {
            return Err(NamesError);
        }
        serde_json::from_slice(&bytes).map_err(|_| NamesError)
    }
    /// Add the panel catalog (bounded like the names file).
    pub fn with_catalog(mut self, reader: impl Read) -> Result<Self, NamesError> {
        let mut bytes = Vec::new();
        reader
            .take(MAX_NAMES_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| NamesError)?;
        if bytes.len() as u64 > MAX_NAMES_BYTES {
            return Err(NamesError);
        }
        self.catalog = serde_json::from_slice(&bytes).map_err(|_| NamesError)?;
        self.picker = self.picker_index();
        Ok(self)
    }
    /// Every item of the tooltip info or the names, with its marketplace category.
    fn picker_index(&self) -> super::admin_item_search::ItemIndex {
        let mut classes: Vec<&String> = self.catalog.item_info.keys().collect();
        classes.extend(
            self.items
                .keys()
                .filter(|c| !self.catalog.item_info.contains_key(*c)),
        );
        super::admin_item_search::ItemIndex::new(classes.into_iter().map(|class| {
            let info = self.catalog.item_info.get(class);
            let text = |key: &str| info.and_then(|i| i.get(key)?.as_str().map(str::to_owned));
            let int = |key: &str| {
                info.and_then(|i| i.get(key)?.as_i64())
                    .and_then(|v| i32::try_from(v).ok())
            };
            let item = super::admin_item_search::PickerItem {
                class: class.to_ascii_lowercase(),
                name: self.item(class).0,
                icon: self.catalog.item_icons.get(class).cloned(),
                category: text("tc"),
                sub: text("ts"),
                required_level: int("rl"),
                rarity: int("r"),
                class_restriction: int("cr"),
                description: None,
                icon_bg: None,
                tooltip: None,
            };
            let hidden = self
                .catalog
                .item_edit
                .get(class)
                .is_some_and(|e| e.hidden == 1);
            (item, hidden)
        }))
    }
    /// The item picker's page (see `admin_item_search::ItemIndex::search`), each item with
    /// its description, icon background and the tooltip of the plain class.
    pub fn pick_items(
        &self,
        filter: &super::admin_item_search::Filter<'_>,
        assets: Option<&super::minimum_assets::Assets>,
    ) -> Result<super::admin_item_search::Page, super::admin_item_search::InvalidFilter> {
        let mut page = self.picker.search(filter)?;
        for item in &mut page.items {
            item.description = self.item(&item.class).1;
            item.icon_bg = self.item_icon_bg(&item.class);
            item.tooltip = Some(super::admin_tooltip::build(
                self,
                assets,
                None,
                &super::admin_tooltip::Input {
                    class: &item.class,
                    attributes: &[],
                    tradable: true,
                    equippable: None,
                },
            ));
        }
        Ok(page)
    }
    /// The marketplace category tree of the catalog.
    pub fn trade_categories(&self) -> &[super::admin_item_search::TradeCategory] {
        &self.catalog.trade_categories
    }
    /// The NPCs that open a shop (by the shop ID as the content writes it).
    pub fn shop_npcs(&self, shop_id: &str) -> Vec<ShopNpc> {
        self.catalog.shops.get(shop_id).cloned().unwrap_or_default()
    }
    pub fn npc_name(&self, npc: &str) -> Option<String> {
        self.catalog
            .npcs
            .get(&npc.to_ascii_uppercase())
            .and_then(|n| n.0.clone())
    }
    /// The icon file of an item class (attributes such as `[ENHANCE:3]` ignored).
    pub fn item_icon(&self, class: &str) -> Option<String> {
        self.catalog
            .item_icons
            .get(&base_class(class).to_ascii_uppercase())
            .cloned()
    }
    fn item(&self, class: &str) -> (Option<String>, Option<String>) {
        let key = base_class(class).to_ascii_uppercase();
        // Never translated (Korean): the same design of another class (same icon).
        self.items
            .get(&key)
            .or_else(|| {
                self.catalog
                    .item_aliases
                    .get(&key)
                    .and_then(|alias| self.items.get(alias))
            })
            .map_or((None, None), |(n, d)| (Some(n.clone()), d.clone()))
    }
    /// Category (`c`) and CostumeType (`ct`) of an item, for its model in player_costume.txt.
    pub fn item_costume(&self, class: &str) -> Option<(String, i64)> {
        let info = self.item_info(class)?;
        Some((
            info.get("c")?.as_str()?.to_owned(),
            info.get("ct")?.as_i64()?,
        ))
    }
    /// Exported items whose model lives in `script`, by CostumeType: (class, English name)
    /// in name order.
    pub fn items_by_costume(
        &self,
        script: &super::model_kit::ModelScript,
    ) -> std::collections::BTreeMap<i64, Vec<(String, Option<String>)>> {
        let mut out: std::collections::BTreeMap<i64, Vec<(String, Option<String>)>> =
            std::collections::BTreeMap::new();
        for (class, info) in &self.catalog.item_info {
            let (Some(category), Some(costume)) = (
                info.get("c").and_then(serde_json::Value::as_str),
                info.get("ct").and_then(serde_json::Value::as_i64),
            ) else {
                continue;
            };
            let equip = info.get("ec").and_then(serde_json::Value::as_str);
            if super::model_kit::ModelScript::of(category, equip).as_ref() == Some(script) {
                out.entry(costume)
                    .or_default()
                    .push((class.to_ascii_lowercase(), self.item(class).0));
            }
        }
        for items in out.values_mut() {
            items.sort_by(|a, b| (a.1.is_none(), &a.1, &a.0).cmp(&(b.1.is_none(), &b.1, &b.0)));
        }
        out
    }
    /// EquipItemInfo.EquipClass (`ec`): the weapon or shield script of the model.
    pub fn item_equip_class(&self, class: &str) -> Option<String> {
        self.item_info(class)?
            .get("ec")?
            .as_str()
            .map(str::to_owned)
    }
    pub(super) fn item_info(&self, class: &str) -> Option<&HashMap<String, serde_json::Value>> {
        self.catalog
            .item_info
            .get(&base_class(class).to_ascii_uppercase())
    }
    pub(super) fn text(&self, key: &str) -> Option<String> {
        self.catalog
            .tooltip_text
            .get(&key.to_ascii_uppercase())
            .cloned()
    }
    /// English title of a quest.
    pub fn quest_title(&self, quest: &str) -> Option<String> {
        self.catalog
            .quest_titles
            .get(&quest.to_ascii_uppercase())
            .cloned()
    }
    /// The client's name of an enchant (GameUI_Heroes_Attribute_Prefix/Suffix_<class>).
    pub fn enchant_name(&self, class: &str, prefix: bool) -> Option<String> {
        let side = if prefix { "PREFIX" } else { "SUFFIX" };
        self.text(&format!("GAMEUI_HEROES_ATTRIBUTE_{side}_{class}"))
    }
    pub(super) fn skill_name(&self, key: &str) -> Option<String> {
        self.skill(key)
    }
    /// IconBG drawn under the icon (inner armor body, for example).
    pub fn item_icon_bg(&self, class: &str) -> Option<String> {
        self.item_info(class)
            .and_then(|i| i.get("bg"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_ascii_lowercase)
    }
    fn skill(&self, key: &str) -> Option<String> {
        self.skills.get(&key.to_ascii_uppercase()).cloned()
    }
    fn status(&self, key: &str) -> Option<String> {
        self.status_effects.get(&key.to_ascii_uppercase()).cloned()
    }
}

/// The class of an ItemClassEx (`class[NAME:value,...]`).
fn base_class(class: &str) -> &str {
    class.split_once('[').map_or(class, |(c, _)| c)
}

fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes(*p))
        .collect();
    String::from_utf16_lossy(&units)
}

fn unix(ticks: Option<i64>) -> Option<i64> {
    ticks.map(|t| (t - EPOCH_TICKS) / 10_000_000)
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct CharacterSummary {
    /// Character ID (a 64-bit number, as text for JavaScript).
    pub cid: String,
    pub name: String,
    /// BaseCharacter: 0 Lethita (Lann), 1 Fiona, 2 Evy, 3 Kalok, 4 Kay, 5 Vella, 6 Hurk,
    /// 7 Lynn, 8 Arisha, 9 Hagie, 10 Delia (laboratory; domain::character_class).
    pub class: i16,
    pub level: i32,
    pub exp: i64,
    pub ap: i32,
    pub connected: bool,
    /// Marked for deletion.
    pub deleting: bool,
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct ItemAttribute {
    pub name: String,
    pub value: String,
    pub arg: i32,
    pub arg2: i32,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CharacterItem {
    /// Item ID (64-bit, as text).
    pub id: String,
    pub class: String,
    pub name: Option<String>,
    pub description: Option<String>,
    /// Icon file (GET /api/icons/item/{icon}).
    pub icon: Option<String>,
    /// Icon drawn under `icon` (ItemClassInfo.IconBG).
    pub icon_bg: Option<String>,
    /// What the game's item tooltip shows (stats, enchants, durability, requirements...).
    pub tooltip: crate::infrastructure::admin_tooltip::ItemTooltip,
    pub count: i32,
    /// 0–8 bags, 100 cash, 101 account (gold), 102 restore, 103 story.
    pub collection: i16,
    pub slot: i32,
    /// Equipped body part (game_equippables.part), when worn.
    pub equipped_part: Option<i16>,
    pub durability_lost: Option<i32>,
    /// Expiry as Unix seconds.
    pub expires_at: Option<i64>,
    pub tradable: bool,
    pub attributes: Vec<ItemAttribute>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CharacterSkill {
    pub skill: String,
    pub name: Option<String>,
    pub rank: i32,
    pub locked: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CharacterStatusEffect {
    pub kind: String,
    pub name: Option<String>,
    pub level: i32,
    /// Expiry as Unix seconds; none for effects without a time limit.
    pub expires_at: Option<i64>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CharacterTitle {
    pub title_id: i32,
    pub acquired: bool,
    pub selected: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CharacterSheet {
    pub summary: CharacterSummary,
    /// Login (game account name) of the owner.
    pub owner: String,
    pub gold: i64,
    /// Cash balance of the owner's account (what the cash shop reads).
    pub cash: i64,
    pub stats: CharacterStats,
    pub items: Vec<CharacterItem>,
    pub skills: Vec<CharacterSkill>,
    pub status_effects: Vec<CharacterStatusEffect>,
    pub titles: Vec<CharacterTitle>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CharacterStats {
    pub str: i32,
    pub dex: i32,
    pub int: i32,
    pub will: i32,
    pub luck: i32,
    pub hp: i32,
    pub stamina: i32,
    pub total_used_ap: i32,
    /// Seconds played.
    pub play_time: i64,
}

type SummaryRow = (i64, Option<Vec<u8>>, i16, i32, i64, i32, bool, bool);

fn summary((id, name, class, level, exp, ap, connected, deleting): SummaryRow) -> CharacterSummary {
    CharacterSummary {
        cid: id.to_string(),
        name: name.map(|n| utf16(&n)).unwrap_or_default(),
        class,
        level,
        exp,
        ap,
        connected,
        deleting,
    }
}

const SUMMARY: &str = "SELECT c.id, c.name_utf16, c.class, c.level, c.exp, c.ap, c.is_connected,
        c.delete_time_units300 IS NOT NULL
    FROM game_character_info c JOIN game_users u ON u.id = c.uid";

/// Characters of the game user whose name key (lower-case game account name) is given.
pub async fn list(pool: &PgPool, name_key: &str) -> Result<Vec<CharacterSummary>, sqlx::Error> {
    let rows: Vec<SummaryRow> = sqlx::query_as(&format!(
        "{SUMMARY} WHERE u.name_key = $1 ORDER BY c.order_num NULLS LAST, c.id LIMIT 64"
    ))
    .bind(name_key)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(summary).collect())
}

/// Character and game account names of `cids` (live channel members, B4.6).
pub async fn names_of(
    pool: &PgPool,
    cids: &[i64],
) -> Result<Vec<(i64, String, String)>, sqlx::Error> {
    let rows: Vec<(i64, Option<Vec<u8>>, String)> = sqlx::query_as(
        "SELECT c.id, c.name_utf16, u.name FROM game_character_info c JOIN game_users u ON u.id = c.uid WHERE c.id = ANY($1)",
    )
    .bind(cids)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(cid, name, account)| (cid, name.map(|n| utf16(&n)).unwrap_or_default(), account))
        .collect())
}

/// Lower-case name key of the character's game user, to check who may read it.
pub async fn owner_key(pool: &PgPool, cid: i64) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT u.name_key FROM game_character_info c JOIN game_users u ON u.id = c.uid WHERE c.id = $1",
    )
    .bind(cid)
    .fetch_optional(pool)
    .await
}

/// Item ID, attribute name, value, arg, arg2.
type AttributeRow = (i64, Vec<u8>, Vec<u8>, i32, i32);

type ItemRow = (
    i64,
    Vec<u8>,
    i32,
    i16,
    i32,
    Option<i64>,
    bool,
    Option<i16>,
    Option<i32>,
    Option<i32>,
    Option<[i32; 3]>,
);

pub async fn sheet(
    pool: &PgPool,
    names: &EnglishNames,
    assets: Option<&super::minimum_assets::Assets>,
    cid: i64,
) -> Result<Option<CharacterSheet>, sqlx::Error> {
    // One snapshot for every part of the sheet.
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let row: Option<SummaryRow> = sqlx::query_as(&format!("{SUMMARY} WHERE c.id = $1"))
        .bind(cid)
        .fetch_optional(&mut *tx)
        .await?;
    let owner: Option<String> = sqlx::query_scalar(
        "SELECT u.name FROM game_character_info c JOIN game_users u ON u.id = c.uid WHERE c.id = $1",
    )
    .bind(cid)
    .fetch_optional(&mut *tx)
    .await?;
    let (Some(row), Some(owner)) = (row, owner) else {
        return Ok(None);
    };
    let stats: (i32, i32, i32, i32, i32, i32, i32, i32, i64, Option<i32>) = sqlx::query_as(
        "SELECT str, dex, int, will, luck, hp, stamina, total_used_ap, play_time, selected_title
         FROM game_character_info WHERE id = $1",
    )
    .bind(cid)
    .fetch_one(&mut *tx)
    .await?;
    let items: Vec<ItemRow> = sqlx::query_as(
        "SELECT i.id, i.item_class_utf16, i.count, i.collection, i.slot, i.expire_ticks,
                i.tradable, e.part, e.reduced_durability, e.max_durability_bonus,
                CASE WHEN e.id IS NULL THEN NULL ELSE ARRAY[e.color1, e.color2, e.color3] END
         FROM game_items i LEFT JOIN game_equippables e ON e.id = i.id
         WHERE i.owner_id = $1 ORDER BY i.collection, i.slot, i.id LIMIT $2",
    )
    .bind(cid)
    .bind(MAX_ITEMS)
    .fetch_all(&mut *tx)
    .await?;
    let attributes: Vec<AttributeRow> = sqlx::query_as(
        "SELECT a.item_id, a.attribute_bytes, a.value_bytes, a.arg, a.arg2
         FROM game_item_attributes a JOIN game_items i ON i.id = a.item_id
         WHERE i.owner_id = $1 ORDER BY a.item_id, a.id LIMIT $2",
    )
    .bind(cid)
    .bind(MAX_ITEMS)
    .fetch_all(&mut *tx)
    .await?;
    let skills: Vec<(Vec<u8>, i32, i16)> = sqlx::query_as(
        "SELECT skill_utf16, rank, locked FROM game_skills WHERE cid = $1 ORDER BY row_id LIMIT $2",
    )
    .bind(cid)
    .bind(MAX_ROWS)
    .fetch_all(&mut *tx)
    .await?;
    let effects: Vec<(Vec<u8>, i32, Option<i64>)> = sqlx::query_as(
        "SELECT type_utf16, level, expire_ticks FROM game_status_effects WHERE cid = $1
         ORDER BY row_id LIMIT $2",
    )
    .bind(cid)
    .bind(MAX_ROWS)
    .fetch_all(&mut *tx)
    .await?;
    let titles: Vec<(i32, bool)> = sqlx::query_as(
        "SELECT title_id, acquired FROM game_titles WHERE cid = $1 ORDER BY title_id LIMIT $2",
    )
    .bind(cid)
    .bind(MAX_ROWS)
    .fetch_all(&mut *tx)
    .await?;
    // The enchant conditions read the owner's level, class, skills and titles.
    let context = match assets {
        Some(_) => super::minimum_item_stat::context(&mut tx, cid).await.ok(),
        None => None,
    };
    let cash: Option<i32> = sqlx::query_scalar(
        "SELECT b.balance FROM game_cash_balances b JOIN game_character_info c ON c.uid = b.uid WHERE c.id = $1",
    )
    .bind(cid)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;

    let ascii = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    let mut by_item: HashMap<i64, Vec<ItemAttribute>> = HashMap::new();
    for (item, name, value, arg, arg2) in attributes {
        by_item.entry(item).or_default().push(ItemAttribute {
            name: ascii(&name),
            value: ascii(&value),
            arg,
            arg2,
        });
    }
    let mut gold = 0;
    let holder = context.as_ref().map(|c| (c, row.3, i32::from(row.2)));
    let mut items: Vec<CharacterItem> = items
        .into_iter()
        .map(
            |(id, class, count, collection, slot, expire, tradable, part, lost, bonus, colors)| {
                let class = utf16(&class);
                if collection == 101 && class == "gold" {
                    gold += i64::from(count);
                }
                let (name, description) = names.item(&class);
                let icon = names.item_icon(&class);
                let attributes = by_item.remove(&id).unwrap_or_default();
                let tooltip = super::admin_tooltip::build(
                    names,
                    assets,
                    holder,
                    &super::admin_tooltip::Input {
                        class: &class,
                        attributes: &attributes,
                        tradable,
                        equippable: colors.map(|c| (lost.unwrap_or(0), bonus.unwrap_or(0), c)),
                    },
                );
                CharacterItem {
                    icon,
                    icon_bg: names.item_icon_bg(&class),
                    tooltip,
                    id: id.to_string(),
                    name,
                    description,
                    count,
                    collection,
                    slot,
                    equipped_part: part,
                    durability_lost: lost,
                    expires_at: unix(expire),
                    tradable,
                    attributes,
                    class,
                }
            },
        )
        .collect();
    // setSetInfo: the pieces of each set this character wears.
    let mut worn: HashMap<String, usize> = HashMap::new();
    for item in items.iter().filter(|i| i.equipped_part.is_some()) {
        if let Some(set) = super::admin_tooltip::set_of(names, &item.class) {
            *worn.entry(set).or_default() += 1;
        }
    }
    for item in &mut items {
        if let Some(set) = super::admin_tooltip::set_of(names, &item.class) {
            super::admin_tooltip::mark_set(&mut item.tooltip, worn.get(&set).copied().unwrap_or(0));
        }
    }
    let (str, dex, int, will, luck, hp, stamina, total_used_ap, play_time, selected) = stats;
    Ok(Some(CharacterSheet {
        summary: summary(row),
        owner,
        gold,
        cash: cash.map_or(0, i64::from),
        stats: CharacterStats {
            str,
            dex,
            int,
            will,
            luck,
            hp,
            stamina,
            total_used_ap,
            play_time,
        },
        items,
        skills: skills
            .into_iter()
            .map(|(skill, rank, locked)| {
                let skill = utf16(&skill);
                CharacterSkill {
                    name: names.skill(&skill),
                    skill,
                    rank,
                    locked: locked != 0,
                }
            })
            .collect(),
        status_effects: effects
            .into_iter()
            .map(|(kind, level, expire)| {
                let kind = utf16(&kind);
                CharacterStatusEffect {
                    name: names.status(&kind),
                    kind,
                    level,
                    expires_at: unix(expire),
                }
            })
            .collect(),
        titles: titles
            .into_iter()
            .map(|(title_id, acquired)| CharacterTitle {
                title_id,
                acquired,
                selected: selected == Some(title_id),
            })
            .collect(),
    }))
}

/// Who owns a character: its game user and the account entering that game user, if any.
pub struct CharacterOwner {
    pub uid: i32,
    pub account_id: Option<uuid::Uuid>,
    pub account_role: Option<crate::domain::admin::PanelRole>,
}

pub async fn owner(pool: &PgPool, cid: i64) -> Result<Option<CharacterOwner>, sqlx::Error> {
    let row: Option<(i32, Option<uuid::Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT c.uid, a.id, a.panel_role
         FROM game_character_info c JOIN game_users u ON u.id = c.uid
         LEFT JOIN accounts a ON lower(COALESCE(a.game_name, a.username)) = u.name_key
         WHERE c.id = $1",
    )
    .bind(cid)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(uid, account_id, role)| CharacterOwner {
        uid,
        account_id,
        account_role: role
            .as_deref()
            .and_then(crate::domain::admin::PanelRole::parse),
    }))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CatalogItem {
    /// Item class as the game writes it (lower case).
    pub class: String,
    pub name: String,
    pub icon: Option<String>,
    pub description: Option<String>,
}

impl EnglishNames {
    /// Items whose class or English name contains `term` (case-insensitive), by name. Items
    /// the item editor hid are left out.
    pub fn search_items(&self, term: &str, limit: usize) -> Vec<CatalogItem> {
        let term = term.to_lowercase();
        let mut found: Vec<CatalogItem> = self
            .items
            .iter()
            .filter(|(class, (name, _))| {
                (class.to_lowercase().contains(&term) || name.to_lowercase().contains(&term))
                    && self
                        .catalog
                        .item_edit
                        .get(*class)
                        .is_none_or(|e| e.hidden != 1)
            })
            .map(|(class, (name, description))| CatalogItem {
                icon: self.catalog.item_icons.get(class).cloned(),
                class: class.to_lowercase(),
                name: name.clone(),
                description: description.clone(),
            })
            .collect();
        found.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.class.cmp(&b.class)));
        found.truncate(limit);
        found
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CatalogEffect {
    /// StatusEffectGroupInfo Type.
    pub kind: String,
    pub level: i32,
    pub name: Option<String>,
    /// Duration in seconds, when the effect has one.
    pub duration: Option<i32>,
}

impl EnglishNames {
    /// Status effects of the content whose Type or English name contains `term`.
    pub fn search_effects(
        &self,
        contents: &crate::domain::status_effect::Contents,
        term: &str,
        limit: usize,
    ) -> Vec<CatalogEffect> {
        let term = term.to_lowercase();
        let mut found: Vec<CatalogEffect> = contents
            .infos()
            .filter_map(|info| {
                let name = self.status(&info.type_name);
                let matches = info.type_name.to_lowercase().contains(&term)
                    || name
                        .as_ref()
                        .is_some_and(|n| n.to_lowercase().contains(&term));
                matches.then(|| CatalogEffect {
                    kind: info.type_name.clone(),
                    level: info.level,
                    name,
                    duration: info.duration,
                })
            })
            .collect();
        found.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.level.cmp(&b.level)));
        found.truncate(limit);
        found
    }
}

impl EnglishNames {
    /// The item editor's view of an exported item.
    pub fn editor_item(&self, class: &str) -> Option<&EditorEntry> {
        self.catalog.item_edit.get(&class.to_ascii_uppercase())
    }
    /// Every exported item (upper-case class) with its editor view.
    pub fn editor_items(&self) -> impl Iterator<Item = (&String, &EditorEntry)> {
        self.catalog.item_edit.iter()
    }
    /// An icon some item of the content uses (scaleform/item_icon/<icon>.tga in the client).
    pub fn known_icon(&self, icon: &str) -> bool {
        self.catalog
            .item_icons
            .values()
            .any(|i| i.eq_ignore_ascii_case(icon))
    }
    /// Equipment set IDs of the content (SetItemInfo).
    pub fn set_ids(&self) -> std::collections::BTreeSet<String> {
        self.catalog
            .item_info
            .values()
            .filter_map(|i| i.get("set")?.as_str().map(str::to_owned))
            .collect()
    }
    /// The equipment set of an item, if any.
    pub fn item_set(&self, class: &str) -> Option<String> {
        self.item_info(class)?
            .get("set")?
            .as_str()
            .map(str::to_owned)
    }
    /// English description of an item.
    pub fn item_description(&self, class: &str) -> Option<String> {
        self.item(class).1
    }

    /// The English name of an item class, if the client localization has one.
    pub fn item_name(&self, class: &str) -> Option<String> {
        self.item(class).0
    }
}

/// Who owns an item class (item editor): its items, the characters holding them, and the
/// queued deliveries not processed yet.
#[derive(Debug, Default, Serialize, utoipa::ToSchema)]
pub struct ItemUsage {
    /// Items of the class in every storage (bags, equipment, shared storage).
    pub total: i64,
    /// Queued deliveries (mail, rewards) of the class not processed yet.
    pub queued: i64,
    /// Characters holding the most of them (up to 50).
    pub characters: Vec<ItemHolder>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ItemHolder {
    /// Character ID (64-bit, as text).
    pub cid: String,
    pub name: String,
    pub count: i64,
}

fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

pub async fn item_usage(pool: &PgPool, class: &str) -> Result<ItemUsage, sqlx::Error> {
    let key = utf16_bytes(class);
    let prefix = utf16_bytes(&format!("{class}["));
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM game_items WHERE item_class_utf16 = $1")
            .bind(&key)
            .fetch_one(&mut *tx)
            .await?;
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_queued_items WHERE NOT processed
           AND (item_class_ex_utf16 = $1
                OR substring(item_class_ex_utf16 FROM 1 FOR octet_length($2)) = $2)",
    )
    .bind(&key)
    .bind(&prefix)
    .fetch_one(&mut *tx)
    .await?;
    let holders: Vec<(i64, Option<Vec<u8>>, i64)> = sqlx::query_as(
        "SELECT i.owner_id, c.name_utf16, count(*)
         FROM game_items i JOIN game_character_info c ON c.id = i.owner_id
         WHERE i.item_class_utf16 = $1
         GROUP BY i.owner_id, c.name_utf16
         ORDER BY count(*) DESC, i.owner_id LIMIT 50",
    )
    .bind(&key)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ItemUsage {
        total,
        queued,
        characters: holders
            .into_iter()
            .map(|(cid, name, count)| ItemHolder {
                cid: cid.to_string(),
                name: name.as_deref().map(utf16).unwrap_or_default(),
                count,
            })
            .collect(),
    })
}
