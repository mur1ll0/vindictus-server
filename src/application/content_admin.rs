//! NPC shops and cash shop prices on the panel (B4.4). The views read the content the game
//! loaded; edits are validated against it, stay pending until an admin publishes a version,
//! and reach the game when the host script applies the version (exporters, client copy,
//! game restart). GM and admin create and withdraw edits; only an admin publishes.
use super::admin_session::AdminError;
use crate::{
    domain::admin::{AuditReason, Permission},
    infrastructure::{
        admin_characters::EnglishNames,
        admin_store::{AuditRecord, SessionAccount},
        content_edits::{self, ContentEdit, ContentVersion, EditOutcome},
        minimum_assets::Assets,
    },
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::BTreeMap, net::IpAddr};
use uuid::Uuid;

pub const MAX_PRICE: i64 = 2_000_000_000;
pub const MAX_CASH_PRICE: i64 = 1_000_000;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ShopSummary {
    pub shop_id: String,
    pub rows: usize,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ShopRowView {
    pub order: i16,
    pub item_class: String,
    pub item_name: Option<String>,
    pub count: i32,
    pub price_item_class: String,
    /// Effective price (the game's PriceCalc of PriceCount).
    pub price_count: i32,
    pub character_bind: bool,
    /// Rows with a purchase limit are not sold by the laboratory yet.
    pub restricted: bool,
    pub feature: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CashCategoryView {
    pub category: String,
    pub products: usize,
    /// Lowest and highest product price of the category (cash).
    pub min_price: i32,
    pub max_price: i32,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EditRequest {
    /// Add a row at a free Order, or change the row at that Order.
    ShopRow {
        shop_id: String,
        order: i16,
        item_class: String,
        count: i32,
        price_item_class: String,
        price_count: i64,
    },
    ShopRowRemove {
        shop_id: String,
        order: i16,
    },
    CashPrice {
        category: String,
        price: i64,
    },
    CashDefault {
        price: i64,
    },
}

pub struct Actor<'a> {
    pub session: &'a SessionAccount,
    pub peer: Option<IpAddr>,
    pub operation_id: Uuid,
    pub reason: &'a str,
}

fn require(session: &SessionAccount, permission: Permission) -> Result<(), AdminError> {
    if session.role.is_some_and(|r| r.allows(permission)) {
        Ok(())
    } else {
        Err(AdminError::Forbidden)
    }
}

fn audit<'a>(
    actor: &'a Actor<'a>,
    reason: &'a AuditReason,
    action: &'static str,
    target_kind: &'static str,
    target: &'a str,
) -> AuditRecord<'a> {
    AuditRecord {
        account_id: Some(actor.session.account_id),
        actor_name: Some(&actor.session.username),
        role: actor.session.role,
        operation_id: Some(actor.operation_id),
        action,
        target_kind: Some(target_kind),
        target_id: Some(target),
        reason: Some(reason),
        before: None,
        after: None,
        refused: false,
        peer: actor.peer,
    }
}

fn outcome(outcome: EditOutcome) -> Result<i64, AdminError> {
    match outcome {
        EditOutcome::Done(v) => Ok(v),
        EditOutcome::NotFound => Err(AdminError::NotFound),
        EditOutcome::Nothing => Err(AdminError::Unchanged("sem_pendentes")),
        EditOutcome::Repeated => Err(AdminError::RepeatedOperation),
    }
}

pub fn shops(
    session: &SessionAccount,
    assets: Option<&Assets>,
) -> Result<Vec<ShopSummary>, AdminError> {
    require(session, Permission::Read)?;
    let shops = assets
        .and_then(Assets::shops)
        .ok_or(AdminError::ContentUnavailable)?;
    let mut by_id: BTreeMap<String, ShopSummary> = BTreeMap::new();
    for row in shops.all() {
        by_id
            .entry(row.shop_id.to_lowercase())
            .or_insert_with(|| ShopSummary {
                shop_id: row.shop_id.clone(),
                rows: 0,
            })
            .rows += 1;
    }
    Ok(by_id.into_values().collect())
}

pub fn shop(
    session: &SessionAccount,
    assets: Option<&Assets>,
    names: &EnglishNames,
    shop_id: &str,
) -> Result<Vec<ShopRowView>, AdminError> {
    require(session, Permission::Read)?;
    let shops = assets
        .and_then(Assets::shops)
        .ok_or(AdminError::ContentUnavailable)?;
    let rows: Vec<ShopRowView> = shops
        .rows_of(shop_id)
        .map(|r| ShopRowView {
            order: r.order,
            item_name: names.item_name(&r.item_class),
            item_class: r.item_class.clone(),
            count: r.count,
            price_item_class: r.price_item_class.clone(),
            price_count: r.price_count,
            character_bind: r.character_bind,
            restricted: r.restriction_count > 0,
            feature: r.feature.clone(),
        })
        .collect();
    if rows.is_empty() {
        return Err(AdminError::NotFound);
    }
    Ok(rows)
}

