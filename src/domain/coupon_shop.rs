//! Coupon shops (ItemContents CouponShopVersionInfo/CouponShopItemInfo,
//! QueryCouponShopItemInfoProcessor and BuyCouponShopItemProcessor) and the time-restricted
//! purchase counts of ShopSession (ShopBuyRestrictGet/Set/Rollback procedures).
use super::{
    feature_policy::FeaturePolicy,
    shop::{BUY_FAILED, BuyResult},
};
use serde::Deserialize;
use std::collections::BTreeMap;

/// CouponShopVersionInfo: ShopVersion, ShopPeriodText, PriceItemClass, Feature.
pub type VersionRow = (i16, Option<String>, Option<String>, Option<String>);
/// CouponShopItemInfo: ShopVersion, Order, RewardItemClass, RewardItemCount, PriceItemCount,
/// IsRare, IsCharacterBind, ResetTime (.NET ticks), PeriodDay, RestrictCount, Feature.
pub type ItemRow = (
    i16,
    i16,
    Option<String>,
    i32,
    i32,
    bool,
    bool,
    Option<i64>,
    Option<i32>,
    Option<i32>,
    Option<String>,
);

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CouponTables {
    #[serde(default)]
    pub versions: Vec<VersionRow>,
    #[serde(default)]
    pub items: Vec<ItemRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub shop_version: i16,
    pub period_text: Option<String>,
    pub price_item_class: String,
    pub feature: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub shop_version: i16,
    pub order: i16,
    pub reward_item_class: String,
    pub reward_item_count: i32,
    pub price_item_count: i32,
    pub is_rare: bool,
    pub is_character_bind: bool,
    pub reset_ticks: Option<i64>,
    pub period_day: i32,
    pub restrict_count: i32,
    pub feature: Option<String>,
}

const TICKS_PER_SECOND: i64 = 10_000_000;
const TICKS_PER_DAY: i64 = 86_400 * TICKS_PER_SECOND;

impl Item {
    /// KeyConverter.DoubleShortToInt(ShopVersion, Order): the ShopBuyRestrict ShopItemID.
    pub fn shop_item_id(&self) -> i32 {
        (i32::from(self.shop_version) << 16) | i32::from(self.order as u16)
    }
    /// ShopTimeRestricted.IsRestricted.
    pub fn is_restricted(&self) -> bool {
        self.restrict_count > 0
    }
    /// ShopTimeRestricted.HasRestrictTimeInfo.
    pub fn has_restrict_time(&self) -> bool {
        self.reset_ticks.is_some() && self.period_day > 0 && self.restrict_count > 0
    }
    /// The current period's start: UtcNow.AddSeconds(−((UtcNow − ResetTime).TotalSeconds
    /// % (PeriodDay·86400))), AddSeconds rounding to milliseconds.
    pub fn period_start(&self, now_ticks: i64) -> Option<i64> {
        if !self.has_restrict_time() {
            return None;
        }
        let reset = self.reset_ticks?;
        let total = (now_ticks - reset) as f64 / TICKS_PER_SECOND as f64;
        let period = f64::from(self.period_day.wrapping_mul(24).wrapping_mul(3600));
        let back = -(total % period);
        let millis = (back * 1000.0 + if back >= 0.0 { 0.5 } else { -0.5 }) as i64;
        Some(now_ticks + millis * 10_000)
    }
    /// ShopSession.TryShopBuyRestrictGet for one restricted item: (BuyableCount,
    /// NextResetTicksDiff) from its ShopBuyRestrict row (PurchaseTime ticks, Count).
    pub fn restriction(&self, row: Option<(i64, i32)>, now_ticks: i64) -> (i32, i64) {
        let mut out = (self.restrict_count, -1);
        let Some((purchase, count)) = row else {
            return out;
        };
        match self.period_start(now_ticks) {
            Some(start) => {
                if purchase >= start {
                    out = (
                        self.restrict_count.wrapping_sub(count),
                        start + i64::from(self.period_day) * TICKS_PER_DAY - purchase,
                    );
                }
            }
            None => out = (self.restrict_count.wrapping_sub(count), -1),
        }
        out
    }
}

/// ShopBuyRestrictSet on the stored row (PurchaseTime, Count): the new row and the count
/// it returns, or None for -1 (nothing written). `start` is the period start or None.
pub fn restrict_set(
    row: Option<(i64, i32)>,
    start: Option<i64>,
    num: i32,
    max: i32,
    now: i64,
) -> Option<((i64, i32), i32)> {
    if num > max {
        return None;
    }
    match row {
        Some((purchase, current)) => {
            if start.is_some_and(|s| purchase < s) {
                return Some(((now, num), num));
            }
            let total = current.checked_add(num)?;
            if total > max {
                return None;
            }
            Some(((now, total), total))
        }
        None => Some(((now, num), num)),
    }
}

