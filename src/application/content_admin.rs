//! NPC shops and cash shop prices on the panel (B4.4). The views read the content the game
//! loaded; edits are validated against it, stay pending until an admin publishes a version,
//! and reach the game when it restarts: publishing schedules a maintenance that warns the
//! players, disconnects them at the end and restarts the server with the version (B4.4c).
//! GM and admin create and withdraw edits; only an admin publishes or cancels.
use super::admin_session::AdminError;
use crate::{
    domain::admin::Permission,
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
    /// A FeatureMatrix event switch on or off (laboratory extension).
    Event {
        key: String,
        enabled: bool,
    },
}

/// The content tables whose Feature the migrated server already evaluates.
pub const MIGRATED_EVENT_TABLES: [&str; 11] = [
    "ShopInfo",
    "StoryLineInfo",
    "QuestInfo",
    "ItemClassInfo",
    "EquipItemInfo",
    "EventDropItemInfo",
    "PeriodicEventInfo",
    "StoryLinePeriodInfo",
    "CouponShopVersionInfo",
    "CouponShopItemInfo",
    "AllUserGoalInfo",
];

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventUseView {
    pub table: String,
    pub rows: i32,
    /// The server already reads this table's Feature (else the rows wait for their system).
    pub migrated: bool,
}
/// A FeatureMatrix event switch of the region (Ajustes de jogo → Eventos).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventView {
    pub key: String,
    /// On in the original content of the region.
    pub original: bool,
    /// On now (the panel's switch, else the original).
    pub enabled: bool,
    pub uses: Vec<EventUseView>,
}

/// An item of an event's description, with its English name and icon (icons/item/<icon>.png).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventItemView {
    pub class: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub count: i64,
}
/// An item sold by an NPC shop or a coupon shop of the event.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventTradeView {
    /// ShopID, or the coupon shop version.
    pub shop: String,
    pub item: EventItemView,
    /// The price item (currency) and its count.
    pub price: EventItemView,
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventDropView {
    /// The monster (Class Variation); None for every boss.
    pub monster: Option<String>,
    pub item: EventItemView,
    /// Weight within the event's table.
    pub probability: i64,
    /// TodayQuest, or None.
    pub condition: Option<String>,
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventQuestView {
    pub id: String,
    pub title: Option<String>,
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventStoryView {
    pub id: String,
    pub npc: Option<ShopNpcView>,
    pub exp: i64,
    pub gold: i64,
    pub ap: i64,
    pub item: Option<EventItemView>,
    /// RewardTitle (TitleInfo ID), 0 without one.
    pub title: i64,
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventPeriodicView {
    /// Gold, AP, Exp or ITEM.
    pub kind: String,
    pub item: Option<EventItemView>,
    pub amount: i64,
    /// Possibility (weight among the enabled rewards).
    pub rate: i64,
}
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventGoalView {
    /// GoalName (a localization key).
    pub name: Option<String>,
    /// The goal's event regex (kill …, …).
    pub goal: Option<String>,
    pub count: i64,
    pub reward: Option<EventItemView>,
    pub start: Option<String>,
    pub end: Option<String>,
}
/// What an event switch brings (the panel's description of Ajustes de jogo → Eventos).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EventDetailView {
    pub key: String,
    /// The NPCs of the event's shops and storylines.
    pub npcs: Vec<ShopNpcView>,
    pub shops: Vec<EventTradeView>,
    pub coupons: Vec<EventTradeView>,
    pub drops: Vec<EventDropView>,
    pub quests: Vec<EventQuestView>,
    pub stories: Vec<EventStoryView>,
    /// TitleInfo IDs.
    pub titles: Vec<i64>,
    pub periodic: Vec<EventPeriodicView>,
    pub goals: Vec<EventGoalView>,
    /// Every table naming the key, with what the server already reads.
    pub uses: Vec<EventUseView>,
    /// The tables the server already reads (each section works when its table is here).
    pub migrated: Vec<String>,
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
    pub events: Vec<EventView>,
}

pub struct Actor<'a> {
    pub session: &'a SessionAccount,
    pub peer: Option<IpAddr>,
    pub operation_id: Uuid,
}

pub(crate) fn require(session: &SessionAccount, permission: Permission) -> Result<(), AdminError> {
    if session.role.is_some_and(|r| r.allows(permission)) {
        Ok(())
    } else {
        Err(AdminError::Forbidden)
    }
}

pub(crate) fn audit<'a>(
    actor: &'a Actor<'a>,
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
        before: None,
        after: None,
        refused: false,
        peer: actor.peer,
    }
}

