//! Character changes from the panel (B4.3b): give an item (ItemClassEx with attributes) or
//! gold, remove items or gold, and credit/debit cash. Item changes go through the game's own
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
