//! Name check ownership comes exclusively from the admitted frontend session.
use super::frontend_session::{FrontendSession, SessionError};
use crate::infrastructure::{
    character_lobby::UserKey,
    character_name_check::{self as storage, NameCheckError, NameCheckResult},
    character_name_proof::NameProof,
};
use sqlx::PgPool;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum AdmittedNameCheckError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Storage(#[from] NameCheckError),
}
pub async fn check_admitted(
    frontend: &FrontendSession,
    pool: &PgPool,
    revision: i64,
    proof: &NameProof,
    name: Option<&str>,
) -> Result<NameCheckResult, AdmittedNameCheckError> {
    let id = frontend.accepted_identity()?;
    Ok(storage::check(
        pool,
        UserKey {
            uid: id.nexon_sn,
            name: &id.nexon_id,
            revision,
        },
        proof,
        name,
    )
    .await?)
}