pub fn cash_categories(
    session: &SessionAccount,
    assets: Option<&Assets>,
) -> Result<Vec<CashCategoryView>, AdminError> {
    require(session, Permission::Read)?;
    let cash = assets
        .and_then(Assets::cash_shop)
        .ok_or(AdminError::ContentUnavailable)?;
    let names: BTreeMap<i32, String> = cash
        .categories()
        .iter()
        .filter_map(|c| Some((c.category_no, c.category_name.clone()?)))
        .collect();
    let mut by_name: BTreeMap<String, CashCategoryView> = BTreeMap::new();
    for p in cash.products() {
        let Some(name) = names.get(&p.element.category_no) else {
            continue;
        };
        let v = by_name
            .entry(name.clone())
            .or_insert_with(|| CashCategoryView {
                category: name.clone(),
                products: 0,
                min_price: i32::MAX,
                max_price: 0,
            });
        v.products += 1;
        v.min_price = v.min_price.min(p.element.sale_price);
        v.max_price = v.max_price.max(p.element.sale_price);
    }
    Ok(by_name.into_values().collect())
}

/// The edit's storage kind, target and value, after the checks against the loaded content.
fn validate(
    assets: &Assets,
    edit: &EditRequest,
) -> Result<(&'static str, String, Option<serde_json::Value>), AdminError> {
    let shop_of = |shop_id: &str| -> Result<String, AdminError> {
        let shops = assets.shops().ok_or(AdminError::ContentUnavailable)?;
        shops
            .rows_of(shop_id)
            .next()
            .map(|r| r.shop_id.clone())
            .ok_or(AdminError::NotFound)
    };
    match edit {
        EditRequest::ShopRow {
            shop_id,
            order,
            item_class,
            count,
            price_item_class,
            price_count,
        } => {
            let shop = shop_of(shop_id)?;
            let valid_class = |c: &str| {
                !c.is_empty() && c.len() <= 50 && c.is_ascii() && assets.has_item_class(c)
            };
            if *order < 0
                || !(1..=9999).contains(count)
                || !(1..=MAX_PRICE).contains(price_count)
                || !valid_class(item_class)
                || !(price_item_class == "gold" || valid_class(price_item_class))
            {
                return Err(AdminError::InvalidInput);
            }
            Ok((
                "shop_row",
                format!("{shop}#{order}"),
                Some(serde_json::json!({
                    "ItemClass": item_class, "Count": count,
                    "PriceItemClass": price_item_class, "PriceCount": price_count
                })),
            ))
        }
        EditRequest::ShopRowRemove { shop_id, order } => {
            let shop = shop_of(shop_id)?;
            if *order < 0 {
                return Err(AdminError::InvalidInput);
            }
            Ok(("shop_row_remove", format!("{shop}#{order}"), None))
        }
        EditRequest::CashPrice { category, price } => {
            let cash = assets.cash_shop().ok_or(AdminError::ContentUnavailable)?;
            let known = cash
                .categories()
                .iter()
                .any(|c| c.category_name.as_deref() == Some(category.as_str()));
            if !known || !(1..=MAX_CASH_PRICE).contains(price) {
                return Err(AdminError::InvalidInput);
            }
            Ok((
                "cash_price",
                category.clone(),
                Some(serde_json::json!({ "price": price })),
            ))
        }
        EditRequest::CashDefault { price } => {
            if !(1..=MAX_CASH_PRICE).contains(price) {
                return Err(AdminError::InvalidInput);
            }
            Ok((
                "cash_default",
                "default".into(),
                Some(serde_json::json!({ "price": price })),
            ))
        }
    }
}

pub async fn create_edit(
    pool: &PgPool,
    assets: Option<&Assets>,
    actor: &Actor<'_>,
    edit: &EditRequest,
) -> Result<i64, AdminError> {
    require(actor.session, Permission::Manage)?;
    let reason = AuditReason::parse(actor.reason).ok_or(AdminError::InvalidInput)?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let (kind, target, value) = validate(assets, edit)?;
    outcome(
        content_edits::create(
            pool,
            kind,
            &target,
            value.as_ref(),
            actor.session.account_id,
            reason.as_str(),
            audit(actor, &reason, "content.edit", "content", &target),
        )
        .await?,
    )
}

pub async fn withdraw(pool: &PgPool, actor: &Actor<'_>, id: i64) -> Result<(), AdminError> {
    require(actor.session, Permission::Manage)?;
    let reason = AuditReason::parse(actor.reason).ok_or(AdminError::InvalidInput)?;
    let target = id.to_string();
    outcome(
        content_edits::withdraw(
            pool,
            id,
            audit(actor, &reason, "content.withdraw", "content", &target),
        )
        .await?,
    )
    .map(|_| ())
}

/// Publish the pending edits as the next version (admin).
pub async fn publish(pool: &PgPool, actor: &Actor<'_>) -> Result<i64, AdminError> {
    require(actor.session, Permission::Administer)?;
    let reason = AuditReason::parse(actor.reason).ok_or(AdminError::InvalidInput)?;
    outcome(
        content_edits::publish(
            pool,
            actor.session.account_id,
            reason.as_str(),
            audit(actor, &reason, "content.publish", "content", "version"),
        )
        .await?,
    )
}

pub async fn edits(
    pool: &PgPool,
    session: &SessionAccount,
) -> Result<(Vec<ContentEdit>, Vec<ContentVersion>), AdminError> {
    require(session, Permission::Read)?;
    Ok((
        content_edits::list(pool, 200).await?,
        content_edits::versions(pool, 50).await?,
    ))
}