pub(crate) fn outcome(outcome: EditOutcome) -> Result<i64, AdminError> {
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
    let current = assets.feature_version().to_owned();
    let events = assets
        .event_switches()
        .iter()
        .map(|x| {
            let original = crate::domain::feature_policy::FeaturePolicy::new([
                ("CurrentVer".to_owned(), Some(current.clone())),
                (x.key.clone(), x.value.clone()),
            ])
            .and_then(|p| p.is_enabled(Some(&x.key)))
            .unwrap_or(false);
            EventView {
                key: x.key.clone(),
                original,
                enabled: assets
                    .event_overrides
                    .get(&x.key)
                    .copied()
                    .unwrap_or(original),
                uses: x
                    .uses
                    .iter()
                    .map(|(table, rows)| EventUseView {
                        migrated: MIGRATED_EVENT_TABLES.contains(&table.as_str()),
                        table: table.clone(),
                        rows: *rows,
                    })
                    .collect(),
            }
        })
        .collect();
    Ok(TuningView {
        features,
        quests,
        enhance,
        enchants,
        events,
    })
}

fn event_item(
    names: &EnglishNames,
    class: Option<&str>,
    count: Option<i64>,
) -> Option<EventItemView> {
    let class = class.filter(|c| !c.is_empty())?;
    Some(EventItemView {
        class: class.to_owned(),
        name: names.item_name(class),
        icon: names.item_icon(class),
        count: count.unwrap_or(0),
    })
}

