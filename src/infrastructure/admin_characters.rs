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
    fn item(&self, class: &str) -> (Option<String>, Option<String>) {
        self.items
            .get(&class.to_ascii_uppercase())
            .map_or((None, None), |(n, d)| (Some(n.clone()), d.clone()))
    }
    fn skill(&self, key: &str) -> Option<String> {
        self.skills.get(&key.to_ascii_uppercase()).cloned()
    }
    fn status(&self, key: &str) -> Option<String> {
        self.status_effects.get(&key.to_ascii_uppercase()).cloned()
    }
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
    /// 7 Lynn, 8 Arisha, 9 Hagie.
    pub class: i16,
    pub level: i32,
    pub exp: i64,
    pub ap: i32,
    pub connected: bool,
    /// Marked for deletion.
    pub deleting: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
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
);

pub async fn sheet(
    pool: &PgPool,
    names: &EnglishNames,
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
                i.tradable, e.part, e.reduced_durability
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
    let items = items
        .into_iter()
        .map(
            |(id, class, count, collection, slot, expire, tradable, part, lost)| {
                let class = utf16(&class);
                if collection == 101 && class == "gold" {
                    gold += i64::from(count);
                }
                let (name, description) = names.item(&class);
                CharacterItem {
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
                    attributes: by_item.remove(&id).unwrap_or_default(),
                    class,
                }
            },
        )
        .collect();
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
    pub description: Option<String>,
}

impl EnglishNames {
    /// Items whose class or English name contains `term` (case-insensitive), by name.
    pub fn search_items(&self, term: &str, limit: usize) -> Vec<CatalogItem> {
        let term = term.to_lowercase();
        let mut found: Vec<CatalogItem> = self
            .items
            .iter()
            .filter(|(class, (name, _))| {
                class.to_lowercase().contains(&term) || name.to_lowercase().contains(&term)
            })
            .map(|(class, (name, description))| CatalogItem {
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
    /// The English name of an item class, if the client localization has one.
    pub fn item_name(&self, class: &str) -> Option<String> {
        self.item(class).0
    }
}
