//! Account-level HomeCafeBonus callback bound to the admitted frontend identity.
use super::frontend_session::{FrontendSession, SessionError};
use crate::{
    domain::feature_policy::FeaturePolicy,
    error::Error,
    infrastructure::home_cafe_bonus::{self as storage, HomeCafeError, UserKey},
};
use sqlx::PgPool;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum HomeCafeQueryError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Storage(#[from] HomeCafeError),
    #[error(transparent)]
    Policy(#[from] Error),
    #[error("home-cafe callback requires complete recovered feature declarations")]
    Profile,
    #[error("active CafeBonus requires its external login operation")]
    CafeService,
}
/// Implements only the local candidate's CafeBonus-disabled path and CID=NULL query.
/// Caller retains the previous CafeType when this fails, matching the source catch.
pub async fn query_admitted(
    frontend: &FrontendSession,
    pool: &PgPool,
    revision: i64,
    policy: &FeaturePolicy,
    current_cafe_type: u8,
) -> Result<u8, HomeCafeQueryError> {
    let id = frontend.accepted_identity()?;
    for key in ["HomeCafeBonus", "CafeBonus"] {
        if !policy.entries().any(|(k, _)| k == key) {
            return Err(HomeCafeQueryError::Profile);
        }
    }
    if policy.is_enabled(Some("CafeBonus"))? {
        return Err(HomeCafeQueryError::CafeService);
    }
    if !policy.is_enabled(Some("HomeCafeBonus"))? || current_cafe_type == 1 {
        return Ok(current_cafe_type);
    }
    Ok(storage::query_account(
        pool,
        UserKey {
            uid: id.nexon_sn,
            name: &id.nexon_id,
            revision,
        },
        Some(0),
    )
    .await?
    .ok_or(HomeCafeError::Export)?)
}
