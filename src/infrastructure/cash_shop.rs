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
    root_category: i32,
    categories: Vec<Option<String>>,
    default_price: i32,
    prices: Vec<Price>,
    customize_items: Vec<CustomizeRow>,
    customize_prices: Vec<PriceRow>,
    customize_coupons: Vec<CouponRow>,
    features: Vec<Feature>,
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
    pub policy: crate::domain::feature_policy::FeaturePolicy,
}

/// One product: its catalog class and the listing element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub class_utf16: Vec<u8>,
    pub element: CashShopProduct,
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
        if p.format_version != 2
            || p.customize_items.len() + p.customize_prices.len() + p.customize_coupons.len()
                > 262_144
            || p.features.len() > 512
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
        for (index, class) in rows {
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
            products.push(Product {
                class_utf16: class,
                element: CashShopProduct {
                    product_no,
                    product_expire: 0,
                    product_pieces: 1,
                    product_id: Some(product_id),
                    product_guid: Some(format!("00000000-0000-4000-8000-{product_no:012x}")),
                    payment_type: NEXON_CASH,
                    product_type: Some(String::new()),
                    sale_price: prices
                        .get(name.as_str())
                        .copied()
                        .unwrap_or(self.default_price),
                    category_no,
                    status: ON_SALE,
                },
            });
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
        Ok(Customize {
            items,
            prices,
            coupons,
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
}
