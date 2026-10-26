//! Laboratory cash shop catalog exported by scripts/content/export_cash_shop.py. The
//! original categories and products came from the Nexon NISMS billing server (not in the
//! evidence); the operator chose the effective cash-tab classes (ItemClassInfo.Storage 2),
//! one category per ItemClassInfo.Category under InGameCashShopRootCategory, priced by
//! category. Products are numbered by the catalog row, so a content update keeps them.
use super::{item_catalog::Package as Catalog, minimum_character::MinimumError};
use crate::protocol::inventory::{CashShopCategory, CashShopProduct};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read};

/// ItemClassInfo.Storage of the cash tab (StorageType.Cash).
const CASH_STORAGE: u8 = 2;
/// Nexon.Nisms.PaymentType.NexonCash and ProductStatus.OnSale.
const NEXON_CASH: i32 = 13001;
const ON_SALE: i32 = 1;
/// First CategoryNo of the item categories (below the root).
const FIRST_CATEGORY: i32 = 1000;
/// ProductNo of the VALUE variants: base + catalog row × 8 + duration index.
const VALUE_PRODUCT_BASE: i32 = 2_000_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Price {
    category: String,
    price: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomizeRow {
    item: Option<String>,
    category: Option<String>,
    shop: Option<String>,
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PriceRow {
    item: Option<String>,
    duration: i32,
    price: i32,
    shop: Option<String>,
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CouponRow {
    item: Option<String>,
    category: Option<String>,
    price: i32,
    shop: Option<String>,
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorRow {
    category: Option<String>,
    order: i32,
    color: i32,
    shop: Option<String>,
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TirRow {
    item: Option<String>,
    duration: Option<i32>,
    price: Option<i32>,
    shop: Option<String>,
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusEffectRow {
    #[serde(rename = "type")]
    type_name: String,
    level: i32,
    abs_level: i32,
    id_tag: Option<String>,
    duration: Option<i32>,
    combat_count: i32,
    kind: String,
    stat_adder: Vec<(String, i32)>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DailyItemRow {
    id: i32,
    start_ticks: i64,
    end_ticks: i64,
    period_minutes: i32,
    cafe_level: i32,
    cafe_type: i32,
    level_min: Option<i32>,
    level_max: Option<i32>,
    query: Option<String>,
    message: Option<String>,
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueProduct {
    item: String,
    days: Vec<i32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Localized {
    name: String,
    description: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feature {
    key: String,
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    localized_sha256: String,
    root_category: i32,
    categories: Vec<Option<String>>,
    /// Client english_eu item name and description of each catalog class.
    names: Vec<Option<Localized>>,
    /// Catalog rows of boxes without RandomItem/RandomItemV2 contents (not sold).
    without_contents: Vec<usize>,
    /// Classes sold once per duration with ProductAttribute3 `[VALUE:n]`.
    value_products: Vec<ValueProduct>,
    /// StatusEffectGroupInfo of the HomeCafeBonus effects.
    status_effects: Vec<StatusEffectRow>,
    /// ItemContents.dailyItemProcessInfo.
    daily_items: Vec<DailyItemRow>,
    default_price: i32,
    prices: Vec<Price>,
    customize_items: Vec<CustomizeRow>,
    customize_prices: Vec<PriceRow>,
    customize_coupons: Vec<CouponRow>,
    customize_colors: Vec<ColorRow>,
    tir_ignored: Vec<TirRow>,
    class_restrictions: Vec<i32>,
    features: Vec<Feature>,
}

/// Cashshop_Tircoin_notsell_list row of the LanguageTag (Duration/Price null match all).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TirIgnored {
    pub item: String,
    pub duration: Option<i32>,
    pub price: Option<i32>,
    pub feature: Option<String>,
}

/// CustomizePriceInfo: Duration (days, 0 or less permanent) and Price.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CustomizePrice {
    pub duration: i32,
    pub price: i32,
}

/// ItemContents.LoadCustomizedItem dictionaries (Feature and CashShopType filtered).
pub struct Customize {
    /// customizeItemInfoDic: ItemClass -> Category.
    pub items: BTreeMap<String, Option<String>>,
    /// customizePriceInfoDic (ToListDictionary, provider order).
    pub prices: BTreeMap<String, Vec<CustomizePrice>>,
    /// customizeCouponInfoDic: ItemClass -> (Category, Price).
    pub coupons: BTreeMap<String, (Option<String>, i32)>,
    /// customizeColorInfoDic: (Category, Color) present (skin_sss excluded).
    pub colors: std::collections::BTreeSet<(String, i32)>,
    /// skinSSSColorDic: skin Color -> skin_sss Color through the shared Order.
    pub skin_sss: BTreeMap<i32, i32>,
    pub tir_ignored: Vec<TirIgnored>,
    /// StatusEffectGroupInfo of the HomeCafeBonus effects.
    pub status_effects: crate::domain::status_effect::Contents,
    /// DailyItemScheduler procedures with an enabled Feature and a known ProcessType.
    pub daily_items: Vec<crate::domain::daily_item::Procedure>,
    /// ItemClassInfo.Category and ClassRestriction of each effective class.
    pub classes: BTreeMap<Vec<u8>, (Option<String>, i32)>,
    pub policy: crate::domain::feature_policy::FeaturePolicy,
}

/// One product: its catalog class and the listing element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub class_utf16: Vec<u8>,
    pub element: CashShopProduct,
    /// Client english_eu name and description; none when the EU never translated it.
    pub name: Option<String>,
    pub description: Option<String>,
    /// ProductAttribute3 (ItemClassEx attributes) of the product, such as `[VALUE:30]`.
    pub attribute3: Option<String>,
}

/// The bound catalog: categories, products and the products by ProductNo.
pub struct Contents {
    categories: Vec<CashShopCategory>,
    products: Vec<Product>,
    by_no: BTreeMap<i32, usize>,
    customize: Customize,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}

impl Profile {
    pub fn read(input: impl Read) -> Result<Self, MinimumError> {
        let mut bytes = Vec::new();
        input
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(fail());
        }
        let p: Self = serde_json::from_slice(&bytes).map_err(|_| fail())?;
        if p.format_version != 7
            || p.daily_items.len() > 4096
            || p.status_effects.len() > 4096
            || p.without_contents.len() > p.categories.len()
            || p.value_products.len() > 64
            || p.value_products.iter().any(|v| {
                v.item.is_empty()
                    || !v.item.is_ascii()
                    || v.days.is_empty()
                    || v.days.len() > 8
                    || v.days.iter().any(|d| !(1..=3650).contains(d))
            })
            || p.names.len() != p.categories.len()
            || p.localized_sha256.len() != 64
            || p.names.iter().flatten().any(|n| {
                n.name.is_empty()
                    || n.name.len() > 1024
                    || n.description.as_ref().is_some_and(|d| d.len() > 16384)
            })
            || p.customize_items.len() + p.customize_prices.len() + p.customize_coupons.len()
                > 262_144
            || p.features.len() > 512
            || p.customize_colors.len() + p.tir_ignored.len() > 65536
            || p.class_restrictions.len() != p.categories.len()
            || p.source != "heroesContents.cash-shop"
            || p.categories.len() > 262_144
            || p.prices.len() > 1024
            || p.default_price <= 0
            || p.prices
                .iter()
                .any(|r| r.price <= 0 || r.category.len() > 256)
            || p.categories
                .iter()
                .flatten()
                .any(|c| c.len() > 256 || !c.is_ascii())
        {
            return Err(fail());
        }
        Ok(p)
    }

    /// Category and default prices with the published panel edits (B4.4).
    pub fn with_overlay(mut self, overlay: &crate::domain::content_overlay::Overlay) -> Self {
        for (category, price) in &overlay.cash_prices {
            match self.prices.iter_mut().find(|p| &p.category == category) {
                Some(p) => p.price = *price,
                None => self.prices.push(Price {
                    category: category.clone(),
                    price: *price,
                }),
            }
        }
        if let Some(price) = overlay.cash_default {
            self.default_price = price;
        }
        self
    }
    pub fn bind(&self, catalog: &Catalog) -> Result<Contents, MinimumError> {
        if self.database_sha256 != catalog.manifest.database_sha256
            || self.config_sha256 != catalog.manifest.config_sha256
            || self.categories.len() != catalog.classes.len()
        {
            return Err(fail());
        }
        let prices: BTreeMap<&str, i32> = self
            .prices
            .iter()
            .map(|p| (p.category.as_str(), p.price))
            .collect();
        let effective = crate::application::item_catalog_inspection::catalog(catalog)?;
        let mut rows: Vec<(usize, Vec<u8>)> = effective
            .items()
            .filter(|(_, item)| {
                catalog
                    .classes
                    .get(item.source_index)
                    .is_some_and(|c| c.storage == CASH_STORAGE)
            })
            .map(|(class, item)| (item.source_index, class.to_vec()))
            .collect();
        rows.sort();
        let mut categories = vec![CashShopCategory {
            category_no: self.root_category,
            category_name: Some("CashShop".to_owned()),
            parent_category_no: 0,
            display_no: 0,
        }];
        let mut numbers: BTreeMap<String, i32> = BTreeMap::new();
        let mut products = Vec::with_capacity(rows.len());
        let hidden: std::collections::BTreeSet<usize> =
            self.without_contents.iter().copied().collect();
        for (index, class) in rows {
            if hidden.contains(&index) {
                continue;
            }
            let name = self
                .categories
                .get(index)
                .ok_or_else(fail)?
                .clone()
                .unwrap_or_else(|| "ETC".to_owned());
            let category_no = match numbers.get(&name) {
                Some(n) => *n,
                None => {
                    let display = i32::try_from(numbers.len()).map_err(|_| fail())? + 1;
                    let n = FIRST_CATEGORY + display - 1;
                    categories.push(CashShopCategory {
                        category_no: n,
                        category_name: Some(name.clone()),
                        parent_category_no: self.root_category,
                        display_no: display,
                    });
                    numbers.insert(name.clone(), n);
                    n
                }
            };
            let units: Vec<u16> = class
                .as_chunks::<2>()
                .0
                .iter()
                .map(|a| u16::from_le_bytes(*a))
                .collect();
            let product_id = String::from_utf16(&units).map_err(|_| fail())?;
            let product_no = i32::try_from(index).map_err(|_| fail())? + 1;
            let localized = self.names.get(index).ok_or_else(fail)?.as_ref();
            let sale_price = prices
                .get(name.as_str())
                .copied()
                .unwrap_or(self.default_price);
            // One product per duration; the base product would be unusable (VALUE 0).
            let variants: Vec<(i32, Option<i32>)> =
                match self.value_products.iter().find(|v| v.item == product_id) {
                    Some(v) => v
                        .days
                        .iter()
                        .enumerate()
                        .map(|(k, d)| {
                            let k = i32::try_from(k).map_err(|_| fail())?;
                            let row = i32::try_from(index).map_err(|_| fail())?;
                            Ok((
                                VALUE_PRODUCT_BASE
                                    .checked_add(row.checked_mul(8).ok_or_else(fail)?)
                                    .and_then(|n| n.checked_add(k))
                                    .ok_or_else(fail)?,
                                Some(*d),
                            ))
                        })
                        .collect::<Result<_, MinimumError>>()?,
                    None => vec![(product_no, None)],
                };
            for (product_no, days) in variants {
                // HEROES_ITEM_NAME "VIP Service ({0} Days)" takes the duration.
                let name = localized.map(|l| match days {
                    Some(d) => l.name.replace("{0}", &d.to_string()),
                    None => l.name.clone(),
                });
                products.push(Product {
                    name,
                    description: localized.and_then(|l| l.description.clone()),
                    class_utf16: class.clone(),
                    attribute3: days.map(|d| format!("[VALUE:{d}]")),
                    element: CashShopProduct {
                        product_no,
                        product_expire: 0,
                        product_pieces: 1,
                        product_id: Some(product_id.clone()),
                        product_guid: Some(format!("00000000-0000-4000-8000-{product_no:012x}")),
                        payment_type: NEXON_CASH,
                        product_type: Some(String::new()),
                        sale_price,
                        category_no,
                        status: ON_SALE,
                    },
                });
            }
        }
        let customize = self.customize(catalog)?;
        let by_no = products
            .iter()
            .enumerate()
            .map(|(i, p)| (p.element.product_no, i))
            .collect();
        Ok(Contents {
            categories,
            products,
            by_no,
            customize,
        })
    }

    fn customize(&self, catalog: &Catalog) -> Result<Customize, MinimumError> {
        let mut rows: BTreeMap<String, Option<String>> = catalog
            .policy
            .entries()
            .map(|(k, v)| (k.to_owned(), Some(v.to_owned())))
            .collect();
        for f in &self.features {
            let value = f.value.clone().unwrap_or_default();
            match rows.get(&f.key) {
                Some(existing) if existing.as_deref().unwrap_or("") != value => {
                    return Err(fail());
                }
                Some(_) => {}
                None => {
                    rows.insert(f.key.clone(), Some(value));
                }
            }
        }
        let policy =
            crate::domain::feature_policy::FeaturePolicy::npc_source(rows).map_err(|_| fail())?;
        let language = policy.get_string("LanguageTag").to_owned();
        let keep =
            |shop: &Option<String>, feature: &Option<String>| -> Result<bool, MinimumError> {
                Ok(policy.is_enabled(feature.as_deref()).map_err(|_| fail())?
                    && shop.as_deref().unwrap_or(&language) == language)
            };
        let mut items = BTreeMap::new();
        for r in &self.customize_items {
            if keep(&r.shop, &r.feature)? {
                // ToDictionary refuses a null or duplicate key.
                let item = r.item.clone().ok_or_else(fail)?;
                if items.insert(item, r.category.clone()).is_some() {
                    return Err(fail());
                }
            }
        }
        let mut prices: BTreeMap<String, Vec<CustomizePrice>> = BTreeMap::new();
        for r in &self.customize_prices {
            if keep(&r.shop, &r.feature)? {
                prices
                    .entry(r.item.clone().ok_or_else(fail)?)
                    .or_default()
                    .push(CustomizePrice {
                        duration: r.duration,
                        price: r.price,
                    });
            }
        }
        let mut coupons = BTreeMap::new();
        for r in &self.customize_coupons {
            if keep(&r.shop, &r.feature)? {
                let item = r.item.clone().ok_or_else(fail)?;
                if coupons
                    .insert(item, (r.category.clone(), r.price))
                    .is_some()
                {
                    return Err(fail());
                }
            }
        }
        let mut colors = std::collections::BTreeSet::new();
        let mut skin_orders: BTreeMap<i32, i32> = BTreeMap::new();
        let color_kept = |r: &ColorRow| -> Result<bool, MinimumError> {
            Ok((r.feature.is_none()
                || policy
                    .is_enabled(r.feature.as_deref())
                    .map_err(|_| fail())?)
                && r.shop.as_deref().is_none_or(|s| s == language))
        };
        for r in &self.customize_colors {
            if !color_kept(r)? {
                continue;
            }
            let category = r.category.clone().ok_or_else(fail)?;
            if category != "skin_sss" {
                // A duplicate (Category, Color) is logged and the first row kept.
                colors.insert((category.clone(), r.color));
            }
            // dictionary4.Add(Order, Color) refuses a duplicate Order.
            if category == "skin" && skin_orders.insert(r.order, r.color).is_some() {
                return Err(fail());
            }
        }
        let mut skin_sss = BTreeMap::new();
        for r in &self.customize_colors {
            if color_kept(r)? && r.category.as_deref() == Some("skin_sss") {
                // dictionary4.TryGetValue(Order) is 0 for an unknown Order.
                skin_sss.insert(skin_orders.get(&r.order).copied().unwrap_or(0), r.color);
            }
        }
        let tir_ignored = self
            .tir_ignored
            .iter()
            .filter(|r| r.shop.as_deref() == Some(language.as_str()))
            .map(|r| {
                Ok(TirIgnored {
                    item: r.item.clone().ok_or_else(fail)?,
                    duration: r.duration,
                    price: r.price,
                    feature: r.feature.clone(),
                })
            })
            .collect::<Result<Vec<_>, MinimumError>>()?;
        let effective = crate::application::item_catalog_inspection::catalog(catalog)?;
        let mut classes = BTreeMap::new();
        for (class, item) in effective.items() {
            classes.insert(
                class.to_vec(),
                (
                    self.categories
                        .get(item.source_index)
                        .ok_or_else(fail)?
                        .clone(),
                    *self
                        .class_restrictions
                        .get(item.source_index)
                        .ok_or_else(fail)?,
                ),
            );
        }
        let status_effects = crate::domain::status_effect::Contents::new(
            self.status_effects
                .iter()
                .map(|r| {
                    use crate::domain::status_effect::{Info, Kind};
                    Ok(Info {
                        type_name: r.type_name.clone(),
                        level: r.level,
                        abs_level: r.abs_level,
                        id_tag: r.id_tag.clone(),
                        duration: r.duration,
                        combat_count: r.combat_count,
                        kind: match r.kind.as_str() {
                            "Client" => Kind::Client,
                            "DeleteOnLogOut" => Kind::DeleteOnLogOut,
                            "UseRemainTime" => Kind::UseRemainTime,
                            "UseExpireTime" => Kind::UseExpireTime,
                            _ => return Err(fail()),
                        },
                        stat_adder: r.stat_adder.clone(),
                    })
                })
                .collect::<Result<Vec<_>, MinimumError>>()?,
        )
        .ok_or_else(fail)?;
        let mut daily_items = Vec::new();
        for r in &self.daily_items {
            if !policy
                .is_enabled(r.feature.as_deref())
                .map_err(|_| fail())?
            {
                continue;
            }
            if let Some(p) = crate::domain::daily_item::Procedure::parse(
                r.id,
                r.start_ticks,
                r.end_ticks,
                r.period_minutes,
                r.cafe_level,
                r.cafe_type,
                r.level_min,
                r.level_max,
                r.query.as_deref().unwrap_or(""),
                r.message.clone(),
            ) {
                daily_items.push(p);
            }
        }
        Ok(Customize {
            daily_items,
            status_effects,
            items,
            prices,
            coupons,
            colors,
            skin_sss,
            tir_ignored,
            classes,
            policy,
        })
    }
}

impl Contents {
    pub fn categories(&self) -> &[CashShopCategory] {
        &self.categories
    }
    pub fn products(&self) -> &[Product] {
        &self.products
    }
    pub fn customize(&self) -> &Customize {
        &self.customize
    }
    /// Service.ProductByProductID.
    pub fn product(&self, product_no: i32) -> Option<&Product> {
        self.by_no.get(&product_no).map(|i| &self.products[*i])
    }

    /// ShopList.aspx read by InGameCashShopDialog_V2 from ingame_cashshop_shopListURL:
    /// TinyXML `ProductList` whose `Table` children carry the product fields as child
    /// elements (`Table2` package contents are not sold here). The client keeps a
    /// product only when `item_no` names a known ItemClass. Values the laboratory does
    /// not model follow the original server's ShopList (status 6, gender mask 1023).
    /// The client stalls on the whole catalog (8,303 products; the original listed 94),
    /// so at most `per_category` products of each category are listed, in catalog order,
    /// and only those with a client english_eu name (product_name, product_desc); the
    /// CashShopProductList still carries every product.
    pub fn shop_list(&self, per_category: usize) -> String {
        use std::fmt::Write as _;
        let mut out = String::with_capacity(self.products.len() * 512 + 64);
        out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<ProductList>\r\n");
        let mut listed: BTreeMap<i32, usize> = BTreeMap::new();
        for p in &self.products {
            let e = &p.element;
            let Some(name) = p.name.as_deref() else {
                continue;
            };
            // VALUE products (VIP/VVIP durations) are always listed.
            if p.attribute3.is_none() {
                let count = listed.entry(e.category_no).or_default();
                if *count >= per_category {
                    continue;
                }
                *count += 1;
            }
            let class = escape(e.product_id.as_deref().unwrap_or(""));
            let guid = escape(e.product_guid.as_deref().unwrap_or(""));
            let name = escape(name);
            let desc = escape(p.description.as_deref().unwrap_or(""));
            // Writing to a String cannot fail.
            let _ = writeln!(
                out,
                "<Table><product_no>{no}</product_no><product_status>{SHOP_LIST_STATUS}</product_status><product_name>{name}</product_name><product_price>{price}</product_price><sale_price>{price}</sale_price><product_desc>{desc}</product_desc><category_no>{category}</category_no><use_sex>U</use_sex><use_level>0</use_level><item_no>{class}</item_no><product_pieces>{pieces}</product_pieces><product_rendering>true</product_rendering><usable_present>false</usable_present><product_expire>{expire}</product_expire><product_status_master>{SHOP_LIST_STATUS}</product_status_master><product_guid>{guid}</product_guid><usable_gender>{SHOP_LIST_GENDERS}</usable_gender><payment_type>{payment}</payment_type><show_inventory>1</show_inventory></Table>\r",
                no = e.product_no,
                price = e.sale_price,
                category = e.category_no,
                pieces = e.product_pieces,
                expire = e.product_expire,
                payment = e.payment_type,
            );
        }
        out.push_str("</ProductList>\r\n");
        out
    }
}

/// ShopList product_status/product_status_master of the original server's sold products.
const SHOP_LIST_STATUS: i32 = 6;
/// ShopList usable_gender of the original server (every class).
const SHOP_LIST_GENDERS: i32 = 1023;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
