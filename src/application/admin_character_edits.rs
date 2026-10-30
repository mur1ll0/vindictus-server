//! Character changes from the panel (B4.3b): give an item (ItemClassEx with attributes) or
//! gold, remove items or gold, and credit/debit cash; move and sort the inventory (B4.4d)
//! with MoveInventoryItem/SortInventory. Item changes go through the game's own
//! planner (`minimum_give`) and need the character offline: a live session owns its inventory.
//! Panel policy: GM and admin change characters; a character of an account with a panel role
//! needs an admin, unless it is the actor's own. Every change is audited in its transaction.
use super::admin_session::AdminError;
use crate::{
    domain::admin::{PanelRole, Permission},
    infrastructure::{
        admin_characters, admin_item_upgrade,
        admin_store::{AuditRecord, SessionAccount},
        minimum_assets::Assets,
        minimum_cash_shop::{self, PanelCashOutcome},
        minimum_character::MinimumError,
        minimum_give::{self, PanelItemChange, PanelItemOutcome},
        minimum_move::{self, PanelStop},
        minimum_sort,
    },
};
use sqlx::PgPool;
use std::net::IpAddr;
use uuid::Uuid;

/// Entity Service.ID of panel-created items; differs from the frontend's and the CLI's
/// (65534), so concurrent authorities never mint the same item ID.
pub const PANEL_SERVICE_ID: i32 = 65533;
pub const MAX_COUNT: i32 = 1_000_000_000;

pub struct Change<'a> {
    pub session: &'a SessionAccount,
    pub peer: Option<IpAddr>,
    pub operation_id: Uuid,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ChangeResult {
    pub before: i64,
    pub after: i64,
}

fn minimum(error: MinimumError) -> AdminError {
    match error {
        MinimumError::Database(e) => AdminError::Unavailable(e),
        other => {
            // The error chain is fixed text: it names the refusing rule, not the request.
            let mut reason = other.to_string();
            let mut source = std::error::Error::source(&other);
            while let Some(inner) = source {
                reason.push_str(": ");
                reason.push_str(&inner.to_string());
                source = inner.source();
            }
            tracing::warn!(reason, "panel character change refused");
            AdminError::NotEditable
        }
    }
}

/// The character's owner key and that account's role, checked against the actor.
async fn authorize(pool: &PgPool, session: &SessionAccount, cid: i64) -> Result<i32, AdminError> {
    if !session.role.is_some_and(|r| r.allows(Permission::Manage)) {
        return Err(AdminError::Forbidden);
    }
    let owner = admin_characters::owner(pool, cid)
        .await?
        .ok_or(AdminError::NotFound)?;
    if owner.account_role.is_some()
        && owner.account_id != Some(session.account_id)
        && session.role != Some(PanelRole::Admin)
    {
        return Err(AdminError::Forbidden);
    }
    Ok(owner.uid)
}

fn audit<'a>(change: &'a Change<'a>, action: &'static str, target: &'a str) -> AuditRecord<'a> {
    AuditRecord {
        account_id: Some(change.session.account_id),
        actor_name: Some(&change.session.username),
        role: change.session.role,
        operation_id: Some(change.operation_id),
        action,
        target_kind: Some("character"),
        target_id: Some(target),
        before: None,
        after: None,
        refused: false,
        peer: change.peer,
    }
}

/// Give (`give` true, an ItemClassEx such as `gold` or `longsword[ENHANCE:6]`) or remove
/// (`give` false, a plain class) `count` items.
pub async fn change_item(
    pool: &PgPool,
    assets: Option<&Assets>,
    change: &Change<'_>,
    cid: &str,
    item: &str,
    count: i32,
    give: bool,
) -> Result<ChangeResult, AdminError> {
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    if !(1..=MAX_COUNT).contains(&count) || item.is_empty() || item.len() > 512 {
        return Err(AdminError::InvalidInput);
    }
    authorize(pool, change.session, cid_value).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let (kind, action) = if give {
        (PanelItemChange::Give(item), "character.item_give")
    } else {
        (PanelItemChange::Destroy(item), "character.item_remove")
    };
    let outcome = minimum_give::panel_change(
        pool,
        assets,
        cid_value,
        kind,
        count,
        PANEL_SERVICE_ID,
        audit(change, action, cid),
    )
    .await
    .map_err(minimum)?;
    match outcome {
        PanelItemOutcome::Applied { before, after } => Ok(ChangeResult { before, after }),
        PanelItemOutcome::Refused(e) => Err(AdminError::ItemRefused(e.to_string())),
        PanelItemOutcome::Attributes(r) => Err(AdminError::ItemRefused(r.name().to_owned())),
        PanelItemOutcome::Online => Err(AdminError::Online),
        PanelItemOutcome::Repeated => Err(AdminError::RepeatedOperation),
    }
}

