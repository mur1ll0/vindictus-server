//! Administration panel login, registration, sessions and audit queries (B4.1, B4.2).
//! Panel policy (limits, lockout, session lifetime), not a recovered game rule. The panel login
//! is the game account's login and password; the in-game second password is never asked here.
//! Every active account logs in: without a panel role it only sees its own account (operator
//! decision 2026-10-26), with a role it administers.
use crate::{
    domain::{
        Username,
        admin::{PanelRole, SessionToken},
        hash_password, verify_password,
    },
    infrastructure::{
        admin_accounts::{self, AccountSummary, Actor, Registration},
        admin_store::{self, AuditEntry, AuditFilter, AuditRecord, RoleChange, SessionAccount},
        game_links,
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
/// New accounts per peer and window on the open registration of the login page.
const REGISTRATIONS_PER_PEER: u32 = 3;
const REGISTRATION_WINDOW: Duration = Duration::from_secs(10 * 60);

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
    #[error("account already exists")]
    Exists,
    #[error("operation already applied")]
    RepeatedOperation,
    /// The character is in a live game session; the change needs it offline.
    #[error("character is online")]
    Online,
    /// The game's item rules refused the change (GiveItem/DestroyItem failure name).
    #[error("item change refused: {0}")]
    ItemRefused(String),
    /// Not an initialized laboratory Fiona, or an item outside the migrated profile.
    #[error("character or item outside the editable profile")]
    NotEditable,
    /// Item changes need the game's content (laboratory frontend with its minimum assets).
    #[error("item content not loaded")]
    ContentUnavailable,
    #[error("not enough balance")]
    NotEnough,
    /// The game's rules made no change; the reason key for the message.
    #[error("no change: {0}")]
    Unchanged(&'static str),
    #[error("panel storage unavailable")]
    Unavailable(#[from] sqlx::Error),
}

/// Fixed-window attempt counters per peer and per account name, in process memory.
#[derive(Default)]
struct Throttle(Mutex<HashMap<String, (Instant, u32)>>);

impl Throttle {
    fn admit(&self, key: String, limit: u32, now: Instant) -> bool {
        self.admit_in(key, limit, WINDOW, now)
    }
    fn admit_in(&self, key: String, limit: u32, window: Duration, now: Instant) -> bool {
        let mut map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= MAX_KEYS {
            map.retain(|_, (start, _)| now.duration_since(*start) < REGISTRATION_WINDOW);
            if map.len() >= MAX_KEYS && !map.contains_key(&key) {
                return false;
            }
        }
        let entry = map.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
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

    /// Check login and password under the throttle and lockout. A refusal is audited as
    /// `action`; a success is not (the caller audits it with its own change).
    async fn authenticate(
        &self,
        username: &str,
        password: Zeroizing<Vec<u8>>,
        peer: Option<IpAddr>,
        action: &'static str,
    ) -> Result<admin_store::LoginAccount, AdminError> {
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
            action,
            target_kind: None,
            target_id: None,
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
        let audit = refused(
            Some(account.id),
            Some(account.username.as_str()),
            account.role,
        );
        if !matched {
            admin_store::record_failure(
                &self.pool,
                Some(account.id),
                &audit,
                MAX_FAILURES,
                LOCK_SECONDS as f64,
            )
            .await?;
            return Err(AdminError::InvalidCredentials);
        }
        // A right password while locked or disabled is not a failure.
        if account.locked {
            admin_store::record_failure(
                &self.pool,
                None,
                &audit,
                MAX_FAILURES,
                LOCK_SECONDS as f64,
            )
            .await?;
            return Err(AdminError::InvalidCredentials);
        }
        Ok(account)
    }

    pub async fn login(
        &self,
        username: &str,
        password: Zeroizing<Vec<u8>>,
        peer: Option<IpAddr>,
    ) -> Result<LoginGranted, AdminError> {
        let account = self
            .authenticate(username, password, peer, "session.login")
            .await?;
        let token = SessionToken::generate();
        admin_store::open_session(
            &self.pool,
            account.id,
            &token.digest(),
            SESSION_SECONDS as f64,
            IDLE_SECONDS as f64,
            &success(&account, account.role, "session.login", peer),
        )
        .await?;
        Ok(LoginGranted {
            token,
            session: SessionAccount {
                account_id: account.id,
                username: account.username,
                role: account.role,
            },
        })
    }

    /// Open registration of the login page: a new account without a panel role, logged in.
    pub async fn register(
        &self,
        username: &str,
        password: Zeroizing<Vec<u8>>,
        peer: Option<IpAddr>,
    ) -> Result<LoginGranted, AdminError> {
        let key = format!(
            "register:{}",
            peer.map_or_else(|| "-".into(), |p| p.to_string())
        );
        if !self.throttle.admit_in(
            key,
            REGISTRATIONS_PER_PEER,
            REGISTRATION_WINDOW,
            Instant::now(),
        ) {
            return Err(AdminError::RateLimited);
        }
        let name = Username::parse(username).map_err(|_| AdminError::InvalidInput)?;
        if name.display().len() > super::admin_accounts::MAX_PLAYABLE_NAME {
            return Err(AdminError::InvalidInput);
        }
        let hash = tokio::task::spawn_blocking(move || hash_password(&password))
            .await
            .map_err(|_| AdminError::Busy)?
            .map_err(|_| AdminError::InvalidInput)?;
        let token = SessionToken::generate();
        match admin_accounts::register(
            &self.pool,
            &name,
            &hash,
            &token.digest(),
            SESSION_SECONDS as f64,
            peer,
        )
        .await?
        {
            Registration::Created(account_id) => Ok(LoginGranted {
                token,
                session: SessionAccount {
                    account_id,
                    username: name.display().to_owned(),
                    role: None,
                },
            }),
            Registration::Exists => Err(AdminError::Exists),
        }
    }

    /// The account's own password, after checking the current one (same throttle and
    /// lockout). Its sessions end, this one included.
    pub async fn change_own_password(
        &self,
        session: &SessionAccount,
        current: Zeroizing<Vec<u8>>,
        new: Zeroizing<Vec<u8>>,
        peer: Option<IpAddr>,
    ) -> Result<(), AdminError> {
        self.authenticate(&session.username, current, peer, "account.password")
            .await?;
        let hash = tokio::task::spawn_blocking(move || hash_password(&new))
            .await
            .map_err(|_| AdminError::Busy)?
            .map_err(|_| AdminError::InvalidInput)?;
        let name = Username::parse(&session.username).map_err(|_| AdminError::NotFound)?;
        let (tx, target) = admin_accounts::lock_target(&self.pool, name.normalized()).await?;
        let target = target.ok_or(AdminError::NotFound)?;
        let actor = Actor {
            session,
            peer,
            operation_id: uuid::Uuid::new_v4(),
        };
        match admin_accounts::set_password(tx, &target, &hash, &actor).await? {
            admin_accounts::AccountChange::Done => Ok(()),
            _ => Err(AdminError::RepeatedOperation),
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
                role: session.role,
                operation_id: None,
                action: "session.logout",
                target_kind: None,
                target_id: None,
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

fn success<'a>(
    account: &'a admin_store::LoginAccount,
    role: Option<PanelRole>,
    action: &'static str,
    peer: Option<IpAddr>,
) -> AuditRecord<'a> {
    AuditRecord {
        account_id: Some(account.id),
        actor_name: Some(&account.username),
        role,
        operation_id: None,
        action,
        target_kind: None,
        target_id: None,
        before: None,
        after: None,
        refused: false,
        peer,
    }
}

/// The session's own account.
pub async fn my_account(
    pool: &PgPool,
    session: &SessionAccount,
) -> Result<AccountSummary, AdminError> {
    admin_accounts::summary(pool, session.account_id)
        .await?
        .ok_or(AdminError::Unauthenticated)
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
) -> Result<RoleChange, AdminError> {
    let name = Username::parse(username).map_err(|_| AdminError::InvalidInput)?;
    match admin_store::set_role(pool, name.normalized(), role, None).await? {
        RoleChange::NoAccount => Err(AdminError::NotFound),
        RoleChange::LastAdmin => Err(AdminError::LastAdmin),
        RoleChange::RepeatedOperation => Err(AdminError::RepeatedOperation),
        change => Ok(change),
    }
}

/// Operator CLI: the account enters an existing game user (keeps its characters).
pub async fn link_game_from_cli(
    pool: &PgPool,
    username: &str,
    game_name: &str,
) -> Result<(), AdminError> {
    let name = Username::parse(username).map_err(|_| AdminError::InvalidInput)?;
    if game_name.is_empty()
        || game_name.len() > 24
        || !game_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(AdminError::InvalidInput);
    }
    match game_links::link_game(pool, name.normalized(), game_name).await? {
        game_links::LinkChange::Done => Ok(()),
        game_links::LinkChange::NoAccount => Err(AdminError::NotFound),
        game_links::LinkChange::Claimed => Err(AdminError::Exists),
    }
}
