//! Bind the recovered internal UserLogin to an admitted loopback session.
//! Success retains the shared reservation but emits no client messages.
use super::{
    frontend_session::{AttemptToken, FrontendSession, SessionError},
    user_login::{UserLoginError, UserLoginOutcome, UserLoginRequest, execute_local_user_login},
};
use crate::{
    domain::{
        game_user::NewGameUser,
        legacy_user_data::SqlDateTime,
        login_effects::EffectClockSamples,
        login_policy::{ApplicationClockSamples, IpRequest},
        user_login_composition::TokenProfile,
    },
    error::Error,
    infrastructure::reward_policy::RewardPolicyPackage,
};
use sqlx::PgPool;
use std::time::Instant;
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Trusted operation metadata and independent source civil clock samples.
pub struct LocalLoginConfig<'a> {
    pub content: &'a RewardPolicyPackage,
    pub service_id: i32,
    pub channel_code: u8,
    pub effects_clock: EffectClockSamples,
    pub loyalty_insert_now: SqlDateTime,
    pub ban_now: SqlDateTime,
    pub application_clock: ApplicationClockSamples,
    pub token_profile: TokenProfile,
}
#[derive(Debug, Error)]
pub enum LocalLoginError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Login(#[from] UserLoginError),
    #[error(transparent)]
    Domain(#[from] Error),
}
/// Correlation/absolute deadline are checked before touching storage and again at completion.
/// Failure, timeout, storage error and cancellation release this session's reservation.
pub async fn evaluate_local_lab_user(
    session: &mut FrontendSession,
    token: AttemptToken,
    pool: &PgPool,
    config: &LocalLoginConfig<'_>,
    reward_clock: impl FnMut() -> Result<SqlDateTime, Error>,
) -> Result<UserLoginOutcome, LocalLoginError> {
    evaluate_lab_user_inner(session, token, pool, config, reward_clock, None).await
}
/// Runtime composition with an operator-owned provenance scope. The network runtime
/// must use this path and a shared session registry; no field from a client supplies scope.
pub async fn evaluate_scoped_lab_user(
    session: &mut FrontendSession,
    token: AttemptToken,
    pool: &PgPool,
    config: &LocalLoginConfig<'_>,
    reward_clock: impl FnMut() -> Result<SqlDateTime, Error>,
    scope: &crate::domain::lab_account::LabAccountScope,
) -> Result<UserLoginOutcome, LocalLoginError> {
    evaluate_lab_user_inner(session, token, pool, config, reward_clock, Some(scope)).await
}
async fn evaluate_lab_user_inner(
    session: &mut FrontendSession,
    token: AttemptToken,
    pool: &PgPool,
    config: &LocalLoginConfig<'_>,
    reward_clock: impl FnMut() -> Result<SqlDateTime, Error>,
    scope: Option<&crate::domain::lab_account::LabAccountScope>,
) -> Result<UserLoginOutcome, LocalLoginError> {
    session.pending(token, Instant::now())?;
    let mut guard = LoginGuard {
        session,
        completed: false,
    };
    let pending = guard.session.pending(token, Instant::now())?;
    let name = Zeroizing::new(pending.identity.nexon_id.clone());
    let request = UserLoginRequest {
        user: NewGameUser::new(
            pending.identity.nexon_sn,
            &name,
            config.service_id,
            config.channel_code,
        )
        .map_err(Error::from)?,
        operation_id: Uuid::new_v4(),
        content: config.content,
        clocks: config.effects_clock,
        loyalty_insert_now: config.loyalty_insert_now,
        ban_now: config.ban_now,
        ip: IpRequest {
            connection_ip: pending.remote_ip,
            secured: false,
            has_cheat_permission: pending.permissions.has_cheat(),
            lock_minutes_feature: config.content.policy.get_integer("IPLockTime")?,
            clock: config.application_clock,
        },
        token_profile: config.token_profile,
    };
    let deadline = guard
        .session
        .attempt_deadline()
        .ok_or(SessionError::InvalidPhase)?;
    let outcome = tokio::time::timeout_at(deadline.into(), async {
        match scope {
            Some(scope) => {
                super::user_login::execute_scoped_lab_user_login(
                    pool,
                    &request,
                    reward_clock,
                    scope,
                )
                .await
            }
            None => execute_local_user_login(pool, &request, reward_clock).await,
        }
    })
    .await
    .map_err(|_| SessionError::Timeout)??;
    match &outcome {
        UserLoginOutcome::Success { .. } => guard.session.accept_policies(token, Instant::now())?,
        UserLoginOutcome::Failure { .. } => guard.session.close(),
    }
    guard.completed = true;
    Ok(outcome)
}
struct LoginGuard<'a> {
    session: &'a mut FrontendSession,
    completed: bool,
}
impl Drop for LoginGuard<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.session.close();
        }
    }
}
