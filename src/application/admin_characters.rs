//! Character views of the panel (B4.3a), read-only. Staff with a role read any account's
//! characters; an account without a role reads only the characters of its own game user. A
//! character the session may not read answers like a missing one.
use super::admin_session::AdminError;
use crate::{
    domain::{Username, admin::Permission},
    infrastructure::{
        admin_characters::{self, CharacterSheet, CharacterSummary, EnglishNames},
        admin_store::SessionAccount,
        game_links,
    },
};
use sqlx::PgPool;

fn staff(session: &SessionAccount) -> bool {
    session.role.is_some_and(|r| r.allows(Permission::Read))
}

/// Characters of an account (staff).
pub async fn of_account(
    pool: &PgPool,
    session: &SessionAccount,
    username: &str,
) -> Result<Vec<CharacterSummary>, AdminError> {
    if !staff(session) {
        return Err(AdminError::Forbidden);
    }
    let name = Username::parse(username).map_err(|_| AdminError::NotFound)?;
    let key = game_links::account_key_by_name(pool, name.normalized())
        .await?
        .ok_or(AdminError::NotFound)?;
    Ok(admin_characters::list(pool, &key).await?)
}

/// Characters of the session's own account.
pub async fn mine(
    pool: &PgPool,
    session: &SessionAccount,
) -> Result<Vec<CharacterSummary>, AdminError> {
    let key = game_links::account_key(pool, session.account_id)
        .await?
        .ok_or(AdminError::Unauthenticated)?;
    Ok(admin_characters::list(pool, &key).await?)
}

pub async fn sheet(
    pool: &PgPool,
    names: &EnglishNames,
    session: &SessionAccount,
    cid: &str,
) -> Result<CharacterSheet, AdminError> {
    let cid: i64 = cid.parse().map_err(|_| AdminError::NotFound)?;
    if !staff(session) {
        let owner = admin_characters::owner_key(pool, cid).await?;
        let own = game_links::account_key(pool, session.account_id).await?;
        if owner.is_none() || owner != own {
            return Err(AdminError::NotFound);
        }
    }
    admin_characters::sheet(pool, names, cid)
        .await?
        .ok_or(AdminError::NotFound)
}
