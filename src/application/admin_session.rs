//! Administration panel login, sessions and audit queries (B4.1).
//! Panel policy (limits, lockout, session lifetime), not a recovered game rule. The panel login
//! is the game account's login and password; the in-game second password is never asked here.
use crate::{
    domain::{
        Username,
        admin::{AuditReason, PanelRole, SessionToken},
        hash_password, verify_password,
    },
    infrastructure::admin_store::{
        self, AuditEntry, AuditFilter, AuditRecord, RoleChange, SessionAccount,
    },
};
use sqlx::PgPool;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::{OnceCell, Semaphore};
use zeroize::Zeroizing;

pub const SESSION_SECONDS: u64 = 8 * 60 * 60;
pub const IDLE_SECONDS: u64 = 30 * 60;
/// Consecutive wrong passwords before the panel login of an account is locked.
pub const MAX_FAILURES: i32 = 10;
pub const LOCK_SECONDS: u64 = 15 * 60;
const WINDOW: Duration = Duration::from_secs(60);
const PER_PEER: u32 = 20;
const PER_NAME: u32 = 10;
/// Bound on remembered throttle keys; beyond it new keys are refused until the window passes.
const MAX_KEYS: usize = 4096;
/// Concurrent Argon2 verifications (memory-hard: about 19 MiB each).
const VERIFIERS: usize = 2;
pub const MAX_AUDIT_PAGE: i64 = 200;

