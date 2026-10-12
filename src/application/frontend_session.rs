//! Internal loopback laboratory session; not wired to the game TCP diagnostic.
//! Passing policies does not mean character services or native client login are ready.
use super::session_registry::{RegistryError, Reservation, SessionRegistry};
use crate::{
    domain::passport::{LoginPermissions, LoginRejection, frontend_passport_gate},
    protocol::identity::{
        IdentityError, IdentityProfile, LabIdentity, LabIdentityOptions, derive_lab_identity,
    },
};
use std::{
    net::IpAddr,
    time::{Duration, Instant},
};
use thiserror::Error;
use uuid::Uuid;

/// Opaque callback correlation, tied to exactly one session and attempt.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AttemptToken(Uuid);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionPhase {
    Initial,
    CheckingPassport,
    CheckingPolicies,
    PoliciesAccepted,
    Closed,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionError {
    #[error("session is not in the required phase")]
    InvalidPhase,
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error("callback does not belong to the active attempt")]
    StaleCallback,
    #[error("laboratory requires loopback peer and NGM_disable")]
    LabScope,
    #[error("laboratory attempt deadline exceeded")]
    Timeout,
    #[error("laboratory deadline is outside the bounded policy")]
    Deadline,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("passport rejected with recovered reason {0:?}")]
    Rejected(LoginRejection),
}
/// Trusted operator configuration. No wire fields are converted into permissions.
pub struct LabAdmission {
    pub identity_options: LabIdentityOptions,
    pub allow_during_maintenance: bool,
    pub allow_cheat: bool,
    pub is_online: bool,
}
pub(crate) struct PendingIdentity {
    pub identity: LabIdentity,
    pub profile: IdentityProfile,
    pub permissions: LoginPermissions,
    pub remote_ip: IpAddr,
}
/// Each connection owns one session; there is no public method to mark gameplay authenticated.
pub struct FrontendSession {
    connection_id: Uuid,
    phase: SessionPhase,
    token: Option<AttemptToken>,
    deadline: Option<Instant>,
    pending: Option<PendingIdentity>,
    admitted: Option<PendingIdentity>,
    registry: SessionRegistry,
    reservation: Option<Reservation>,
}
impl FrontendSession {
    /// Every connection must receive the runtime's shared registry; no implicit private registry.
    pub fn new(registry: &SessionRegistry) -> Self {
        Self {
            connection_id: Uuid::new_v4(),
            phase: SessionPhase::Initial,
            token: None,
            deadline: None,
            pending: None,
            admitted: None,
            registry: registry.clone(),
            reservation: None,
        }
    }
    /// Borrow only an admitted policies-accepted identity; password messages carry no owner IDs.
    pub(crate) fn accepted_identity(&self) -> Result<&LabIdentity, SessionError> {
        if self.phase != SessionPhase::PoliciesAccepted {
            return Err(SessionError::InvalidPhase);
        }
        self.admitted
            .as_ref()
            .map(|p| &p.identity)
            .ok_or(SessionError::InvalidPhase)
    }
    pub(crate) fn accepted_profile(&self) -> Result<IdentityProfile, SessionError> {
        self.accepted_identity()?;
        self.admitted
            .as_ref()
            .map(|p| p.profile)
            .ok_or(SessionError::InvalidPhase)
    }
    pub(crate) fn accepted_connection(&self) -> Result<Uuid, SessionError> {
        self.accepted_identity()?;
        Ok(self.connection_id)
    }
    pub fn phase(&self) -> SessionPhase {
        self.phase
    }
    pub fn begin(&mut self, now: Instant, timeout: Duration) -> Result<AttemptToken, SessionError> {
        if self.phase != SessionPhase::Initial {
            return Err(SessionError::InvalidPhase);
        }
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(SessionError::Deadline);
        }
        let deadline = now.checked_add(timeout).ok_or(SessionError::Deadline)?;
        let token = AttemptToken(Uuid::new_v4());
        self.token = Some(token);
        self.deadline = Some(deadline);
        self.phase = SessionPhase::CheckingPassport;
        Ok(token)
    }
    fn validate(
        &mut self,
        token: AttemptToken,
        now: Instant,
        phase: SessionPhase,
    ) -> Result<(), SessionError> {
        if self.token != Some(token) {
            return Err(SessionError::StaleCallback);
        }
        if self.phase != phase {
            return Err(SessionError::InvalidPhase);
        }
        if self.deadline.is_none_or(|deadline| now >= deadline) {
            self.close();
            return Err(SessionError::Timeout);
        }
        Ok(())
    }
    /// Raw passport is borrowed only for this call and is not stored in the session.
    pub fn admit_lab(
        &mut self,
        token: AttemptToken,
        now: Instant,
        passport: Option<&str>,
        peer: IpAddr,
        config: &LabAdmission,
    ) -> Result<(), SessionError> {
        self.validate(token, now, SessionPhase::CheckingPassport)?;
        let result = (|| {
            if !peer.is_loopback() || !config.identity_options.ngm_disable {
                return Err(SessionError::LabScope);
            }
            let identity = derive_lab_identity(passport, config.identity_options)?;
            let bits = LoginPermissions::LOGIN
                | if config.allow_during_maintenance {
                    LoginPermissions::MAINTENANCE
                } else {
                    0
                }
                | if config.allow_cheat {
                    LoginPermissions::CHEAT
                } else {
                    0
                };
            let permissions = LoginPermissions::from_recovered_bits(bits);
            frontend_passport_gate(
                permissions,
                config.is_online,
                LoginRejection::PermissionFailed,
            )
            .map_err(|e| SessionError::Rejected(e.0))?;
            Ok(PendingIdentity {
                profile: config.identity_options.profile,
                identity,
                permissions,
                remote_ip: peer,
            })
        })();
        match result {
            Ok(pending) => {
                let reservation = match self.registry.reserve(pending.identity.nexon_sn) {
                    Ok(reservation) => reservation,
                    Err(RegistryError::Duplicate) => {
                        self.reset();
                        return Err(SessionError::Rejected(LoginRejection::DuplicateLogin));
                    }
                    Err(error) => {
                        self.reset();
                        return Err(error.into());
                    }
                };
                self.reservation = Some(reservation);
                self.pending = Some(pending);
                self.phase = SessionPhase::CheckingPolicies;
                Ok(())
            }
            Err(error) => {
                self.reset();
                Err(error)
            }
        }
    }
    pub(crate) fn pending(
        &mut self,
        token: AttemptToken,
        now: Instant,
    ) -> Result<&PendingIdentity, SessionError> {
        self.validate(token, now, SessionPhase::CheckingPolicies)?;
        self.pending.as_ref().ok_or(SessionError::InvalidPhase)
    }
    pub(crate) fn accept_policies(
        &mut self,
        token: AttemptToken,
        now: Instant,
    ) -> Result<(), SessionError> {
        self.validate(token, now, SessionPhase::CheckingPolicies)?;
        self.phase = SessionPhase::PoliciesAccepted;
        self.admitted = self.pending.take();
        self.token = None;
        self.deadline = None;
        Ok(())
    }
    pub fn reject(
        &mut self,
        token: AttemptToken,
        now: Instant,
        reason: LoginRejection,
    ) -> Result<(), SessionError> {
        if self.token != Some(token) {
            return Err(SessionError::StaleCallback);
        }
        if !matches!(
            self.phase,
            SessionPhase::CheckingPassport | SessionPhase::CheckingPolicies
        ) {
            return Err(SessionError::InvalidPhase);
        }
        if self.deadline.is_none_or(|deadline| now >= deadline) {
            self.close();
            return Err(SessionError::Timeout);
        }
        self.reset();
        Err(SessionError::Rejected(reason))
    }
    pub(crate) fn attempt_deadline(&self) -> Option<Instant> {
        self.deadline
    }
    fn reset(&mut self) {
        self.phase = SessionPhase::Initial;
        self.token = None;
        self.deadline = None;
        if let Some(p) = self.pending.as_mut() {
            zeroize::Zeroize::zeroize(&mut p.identity.nexon_id);
        }
        if let Some(p) = self.admitted.as_mut() {
            zeroize::Zeroize::zeroize(&mut p.identity.nexon_id);
        }
        self.pending = None;
        self.admitted = None;
        self.reservation = None;
    }
    pub fn close(&mut self) {
        self.reset();
        self.phase = SessionPhase::Closed;
    }
}

impl Drop for FrontendSession {
    fn drop(&mut self) {
        self.close();
    }
}
