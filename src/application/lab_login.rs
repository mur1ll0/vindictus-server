//! Bounded internal adapter for existing laboratory users; no account auto-creation.
//! Legacy dormancy/rewards and character services remain unimplemented.
use super::{
    frontend_session::{AttemptToken, FrontendSession, SessionError},
    login_policy::check_login_ip,
};
use crate::{
    domain::{
        game_user::LegacyNameKey,
        legacy_user_data::SqlDateTime,
        login_policy::{ApplicationClockSamples, IpRequest, MemberState, member_state},
        passport::LoginRejection,
    },
    error::Error,
    infrastructure::login_policy::get_user_ban_info,
};
use sqlx::PgPool;
use std::time::Instant;
use thiserror::Error;

pub struct PolicyClock {
    pub application: ApplicationClockSamples,
    pub sql_local_now: SqlDateTime,
}
#[derive(Debug, Error)]
pub enum LabPolicyError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("laboratory storage policy failed")]
    Storage(#[source] Error),
    #[error("laboratory policy deadline exceeded; reread persisted revision before retry")]
    Timeout,
}
/// No native LoginSuccess is emitted from this result.
pub struct AcceptedPolicies {
    pub revision: i64,
    pub client_lock_seconds: i32,
    /// Source LimitedAccount result expressed as its returned 0/1 integer.
    pub limited: i32,
}

/// Existing user must match both Framework SN and SQL-compatible name key.
/// Ban is checked before IP mutation, matching the recovered UserLoginProcessor.
/// Ban changes between statements are not made atomic by this adapter (same legacy limitation).
pub async fn evaluate_existing_lab_user(
    session: &mut FrontendSession,
    token: AttemptToken,
    pool: &PgPool,
    expected_revision: i64,
    clocks: PolicyClock,
    lock_minutes_feature: i32,
) -> Result<AcceptedPolicies, LabPolicyError> {
    session.pending(token, Instant::now())?;
    let mut guard = PolicyGuard {
        session,
        completed: false,
    };
    let pending = guard.session.pending(token, Instant::now())?;
    let id = pending.identity.nexon_sn;
    let pending_name = pending.identity.nexon_id.clone();
    let name_key = LegacyNameKey::parse(&pending.identity.nexon_id)
        .map_err(|e| LabPolicyError::Storage(e.into()))?;
    let request = IpRequest {
        connection_ip: pending.remote_ip,
        secured: false, // NGM_disable UserLogin never sets IsSecuredLogin; OTP is NOT a client flag.
        lock_minutes_feature,
        has_cheat_permission: pending.permissions.has_cheat(),
        clock: clocks.application,
    };
    let deadline = guard
        .session
        .attempt_deadline()
        .ok_or(SessionError::InvalidPhase)?;
    let evaluation = tokio::time::timeout_at(deadline.into(), async {
        let matches: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM game_users WHERE id=$1 AND name_key=$2)",
        )
        .bind(id)
        .bind(name_key.as_str())
        .fetch_one(pool)
        .await
        .map_err(Error::from)?;
        if !matches {
            return Ok(Err(LoginRejection::NoUserInfo));
        }
        let ban = get_user_ban_info(pool, id, clocks.sql_local_now).await?;
        match member_state(Ok(ban.as_ref()), false) {
            MemberState::Banned(_) => return Ok(Err(LoginRejection::Banned)),
            MemberState::Unknown => return Ok(Err(LoginRejection::NoUserInfo)),
            MemberState::Clear => {}
        }
        let checked = check_login_ip(pool, id, expected_revision, &request).await?;
        let limited =
            crate::infrastructure::limited_accounts::is_limited(pool, &pending_name).await?;
        Ok::<_, Error>(Ok(AcceptedPolicies {
            revision: checked.revision,
            client_lock_seconds: checked.evaluation.client_lock_seconds,
            limited: i32::from(limited),
        }))
    })
    .await;
    match evaluation {
        Ok(Ok(Ok(accepted))) => {
            guard.session.accept_policies(token, Instant::now())?;
            guard.completed = true;
            Ok(accepted)
        }
        Ok(Ok(Err(reason))) => {
            let rejection = guard.session.reject(token, Instant::now(), reason);
            guard.completed = true;
            rejection?;
            Err(SessionError::InvalidPhase.into())
        }
        Ok(Err(error)) => {
            // Unknown DB state never becomes Clear. Drop any pending identity before returning.
            let _ = guard
                .session
                .reject(token, Instant::now(), LoginRejection::NoUserInfo);
            guard.completed = true;
            Err(LabPolicyError::Storage(error))
        }
        Err(_) => {
            guard.session.close();
            guard.completed = true;
            Err(LabPolicyError::Timeout)
        }
    }
}

/// Synchronous Drop covers caller cancellation/abort without spawning cleanup work.
struct PolicyGuard<'a> {
    session: &'a mut FrontendSession,
    completed: bool,
}
impl Drop for PolicyGuard<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.session.close();
        }
    }
}
