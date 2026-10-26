//! Account administration use cases of the panel (B4.2): list, create, role, new password
//! and enable/disable. Panel policy: a GM manages game accounts; an account that has a panel
//! role (and every role change) needs an admin, so a GM cannot take over a higher account.
use super::admin_session::AdminError;
use crate::{
    domain::{
        Username,
        admin::{AuditReason, PanelRole, Permission},
        hash_password,
    },
    infrastructure::{
        admin_accounts::{self, AccountChange, AccountSummary, Actor, Target},
        admin_store::{self, RoleChange, SessionAccount},
    },
};
use sqlx::PgPool;
use std::net::IpAddr;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Longest name a playable account can have: the whole name fits the game's 24-character
/// NexonID (PassportCheck truncates longer passports).
pub const MAX_PLAYABLE_NAME: usize = 24;
pub const MAX_ACCOUNT_PAGE: i64 = 100;

/// Who asks for the change and why.
pub struct Request<'a> {
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

fn reason(value: &str) -> Result<AuditReason, AdminError> {
    AuditReason::parse(value).ok_or(AdminError::InvalidInput)
}

fn outcome(change: AccountChange) -> Result<(), AdminError> {
    match change {
        AccountChange::Done => Ok(()),
        AccountChange::NoAccount => Err(AdminError::NotFound),
        AccountChange::Exists => Err(AdminError::Exists),
        AccountChange::RepeatedOperation => Err(AdminError::RepeatedOperation),
    }
}

async fn hash(password: Zeroizing<Vec<u8>>) -> Result<String, AdminError> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| AdminError::Busy)?
        .map_err(|_| AdminError::InvalidInput)
}

/// Changing an account that has a panel role needs an admin, except one's own password.
fn may_change(session: &SessionAccount, target: &Target) -> Result<(), AdminError> {
    if target.role.is_some() && target.id != session.account_id {
        require(session, Permission::Administer)
    } else {
        require(session, Permission::Manage)
    }
}

pub async fn list(
    pool: &PgPool,
    search: Option<&str>,
    after: Option<&str>,
    limit: i64,
) -> Result<Vec<AccountSummary>, AdminError> {
    let valid = |v: &str| {
        !v.is_empty() && v.len() <= 32 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    };
    if !(1..=MAX_ACCOUNT_PAGE).contains(&limit)
        || search.is_some_and(|v| !valid(v))
        || after.is_some_and(|v| !valid(v))
    {
        return Err(AdminError::InvalidInput);
    }
    let lower = |v: Option<&str>| v.map(str::to_ascii_lowercase);
    Ok(admin_accounts::list(
        pool,
        lower(search).as_deref(),
        lower(after).as_deref(),
        limit,
    )
    .await?)
}

pub async fn create(
    pool: &PgPool,
    request: &Request<'_>,
    username: &str,
    password: Zeroizing<Vec<u8>>,
    role: Option<PanelRole>,
) -> Result<(), AdminError> {
    require(
        request.session,
        if role.is_some() {
            Permission::Administer
        } else {
            Permission::Manage
        },
    )?;
    let name = Username::parse(username).map_err(|_| AdminError::InvalidInput)?;
    if name.display().len() > MAX_PLAYABLE_NAME {
        return Err(AdminError::InvalidInput);
    }
    let reason = reason(request.reason)?;
    let hash = hash(password).await?;
    let actor = Actor {
        session: request.session,
        peer: request.peer,
        operation_id: request.operation_id,
        reason: &reason,
    };
    outcome(admin_accounts::create(pool, &name, &hash, role, &actor).await?)
}

pub async fn set_role(
    pool: &PgPool,
    request: &Request<'_>,
    username: &str,
    role: Option<PanelRole>,
) -> Result<(), AdminError> {
    require(request.session, Permission::Administer)?;
    let name = Username::parse(username).map_err(|_| AdminError::NotFound)?;
    let reason = reason(request.reason)?;
    match admin_store::set_role(
        pool,
        name.normalized(),
        role,
        &reason,
        Some((request.session, request.peer, request.operation_id)),
    )
    .await?
    {
        RoleChange::Changed { .. } => Ok(()),
        RoleChange::NoAccount => Err(AdminError::NotFound),
        RoleChange::LastAdmin => Err(AdminError::LastAdmin),
        RoleChange::RepeatedOperation => Err(AdminError::RepeatedOperation),
    }
}

pub async fn set_password(
    pool: &PgPool,
    request: &Request<'_>,
    username: &str,
    password: Zeroizing<Vec<u8>>,
) -> Result<(), AdminError> {
    require(request.session, Permission::Manage)?;
    let name = Username::parse(username).map_err(|_| AdminError::NotFound)?;
    let reason = reason(request.reason)?;
    let hash = hash(password).await?;
    let (tx, target) = admin_accounts::lock_target(pool, name.normalized()).await?;
    let target = target.ok_or(AdminError::NotFound)?;
    may_change(request.session, &target)?;
    let actor = Actor {
        session: request.session,
        peer: request.peer,
        operation_id: request.operation_id,
        reason: &reason,
    };
    outcome(admin_accounts::set_password(tx, &target, &hash, &actor).await?)
}

pub async fn set_status(
    pool: &PgPool,
    request: &Request<'_>,
    username: &str,
    active: bool,
) -> Result<(), AdminError> {
    require(request.session, Permission::Manage)?;
    let name = Username::parse(username).map_err(|_| AdminError::NotFound)?;
    let reason = reason(request.reason)?;
    let (mut tx, target) = admin_accounts::lock_target(pool, name.normalized()).await?;
    let target = target.ok_or(AdminError::NotFound)?;
    if target.id == request.session.account_id {
        return Err(AdminError::InvalidInput);
    }
    may_change(request.session, &target)?;
    if !active
        && target.role == Some(PanelRole::Admin)
        && admin_accounts::other_active_admins(&mut tx, target.id).await? == 0
    {
        return Err(AdminError::LastAdmin);
    }
    let actor = Actor {
        session: request.session,
        peer: request.peer,
        operation_id: request.operation_id,
        reason: &reason,
    };
    outcome(admin_accounts::set_status(tx, &target, active, &actor).await?)
}
