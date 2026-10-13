//! QueryReservedInfo uses the admitted account, never a wire-supplied owner.
use super::frontend_session::{FrontendSession, SessionError};
use crate::infrastructure::{
    character_lobby::UserKey,
    character_reservations::{self as storage, ReservationError, ReservedInfo},
};
use sqlx::PgPool;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum QueryReservedError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Storage(#[from] ReservationError),
}
pub async fn query_admitted(
    frontend: &FrontendSession,
    pool: &PgPool,
    revision: i64,
) -> Result<ReservedInfo, QueryReservedError> {
    let id = frontend.accepted_identity()?;
    Ok(storage::query(
        pool,
        UserKey {
            uid: id.nexon_sn,
            name: &id.nexon_id,
            revision,
        },
    )
    .await?)
}
