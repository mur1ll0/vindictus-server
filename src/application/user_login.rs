//! Recovered local UserLogin sequence for an already admitted laboratory identity.
//! Caller must retain the session reservation; this is not exposed over HTTP/TCP.
use crate::{
    application::{
        login_policy::check_login_ip,
        login_rewards::{LoginRewardRequest, apply_existing_login_rewards},
    },
    domain::{
        game_user::NewGameUser,
        legacy_user_data::SqlDateTime,
        login_effects::EffectClockSamples,
        login_policy::{BanReason, IpRequest, MemberState, member_state},
        passport::LoginRejection,
        user_login_composition::{
            CompositionError, LoginSuccess, TokenProfile, compose_user_login,
        },
    },
    error::Error,
    infrastructure::{
        limited_accounts,
        login_policy::get_user_ban_info,
        reward_policy::RewardPolicyPackage,
        user_login::{prepare_user, read_token},
    },
};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct UserLoginRequest<'a> {
    pub user: NewGameUser<'a>,
    pub operation_id: Uuid,
    pub content: &'a RewardPolicyPackage,
    pub clocks: EffectClockSamples,
    pub loyalty_insert_now: SqlDateTime,
    pub ban_now: SqlDateTime,
    pub ip: IpRequest,
    pub token_profile: TokenProfile,
}
pub enum UserLoginOutcome {
    Success {
        feedback: LoginSuccess,
        created: bool,
        revision: i64,
    },
    Failure {
        reason: LoginRejection,
        text: Zeroizing<Vec<u16>>,
    },
}
#[derive(Debug, Error)]
pub enum UserLoginError {
    #[error(transparent)]
    Storage(#[from] Error),
    #[error(transparent)]
    Composition(#[from] CompositionError),
    #[error(
        "PassportImmigrator requires its recovered external service; local adapter refuses this profile"
    )]
    PassportImmigrator,
    #[error(
        "local UserLogin requires an explicit NGM_disable profile and all authentication policy declarations"
    )]
    LocalProfile,
    #[error("internal UserLogin deadline exceeded; committed effects must be reread before retry")]
    Deadline,
}
/// Effects commit before ban/IP, preserving the recovered boundary. A banned login
/// does not undo earlier effects. Source's swallowed IP persistence error is deliberately
/// fail-closed here. A periodic CheckBanType task and TCP are separate pending dependencies.
pub async fn execute_local_user_login(
    pool: &PgPool,
    request: &UserLoginRequest<'_>,
    reward_clock: impl FnMut() -> Result<SqlDateTime, Error>,
) -> Result<UserLoginOutcome, UserLoginError> {
    execute_user_login_inner(pool, request, reward_clock, None).await
}
/// Laboratory runtime entry: imported or unmarked rows cannot reach reward/ban/IP writes.
pub async fn execute_scoped_lab_user_login(
    pool: &PgPool,
    request: &UserLoginRequest<'_>,
    reward_clock: impl FnMut() -> Result<SqlDateTime, Error>,
    scope: &crate::domain::lab_account::LabAccountScope,
) -> Result<UserLoginOutcome, UserLoginError> {
    execute_user_login_inner(pool, request, reward_clock, Some(scope)).await
}
async fn execute_user_login_inner(
    pool: &PgPool,
    request: &UserLoginRequest<'_>,
    mut reward_clock: impl FnMut() -> Result<SqlDateTime, Error>,
    scope: Option<&crate::domain::lab_account::LabAccountScope>,
) -> Result<UserLoginOutcome, UserLoginError> {
    let policy = &request.content.policy;
    for key in [
        "NGM_disable",
        "PassportImmigrator",
        "NotifyBanReason",
        "IPLockTime",
    ] {
        if !policy.entries().any(|(k, _)| k == key) {
            return Err(UserLoginError::LocalProfile);
        }
    }
    if !policy.is_enabled(Some("NGM_disable"))? || request.ip.secured {
        return Err(UserLoginError::LocalProfile);
    }
    if request
        .content
        .policy
        .is_enabled(Some("PassportImmigrator"))?
    {
        return Err(UserLoginError::PassportImmigrator);
    }
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let user = &request.user;
        let friend = request
            .content
            .policy
            .is_enabled(Some("friend_loyalty_system"))?;
        let prepared = match scope {
            Some(scope) => {
                crate::infrastructure::user_login::prepare_scoped_lab_user(
                    pool,
                    user,
                    friend,
                    request.loyalty_insert_now,
                    scope,
                )
                .await?
            }
            None => prepare_user(pool, user, friend, request.loyalty_insert_now).await?,
        };
        let effects = apply_existing_login_rewards(
            pool,
            &LoginRewardRequest {
                operation_id: request.operation_id,
                uid: user.id(),
                name: user.name(),
                expected_revision: prepared.revision,
                clocks: request.clocks,
                loyalty_insert_now: request.loyalty_insert_now,
                content: request.content,
            },
            &mut reward_clock,
        )
        .await?;
        // No ban failure becomes Clear. Unknown storage failures return a typed error.
        let ban = get_user_ban_info(pool, user.id(), request.ban_now).await?;
        match member_state(
            Ok(ban.as_ref()),
            request.content.policy.is_enabled(Some("NotifyBanReason"))?,
        ) {
            MemberState::Unknown => {
                return Ok(UserLoginOutcome::Failure {
                    reason: LoginRejection::NoUserInfo,
                    text: Zeroizing::new(Vec::new()),
                });
            }
            MemberState::Banned(reason) => {
                let text = match reason {
                    BanReason::Empty => Zeroizing::new(Vec::new()),
                    BanReason::AccountStolen => Zeroizing::new(
                        "GameUI_Login_Fail_BannedReason_AccountStolen"
                            .encode_utf16()
                            .collect(),
                    ),
                    BanReason::Source(bytes) => Zeroizing::new(
                        bytes
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| u16::from_le_bytes([b[0], b[1]]))
                            .collect(),
                    ),
                };
                return Ok(UserLoginOutcome::Failure {
                    reason: LoginRejection::Banned,
                    text,
                });
            }
            MemberState::Clear => {}
        }
        let ip = check_login_ip(pool, user.id(), effects.revision, &request.ip).await?;
        let token = read_token(pool, user.id(), ip.revision).await?;
        let limited = limited_accounts::is_limited(pool, user.name()).await?;
        let feedback = compose_user_login(
            ip.evaluation.previous_ip.as_ref().map(|b| b.as_bytes()),
            ip.evaluation.lock_seconds,
            limited,
            effects.flags,
            token.as_ref().map(|b| b.as_bytes()),
            request.token_profile,
        )?;
        Ok::<_, UserLoginError>(UserLoginOutcome::Success {
            feedback,
            created: prepared.created,
            revision: ip.revision,
        })
    })
    .await
    .map_err(|_| UserLoginError::Deadline)?
}
