//! Cumulative overlay of the published panel content edits (B4.4), applied to the loaded
//! ShopInfo rows and cash shop prices when the game server starts. Same rules as
//! scripts/content/content_edits.py, which applies it to the client copy.
use super::shop::ShopRow;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct Overlay {
    pub format_version: u32,
    /// The newest published version included (0 = nothing published).
    pub version: i32,
    pub shop_rows: Vec<OverlayRow>,
    pub cash_prices: BTreeMap<String, i32>,
    pub cash_default: Option<i32>,
    /// B4.5: FeatureMatrix rates (percent) by key.
    #[serde(default)]
    pub features: BTreeMap<String, i32>,
    /// QuestInfo DropItemCountMin/Max by QuestID.
    #[serde(default)]
    pub quest_drops: BTreeMap<String, [i32; 2]>,
    /// EnhanceServerInfo SuccessRatio by "<EnhanceType>#<EnhanceLevel>".
    #[serde(default)]
    pub enhance_ratios: BTreeMap<String, f32>,
    /// EnchantInfo Min/MaxSuccessRatio (percent) by EnchantClass.
    #[serde(default)]
    pub enchant_ratios: BTreeMap<String, [i32; 2]>,
}

/// The FeatureMatrix rates the panel tunes (percent; 100 = the original value).
pub const TUNABLE_FEATURES: [&str; 7] = [
    "ExpRate_Combat",
    "GoldRate_Combat",
    "APRate_Combat",
    "ExpRate_Story",
    "GoldRate_Story",
    "APRate_Story",
    "ExpRate_Monster",
];
pub const MAX_RATE: i32 = 10_000;
pub const MAX_DROP_COUNT: i32 = 100;

