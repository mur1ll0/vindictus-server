//! Explicit clock/permissions inputs come from a future authenticated session, not HTTP or IP trust.
use crate::{
    domain::{
        legacy_user_data::SqlDateTime2,
        login_policy::{self, IpEvaluation, IpRequest},
    },
    error::Error,
    infrastructure::login_policy::{commit_ip_state, read_ip_state},
};
use sqlx::PgPool;

pub struct PersistedIpEvaluation {
    pub evaluation: IpEvaluation,
    pub revision: i64,
}
pub async fn check_login_ip(
    pool: &PgPool,
    id: i32,
    expected_revision: i64,
    request: &IpRequest,
) -> Result<PersistedIpEvaluation, Error> {
    let stored = read_ip_state(pool, id).await?;
    if stored.revision != expected_revision {
        return Err(Error::GameUserRevisionConflict);
    }
    let old_ip = stored
        .state
        .last_login_ip
        .as_ref()
        .map(|b| zeroize::Zeroizing::new(b.as_bytes().to_vec()));
    let old_lock = stored.state.lock_until;
    let evaluation = login_policy::check_last_login_ip(stored.state, request)?;
    let changed = old_ip.as_ref().map(|b| b.as_slice())
        != evaluation
            .state
            .last_login_ip
            .as_ref()
            .map(|b| b.as_bytes())
        || old_lock != evaluation.state.lock_until;
    let revision = commit_ip_state(pool, id, expected_revision, &evaluation.state, changed).await?;
    Ok(PersistedIpEvaluation {
        evaluation,
        revision,
    })
}
pub async fn update_expired_lock(
    pool: &PgPool,
    id: i32,
    expected_revision: i64,
    connection_ip: std::net::IpAddr,
    now: SqlDateTime2,
) -> Result<i64, Error> {
    let stored = read_ip_state(pool, id).await?;
    if stored.revision != expected_revision {
        return Err(Error::GameUserRevisionConflict);
    }
    let changed = stored
        .state
        .lock_until
        .is_some_and(|t| t.ticks() <= now.ticks());
    let updated = login_policy::update_expired_ip_lock(stored.state, connection_ip, now)?;
    commit_ip_state(pool, id, expected_revision, &updated, changed).await
}