/// What BuyCouponShopItemProcessor does before the restriction and the inventory.
#[derive(Debug, PartialEq, Eq)]
pub enum BuyPlan<'a> {
    /// FailMessage only: nothing reaches the client.
    Silent,
    /// BuyItemResultMessage(Failed, …) without changes.
    Refused(BuyResult),
    /// Restriction (if any), then CraftItem: destroy `price` of the version's price item
    /// and give `give` rewards.
    Trade {
        version: &'a Version,
        item: &'a Item,
        give: i32,
        price: i32,
    },
}

#[derive(Default)]
pub struct CouponShops {
    versions: BTreeMap<i16, Version>,
    items: BTreeMap<i16, Vec<Item>>,
}

impl CouponShops {
    /// ToDictionary(ShopVersion) (a duplicate throws) and ToMultiDictionary in table order.
    pub fn load(t: CouponTables) -> Option<Self> {
        let mut versions = BTreeMap::new();
        for (shop_version, period_text, price, feature) in t.versions {
            let version = Version {
                shop_version,
                period_text,
                price_item_class: price.unwrap_or_default(),
                feature,
            };
            if versions.insert(shop_version, version).is_some() {
                return None;
            }
        }
        let mut items: BTreeMap<i16, Vec<Item>> = BTreeMap::new();
        for (v, order, reward, count, price, rare, bind, reset, period, restrict, feature) in
            t.items
        {
            items.entry(v).or_default().push(Item {
                shop_version: v,
                order,
                reward_item_class: reward.unwrap_or_default(),
                reward_item_count: count,
                price_item_count: price,
                is_rare: rare,
                is_character_bind: bind,
                reset_ticks: reset,
                period_day: period.unwrap_or(0),
                restrict_count: restrict.unwrap_or(0),
                feature,
            });
        }
        Some(Self { versions, items })
    }

    pub fn version(&self, shop_version: i16) -> Option<&Version> {
        self.versions.get(&shop_version)
    }

    /// GetCouponShopItemInfos: None for a version without items.
    pub fn items(&self, shop_version: i16) -> Option<&[Item]> {
        self.items.get(&shop_version).map(Vec::as_slice)
    }

    pub fn versions(&self) -> impl Iterator<Item = &Version> {
        self.versions.values()
    }

    fn failed(version: &Version, item: &Item) -> BuyResult {
        BuyResult {
            result: BUY_FAILED,
            item_class: item.reward_item_class.clone(),
            item_count: item.reward_item_count,
            price_item_class: version.price_item_class.clone(),
            price_count: item.price_item_count,
            restriction_order: -1,
            restriction_count: -1,
        }
    }

    /// BuyCouponShopItemProcessor up to the restriction: `available_for` is
    /// ItemClassInfo.IsAvailableFor(BaseCharacter) of the reward class (None without
    /// ItemClassInfo) and `exists` whether MakeGiveItemRequest finds the class.
    pub fn plan(
        &self,
        policy: &FeaturePolicy,
        shop_version: i16,
        order: i16,
        count: i32,
        available_for: impl Fn(&str) -> Option<bool>,
        exists: impl Fn(&str) -> bool,
    ) -> BuyPlan<'_> {
        if count <= 0 {
            return BuyPlan::Silent;
        }
        let Some(version) = self.version(shop_version) else {
            return BuyPlan::Silent;
        };
        if !policy
            .is_enabled(version.feature.as_deref())
            .unwrap_or(false)
        {
            return BuyPlan::Silent;
        }
        let Some(item) = self
            .items(shop_version)
            .and_then(|items| items.iter().find(|i| i.order == order))
        else {
            return BuyPlan::Silent;
        };
        if !policy.is_enabled(item.feature.as_deref()).unwrap_or(false) {
            return BuyPlan::Silent;
        }
        let class = crate::domain::mission::item_class(&item.reward_item_class);
        if available_for(class) == Some(false) {
            return BuyPlan::Refused(Self::failed(version, item));
        }
        if !exists(class) {
            return BuyPlan::Refused(Self::failed(version, item));
        }
        BuyPlan::Trade {
            version,
            item,
            give: item.reward_item_count.wrapping_mul(count),
            price: item.price_item_count.wrapping_mul(count),
        }
    }

    /// The failure result after a restriction or CraftItem refusal.
    pub fn refused(version: &Version, item: &Item, result: i32) -> BuyResult {
        BuyResult {
            result,
            ..Self::failed(version, item)
        }
    }

    /// The success result: the given and destroyed totals, the Order and the remaining
    /// restricted count (RestrictCount − current when current > 0, else -1).
    pub fn succeeded(
        version: &Version,
        item: &Item,
        give: i32,
        price: i32,
        current: i32,
    ) -> BuyResult {
        BuyResult {
            result: super::shop::BUY_SUCCEEDED,
            item_class: item.reward_item_class.clone(),
            item_count: give,
            price_item_class: version.price_item_class.clone(),
            price_count: price,
            restriction_order: i32::from(item.order),
            restriction_count: if current > 0 {
                item.restrict_count.wrapping_sub(current)
            } else {
                -1
            },
        }
    }
}