/// Split "<EnhanceType>#<EnhanceLevel>".
pub fn enhance_target(target: &str) -> Option<(&str, i32)> {
    let (kind, level) = target.rsplit_once('#')?;
    let level: i32 = level.parse().ok()?;
    (!kind.is_empty() && (0..=99).contains(&level)).then_some((kind, level))
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct OverlayRow {
    #[serde(rename = "ShopID")]
    pub shop_id: String,
    #[serde(rename = "Order")]
    pub order: i16,
    #[serde(rename = "Remove", default)]
    pub remove: bool,
    #[serde(rename = "ItemClass")]
    pub item_class: Option<String>,
    #[serde(rename = "Count")]
    pub count: Option<i32>,
    #[serde(rename = "PriceItemClass")]
    pub price_item_class: Option<String>,
    #[serde(rename = "PriceCount")]
    pub price_count: Option<i32>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("content overlay outside its bounds")]
pub struct OverlayError;

fn text(v: &str, limit: usize) -> bool {
    !v.is_empty() && v.len() <= limit && v.is_ascii()
}

impl Overlay {
    /// The bounds the panel validates and the Python loader checks.
    pub fn validate(self) -> Result<Self, OverlayError> {
        let rows_ok = self.shop_rows.iter().all(|r| {
            text(&r.shop_id, 128)
                && r.order >= 0
                && (r.remove
                    || (r.item_class.as_deref().is_some_and(|c| text(c, 50))
                        && r.price_item_class.as_deref().is_some_and(|c| text(c, 50))
                        && r.count.is_some_and(|c| (1..=9999).contains(&c))
                        && r.price_count
                            .is_some_and(|c| (1..=2_000_000_000).contains(&c))))
        });
        let prices_ok = self
            .cash_prices
            .iter()
            .all(|(k, v)| text(k, 128) && (1..=1_000_000).contains(v));
        let default_ok = self
            .cash_default
            .is_none_or(|v| (1..=1_000_000).contains(&v));
        let tuning_ok = self
            .features
            .iter()
            .all(|(k, v)| TUNABLE_FEATURES.contains(&k.as_str()) && (0..=MAX_RATE).contains(v))
            && self.quest_drops.iter().all(|(k, [min, max])| {
                text(k, 128) && (0..=*max).contains(min) && *max <= MAX_DROP_COUNT
            })
            && self.enhance_ratios.iter().all(|(k, r)| {
                text(k, 128) && enhance_target(k).is_some() && (0.0..=1.0).contains(r)
            })
            && self
                .enchant_ratios
                .iter()
                .all(|(k, [min, max])| text(k, 128) && (0..=*max).contains(min) && *max <= 100);
        if self.format_version == 1
            && self.version >= 0
            && self.shop_rows.len() <= 100_000
            && self.cash_prices.len() <= 1024
            && rows_ok
            && prices_ok
            && default_ok
            && tuning_ok
        {
            Ok(self)
        } else {
            Err(OverlayError)
        }
    }

    /// ShopInfo rows with the overlay: remove, change the sold fields, or add a row. A
    /// key is (ShopID, Order) as exported; applying it twice gives the same rows.
    pub fn apply_shop_rows(&self, rows: Vec<ShopRow>) -> Vec<ShopRow> {
        let mut out: Vec<Option<ShopRow>> = rows.into_iter().map(Some).collect();
        let mut index: BTreeMap<(String, i16), usize> = BTreeMap::new();
        for (i, r) in out.iter().enumerate() {
            if let Some(r) = r {
                index.entry((r.shop_id.clone(), r.order)).or_insert(i);
            }
        }
        let mut removed: Vec<Option<ShopRow>> = vec![None; out.len()];
        for e in &self.shop_rows {
            let key = (e.shop_id.clone(), e.order);
            let at = index.get(&key).copied();
            if e.remove {
                if let Some(i) = at
                    && let Some(row) = out[i].take()
                {
                    removed[i] = Some(row);
                }
                continue;
            }
            let (Some(item), Some(count), Some(price_item), Some(price)) = (
                e.item_class.clone(),
                e.count,
                e.price_item_class.clone(),
                e.price_count,
            ) else {
                continue;
            };
            match at {
                Some(i) => {
                    // A later edit of a removed row brings it back with its other fields.
                    let mut row = out[i]
                        .take()
                        .or_else(|| removed[i].take())
                        .unwrap_or(ShopRow {
                            shop_id: e.shop_id.clone(),
                            order: e.order,
                            item_class: String::new(),
                            count: 0,
                            price_item_class: String::new(),
                            price_count: 0,
                            character_bind: false,
                            restriction_count: 0,
                            feature: None,
                        });
                    row.item_class = item;
                    row.count = count;
                    row.price_item_class = price_item;
                    row.price_count = price;
                    out[i] = Some(row);
                }
                None => {
                    index.insert(key, out.len());
                    out.push(Some(ShopRow {
                        shop_id: e.shop_id.clone(),
                        order: e.order,
                        item_class: item,
                        count,
                        price_item_class: price_item,
                        price_count: price,
                        character_bind: false,
                        restriction_count: 0,
                        feature: None,
                    }));
                    removed.push(None);
                }
            }
        }
        out.into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(shop: &str, order: i16, item: &str, price: i32) -> ShopRow {
        ShopRow {
            shop_id: shop.into(),
            order,
            item_class: item.into(),
            count: 1,
            price_item_class: "gold".into(),
            price_count: price,
            character_bind: false,
            restriction_count: 0,
            feature: Some("TestServer".into()),
        }
    }

    #[test]
    fn overlay_changes_adds_and_removes_rows_idempotently() {
        let overlay: Overlay = serde_json::from_value(serde_json::json!({
            "format_version": 1, "version": 2,
            "shop_rows": [
                {"ShopID": "A", "Order": 1, "ItemClass": "elixir", "Count": 2, "PriceItemClass": "gold", "PriceCount": 3500},
                {"ShopID": "A", "Order": 2, "Remove": true},
                {"ShopID": "A", "Order": 9, "ItemClass": "potion", "Count": 1, "PriceItemClass": "gold", "PriceCount": 100}
            ],
            "cash_prices": {"HAIR": 900}, "cash_default": null
        }))
        .unwrap();
        let overlay = overlay.validate().unwrap();
        let rows = vec![row("A", 1, "elixir", 4000), row("A", 2, "sword", 10)];
        let once = overlay.apply_shop_rows(rows);
        assert_eq!(once.len(), 2);
        assert_eq!((once[0].price_count, once[0].count), (3500, 2));
        // Fields the panel does not edit stay.
        assert_eq!(once[0].feature.as_deref(), Some("TestServer"));
        assert_eq!((once[1].order, once[1].feature.as_deref()), (9, None));
        let twice = overlay.apply_shop_rows(once.clone());
        assert_eq!(once, twice);
    }

    #[test]
    fn tuning_bounds_and_targets() {
        let ok: Overlay = serde_json::from_value(serde_json::json!({
            "format_version": 1, "version": 3, "shop_rows": [], "cash_prices": {}, "cash_default": null,
            "features": {"ExpRate_Combat": 200}, "quest_drops": {"Q": [1, 2]},
            "enhance_ratios": {"5#5": 0.5}, "enchant_ratios": {"simple": [10, 20]}
        }))
        .unwrap();
        assert!(ok.validate().is_ok());
        for bad in [
            serde_json::json!({"features": {"NGM_disable": 1}}),
            serde_json::json!({"quest_drops": {"Q": [3, 2]}}),
            serde_json::json!({"enhance_ratios": {"5": 0.5}}),
            serde_json::json!({"enchant_ratios": {"simple": [10, 101]}}),
        ] {
            let mut value = serde_json::json!({
                "format_version": 1, "version": 1, "shop_rows": [], "cash_prices": {}, "cash_default": null
            });
            value.as_object_mut().unwrap().extend(bad.as_object().unwrap().clone());
            let overlay: Overlay = serde_json::from_value(value).unwrap();
            assert_eq!(overlay.validate(), Err(OverlayError));
        }
        assert_eq!(enhance_target("LONGSWORD_LVL1#12"), Some(("LONGSWORD_LVL1", 12)));
    }

    #[test]
    fn overlay_outside_bounds_is_refused() {
        let overlay: Overlay = serde_json::from_value(serde_json::json!({
            "format_version": 1, "version": 1,
            "shop_rows": [{"ShopID": "A", "Order": 1, "ItemClass": "x", "Count": 0, "PriceItemClass": "gold", "PriceCount": 1}],
            "cash_prices": {}, "cash_default": null
        }))
        .unwrap();
        assert_eq!(overlay.validate(), Err(OverlayError));
    }
}
