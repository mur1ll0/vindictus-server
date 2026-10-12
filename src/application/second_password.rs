//! Internal second-password session, tied to the frontend reservation's identity.
//! No network messages are emitted by this adapter.
use super::frontend_session::{FrontendSession, SessionError};
use crate::{
    domain::{
        feature_policy::FeaturePolicy,
        second_password::{
            CheckResult, ClientState, ClockSamples, HashProfile, QueryResult, SecondPasswordError,
        },
    },
    error::Error,
    infrastructure::second_password::{self as storage, StorageError, UserKey},
};
use sqlx::PgPool;
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum PasswordSessionError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Policy(#[from] Error),
    #[error(transparent)]
    Domain(#[from] SecondPasswordError),
    #[error("second-password policy is incomplete or disabled")]
    Profile,
    #[error("second-password session is not in the required phase")]
    State,
    #[error("active second-password IP limit requires its recovered procedure")]
    IpLimit,
}
pub struct PasswordConfig {
    identity: crate::protocol::identity::IdentityProfile,
    profile: HashProfile,
    essential: bool,
    clear: bool,
    ip_limit: bool,
}
impl PasswordConfig {
    pub fn recovered(
        policy: &FeaturePolicy,
        identity: crate::protocol::identity::IdentityProfile,
    ) -> Result<Self, PasswordSessionError> {
        for key in [
            "SecondPassword",
            "EssentialSecondPassword",
            "SecondPasswordClear",
            "SecondPassword_UsingSHA",
            "SecondPassword_IP_Limit",
        ] {
            if !policy.entries().any(|(k, _)| k == key) {
                return Err(PasswordSessionError::Profile);
            }
        }
        if !policy.is_enabled(Some("SecondPassword"))? {
            return Err(PasswordSessionError::Profile);
        }
        Ok(Self {
            identity,
            profile: match policy.get_integer("SecondPassword_UsingSHA")? {
                0 => HashProfile::Legacy(identity),
                v => HashProfile::try_from(v)?,
            },
            essential: policy.is_enabled(Some("EssentialSecondPassword"))?,
            clear: policy.is_enabled(Some("SecondPasswordClear"))?,
            ip_limit: policy.is_enabled(Some("SecondPassword_IP_Limit"))?,
        })
    }
}
pub struct PasswordSession {
    connection_id: uuid::Uuid,
    uid: i32,
    name: Zeroizing<String>,
    revision: i64,
    state: ClientState,
    restricted_until: Option<crate::domain::legacy_user_data::SqlDateTime2>,
    config: PasswordConfig,
}
impl PasswordSession {
    pub fn new(
        frontend: &FrontendSession,
        revision: i64,
        config: PasswordConfig,
    ) -> Result<Self, PasswordSessionError> {
        let id = frontend.accepted_identity()?;
        if frontend.accepted_profile()? != config.identity {
            return Err(PasswordSessionError::Profile);
        }
        Ok(Self {
            connection_id: frontend.accepted_connection()?,
            uid: id.nexon_sn,
            name: Zeroizing::new(id.nexon_id.clone()),
            revision,
            state: ClientState::Unknown,
            restricted_until: None,
            config,
        })
    }
    fn owner(&self, frontend: &FrontendSession) -> Result<(), PasswordSessionError> {
        let id = frontend.accepted_identity()?;
        if frontend.accepted_connection()? != self.connection_id
            || id.nexon_sn != self.uid
            || id.nexon_id != *self.name
        {
            return Err(PasswordSessionError::State);
        }
        Ok(())
    }
    fn allowed(&self, clock: ClockSamples) -> Result<(), PasswordSessionError> {
        if self
            .restricted_until
            .is_some_and(|until| until.ticks() > clock.operation_now.ticks())
        {
            return Err(SecondPasswordError::Locked.into());
        }
        Ok(())
    }
    fn key(&self) -> UserKey<'_> {
        UserKey {
            uid: self.uid,
            name: &self.name,
            revision: self.revision,
        }
    }
    pub fn state(&self) -> ClientState {
        self.state
    }
    pub fn revision(&self) -> i64 {
        self.revision
    }
    pub fn selection_allowed(&self, frontend: &FrontendSession) -> bool {
        self.owner(frontend).is_ok()
            && self.state.passed(true)
            && self.state.essential_passed(true, self.config.essential)
    }
    pub async fn query(
        &mut self,
        frontend: &FrontendSession,
        pool: &PgPool,
        clock: ClockSamples,
    ) -> Result<QueryResult, PasswordSessionError> {
        self.owner(frontend)?;
        if self.state.passed(true) {
            return Ok(QueryResult {
                restricted_until: self.restricted_until,
                first: false,
                has_password: self.state == ClientState::Passed,
                fail_count: 0,
                retry_seconds: -1,
            });
        }
        self.state = ClientState::Unknown;
        let (result, revision) = storage::query(pool, &self.key(), clock).await?;
        self.revision = revision;
        self.restricted_until = result.restricted_until;
        self.state = if result.has_password {
            ClientState::Locked
        } else {
            ClientState::NoPassword
        };
        Ok(result)
    }
    pub async fn check(
        &mut self,
        frontend: &FrontendSession,
        pool: &PgPool,
        password: &[u16],
        clock: ClockSamples,
    ) -> Result<CheckResult, PasswordSessionError> {
        self.owner(frontend)?;
        if self.state == ClientState::Unknown {
            return Err(PasswordSessionError::State);
        }
        self.allowed(clock)?;
        self.state = ClientState::Locked;
        let (result, revision) = storage::check(
            pool,
            &self.key(),
            password,
            self.config.profile,
            self.config.identity,
            clock,
        )
        .await?;
        self.revision = revision;
        self.restricted_until =
            crate::domain::second_password::retry_until(result.fail_count, clock.operation_now)?;
        if result.passed {
            self.state = ClientState::Passed
        };
        Ok(result)
    }
    pub async fn change(
        &mut self,
        frontend: &FrontendSession,
        pool: &PgPool,
        password: &[u16],
        clock: ClockSamples,
    ) -> Result<(), PasswordSessionError> {
        self.owner(frontend)?;
        if !self.state.passed(true) || self.state == ClientState::Unknown {
            return Err(PasswordSessionError::State);
        }
        self.allowed(clock)?;
        if password.is_empty() && !self.config.clear {
            return Err(PasswordSessionError::State);
        }
        if !password.is_empty() && self.config.ip_limit {
            return Err(PasswordSessionError::IpLimit);
        }
        self.state = ClientState::Locked;
        self.revision =
            storage::change(pool, &self.key(), password, self.config.profile, clock).await?;
        self.restricted_until = None;
        self.state = if password.is_empty() {
            ClientState::NoPassword
        } else {
            ClientState::Passed
        };
        Ok(())
    }
}
