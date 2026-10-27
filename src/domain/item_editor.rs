//! Item editor of the admin panel (docs/future/item-editor.md, cut 2): the columns it edits, their
//! bounds, item text, the CostumeType of a new model and the player_costume.txt entries it reads.
//! The same column list is `EDITABLE` in scripts/content/content_additions.py, which applies the
//! published edits to the server exports and the client copy.
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    /// Integer inside the bounds.
    Int(i64, i64),
    /// Number inside the bounds (EquipItemInfo.Weight is a float column).
    Float(f64, f64),
    /// Content code: `[A-Za-z0-9_]`, up to the length.
    Code(usize),
    /// Icon file name: `[A-Za-z0-9_]`, up to 96.
    Icon,
}

pub const ITEM_TABLE: &str = "ItemClassInfo";
pub const EQUIP_TABLE: &str = "EquipItemInfo";
const STAT: Field = Field::Int(-1_000_000, 1_000_000);
const PRICE: Field = Field::Int(0, 2_000_000_000);

/// (table, column, rule) of every editable column.
pub const EDITABLE: [(&str, &str, Field); 30] = [
    (ITEM_TABLE, "Icon", Field::Icon),
    (ITEM_TABLE, "Category", Field::Code(32)),
    (ITEM_TABLE, "TradeCategory", Field::Code(32)),
    (ITEM_TABLE, "TradeCategorySub", Field::Code(32)),
    (ITEM_TABLE, "Rarity", Field::Int(0, 10)),
    (ITEM_TABLE, "Level", Field::Int(0, 200)),
    (ITEM_TABLE, "RequiredLevel", Field::Int(0, 200)),
    // Bit per class (Lann 1 … Delia 1024); the 1.69 client knows the first 10.
    (ITEM_TABLE, "ClassRestriction", Field::Int(0, 4095)),
    (ITEM_TABLE, "MaxStack", Field::Int(0, 9999)),
    (ITEM_TABLE, "Price", PRICE),
    (ITEM_TABLE, "SellPrice", PRICE),
    (ITEM_TABLE, "ExpireIn", PRICE),
    (EQUIP_TABLE, "CostumeType", Field::Int(-1, 999_999_999)),
    (EQUIP_TABLE, "MaxDurability", Field::Int(0, 1_000_000)),
    (EQUIP_TABLE, "ATK", STAT),
    (EQUIP_TABLE, "ATK_Speed", STAT),
    (EQUIP_TABLE, "Critical", STAT),
    (EQUIP_TABLE, "Balance", STAT),
    (EQUIP_TABLE, "MATK", STAT),
    (EQUIP_TABLE, "DEF", STAT),
    (EQUIP_TABLE, "Res_Critical", STAT),
    (EQUIP_TABLE, "STR", STAT),
    (EQUIP_TABLE, "DEX", STAT),
    (EQUIP_TABLE, "INT", STAT),
    (EQUIP_TABLE, "WILL", STAT),
    (EQUIP_TABLE, "LUCK", STAT),
    (EQUIP_TABLE, "HP", STAT),
    (EQUIP_TABLE, "STAMINA", STAT),
    (EQUIP_TABLE, "Weight", Field::Float(0.0, 10_000.0)),
    (EQUIP_TABLE, "Dyeable", Field::Int(0, 1)),
];

pub const MAX_NAME: usize = 128;
pub const MAX_DESCRIPTION: usize = 512;
/// New models take CostumeType numbers from here (the base inner armors stop below 100 apart
/// from a few event entries; 9100 and 9101 are the first laboratory items).
pub const FIRST_COSTUME_TYPE: i64 = 9100;
pub const LAST_COSTUME_TYPE: i64 = 99_999;
/// Bodies of a player_costume.txt entry.
pub const BODIES: [&str; 4] = ["male", "female", "giant", "tall"];

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DraftError {
    #[error("unknown editable column")]
    Column,
    #[error("value outside the column bounds")]
    Value,
    #[error("item class must be 1-64 of [a-z0-9_]")]
    ItemClass,
    #[error("item text must be bounded, without quotes or control characters")]
    Text,
}

pub fn rule(column: &str) -> Option<(&'static str, Field)> {
    EDITABLE
        .iter()
        .find(|(_, c, _)| *c == column)
        .map(|(t, _, f)| (*t, *f))
}