#[derive(Debug, Error)]
pub enum AdminError {
    /// Wrong name/password, no panel role or locked: one answer, so a probe learns nothing.
    #[error("invalid login or no panel access")]
    InvalidCredentials,
    #[error("too many login attempts")]
    RateLimited,
    #[error("authentication busy")]
    Busy,
    #[error("session ended")]
    Unauthenticated,
    #[error("role does not allow this action")]
    Forbidden,
    #[error("invalid input")]
    InvalidInput,
    #[error("account not found")]
    NotFound,
    #[error("the last admin cannot lose the admin role")]
    LastAdmin,
    #[error("panel storage unavailable")]
    Unavailable(#[from] sqlx::Error),
}

/// Fixed-window attempt counters per peer and per account name, in process memory.
#[derive(Default)]
struct Throttle(Mutex<HashMap<String, (Instant, u32)>>);

impl Throttle {
    fn admit(&self, key: String, limit: u32, now: Instant) -> bool {
        let mut map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= MAX_KEYS {
            map.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
            if map.len() >= MAX_KEYS && !map.contains_key(&key) {
                return false;
            }
        }
        let entry = map.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0) >= WINDOW {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= limit
    }
}

pub struct LoginGranted {
    pub token: SessionToken,
    pub session: SessionAccount,
}

#[derive(Clone)]
pub struct PanelAuth {
    pool: PgPool,
    throttle: Arc<Throttle>,
    verifiers: Arc<Semaphore>,
    /// Hash checked when the name is unknown, so unknown names cost the same time.
    dummy: Arc<OnceCell<String>>,
}

impl PanelAuth {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            throttle: Arc::default(),
            verifiers: Arc::new(Semaphore::new(VERIFIERS)),
            dummy: Arc::default(),
        }
    }
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    async fn verify(
        &self,
        password: Zeroizing<Vec<u8>>,
        hash: Option<String>,
    ) -> Result<bool, AdminError> {
        let _permit = self
            .verifiers
            .clone()
            .try_acquire_owned()
            .map_err(|_| AdminError::Busy)?;
        let hash = match hash {
            Some(hash) => hash,
            None => self
                .dummy
                .get_or_try_init(|| async {
                    tokio::task::spawn_blocking(|| hash_password(b"panel-timing-equalizer"))
                        .await
                        .map_err(|_| AdminError::Busy)?
                        .map_err(|_| AdminError::Busy)
                })
                .await?
                .clone(),
        };
        tokio::task::spawn_blocking(move || verify_password(&password, &hash))
            .await
            .map_err(|_| AdminError::Busy)
    }

    pub async fn login(
        &self,
        username: &str,
        password: Zeroizing<Vec<u8>>,
        peer: Option<IpAddr>,
    ) -> Result<LoginGranted, AdminError> {
        let now = Instant::now();
        let name = Username::parse(username).ok();
        let peer_key = format!(
            "peer:{}",
            peer.map_or_else(|| "-".into(), |p| p.to_string())
        );
        let name_key = format!("name:{}", name.as_ref().map_or("-", Username::normalized));
        if !self.throttle.admit(peer_key, PER_PEER, now)
            || !self.throttle.admit(name_key, PER_NAME, now)
        {
            return Err(AdminError::RateLimited);
        }
        if password.len() > 128 {
            return Err(AdminError::InvalidCredentials);
        }
        let account = match &name {
            Some(name) => admin_store::login_account(&self.pool, name.normalized()).await?,
            None => None,
        };
        let matched = self
            .verify(password, account.as_ref().map(|a| a.password_hash.clone()))
            .await?;
        let refused = |account_id, actor_name, role| AuditRecord {
            account_id,
            actor_name,
            role,
            operation_id: None,
            action: "session.login",
            target_kind: None,
            target_id: None,
            reason: None,
            before: None,
            after: None,
            refused: true,
            peer,
        };
        let Some(account) = account else {
            admin_store::record_failure(
                &self.pool,
                None,
                &refused(None, None, None),
                MAX_FAILURES,
                LOCK_SECONDS as f64,
            )
            .await?;
            return Err(AdminError::InvalidCredentials);
        };
        let audit_name = Some(account.username.as_str());
        match (matched, account.role, account.locked) {
            (true, Some(role), false) => {
                let token = SessionToken::generate();
                admin_store::open_session(
                    &self.pool,
                    account.id,
                    &token.digest(),
                    SESSION_SECONDS as f64,
                    IDLE_SECONDS as f64,
                    &AuditRecord {
                        refused: false,
                        ..refused(Some(account.id), audit_name, Some(role))
                    },
                )
                .await?;
                Ok(LoginGranted {
                    token,
                    session: SessionAccount {
                        account_id: account.id,
                        username: account.username.clone(),
                        role,
                    },
                })
            }
            // A right password of an account without a role or while locked is not a failure.
            (true, role, _) => {
                admin_store::record_failure(
                    &self.pool,
                    None,
                    &refused(Some(account.id), audit_name, role),
                    MAX_FAILURES,
                    LOCK_SECONDS as f64,
                )
                .await?;
                Err(AdminError::InvalidCredentials)
            }
            (false, role, _) => {
                admin_store::record_failure(
                    &self.pool,
                    Some(account.id),
                    &refused(Some(account.id), audit_name, role),
                    MAX_FAILURES,
                    LOCK_SECONDS as f64,
                )
                .await?;
                Err(AdminError::InvalidCredentials)
            }
        }
    }

    pub async fn authorize(&self, token: &SessionToken) -> Result<SessionAccount, AdminError> {
        admin_store::touch_session(&self.pool, &token.digest(), IDLE_SECONDS as f64)
            .await?
            .ok_or(AdminError::Unauthenticated)
    }

    pub async fn logout(
        &self,
        token: &SessionToken,
        session: &SessionAccount,
        peer: Option<IpAddr>,
    ) -> Result<(), AdminError> {
        admin_store::close_session(
            &self.pool,
            &token.digest(),
            &AuditRecord {
                account_id: Some(session.account_id),
                actor_name: Some(&session.username),
                role: Some(session.role),
                operation_id: None,
                action: "session.logout",
                target_kind: None,
                target_id: None,
                reason: None,
                before: None,
                after: None,
                refused: false,
                peer,
            },
        )
        .await?;
        Ok(())
    }
}

pub async fn audit_page(
    pool: &PgPool,
    mut filter: AuditFilter,
) -> Result<Vec<AuditEntry>, AdminError> {
    if !(1..=MAX_AUDIT_PAGE).contains(&filter.limit)
        || filter.before_id.is_some_and(|id| id < 1)
        || [&filter.action, &filter.target_kind, &filter.target_id]
            .iter()
            .any(|v| v.as_ref().is_some_and(|v| v.is_empty() || v.len() > 128))
    {
        return Err(AdminError::InvalidInput);
    }
    filter.limit = filter.limit.min(MAX_AUDIT_PAGE);
    Ok(admin_store::audit_page(pool, &filter).await?)
}

/// Operator CLI (no panel session): give, change or remove the panel role of an account.
pub async fn set_role_from_cli(
    pool: &PgPool,
    username: &str,
    role: Option<PanelRole>,
    reason: &str,
) -> Result<RoleChange, AdminError> {
    let name = Username::parse(username).map_err(|_| AdminError::InvalidInput)?;
    let reason = AuditReason::parse(reason).ok_or(AdminError::InvalidInput)?;
    match admin_store::set_role(pool, name.normalized(), role, &reason, None).await? {
        RoleChange::NoAccount => Err(AdminError::NotFound),
        RoleChange::LastAdmin => Err(AdminError::LastAdmin),
        change => Ok(change),
    }
}