fn stopped(stop: PanelStop) -> AdminError {
    match stop {
        PanelStop::Online => AdminError::Online,
        PanelStop::Repeated => AdminError::RepeatedOperation,
    }
}

/// MoveInventoryItem: put item `item` in bag `storage` at `slot` (0–47), with the game's
/// SwapSlot (an occupant swaps places, or the same class stacks onto it).
pub async fn move_item(
    pool: &PgPool,
    assets: Option<&Assets>,
    change: &Change<'_>,
    cid: &str,
    item: &str,
    storage: u8,
    slot: i32,
) -> Result<(), AdminError> {
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    let item_id: i64 = item.parse().map_err(|_| AdminError::InvalidInput)?;
    authorize(pool, change.session, cid_value).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let mut record = audit(change, "character.item_move", cid);
    record.after =
        Some(serde_json::json!({ "item": item, "storage": storage, "slot": slot }).to_string());
    let request = crate::protocol::inventory::Move {
        item_id,
        storage,
        slot,
    };
    match minimum_move::panel_move(pool, assets, cid_value, request, &record)
        .await
        .map_err(minimum)?
    {
        Ok(minimum_move::Outcome::Applied(_)) => Ok(()),
        Ok(minimum_move::Outcome::Refused(_)) => Err(AdminError::ItemRefused(
            "o jogo recusou o movimento (bolsa indisponível, outro tipo de inventário ou espaço inválido)".into(),
        )),
        Ok(minimum_move::Outcome::Unchanged) => Err(AdminError::Unchanged("mover")),
        Err(stop) => Err(stopped(stop)),
    }
}

/// SortInventory: -1 every bag, one bag (0–8), 100 cash or 103 quest; returns how many
/// items changed place.
pub async fn sort_inventory(
    pool: &PgPool,
    assets: Option<&Assets>,
    change: &Change<'_>,
    cid: &str,
    target: i32,
) -> Result<usize, AdminError> {
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    if !matches!(target, -1..=8 | 100 | 103) {
        return Err(AdminError::InvalidInput);
    }
    authorize(pool, change.session, cid_value).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let mut record = audit(change, "character.inventory_sort", cid);
    record.after = Some(serde_json::json!({ "target": target }).to_string());
    match minimum_sort::panel_sort(pool, assets, cid_value, target, &record)
        .await
        .map_err(minimum)?
    {
        Ok(Some(0)) | Ok(None) => Err(AdminError::Unchanged("organizar")),
        Ok(Some(moved)) => Ok(moved),
        Err(stop) => Err(stopped(stop)),
    }
}