/// A checked value of an editable column, as the content stores it.
pub fn value(column: &str, value: &serde_json::Value) -> Result<serde_json::Value, DraftError> {
    let (_, field) = rule(column).ok_or(DraftError::Column)?;
    let ok = match field {
        Field::Int(low, high) => value
            .as_i64()
            .filter(|v| (low..=high).contains(v))
            .map(serde_json::Value::from),
        Field::Float(low, high) => value
            .as_f64()
            .filter(|v| v.is_finite() && (low..=high).contains(v))
            .map(serde_json::Value::from),
        Field::Code(limit) => value
            .as_str()
            .filter(|v| {
                (1..=limit).contains(&v.len())
                    && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
            .map(serde_json::Value::from),
        Field::Icon => value
            .as_str()
            .filter(|v| {
                (1..=96).contains(&v.len())
                    && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
            .map(serde_json::Value::from),
    };
    ok.ok_or(DraftError::Value)
}

pub fn item_class(class: &str) -> Result<(), DraftError> {
    let ok = (1..=64).contains(&class.len())
        && class
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    ok.then_some(()).ok_or(DraftError::ItemClass)
}

/// Item name or description: the client's localization file quotes it and has one per line.
pub fn text(value: &str, limit: usize) -> Result<(), DraftError> {
    let ok = !value.trim().is_empty()
        && value.chars().count() <= limit
        && !value.chars().any(|c| c == '"' || c.is_control());
    ok.then_some(()).ok_or(DraftError::Text)
}

/// Entry numbers of one section of player_costume.txt (`"<section>" { "<n>" { ... } }`).
pub fn costume_keys(text: &[u8], section: &str) -> BTreeSet<i64> {
    let Some(body) = section_body(text, section) else {
        return BTreeSet::new();
    };
    body.lines()
        .filter(|l| l.starts_with("\t\"") && !l.starts_with("\t\t"))
        .filter_map(|l| l.trim().trim_matches('"').parse().ok())
        .collect()
}

/// The `category` (body) of an entry, when the section has it.
pub fn costume_body(text: &[u8], section: &str, key: i64) -> Option<String> {
    let body = section_body(text, section)?;
    let entry = format!("\n\t\"{key}\"");
    let at = body.find(&entry)?;
    let block = &body[at..];
    let block = &block[..block.find("\n\t}").unwrap_or(block.len())];
    let line = block
        .lines()
        .find(|l| l.trim_start().starts_with("\"category\""))?;
    let value = line.split('"').nth(3)?.to_ascii_lowercase();
    BODIES.contains(&value.as_str()).then_some(value)
}

fn section_body(text: &[u8], section: &str) -> Option<String> {
    let text = String::from_utf8_lossy(text).replace("\r\n", "\n");
    let header = format!("\n\"{section}\"");
    let start = text.find(&header)?;
    let rest = &text[start + header.len()..];
    let end = rest.find("\n\"").unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

/// The first free CostumeType of the editor's range.
pub fn allocate_costume_type(used: &BTreeSet<i64>) -> Option<i64> {
    (FIRST_COSTUME_TYPE..=LAST_COSTUME_TYPE).find(|k| !used.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn values_follow_their_column_rules() {
        assert_eq!(value("ATK", &json!(120)), Ok(json!(120)));
        assert_eq!(value("Weight", &json!(2.5)), Ok(json!(2.5)));
        assert_eq!(
            value("Category", &json!("INNERARMOR")),
            Ok(json!("INNERARMOR"))
        );
        assert_eq!(
            value("Icon", &json!("fiona_inner_fringe")),
            Ok(json!("fiona_inner_fringe"))
        );
        for (column, bad) in [
            ("ATK", json!("120")),
            ("Rarity", json!(11)),
            ("Weight", json!(-1)),
            ("Category", json!("inner armor")),
            ("Icon", json!("../x")),
            ("Dyeable", json!(2)),
        ] {
            assert_eq!(value(column, &bad), Err(DraftError::Value), "{column}");
        }
        assert_eq!(value("ItemClass", &json!("x")), Err(DraftError::Column));
        assert_eq!(value("Feature", &json!("x")), Err(DraftError::Column));
    }

    #[test]
    fn classes_and_text_are_bounded() {
        assert!(item_class("fiona_inner_lab2").is_ok());
        for bad in ["", "Fiona", "a-b", &"a".repeat(65)] {
            assert_eq!(item_class(bad), Err(DraftError::ItemClass));
        }
        assert!(text("Lab Inner Armor", MAX_NAME).is_ok());
        for bad in ["", "  ", "a \"b\"", "a\nb", &"a".repeat(129)] {
            assert_eq!(text(bad, MAX_NAME), Err(DraftError::Text));
        }
    }

    #[test]
    fn costume_keys_bodies_and_allocation() {
        let text = b"\"base\"\r\n{\r\n\t\"9100\"\r\n\t{\r\n\t}\r\n}\r\n\"inner_armor\"\r\n{\r\n\t\"fiona\"\r\n\t{\r\n\t\t\"default\"\t\"x\"\r\n\t}\r\n\t\"16\"\r\n\t{\r\n\t\t\"category\"\t\"female\"\r\n\t\t\"normal\"\t\"models/a.mdl\"\r\n\t}\r\n\t\"9100\"\r\n\t{\r\n\t\t\"category\"\t\"Female\"\r\n\t}\r\n}\r\n";
        let keys = costume_keys(text, "inner_armor");
        assert_eq!(keys, BTreeSet::from([16, 9100]));
        assert_eq!(
            costume_body(text, "inner_armor", 16).as_deref(),
            Some("female")
        );
        assert_eq!(
            costume_body(text, "inner_armor", 9100).as_deref(),
            Some("female")
        );
        assert_eq!(costume_body(text, "inner_armor", 17), None);
        assert_eq!(allocate_costume_type(&keys), Some(9101));
        assert_eq!(costume_keys(text, "hair"), BTreeSet::new());
    }
}
