//! ItemContents.shopInfoDic and BuyItemProcessor's normal shop branch for gold or item
//! prices. Restricted rows, AP prices and attribute ItemClassEx are not migrated.
use super::{
    feature_policy::FeaturePolicy,
    shop_price::{self, Arch},
};
use std::collections::BTreeMap;

/// ShopInfo row (HeroesContentsLoader.GetTable order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopRow {
    pub shop_id: String,
    pub order: i16,
    pub item_class: String,
    pub count: i32,
    pub price_item_class: String,
    pub price_count: i32,
    pub character_bind: bool,
    pub restriction_count: i32,
    pub feature: Option<String>,
}

/// shopInfoDic: (ShopID.ToLower(), Order), the first duplicate kept, PriceCount
/// replaced by PriceCalc (Ceiling10 for gold, Ceiling otherwise).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Shops {
    rows: BTreeMap<(String, i16), ShopRow>,
}
impl Shops {
    pub fn load(rows: Vec<ShopRow>, ratio: f32, arch: Arch) -> Option<Self> {
        let mut out = BTreeMap::new();
        for mut r in rows {
            r.price_count = if r.price_item_class == "gold" {
                shop_price::ceiling10(r.price_count, ratio, arch)?
            } else {
                shop_price::ceiling(r.price_count, ratio, arch)?
            };
            out.entry((r.shop_id.to_lowercase(), r.order)).or_insert(r);
        }
        Some(Self { rows: out })
    }
    /// Every row, by (lower-case ShopID, Order).
    pub fn all(&self) -> impl Iterator<Item = &ShopRow> {
        self.rows.values()
    }
    /// GetShopRetrictCountInfo: every row of a shop (case-insensitive ShopID).
    pub fn rows_of(&self, shop_id: &str) -> impl Iterator<Item = &ShopRow> {
        let lower = shop_id.to_lowercase();
        self.rows
            .range((lower.clone(), i16::MIN)..=(lower, i16::MAX))
            .map(|(_, r)| r)
    }
    pub fn get(&self, shop_id: &str, order: i32) -> Option<&ShopRow> {
        let order = i16::try_from(order).ok()?;
        self.rows.get(&(shop_id.to_lowercase(), order))
    }
}

/// BuyItemResultMessage.BuyItemResult.
pub const BUY_SUCCEEDED: i32 = 1;
pub const BUY_FAILED: i32 = 2;
pub const BUY_FAILED_UNIQUE: i32 = 3;

/// BuyItemResultMessage fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuyResult {
    pub result: i32,
    pub item_class: String,
    pub item_count: i32,
    pub price_item_class: String,
    pub price_count: i32,
    pub restriction_order: i32,
    pub restriction_count: i32,
}
impl BuyResult {
    /// The five-argument constructor (restriction fields -1) with the row unit values.
    pub fn failed(row: &ShopRow, result: i32) -> Self {
        Self {
            result,
            item_class: row.item_class.clone(),
            item_count: row.count,
            price_item_class: row.price_item_class.clone(),
            price_count: row.price_count,
            restriction_order: -1,
            restriction_count: -1,
        }
    }
    /// Success: totals for the bought count, the row Order and no restriction.
    pub fn succeeded(row: &ShopRow, count: i32) -> Self {
        Self {
            result: BUY_SUCCEEDED,
            item_class: row.item_class.clone(),
            item_count: row.count.wrapping_mul(count),
            price_item_class: row.price_item_class.clone(),
            price_count: row.price_count.wrapping_mul(count),
            restriction_order: i32::from(row.order),
            restriction_count: -1,
        }
    }
}

/// What BuyItemProcessor does before touching the inventory.
#[derive(Debug, PartialEq, Eq)]
pub enum BuyPlan<'a> {
    /// FailMessage only (Count <= 0, or a null GetShopInfo that throws).
    Silent,
    /// BuyItemResultMessage(Failed) without changes.
    Refused(BuyResult),
    /// Destroy `price` of the price item and give `give_count` bought items.
    Trade {
        row: &'a ShopRow,
        give_count: i32,
        price: i32,
    },
    /// Restricted rows, AP prices, buyback/coupon shops or attribute ItemClassEx.
    Unsupported,
}

pub fn plan<'a>(
    shops: &'a Shops,
    policy: &FeaturePolicy,
    shop_id: &str,
    order: i32,
    count: i32,
    item_exists: impl Fn(&str) -> bool,
) -> BuyPlan<'a> {
    if count <= 0 {
        return BuyPlan::Silent;
    }
    let lower = shop_id.to_lowercase();
    if lower == "buyback_byap" || lower == "buyback" || lower == "ingamecashshop_couponbuy" {
        return BuyPlan::Unsupported;
    }
    let Some(row) = shops.get(shop_id, order) else {
        return BuyPlan::Silent;
    };
    match policy.is_enabled(row.feature.as_deref()) {
        Ok(true) => {}
        Ok(false) => return BuyPlan::Refused(BuyResult::failed(row, BUY_FAILED)),
        Err(_) => return BuyPlan::Unsupported,
    }
    if row.restriction_count > 0
        || row.price_item_class.eq_ignore_ascii_case("ap")
        || row.item_class.contains('[')
    {
        return BuyPlan::Unsupported;
    }
    // MakeGiveItemRequest: an unknown ItemClass is GiveItemReqFail.
    if !item_exists(&row.item_class) {
        return BuyPlan::Refused(BuyResult::failed(row, BUY_FAILED));
    }
    BuyPlan::Trade {
        row,
        give_count: row.count.wrapping_mul(count),
        price: row.price_count.wrapping_mul(count),
    }
}
