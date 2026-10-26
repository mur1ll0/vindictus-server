//! NPC shops and cash shop prices on the panel (B4.4). The views read the content the game
//! loaded; edits are validated against it, stay pending until an admin publishes a version,
//! and reach the game when it restarts: publishing schedules a maintenance that warns the
//! players, disconnects them at the end and restarts the server with the version (B4.4c).
//! GM and admin create and withdraw edits; only an admin publishes or cancels.
use super::admin_session::AdminError;
use crate::{
    domain::admin::{AuditReason, Permission},
    infrastructure::{
        admin_characters::EnglishNames,
        admin_store::{AuditRecord, SessionAccount},
        content_edits::{self, ContentEdit, ContentVersion, EditOutcome},
        maintenance::{Maintenance, MaintenanceError, Plan},
        minimum_assets::Assets,
    },
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::BTreeMap, net::IpAddr};
use uuid::Uuid;

pub const MAX_PRICE: i64 = 2_000_000_000;
pub const MAX_CASH_PRICE: i64 = 1_000_000;
/// The longest maintenance notice the panel schedules.
pub const MAX_RESTART_MINUTES: i32 = 60;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ShopNpcView {
    pub npc: String,
    /// English name of the NPC, when the client has one.
    pub name: Option<String>,
    /// Place of the NPC (Colhen, Rocheste, Malina (Morvan), Eventos, ...).
    pub place: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ShopSummary {
    pub shop_id: String,
    pub rows: usize,
    /// Rows the game sells in this region (FeatureMatrix of each row).
    pub available_rows: usize,
    /// NPCs that open the shop, from the NPC talk scripts; empty when none does.
    pub npcs: Vec<ShopNpcView>,
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
    /// The row's FeatureMatrix is enabled in this region, so the client shows it (NotSell,
    /// TestServer and other regions' events are not).
    pub available: bool,
    /// Icon file (GET /api/icons/item/{icon}).
    pub icon: Option<String>,
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
    /// B4.5: a FeatureMatrix rate in percent (ExpRate_Combat, GoldRate_Story, ...).
    Feature {
        key: String,
        value: i32,
    },
    /// QuestInfo DropItemCountMin/Max: how many common monsters of a stage carry a drop.
    QuestDrop {
        quest: String,
        min: i32,
        max: i32,
    },
    /// EnhanceServerInfo SuccessRatio (0–1) of a type and level.
    EnhanceRatio {
        enhance_type: String,
        level: i32,
        ratio: f32,
    },
    /// EnchantInfo Min/MaxSuccessRatio (percent) of an enchant.
    EnchantRatio {
        class: String,
        min: i32,
        max: i32,
    },
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct FeatureView {
    pub key: String,
    pub value: i32,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct QuestDropView {
    pub quest: String,
    pub title: Option<String>,
    pub min: i32,
    pub max: i32,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EnhanceRatioView {
    pub enhance_type: String,
    pub level: i32,
    pub ratio: f32,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EnchantRatioView {
    pub class: String,
    pub name: Option<String>,
    pub prefix: bool,
    pub rank: i32,
    pub min: i32,
    pub max: i32,
}

/// The tunable values the game loaded (published versions included).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TuningView {
    pub features: Vec<FeatureView>,
    pub quests: Vec<QuestDropView>,
    pub enhance: Vec<EnhanceRatioView>,
    pub enchants: Vec<EnchantRatioView>,
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
    names: &EnglishNames,
) -> Result<Vec<ShopSummary>, AdminError> {
    require(session, Permission::Read)?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let shops = assets.shops().ok_or(AdminError::ContentUnavailable)?;
    let mut by_id: BTreeMap<String, ShopSummary> = BTreeMap::new();
    for row in shops.all() {
        let summary = by_id
            .entry(row.shop_id.to_lowercase())
            .or_insert_with(|| ShopSummary {
                shop_id: row.shop_id.clone(),
                rows: 0,
                available_rows: 0,
                npcs: names
                    .shop_npcs(&row.shop_id)
                    .into_iter()
                    .map(|n| ShopNpcView {
                        name: names.npc_name(&n.npc),
                        npc: n.npc,
                        place: n.place,
                    })
                    .collect(),
            });
        summary.rows += 1;
        if assets.feature_enabled(row.feature.as_deref()) {
            summary.available_rows += 1;
        }
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
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let shops = assets.shops().ok_or(AdminError::ContentUnavailable)?;
    let rows: Vec<ShopRowView> = shops
        .rows_of(shop_id)
        .map(|r| ShopRowView {
            available: assets.feature_enabled(r.feature.as_deref()),
            icon: names.item_icon(&r.item_class),
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

pub fn tuning(
    session: &SessionAccount,
    assets: Option<&Assets>,
    names: &EnglishNames,
) -> Result<TuningView, AdminError> {
    require(session, Permission::Read)?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let policy = assets.npc_policy();
    let features = crate::domain::content_overlay::TUNABLE_FEATURES
        .iter()
        .filter_map(|key| {
            let value = policy?.get_integer(key).ok()?;
            Some(FeatureView {
                key: (*key).to_owned(),
                value,
            })
        })
        .collect();
    let quests = assets
        .mission()
        .map(|m| {
            m.drop_counts()
                .map(|(quest, min, max)| QuestDropView {
                    quest: quest.to_owned(),
                    title: names.quest_title(quest),
                    min,
                    max,
                })
                .collect()
        })
        .unwrap_or_default();
    let (enhance, enchants) = match assets.enhance_contents() {
        Some(c) => (
            c.server_ratios()
                .map(|(t, level, ratio)| EnhanceRatioView {
                    enhance_type: t.to_owned(),
                    level,
                    ratio,
                })
                .collect(),
            c.enchants()
                .map(|e| EnchantRatioView {
                    class: e.class.clone(),
                    name: names.enchant_name(&e.class, e.is_prefix),
                    prefix: e.is_prefix,
                    rank: e.level,
                    min: e.min_ratio,
                    max: e.max_ratio,
                })
                .collect(),
        ),
        None => (Vec::new(), Vec::new()),
    };
    Ok(TuningView {
        features,
        quests,
        enhance,
        enchants,
    })
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
        EditRequest::Feature { key, value } => {
            let known = crate::domain::content_overlay::TUNABLE_FEATURES.contains(&key.as_str())
                && assets
                    .npc_policy()
                    .is_some_and(|p| p.get_integer(key).is_ok());
            if !known || !(0..=crate::domain::content_overlay::MAX_RATE).contains(value) {
                return Err(AdminError::InvalidInput);
            }
            Ok((
                "feature",
                key.clone(),
                Some(serde_json::json!({ "value": value })),
            ))
        }
        EditRequest::QuestDrop { quest, min, max } => {
            let mission = assets.mission().ok_or(AdminError::ContentUnavailable)?;
            if !mission.drop_counts().any(|(q, _, _)| q == quest) {
                return Err(AdminError::NotFound);
            }
            if !(0..=*max).contains(min) || *max > crate::domain::content_overlay::MAX_DROP_COUNT {
                return Err(AdminError::InvalidInput);
            }
            Ok((
                "quest_drop",
                quest.clone(),
                Some(serde_json::json!({ "min": min, "max": max })),
            ))
        }
        EditRequest::EnhanceRatio {
            enhance_type,
            level,
            ratio,
        } => {
            let contents = assets
                .enhance_contents()
                .ok_or(AdminError::ContentUnavailable)?;
            if contents.server_ratio(enhance_type, *level).is_none() {
                return Err(AdminError::NotFound);
            }
            if !(0.0..=1.0).contains(ratio) {
                return Err(AdminError::InvalidInput);
            }
            Ok((
                "enhance_ratio",
                format!("{enhance_type}#{level}"),
                Some(serde_json::json!({ "ratio": ratio })),
            ))
        }
        EditRequest::EnchantRatio { class, min, max } => {
            let contents = assets
                .enhance_contents()
                .ok_or(AdminError::ContentUnavailable)?;
            if contents.enchant(class).is_none() {
                return Err(AdminError::NotFound);
            }
            if !(0..=*max).contains(min) || *max > 100 {
                return Err(AdminError::InvalidInput);
            }
            Ok((
                "enchant_ratio",
                class.clone(),
                Some(serde_json::json!({ "min": min, "max": max })),
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

/// Publish the pending edits as the next version and schedule the restart that applies it
/// in `minutes` (0 = now). Without a game runtime the version stays to be applied.
pub async fn publish(
    pool: &PgPool,
    actor: &Actor<'_>,
    maintenance: Option<&Maintenance>,
    minutes: i32,
) -> Result<(i64, Option<Plan>), AdminError> {
    require(actor.session, Permission::Administer)?;
    let reason = AuditReason::parse(actor.reason).ok_or(AdminError::InvalidInput)?;
    if !(0..=MAX_RESTART_MINUTES).contains(&minutes) {
        return Err(AdminError::InvalidInput);
    }
    if maintenance.is_some_and(|m| m.current().is_some()) {
        return Err(AdminError::Unchanged("manutencao"));
    }
    let mut record = audit(actor, &reason, "content.publish", "content", "version");
    record.after = Some(serde_json::json!({ "restart_minutes": minutes }).to_string());
    let version = outcome(
        content_edits::publish(pool, actor.session.account_id, reason.as_str(), record).await?,
    )?;
    let plan = match maintenance {
        Some(m) => {
            let v = i32::try_from(version).map_err(|_| AdminError::InvalidInput)?;
            match m.schedule(minutes * 60, v, &actor.session.username) {
                Ok(plan) => Some(plan),
                // Another publish scheduled first; its restart loads every published version.
                Err(MaintenanceError::Busy) => m.current(),
                Err(MaintenanceError::Delay) => return Err(AdminError::InvalidInput),
            }
        }
        None => None,
    };
    Ok((version, plan))
}

/// Cancel the scheduled maintenance (Delay -1 of the original operation); audited.
pub async fn cancel_maintenance(
    pool: &PgPool,
    actor: &Actor<'_>,
    maintenance: &Maintenance,
) -> Result<(), AdminError> {
    require(actor.session, Permission::Administer)?;
    let reason = AuditReason::parse(actor.reason).ok_or(AdminError::InvalidInput)?;
    let current = maintenance.current().ok_or(AdminError::NotFound)?;
    let target = current.version.to_string();
    let mut record = audit(
        actor,
        &reason,
        "content.maintenance_cancel",
        "content",
        &target,
    );
    record.before =
        Some(serde_json::json!({ "remaining_seconds": current.remaining_seconds }).to_string());
    let mut tx = pool.begin().await?;
    match crate::infrastructure::admin_store::insert_audit(&mut tx, &record).await {
        Ok(()) => tx.commit().await?,
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(AdminError::RepeatedOperation);
        }
        Err(e) => return Err(e.into()),
    }
    maintenance.cancel();
    Ok(())
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
