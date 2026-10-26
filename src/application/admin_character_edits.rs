//! Character changes from the panel (B4.3b): give an item (ItemClassEx with attributes) or
//! gold, remove items or gold, and credit/debit cash; move and sort the inventory (B4.4d)
//! with MoveInventoryItem/SortInventory. Item changes go through the game's own
//! planner (`minimum_give`) and need the character offline: a live session owns its inventory.
//! Panel policy: GM and admin change characters; a character of an account with a panel role
//! needs an admin, unless it is the actor's own. Every change is audited in its transaction.
use super::admin_session::AdminError;
use crate::{
    domain::admin::{AuditReason, PanelRole, Permission},
    infrastructure::{
        admin_characters,
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
    pub reason: &'a str,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ChangeResult {
    pub before: i64,
    pub after: i64,
}

fn minimum(error: MinimumError) -> AdminError {
    match error {
        MinimumError::Database(e) => AdminError::Unavailable(e),
        _ => AdminError::NotEditable,
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

fn audit<'a>(
    change: &'a Change<'a>,
    reason: &'a AuditReason,
    action: &'static str,
    target: &'a str,
) -> AuditRecord<'a> {
    AuditRecord {
        account_id: Some(change.session.account_id),
        actor_name: Some(&change.session.username),
        role: change.session.role,
        operation_id: Some(change.operation_id),
        action,
        target_kind: Some("character"),
        target_id: Some(target),
        reason: Some(reason),
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
    let reason = AuditReason::parse(change.reason).ok_or(AdminError::InvalidInput)?;
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
        audit(change, &reason, action, cid),
    )
    .await
    .map_err(minimum)?;
    match outcome {
        PanelItemOutcome::Applied { before, after } => Ok(ChangeResult { before, after }),
        PanelItemOutcome::Refused(e) => Err(AdminError::ItemRefused(e.to_string())),
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
    let reason = AuditReason::parse(change.reason).ok_or(AdminError::InvalidInput)?;
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    let item_id: i64 = item.parse().map_err(|_| AdminError::InvalidInput)?;
    authorize(pool, change.session, cid_value).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let mut record = audit(change, &reason, "character.item_move", cid);
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
    let reason = AuditReason::parse(change.reason).ok_or(AdminError::InvalidInput)?;
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    if !matches!(target, -1..=8 | 100 | 103) {
        return Err(AdminError::InvalidInput);
    }
    authorize(pool, change.session, cid_value).await?;
    let assets = assets.ok_or(AdminError::ContentUnavailable)?;
    let mut record = audit(change, &reason, "character.inventory_sort", cid);
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
    let reason = AuditReason::parse(change.reason).ok_or(AdminError::InvalidInput)?;
    let cid_value: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    if delta == 0 || delta.unsigned_abs() > MAX_COUNT as u32 {
        return Err(AdminError::InvalidInput);
    }
    let uid = authorize(pool, change.session, cid_value).await?;
    match minimum_cash_shop::panel_adjust(
        pool,
        uid,
        delta,
        audit(change, &reason, "character.cash", cid),
    )
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
    let reason = AuditReason::parse(change.reason).ok_or(AdminError::InvalidInput)?;
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
        audit(change, &reason, "character.progression", cid),
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