/// Credit (`delta` > 0) or debit (`delta` < 0) the cash balance of the character's account.
pub async fn adjust_cash(
    pool: &PgPool,
    change: &Change<'_>,
    cid: &str,
    delta: i32,
) -> Result<ChangeResult, AdminError> {
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    if delta == 0 || delta.unsigned_abs() > MAX_COUNT as u32 {
        return Err(AdminError::InvalidInput);
    }
    let uid = authorize(pool, change.session, cid_value).await?;
    match minimum_cash_shop::panel_adjust(pool, uid, delta, audit(change, "character.cash", cid))
        .await
        .map_err(minimum)?
    {
        PanelCashOutcome::Applied { before, after } => Ok(ChangeResult { before, after }),
        PanelCashOutcome::NotEnough => Err(AdminError::NotEnough),
        PanelCashOutcome::Repeated => Err(AdminError::RepeatedOperation),
    }
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ProgressionResult {
    /// Level, EXP, AP, acquired titles and status effects before the change.
    #[schema(value_type = Object)]
    pub before: serde_json::Value,
    #[schema(value_type = Object)]
    pub after: serde_json::Value,
}

/// EXP, target level, AP, title or status effect (B4.3c), offline, through the game's rules.
pub async fn change_progression(
    pool: &PgPool,
    runtime: Option<&crate::infrastructure::lab_frontend::Runtime>,
    change: &Change<'_>,
    cid: &str,
    growth: crate::infrastructure::admin_progression::PanelGrowth<'_>,
) -> Result<ProgressionResult, AdminError> {
    use crate::infrastructure::{admin_progression, minimum_item_logs};
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    authorize(pool, change.session, cid_value).await?;
    let runtime = runtime.ok_or(AdminError::ContentUnavailable)?;
    let assets = runtime
        .minimum_assets()
        .ok_or(AdminError::ContentUnavailable)?;
    let flags = minimum_item_logs::flags(&assets).map_err(minimum)?;
    let journal = minimum_item_logs::operation_journal(pool, flags).await;
    let ctx = runtime
        .growth_context(journal)
        .map_err(|_| AdminError::ContentUnavailable)?;
    match admin_progression::panel_change(
        pool,
        &assets,
        cid_value,
        growth,
        ctx,
        PANEL_SERVICE_ID,
        audit(change, "character.progression", cid),
    )
    .await
    .map_err(minimum)?
    {
        admin_progression::PanelGrowthOutcome::Applied { before, after } => {
            Ok(ProgressionResult { before, after })
        }
        admin_progression::PanelGrowthOutcome::Unchanged(why) => Err(AdminError::Unchanged(why)),
        admin_progression::PanelGrowthOutcome::Online => Err(AdminError::Online),
        admin_progression::PanelGrowthOutcome::Repeated => Err(AdminError::RepeatedOperation),
    }
}

/// Enhance level and enchants one equipment item of the character accepts (staff).
pub async fn item_upgrade_options(
    pool: &PgPool,
    names: &crate::infrastructure::admin_characters::EnglishNames,
    assets: Option<&Assets>,
    session: &SessionAccount,
    cid: &str,
    item: &str,
) -> Result<admin_item_upgrade::UpgradeOptions, AdminError> {
    let (cid, item) = ids(cid, item)?;
    authorize(pool, session, cid).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    upgraded(
        admin_item_upgrade::options(pool, names, assets, cid, item)
            .await
            .map_err(minimum)?,
    )
}

/// Before/after tooltips of an Enhance/enchant change, without writing (staff).
pub async fn preview_item_upgrade(
    pool: &PgPool,
    names: &crate::infrastructure::admin_characters::EnglishNames,
    assets: Option<&Assets>,
    session: &SessionAccount,
    cid: &str,
    item: &str,
    upgrade: &admin_item_upgrade::Upgrade,
) -> Result<admin_item_upgrade::UpgradePreview, AdminError> {
    let (cid, item) = ids(cid, item)?;
    authorize(pool, session, cid).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    upgraded(
        admin_item_upgrade::preview(pool, names, assets, cid, item, upgrade)
            .await
            .map_err(minimum)?,
    )
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct UpgradeResult {
    /// ENHANCE, PREFIX and SUFFIX (with Arg) and tradable, before and after (JSON text).
    pub before: String,
    pub after: String,
}

/// Write the Enhance level and enchants of an offline character's item, audited.
pub async fn upgrade_item(
    pool: &PgPool,
    assets: Option<&Assets>,
    change: &Change<'_>,
    cid: &str,
    item: &str,
    upgrade: &admin_item_upgrade::Upgrade,
) -> Result<UpgradeResult, AdminError> {
    let (cid_value, item) = ids(cid, item)?;
    authorize(pool, change.session, cid_value).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let (before, after) = upgraded(
        admin_item_upgrade::apply(
            pool,
            assets,
            cid_value,
            item,
            upgrade,
            audit(change, "character.item_upgrade", cid),
        )
        .await
        .map_err(minimum)?,
    )?;
    Ok(UpgradeResult { before, after })
}

fn ids(cid: &str, item: &str) -> Result<(i64, i64), AdminError> {
    Ok((
        cid.parse().map_err(|_| AdminError::NotFound)?,
        item.parse().map_err(|_| AdminError::NotFound)?,
    ))
}

fn upgraded<T>(outcome: admin_item_upgrade::Outcome<T>) -> Result<T, AdminError> {
    use admin_item_upgrade::Outcome;
    match outcome {
        Outcome::Done(value) => Ok(value),
        Outcome::NotFound => Err(AdminError::NotFound),
        Outcome::Refused(r) => {
            tracing::warn!(check = r.name(), "panel item upgrade refused");
            Err(AdminError::ItemRefused(r.name().to_owned()))
        }
        Outcome::Unchanged => Err(AdminError::Unchanged("sem_mudanca")),
        Outcome::Online => Err(AdminError::Online),
        Outcome::Repeated => Err(AdminError::RepeatedOperation),
    }
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PlanQuestView {
    pub id: String,
    /// English stage name (HEROES_QUEST_TITLE_<ID>).
    pub title: Option<String>,
    pub level: i32,
    pub difficulty: i32,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PlanSkipView {
    pub id: String,
    pub title: Option<String>,
    /// feature, special, no_mission, difficulty or requirement.
    pub reason: String,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ReachPlan {
    pub level: i32,
    pub quests: Vec<PlanQuestView>,
    pub skipped: Vec<PlanSkipView>,
    /// The main story: completed storylines, opened ones (next main and side), quests given,
    /// titles and skills.
    pub story: crate::domain::progression_plan::StoryPlan,
}

/// The quests "reach a level" would complete for the character (staff, read only).
pub async fn reach_plan(
    pool: &PgPool,
    runtime: Option<&crate::infrastructure::lab_frontend::Runtime>,
    names: &crate::infrastructure::admin_characters::EnglishNames,
    session: &SessionAccount,
    cid: &str,
    level: i32,
) -> Result<ReachPlan, AdminError> {
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    authorize(pool, session, cid_value).await?;
    let assets = runtime
        .ok_or(AdminError::ContentUnavailable)?
        .minimum_assets()
        .ok_or(AdminError::ContentUnavailable)?;
    let (plan, story) =
        crate::infrastructure::admin_progression::reach_preview(pool, &assets, cid_value, level)
            .await
            .map_err(minimum)?
            .ok_or(AdminError::InvalidInput)?;
    Ok(ReachPlan {
        story,
        level: plan.level,
        quests: plan
            .quests
            .into_iter()
            .map(|q| PlanQuestView {
                title: names.quest_title(&q.id),
                id: q.id,
                level: q.level,
                difficulty: q.difficulty,
            })
            .collect(),
        skipped: plan
            .skipped
            .into_iter()
            .map(|s| PlanSkipView {
                title: names.quest_title(&s.id),
                id: s.id,
                reason: s.reason.to_owned(),
            })
            .collect(),
    })
}

/// Pets of the character's account (B5): the owner or staff may read them.
pub async fn pets(
    pool: &PgPool,
    assets: Option<&Assets>,
    session: &SessionAccount,
    cid: &str,
) -> Result<Vec<crate::infrastructure::minimum_pet::PanelPet>, AdminError> {
    let cid: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    let owner = admin_characters::owner(pool, cid)
        .await?
        .ok_or(AdminError::NotFound)?;
    let staff = session.role.is_some_and(|r| r.allows(Permission::Read));
    if !staff && owner.account_id != Some(session.account_id) {
        return Err(AdminError::NotFound);
    }
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    crate::infrastructure::minimum_pet::panel_list(pool, assets, owner.uid)
        .await
        .map_err(minimum)
}

fn pet_refusal(error: crate::infrastructure::minimum_pet::PanelError) -> AdminError {
    use crate::infrastructure::minimum_pet::PanelError;
    match error {
        PanelError::Online => AdminError::Online,
        PanelError::Repeated => AdminError::RepeatedOperation,
        PanelError::Unknown => AdminError::NotFound,
        PanelError::Refused(result) => AdminError::ItemRefused(result.name().to_owned()),
    }
}

/// A pet of `pet_type` for the character's account (offline), with the game's checks.
pub async fn add_pet(
    pool: &PgPool,
    assets: Option<&Assets>,
    change: &Change<'_>,
    cid: &str,
    pet_type: i32,
    name: &str,
) -> Result<crate::infrastructure::minimum_pet::PanelPet, AdminError> {
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    if name.chars().count() > 32 {
        return Err(AdminError::InvalidInput);
    }
    let uid = authorize(pool, change.session, cid_value).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    crate::infrastructure::minimum_pet::panel_add(
        pool,
        assets,
        uid,
        pet_type,
        name,
        audit(change, "character.pet.add", cid),
    )
    .await
    .map_err(minimum)?
    .map_err(pet_refusal)
}

/// Removes a pet of the character's account (offline).
pub async fn remove_pet(
    pool: &PgPool,
    change: &Change<'_>,
    cid: &str,
    pet_id: &str,
) -> Result<(), AdminError> {
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    let pet: i64 = pet_id.parse().map_err(|_| AdminError::NotFound)?;
    let uid = authorize(pool, change.session, cid_value).await?;
    crate::infrastructure::minimum_pet::panel_remove(
        pool,
        uid,
        pet,
        audit(change, "character.pet.remove", cid),
    )
    .await
    .map_err(minimum)?
    .map_err(pet_refusal)
}
