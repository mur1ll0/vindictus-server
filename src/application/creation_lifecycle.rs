//! Admitted normal-Fiona creation lifecycle. The actual initializer remains absent.
//! Success is unrepresentable at this boundary until a verified initializer replaces Infallible.
use super::{
    character_creation::{
        self as maker, BaseOutcome, DormantOutcome, EntityAuthority, MakerError, MakerSettings,
        NormalFionaRequest,
    },
    frontend_session::{FrontendSession, SessionError},
    session_registry::MAX_REGISTRY_CAPACITY,
};
use crate::{
    domain::{character_creation::CreateResult, creation_lifecycle::CreationCallbacks},
    error::Error,
    infrastructure::{
        character_creation_admission::CreationAdmissionError,
        character_creation_effects::{self as effects, CreationCleanupLease},
        character_lobby::UserKey,
        character_name_proof::NameProof,
        lab_clock,
        reward_policy::RewardPolicyPackage,
    },
};
use sqlx::PgPool;
use std::{convert::Infallible, future::Future, io::Read, sync::Arc};
use thiserror::Error;
use tokio::{
    sync::{Mutex as AsyncMutex, oneshot, watch},
    task::JoinHandle,
};
use zeroize::Zeroizing;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitializationFailure {
    InitCharacterFailed,
    CustomizeOperationFailed,
    CostumeSaveFailed,
}
impl InitializationFailure {
    pub fn source_result(self) -> CreateResult {
        match self {
            Self::InitCharacterFailed => CreateResult::InitCharacterFailed,
            Self::CustomizeOperationFailed => CreateResult::CustomizeOperationFailed,
            Self::CostumeSaveFailed => CreateResult::CostumeSaveFailed,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreationBase {
    pub cid: i64,
    pub serial: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UninitializedOutcome {
    Rejected(CreateResult),
    InitializationFailed(InitializationFailure),
}
#[derive(Debug, Error)]
pub enum CreationRunError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Policy(#[from] Error),
    #[error(transparent)]
    Maker(#[from] MakerError),
    #[error("creation is outside the admitted normal Fiona profile")]
    Profile,
    #[error("creation runtime is closed, full or synchronization failed")]
    Runtime,
    #[error("creation initializer failed as an operation, not a game result")]
    Initializer,
    #[error("creation compensation failed; no success response is available")]
    Cleanup,
}
struct ProcessState {
    authority: EntityAuthority,
    stopped: bool,
}
struct Inner {
    pool: PgPool,
    proof: Arc<NameProof>,
    server_code: i32,
    max_free: i32,
    dormant: bool,
    process: Arc<AsyncMutex<ProcessState>>,
    shutdown: watch::Sender<bool>,
}
struct Workers {
    closed: bool,
    handles: Vec<JoinHandle<bool>>,
    cleanup_failed: bool,
}
/// Share one runtime per entity authority. No caller supplies owner IDs or feature values.
pub struct CreationRuntime {
    inner: Arc<Inner>,
    workers: AsyncMutex<Workers>,
    capacity: usize,
}
struct Owner {
    uid: i32,
    name: Zeroizing<String>,
    revision: i64,
}
impl Owner {
    fn key(&self) -> UserKey<'_> {
        UserKey {
            uid: self.uid,
            name: &self.name,
            revision: self.revision,
        }
    }
}
impl CreationRuntime {
    /// Package/manifest comes from the pinned original settings/content loader.
    /// Service.ID is explicitly assigned to THIS target authority, not taken from User.service_id.
    pub fn new(
        pool: PgPool,
        package: &RewardPolicyPackage,
        proof_reader: impl Read,
        entity_service_id: i32,
        capacity: usize,
    ) -> Result<Self, CreationRunError> {
        if !(1..=MAX_REGISTRY_CAPACITY).contains(&capacity) {
            return Err(CreationRunError::Runtime);
        }
        for key in ["MaxCharacterSlot", "DormantAccount"] {
            if !package.policy.entries().any(|(k, _)| k == key) {
                return Err(CreationRunError::Profile);
            }
        }
        let proof =
            NameProof::read(proof_reader, package).map_err(|_| CreationRunError::Profile)?;
        let max_free = package.policy.get_integer("MaxCharacterSlot")?;
        let dormant = package.policy.is_enabled(Some("DormantAccount"))?;
        let (shutdown, _) = watch::channel(false);
        Ok(Self {
            inner: Arc::new(Inner {
                pool,
                proof: Arc::new(proof),
                server_code: package.manifest.server_code,
                max_free,
                dormant,
                process: Arc::new(AsyncMutex::new(ProcessState {
                    authority: EntityAuthority::new(entity_service_id),
                    stopped: false,
                })),
                shutdown,
            }),
            workers: AsyncMutex::new(Workers {
                closed: false,
                handles: Vec::new(),
                cleanup_failed: false,
            }),
            capacity,
        })
    }
    /// Service.MakeEntityID ticks from this runtime's authority, for rows that in-game
    /// operations create (items, skills). Unused IDs are simply skipped, as in the source.
    pub async fn entity_ids(&self, count: usize) -> Result<Vec<i64>, CreationRunError> {
        if count > 4096 {
            return Err(CreationRunError::Runtime);
        }
        let mut state = self.inner.process.lock().await;
        (0..count)
            .map(|_| {
                let now = lab_clock::utc_now().map_err(|_| CreationRunError::Runtime)?;
                state
                    .authority
                    .allocate(now)
                    .map(|a| a.entity_time.ticks())
                    .map_err(|_| CreationRunError::Runtime)
            })
            .collect()
    }
    /// Initial stopped=false is the original unassigned bool property, not a feature override.
    pub async fn set_stopped(&self, stopped: bool) {
        self.inner.process.lock().await.stopped = stopped;
    }
    pub async fn run_uninitialized_admitted<F, Fut>(
        &self,
        session: &FrontendSession,
        revision: i64,
        name: &str,
        initialize: F,
    ) -> Result<UninitializedOutcome, CreationRunError>
    where
        F: FnOnce(CreationBase) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Infallible, InitializationFailure>> + Send + 'static,
    {
        let identity = session.accepted_identity()?;
        if session.accepted_permissions()?.has_cheat() || name.len() > 256 {
            return Err(CreationRunError::Profile);
        }
        let owner = Owner {
            uid: identity.nexon_sn,
            name: Zeroizing::new(identity.nexon_id.clone()),
            revision,
        };
        let name = Zeroizing::new(name.to_owned());
        let (tx, rx) = oneshot::channel();
        {
            let mut workers = self.workers.lock().await;
            if workers.closed
                || *self.inner.shutdown.borrow()
                || workers.handles.len() >= self.capacity
            {
                return Err(CreationRunError::Runtime);
            }
            let inner = Arc::clone(&self.inner);
            workers
                .handles
                .push(tokio::spawn(worker(inner, owner, name, tx, initialize)));
        }
        // Dropping this receiver signals cancellation; it doesn't abort a possibly committed base.
        rx.await.map_err(|_| CreationRunError::Runtime)?
    }
    pub async fn run_minimum_admitted(
        &self,
        session: &FrontendSession,
        request: MinimumRequest<'_>,
    ) -> Result<MinimumOutcome, CreationRunError> {
        let MinimumRequest {
            revision,
            name,
            assets,
            body,
            offset,
            config,
            delete_delay,
        } = request;
        use crate::protocol::character_creation::{self as wire, Kind, Message};
        let Some(Message::Create {
            name: Some(request_name),
            template: Some(template),
            preset: false,
            premium: false,
        }) = wire::decode(Kind::Create, &body, crate::protocol::DEFAULT_MAX_BODY)
            .map_err(|_| CreationRunError::Profile)?
        else {
            return Err(CreationRunError::Profile);
        };
        // BaseCharacter with every initial content exported (initializer, items, INNERARMOR).
        if request_name != name || !assets.supports_class(template.character_class) {
            return Err(CreationRunError::Profile);
        }
        let class = template.character_class;
        let identity = session.accepted_identity()?;
        if session.accepted_permissions()?.has_cheat() || name.len() > 256 {
            return Err(CreationRunError::Profile);
        }
        self.drain().await?;
        let owner = Owner {
            uid: identity.nexon_sn,
            name: Zeroizing::new(identity.nexon_id.clone()),
            revision,
        };
        let name = Zeroizing::new(name.to_owned());
        let (tx, rx) = oneshot::channel();
        {
            let mut workers = self.workers.lock().await;
            if workers.closed
                || *self.inner.shutdown.borrow()
                || workers.handles.len() >= self.capacity
            {
                return Err(CreationRunError::Runtime);
            }
            workers.handles.push(tokio::spawn(worker_minimum(
                Arc::clone(&self.inner),
                owner,
                name,
                tx,
                MinimumJob {
                    class,
                    assets,
                    body,
                    offset,
                    config,
                    delete_delay,
                },
            )))
        }
        rx.await.map_err(|_| CreationRunError::Runtime)?
    }
    /// Drain finished/active attempts without cancelling their client. No failure is hidden.
    pub async fn drain(&self) -> Result<(), CreationRunError> {
        let mut workers = self.workers.lock().await;
        while let Some(handle) = workers.handles.last_mut() {
            // Keep the handle registered across await: cancelling drain cannot detach it.
            let clean = handle.await.unwrap_or(false);
            workers.cleanup_failed |= !clean;
            workers.handles.pop();
        }
        if workers.cleanup_failed {
            Err(CreationRunError::Cleanup)
        } else {
            Ok(())
        }
    }
    /// Signal before taking the worker lock: a concurrent drain can be awaiting an initializer.
    pub async fn shutdown(&self) -> Result<(), CreationRunError> {
        self.inner.shutdown.send_replace(true);
        {
            self.workers.lock().await.closed = true;
        }
        self.drain().await
    }
}
impl Drop for CreationRuntime {
    fn drop(&mut self) {
        self.inner.shutdown.send_replace(true);
    }
}
async fn compensate(
    inner: &Inner,
    lease: &CreationCleanupLease,
    callbacks: &mut CreationCallbacks,
    failed: bool,
) -> bool {
    let needed = if failed {
        callbacks.failed()
    } else {
        callbacks.completed(false)
    };
    !needed
        || effects::cleanup_attempt(&inner.pool, lease, &inner.proof)
            .await
            .is_ok()
}
async fn worker<F, Fut>(
    inner: Arc<Inner>,
    owner: Owner,
    name: Zeroizing<String>,
    mut reply: oneshot::Sender<Result<UninitializedOutcome, CreationRunError>>,
    initialize: F,
) -> bool
where
    F: FnOnce(CreationBase) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Infallible, InitializationFailure>> + Send + 'static,
{
    let mut shutdown = inner.shutdown.subscribe();
    if reply.is_closed() || *shutdown.borrow() {
        return true;
    }
    let result = {
        let mut state = inner.process.lock().await;
        if reply.is_closed() || *shutdown.borrow() {
            return true;
        }
        let settings = MakerSettings {
            stopped: state.stopped,
            max_free: inner.max_free,
            server_code: inner.server_code,
        };
        maker::make_normal_fiona_base(
            &inner.pool,
            NormalFionaRequest {
                owner: owner.key(),
                proof: &inner.proof,
                name: &name,
                class: 1,
            },
            &settings,
            &mut state.authority,
            || lab_clock::utc_now().map_err(|_| CreationAdmissionError::Clock),
        )
        .await
    };
    let base = match result {
        Ok(BaseOutcome::Created { cid, serial }) => CreationBase { cid, serial },
        Ok(BaseOutcome::Rejected { result, .. }) => {
            let _ = reply.send(Ok(UninitializedOutcome::Rejected(result)));
            return true;
        }
        Err(error) => {
            let _ = reply.send(Err(CreationRunError::Maker(error)));
            return true;
        }
    };
    let lease = CreationCleanupLease::issued_after_base(&owner.key(), &name, base.cid);
    let mut callbacks = CreationCallbacks::default();
    if reply.is_closed() || *shutdown.borrow() {
        return compensate(&inner, &lease, &mut callbacks, true).await;
    }
    if inner.dormant {
        let outcome = match lab_clock::utc_now() {
            Ok(now) => maker::prepare_dormant(&inner.pool, &owner.key(), base.cid, now).await,
            Err(_) => Err(MakerError::Admission(CreationAdmissionError::Clock)),
        };
        match outcome {
            Ok(DormantOutcome::Ready { .. }) => {}
            Ok(DormantOutcome::InitCharacterFailed) => {
                let clean = compensate(&inner, &lease, &mut callbacks, false).await;
                let _ = reply.send(if clean {
                    Ok(UninitializedOutcome::InitializationFailed(
                        InitializationFailure::InitCharacterFailed,
                    ))
                } else {
                    Err(CreationRunError::Cleanup)
                });
                return clean;
            }
            Err(error) => {
                let clean = compensate(&inner, &lease, &mut callbacks, true).await;
                let _ = reply.send(if clean {
                    Err(CreationRunError::Maker(error))
                } else {
                    Err(CreationRunError::Cleanup)
                });
                return clean;
            }
        }
    }
    if reply.is_closed() || *shutdown.borrow() {
        return compensate(&inner, &lease, &mut callbacks, true).await;
    }
    // Source Dispose disposes the enumerator BEFORE OnFail. Join cancellation before cleanup.
    let mut init = tokio::spawn(async move { initialize(base).await });
    let result = tokio::select! {
        _=reply.closed()=>None,
        _=shutdown.changed()=>None,
        outcome=&mut init=>Some(outcome),
    };
    let response = match result {
        None => {
            init.abort();
            let _ = init.await;
            let clean = compensate(&inner, &lease, &mut callbacks, true).await;
            return clean;
        }
        Some(Err(_)) => Err(CreationRunError::Initializer),
        Some(Ok(Err(reason))) => Ok(UninitializedOutcome::InitializationFailed(reason)),
        Some(Ok(Ok(never))) => match never {},
    };
    let clean = compensate(&inner, &lease, &mut callbacks, response.is_err()).await;
    let _ = reply.send(if clean {
        response
    } else {
        Err(CreationRunError::Cleanup)
    });
    clean
}

/// Explicit native minimum outcome;does not certify legacy service actors or map compatibility.
pub enum MinimumOutcome {
    Rejected(CreateResult),
    InitializationFailed(InitializationFailure),
    Created(crate::infrastructure::minimum_assets::Created),
}
async fn worker_minimum(
    inner: Arc<Inner>,
    owner: Owner,
    name: Zeroizing<String>,
    mut reply: oneshot::Sender<Result<MinimumOutcome, CreationRunError>>,
    job: MinimumJob,
) -> bool {
    let MinimumJob {
        class,
        assets,
        body,
        offset,
        config,
        delete_delay,
    } = job;
    let mut shutdown = inner.shutdown.subscribe();
    if reply.is_closed() || *shutdown.borrow() {
        return true;
    }
    let mut state = inner.process.clone().lock_owned().await;
    let result = {
        if reply.is_closed() || *shutdown.borrow() {
            return true;
        }
        let settings = MakerSettings {
            stopped: state.stopped,
            max_free: inner.max_free,
            server_code: inner.server_code,
        };
        maker::make_normal_fiona_base(
            &inner.pool,
            NormalFionaRequest {
                owner: owner.key(),
                proof: &inner.proof,
                name: &name,
                class,
            },
            &settings,
            &mut state.authority,
            || lab_clock::utc_now().map_err(|_| CreationAdmissionError::Clock),
        )
        .await
    };
    let base = match result {
        Ok(BaseOutcome::Created { cid, serial }) => CreationBase { cid, serial },
        Ok(BaseOutcome::Rejected { result, .. }) => {
            let _ = reply.send(Ok(MinimumOutcome::Rejected(result)));
            return true;
        }
        Err(error) => {
            let _ = reply.send(Err(CreationRunError::Maker(error)));
            return true;
        }
    };
    let lease = CreationCleanupLease::issued_after_base(&owner.key(), &name, base.cid);
    let mut callbacks = CreationCallbacks::default();
    if reply.is_closed() || *shutdown.borrow() {
        return compensate(&inner, &lease, &mut callbacks, true).await;
    }
    if inner.dormant {
        let outcome = match lab_clock::utc_now() {
            Ok(now) => maker::prepare_dormant(&inner.pool, &owner.key(), base.cid, now).await,
            Err(_) => Err(MakerError::Admission(CreationAdmissionError::Clock)),
        };
        match outcome {
            Ok(DormantOutcome::Ready { .. }) => {}
            Ok(DormantOutcome::InitCharacterFailed) => {
                let clean = compensate(&inner, &lease, &mut callbacks, false).await;
                let _ = reply.send(if clean {
                    Ok(MinimumOutcome::InitializationFailed(
                        InitializationFailure::InitCharacterFailed,
                    ))
                } else {
                    Err(CreationRunError::Cleanup)
                });
                return clean;
            }
            Err(error) => {
                let clean = compensate(&inner, &lease, &mut callbacks, true).await;
                let _ = reply.send(if clean {
                    Err(CreationRunError::Maker(error))
                } else {
                    Err(CreationRunError::Cleanup)
                });
                return clean;
            }
        }
    }
    if reply.is_closed() || *shutdown.borrow() {
        return compensate(&inner, &lease, &mut callbacks, true).await;
    }
    // Source Dispose disposes the enumerator BEFORE OnFail. Join cancellation before cleanup.
    let pool = inner.pool.clone();
    let mut init = tokio::spawn(async move {
        assets
            .initialize_and_list(
                &pool,
                &owner.key(),
                &mut state.authority,
                crate::infrastructure::minimum_assets::Attempt {
                    cid: base.cid,
                    body: &body,
                    offset,
                    config,
                    delay: delete_delay,
                },
            )
            .await
            .map_err(|error| {
                let result=match &error {
                    crate::infrastructure::minimum_character::MinimumError::Customization(crate::infrastructure::fiona_customization::CustomizationError::Storage(_))=>InitializationFailure::CostumeSaveFailed,
                    crate::infrastructure::minimum_character::MinimumError::Customization(crate::infrastructure::fiona_customization::CustomizationError::Inventory(_) | crate::infrastructure::fiona_customization::CustomizationError::Appearance(_))=>InitializationFailure::CustomizeOperationFailed,
                    _=>InitializationFailure::InitCharacterFailed,
                };
                // The error chain's texts are fixed messages without request values.
                let mut reason = error.to_string();
                let mut source = std::error::Error::source(&error);
                while let Some(inner) = source {
                    reason.push_str(": ");
                    reason.push_str(&inner.to_string());
                    source = inner.source();
                }
                let stage = match error {
                    crate::infrastructure::minimum_character::MinimumError::State(_) => "state",
                    crate::infrastructure::minimum_character::MinimumError::Managers(_) => {
                        "base-managers"
                    }
                    crate::infrastructure::minimum_character::MinimumError::Customization(_) => {
                        "inventory-or-appearance"
                    }
                    crate::infrastructure::minimum_character::MinimumError::Content(_) => {
                        "content-or-skills"
                    }
                    crate::infrastructure::minimum_character::MinimumError::Database(_) => {
                        "database"
                    }
                    crate::infrastructure::minimum_character::MinimumError::Conflict => {
                        "profile-or-readback"
                    }
                };
                tracing::warn!(
                    stage,
                    reason,
                    "native minimum creation failed; compensation required"
                );
                result
            })
    });
    let result = tokio::select! {
        _=reply.closed()=>None,
        _=shutdown.changed()=>None,
        outcome=&mut init=>Some(outcome),
    };
    let response = match result {
        None => {
            init.abort();
            let _ = init.await;
            let clean = compensate(&inner, &lease, &mut callbacks, true).await;
            return clean;
        }
        Some(Err(_)) => Err(CreationRunError::Initializer),
        Some(Ok(Err(reason))) => Ok(MinimumOutcome::InitializationFailed(reason)),
        Some(Ok(Ok(created))) => {
            callbacks.completed(true);
            let _ = reply.send(Ok(MinimumOutcome::Created(created)));
            return true;
        }
    };
    let clean = compensate(&inner, &lease, &mut callbacks, response.is_err()).await;
    let _ = reply.send(if clean {
        response
    } else {
        Err(CreationRunError::Cleanup)
    });
    clean
}

pub struct MinimumRequest<'a> {
    pub revision: i64,
    pub name: &'a str,
    pub assets: Arc<crate::infrastructure::minimum_assets::Assets>,
    pub body: Zeroizing<Vec<u8>>,
    pub offset: i32,
    pub config: crate::infrastructure::character_lobby::QueryConfig,
    pub delete_delay: i32,
}
struct MinimumJob {
    class: u8,
    assets: Arc<crate::infrastructure::minimum_assets::Assets>,
    body: Zeroizing<Vec<u8>>,
    offset: i32,
    config: crate::infrastructure::character_lobby::QueryConfig,
    delete_delay: i32,
}