/// The description of one event switch: its shops, coupon shops, drops, quests, stories,
/// titles, periodic rewards and collective goals, with English names and icons.
pub fn event_detail(
    session: &SessionAccount,
    assets: Option<&Assets>,
    names: &EnglishNames,
    key: &str,
) -> Result<EventDetailView, AdminError> {
    require(session, Permission::Read)?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let event = assets
        .event_switches()
        .iter()
        .find(|x| x.key == key)
        .ok_or(AdminError::NotFound)?;
    let d = &event.detail;
    let npc = |id: &str, place: String| ShopNpcView {
        name: names.npc_name(id),
        npc: id.to_owned(),
        place,
    };
    let mut npcs: Vec<ShopNpcView> = Vec::new();
    let mut add_npc = |view: ShopNpcView| {
        if !npcs.iter().any(|n| n.npc.eq_ignore_ascii_case(&view.npc)) {
            npcs.push(view);
        }
    };
    let trade = |shop: String,
                 item: Option<&str>,
                 count: Option<i64>,
                 price: Option<&str>,
                 price_count: Option<i64>| {
        Some(EventTradeView {
            shop,
            item: event_item(names, item, count)?,
            price: event_item(names, price, price_count)?,
        })
    };
    let shops: Vec<EventTradeView> = d
        .shops
        .iter()
        .filter_map(|(shop, item, count, price, price_count)| {
            let shop = shop.clone().unwrap_or_default();
            for n in names.shop_npcs(&shop) {
                add_npc(npc(&n.npc, n.place));
            }
            trade(
                shop,
                item.as_deref(),
                *count,
                price.as_deref(),
                *price_count,
            )
        })
        .collect();
    let stories = d
        .stories
        .iter()
        .filter_map(|(id, owner, exp, gold, ap, item, item_count, title)| {
            let owner = owner
                .as_deref()
                .filter(|n| !n.is_empty())
                .map(|n| npc(n, String::new()));
            if let Some(o) = &owner {
                add_npc(npc(&o.npc, String::new()));
            }
            Some(EventStoryView {
                id: id.clone()?,
                npc: owner,
                exp: exp.unwrap_or(0),
                gold: gold.unwrap_or(0),
                ap: ap.unwrap_or(0),
                item: event_item(names, item.as_deref(), *item_count),
                title: title.unwrap_or(0),
            })
        })
        .collect();
    Ok(EventDetailView {
        key: event.key.clone(),
        npcs,
        shops,
        coupons: d
            .coupons
            .iter()
            .filter_map(|(version, item, count, price, price_count)| {
                trade(
                    version.map(|v| v.to_string()).unwrap_or_default(),
                    item.as_deref(),
                    *count,
                    price.as_deref(),
                    *price_count,
                )
            })
            .collect(),
        drops: d
            .drops
            .iter()
            .filter_map(|(monster, item, probability, condition)| {
                Some(EventDropView {
                    monster: monster.clone(),
                    item: event_item(names, item.as_deref(), Some(1))?,
                    probability: probability.unwrap_or(0),
                    condition: condition.clone(),
                })
            })
            .collect(),
        quests: d
            .quests
            .iter()
            .flatten()
            .map(|id| EventQuestView {
                title: names.quest_title(id),
                id: id.clone(),
            })
            .collect(),
        stories,
        titles: d.titles.iter().flatten().copied().collect(),
        periodic: d
            .periodic
            .iter()
            .map(|(kind, arg, amount, rate)| EventPeriodicView {
                kind: kind.clone().unwrap_or_default(),
                item: event_item(names, arg.as_deref(), *amount),
                amount: amount.unwrap_or(0),
                rate: rate.unwrap_or(0),
            })
            .collect(),
        goals: d
            .goals
            .iter()
            .map(
                |(name, goal, count, reward, num, start, end)| EventGoalView {
                    name: name.clone(),
                    goal: goal.clone(),
                    count: count.unwrap_or(0),
                    reward: event_item(names, reward.as_deref(), *num),
                    start: start.clone(),
                    end: end.clone(),
                },
            )
            .collect(),
        uses: event
            .uses
            .iter()
            .map(|(table, rows)| EventUseView {
                migrated: MIGRATED_EVENT_TABLES.contains(&table.as_str()),
                table: table.clone(),
                rows: *rows,
            })
            .collect(),
        migrated: MIGRATED_EVENT_TABLES
            .iter()
            .map(|t| (*t).to_owned())
            .collect(),
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
        EditRequest::Event { key, enabled } => {
            if !assets.event_switches().iter().any(|x| &x.key == key) {
                return Err(AdminError::NotFound);
            }
            Ok((
                "event",
                key.clone(),
                Some(serde_json::json!({ "enabled": enabled })),
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
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let (kind, target, value) = validate(assets, edit)?;
    outcome(
        content_edits::create(
            pool,
            kind,
            &target,
            value.as_ref(),
            actor.session.account_id,
            audit(actor, "content.edit", "content", &target),
        )
        .await?,
    )
}

pub async fn withdraw(pool: &PgPool, actor: &Actor<'_>, id: i64) -> Result<(), AdminError> {
    require(actor.session, Permission::Manage)?;
    let target = id.to_string();
    outcome(
        content_edits::withdraw(
            pool,
            id,
            audit(actor, "content.withdraw", "content", &target),
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
    if !(0..=MAX_RESTART_MINUTES).contains(&minutes) {
        return Err(AdminError::InvalidInput);
    }
    if maintenance.is_some_and(|m| m.current().is_some()) {
        return Err(AdminError::Unchanged("manutencao"));
    }
    let mut record = audit(actor, "content.publish", "content", "version");
    record.after = Some(serde_json::json!({ "restart_minutes": minutes }).to_string());
    let version = outcome(content_edits::publish(pool, actor.session.account_id, record).await?)?;
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

/// What a tuning save did (B4.5b).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TuningApplied {
    pub version: i64,
    /// The game took the values now, without a restart. When false the version stays
    /// published and the next restart applies it.
    pub live: bool,
    /// Enhance/Enchant chances also show in the client's own windows: its copy takes them
    /// when opened through start-lab.bat.
    pub client_refresh: bool,
    /// Event switches reach the client in SyncFeatureMatrix: it sees them at its next login.
    pub relog: bool,
}

pub const MAX_TUNING_EDITS: usize = 300;

/// The server's own marketplace listings (laboratory extension, Ajustes de jogo).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MarketAutoView {
    pub enabled: bool,
    /// Hours between two runs (1–168).
    pub interval_hours: i32,
    /// Listings per run (1–1000).
    pub items: i32,
    /// Days each listing stays open (1–30).
    pub days: i32,
    /// Seconds since the Unix epoch of the last run.
    pub last_run_unix: Option<i64>,
    pub last_listed: i32,
    /// Classes the loaded content allows (None without the game content).
    pub candidates: Option<usize>,
}
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct MarketAutoRequest {
    pub operation_id: Uuid,
    pub enabled: bool,
    pub interval_hours: i32,
    pub items: i32,
    pub days: i32,
}
fn market_auto_view(
    state: crate::infrastructure::minimum_marketplace::AutoState,
    runtime: Option<&crate::infrastructure::lab_frontend::Runtime>,
) -> MarketAutoView {
    const EPOCH_TICKS: i64 = 621_355_968_000_000_000;
    MarketAutoView {
        enabled: state.settings.enabled,
        interval_hours: state.settings.interval_hours,
        items: state.settings.items,
        days: state.settings.days,
        last_run_unix: state.last_run.map(|t| (t - EPOCH_TICKS) / 10_000_000),
        last_listed: state.last_listed,
        candidates: runtime
            .and_then(|r| r.minimum_assets())
            .map(|a| crate::infrastructure::minimum_marketplace::auto_candidates(&a)),
    }
}
pub async fn market_auto(
    pool: &PgPool,
    session: &SessionAccount,
    runtime: Option<&crate::infrastructure::lab_frontend::Runtime>,
) -> Result<MarketAutoView, AdminError> {
    require(session, Permission::Read)?;
    let state = crate::infrastructure::minimum_marketplace::auto_settings(pool)
        .await
        .map_err(|_| AdminError::ContentUnavailable)?;
    Ok(market_auto_view(state, runtime))
}
/// Save the auto-listing settings (admin); audited with the values before and after.
/// The game's minute tick reads them, so they apply without a restart.
pub async fn save_market_auto(
    pool: &PgPool,
    runtime: Option<&crate::infrastructure::lab_frontend::Runtime>,
    actor: &Actor<'_>,
    request: &MarketAutoRequest,
) -> Result<MarketAutoView, AdminError> {
    use crate::domain::marketplace::auto::Settings;
    require(actor.session, Permission::Administer)?;
    let settings = Settings {
        enabled: request.enabled,
        interval_hours: request.interval_hours,
        items: request.items,
        days: request.days,
    };
    if !settings.valid() {
        return Err(AdminError::InvalidInput);
    }
    let json = |s: &Settings| {
        serde_json::json!({"enabled": s.enabled, "interval_hours": s.interval_hours, "items": s.items, "days": s.days})
            .to_string()
    };
    let mut record = audit(actor, "content.market_auto", "content", "market_auto");
    let mut tx = pool.begin().await?;
    let before = crate::infrastructure::minimum_marketplace::save_auto_settings(&mut tx, &settings)
        .await
        .map_err(|_| AdminError::InvalidInput)?;
    record.before = Some(json(&before.settings));
    record.after = Some(json(&settings));
    match crate::infrastructure::admin_store::insert_audit(&mut tx, &record).await {
        Ok(()) => tx.commit().await?,
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(AdminError::RepeatedOperation);
        }
        Err(e) => return Err(e.into()),
    }
    let state = crate::infrastructure::minimum_marketplace::auto_settings(pool)
        .await
        .map_err(|_| AdminError::ContentUnavailable)?;
    Ok(market_auto_view(state, runtime))
}

/// Save tuning values (admin): FeatureMatrix rates, quest drop counts, Enhance and Enchant
/// chances are read by the server when used, so they publish as their own version and the
/// game reloads its contents at once. Shop and cash edits keep the pending/maintenance cycle.
pub async fn apply_tuning(
    pool: &PgPool,
    runtime: Option<&crate::infrastructure::lab_frontend::Runtime>,
    actor: &Actor<'_>,
    edits: &[EditRequest],
) -> Result<TuningApplied, AdminError> {
    require(actor.session, Permission::Administer)?;
    let runtime = runtime.ok_or(AdminError::ContentUnavailable)?;
    let assets = runtime
        .minimum_assets()
        .ok_or(AdminError::ContentUnavailable)?;
    if edits.is_empty() || edits.len() > MAX_TUNING_EDITS {
        return Err(AdminError::InvalidInput);
    }
    let mut rows = Vec::with_capacity(edits.len());
    let mut client_refresh = false;
    let mut relog = false;
    for edit in edits {
        match edit {
            EditRequest::Feature { .. } | EditRequest::QuestDrop { .. } => {}
            EditRequest::EnhanceRatio { .. } | EditRequest::EnchantRatio { .. } => {
                client_refresh = true;
            }
            EditRequest::Event { .. } => relog = true,
            _ => return Err(AdminError::InvalidInput),
        }
        rows.push(validate(&assets, edit)?);
    }
    drop(assets);
    let record = audit(actor, "content.tuning", "content", "tuning");
    let version =
        outcome(content_edits::publish_now(pool, actor.session.account_id, &rows, record).await?)?;
    let overlay = content_edits::loaded_overlay(pool).await?;
    let live = match runtime.reload_assets(overlay).await {
        Ok(()) => {
            let v = i32::try_from(version).map_err(|_| AdminError::InvalidInput)?;
            content_edits::mark_version_applied(pool, v).await?;
            if let Some(path) = runtime.content_output() {
                let value = content_edits::overlay(pool).await?;
                if let Err(e) = write_atomic(path, value.to_string().as_bytes()).await {
                    tracing::warn!(error = %e, "content edits for the client launcher not written");
                }
            }
            true
        }
        Err(e) => {
            tracing::warn!(error = ?e, version, "live tuning not loaded; the next restart applies it");
            false
        }
    };
    Ok(TuningApplied {
        version,
        live,
        client_refresh,
        relog,
    })
}

async fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let partial = path.with_extension("partial");
    tokio::fs::write(&partial, bytes).await?;
    tokio::fs::rename(&partial, path).await
}

/// Cancel the scheduled maintenance (Delay -1 of the original operation); audited.
pub async fn cancel_maintenance(
    pool: &PgPool,
    actor: &Actor<'_>,
    maintenance: &Maintenance,
) -> Result<(), AdminError> {
    require(actor.session, Permission::Administer)?;
    let current = maintenance.current().ok_or(AdminError::NotFound)?;
    let target = current.version.to_string();
    let mut record = audit(actor, "content.maintenance_cancel", "content", &target);
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
