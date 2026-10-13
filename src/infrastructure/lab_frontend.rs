//! Restricted loopback candidate frontend. Login/lobby only; character services remain pending.
use crate::{
    application::{
        character_lobby::prepare_empty_character_list,
        frontend_session::{FrontendSession, LabAdmission, SessionError, SessionPhase},
        home_cafe_bonus::query_admitted,
        local_login_session::{LocalLoginConfig, evaluate_scoped_lab_user},
        login_policy::update_expired_lock,
        second_password::{PasswordConfig, PasswordSession, PasswordSessionError},
        session_registry::SessionRegistry,
        user_login::UserLoginOutcome,
    },
    domain::{
        legacy_user_data::SqlDateTime2, login_effects::EffectClockSamples,
        login_policy::ApplicationClockSamples, passport::LoginRejection,
        second_password::ClockSamples, user_login_composition::TokenProfile,
        user_login_return::UserLoginReturn,
    },
    error::Error,
    infrastructure::{
        lab_clock, lab_runtime_config::LabRuntimeConfig, reward_policy::RewardPolicyPackage,
    },
    protocol::{
        DEFAULT_MAX_BODY, Frame,
        bootstrap::encode_initial_packets,
        empty_character_list,
        features::{FeatureEntry, FeatureMatrix},
        frontend_control::{
            self as ctl, HeroesText, Kind as ControlKind, Message as ControlMessage,
        },
        login_failure::{LoginFailMessage, encode_login_failure},
        login_ingress::decode_login_ingress,
        second_password::{
            self as pwd, HasPassword, Kind as PasswordKind, Message as PasswordMessage,
            PasswordResult,
        },
        server_profile::{
            LOGIN_FAIL_MESSAGE_CATEGORY, USER_LOGIN_MESSAGE_CATEGORY, USER_LOGIN_MESSAGE_TYPE_ID,
        },
        transport::{CipherProfile, LegacyReceiver, LegacySender},
    },
};
use sqlx::PgPool;
use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinSet,
};
use zeroize::Zeroizing;

pub const MAX_CONNECTIONS: usize = 8;
pub const ADMISSION_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const FAILURE_DISCONNECT: Duration = Duration::from_secs(5);
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("laboratory frontend profile requires unavailable dependencies")]
    Profile,
    #[error("laboratory frontend operation failed; values omitted")]
    Operation,
    #[error("laboratory frontend network operation failed")]
    Io(#[source] std::io::Error),
    #[error("laboratory frontend deadline exceeded")]
    Timeout,
    #[error("laboratory frontend received an incomplete or unsupported message")]
    Message,
}
impl From<std::io::Error> for RuntimeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<Error> for RuntimeError {
    fn from(_: Error) -> Self {
        Self::Operation
    }
}
/// Validate dependencies before connecting storage/binding. No external flag is disabled here.
pub struct Runtime {
    profile: LabRuntimeConfig,
    content: RewardPolicyPackage,
    registry: SessionRegistry,
}
impl Runtime {
    pub fn new(
        profile: LabRuntimeConfig,
        content: RewardPolicyPackage,
    ) -> Result<Self, RuntimeError> {
        let p = &content.policy;
        // This runtime supports the recovered candidate's false PingPong and local EN path only.
        if profile.ping_pong()
            || profile.identity_options().zh_cn
            || p.get_string("NGM_disable") != "1"
            || !p.is_enabled(Some("HackshieldOff"))?
        {
            return Err(RuntimeError::Profile);
        }
        for key in [
            "PassportImmigrator",
            "CafeBonus",
            "CashShop",
            "FraudDetector",
            "XignCode",
            "OldNexonIDProtect",
            "SecondPassword_IP_Limit",
        ] {
            if p.is_enabled(Some(key))? {
                return Err(RuntimeError::Profile);
            }
        }
        PasswordConfig::recovered(p, profile.identity_options().profile)
            .map_err(|_| RuntimeError::Profile)?;
        Ok(Self {
            profile,
            content,
            registry: SessionRegistry::with_capacity(MAX_CONNECTIONS)
                .map_err(|_| RuntimeError::Operation)?,
        })
    }
    pub fn bind(&self) -> std::net::SocketAddr {
        self.profile.bind()
    }
    pub fn active_count(&self) -> Result<usize, RuntimeError> {
        self.registry
            .active_count()
            .map_err(|_| RuntimeError::Operation)
    }
    fn civil(&self) -> Result<SqlDateTime2, RuntimeError> {
        lab_clock::civil_now(self.profile.civil_offset_seconds())
            .map_err(|_| RuntimeError::Operation)
    }
    fn clock(&self) -> Result<ClockSamples, RuntimeError> {
        Ok(ClockSamples {
            operation_now: lab_clock::utc_now().map_err(|_| RuntimeError::Operation)?,
            retry_compare_now: lab_clock::utc_now().map_err(|_| RuntimeError::Operation)?,
            retry_remaining_now: lab_clock::utc_now().map_err(|_| RuntimeError::Operation)?,
        })
    }
}
async fn send(
    stream: &mut TcpStream,
    sender: &mut LegacySender,
    category: i32,
    body: &[u8],
) -> Result<(), RuntimeError> {
    let bytes = Zeroizing::new(
        sender
            .encode(&Frame {
                instance_id: 0,
                category_id: category,
                body,
            })
            .map_err(|_| RuntimeError::Message)?,
    );
    tokio::time::timeout(WRITE_TIMEOUT, stream.write_all(&bytes))
        .await
        .map_err(|_| RuntimeError::Timeout)??;
    Ok(())
}
async fn failure(
    stream: &mut TcpStream,
    sender: &mut LegacySender,
    reason: i32,
    text: Option<&str>,
) -> Result<(), RuntimeError> {
    let bytes = Zeroizing::new(
        encode_login_failure(
            Some(&LoginFailMessage {
                reason,
                banned_reason: text,
            }),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| RuntimeError::Message)?,
    );
    send(stream, sender, LOGIN_FAIL_MESSAGE_CATEGORY, &bytes).await
}
fn matches(category: i32, body: &[u8], typed: i32, type_id: uuid::Uuid) -> bool {
    category == typed || (category == 0 && body.get(..16) == Some(type_id.as_bytes().as_slice()))
}
async fn send_password(
    stream: &mut TcpStream,
    sender: &mut LegacySender,
    message: PasswordMessage<'_>,
) -> Result<(), RuntimeError> {
    let kind = message.kind();
    let bytes =
        pwd::encode(kind, Some(&message), DEFAULT_MAX_BODY).map_err(|_| RuntimeError::Message)?;
    send(stream, sender, kind.category(), &bytes).await
}
async fn locked(stream: &mut TcpStream, sender: &mut LegacySender) -> Result<(), RuntimeError> {
    let bytes = ctl::second_password_locked(DEFAULT_MAX_BODY).map_err(|_| RuntimeError::Message)?;
    send(stream, sender, ControlKind::System.category(), &bytes).await
}
struct Connection {
    frontend: FrontendSession,
    password: Option<PasswordSession>,
    revision: Option<i64>,
    peer: std::net::IpAddr,
    close_at: Option<tokio::time::Instant>,
    login_deadline: tokio::time::Instant,
    ignored_messages: u32,
}
impl Connection {
    fn new(runtime: &Runtime, peer: std::net::IpAddr) -> Self {
        Self {
            frontend: FrontendSession::new(&runtime.registry),
            password: None,
            revision: None,
            peer,
            close_at: None,
            login_deadline: tokio::time::Instant::now() + ADMISSION_TIMEOUT,
            ignored_messages: 0,
        }
    }
    async fn cleanup(&mut self, runtime: &Runtime, pool: &PgPool) {
        let revision = self
            .password
            .as_ref()
            .map(PasswordSession::revision)
            .or(self.revision);
        if let (Ok(id), Some(revision), Ok(now)) =
            (self.frontend.accepted_identity(), revision, runtime.civil())
        {
            // Recovered LoginConn Closed: expired IP lock only; journal failure is non-denying.
            if update_expired_lock(pool, id.nexon_sn, revision, self.peer, now)
                .await
                .is_err()
            {
                tracing::warn!("laboratory frontend cleanup IP operation unavailable");
            }
            tracing::info!(
                legacy_journal_available = false,
                "laboratory frontend Logout journal unavailable; source catch permits failure"
            );
        }
        self.password = None;
        self.frontend.close();
    }
    async fn login(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
    ) -> Result<(), RuntimeError> {
        if self.frontend.phase() != SessionPhase::Initial {
            return Ok(());
        }
        let message = decode_login_ingress(category, body, DEFAULT_MAX_BODY)
            .map_err(|_| RuntimeError::Message)?
            .ok_or(RuntimeError::Message)?;
        // Passport allowlist is a local restriction, checked before reservation or any SQL effect.
        if !runtime.profile.permits(message.passport) {
            failure(
                stream,
                sender,
                LoginRejection::PermissionFailed as i32,
                Some(""),
            )
            .await?;
            return Ok(());
        }
        let token = self
            .frontend
            .begin(Instant::now(), ADMISSION_TIMEOUT)
            .map_err(|_| RuntimeError::Operation)?;
        let admission = LabAdmission {
            identity_options: runtime.profile.identity_options(),
            allow_during_maintenance: false,
            allow_cheat: false,
            is_online: true,
        };
        if let Err(error) = self.frontend.admit_lab(
            token,
            Instant::now(),
            message.passport,
            self.peer,
            &admission,
        ) {
            let reason = match error {
                SessionError::Rejected(r) => r as i32,
                _ => 0,
            };
            failure(stream, sender, reason, Some("")).await?;
            return Ok(());
        }
        // NGM_disable uses remote socket address; client hardware/version/cheat identity are not authority.
        let c = LocalLoginConfig {
            content: &runtime.content,
            service_id: runtime.profile.scope().service_id(),
            channel_code: runtime.profile.scope().channel_code(),
            effects_clock: EffectClockSamples {
                loyalty_now: runtime.civil()?,
                dormant_elapsed_now: runtime.civil()?,
                latest_elapsed_now: runtime.civil()?,
                set_dormant_now: runtime.civil()?,
                set_event_now: runtime.civil()?,
            },
            loyalty_insert_now: runtime
                .civil()?
                .sql_parameter()
                .map_err(|_| RuntimeError::Operation)?,
            ban_now: runtime
                .civil()?
                .sql_parameter()
                .map_err(|_| RuntimeError::Operation)?,
            application_clock: ApplicationClockSamples {
                set_lock_now: runtime.civil()?,
                check_lock_now: runtime.civil()?,
            },
            token_profile: TokenProfile::SqlLatin1GeneralCp1,
        };
        let offset = runtime.profile.civil_offset_seconds();
        tracing::info!(
            legacy_journal_available = false,
            ng_security_module_available = false,
            "laboratory frontend Connect journal and NG service unavailable; recovered failure paths retained"
        );
        let result = evaluate_scoped_lab_user(
            &mut self.frontend,
            token,
            pool,
            &c,
            || {
                lab_clock::civil_now(offset)
                    .map_err(|_| Error::LegacyClockOverflow)?
                    .sql_parameter()
                    .map_err(Error::from)
            },
            runtime.profile.scope(),
        )
        .await;
        let (feedback, revision) = match result {
            Ok(UserLoginOutcome::Success {
                feedback, revision, ..
            }) => (feedback, revision),
            Ok(UserLoginOutcome::Failure { reason, text }) => {
                let text =
                    Zeroizing::new(String::from_utf16(&text).map_err(|_| RuntimeError::Message)?);
                failure(stream, sender, reason as i32, Some(&text)).await?;
                return Err(RuntimeError::Operation);
            }
            Err(_) => {
                failure(stream, sender, 0, Some("")).await?;
                return Err(RuntimeError::Operation);
            }
        };
        self.revision = Some(revision);
        tracing::info!(
            legacy_journal_available = false,
            "laboratory frontend Login journal unavailable; source catch permits failure"
        );
        // Actual zero-character query; an existing summary is refused, never replaced with an empty list.
        let list = match prepare_empty_character_list(
            &self.frontend,
            pool,
            revision,
            &runtime.content.policy,
            runtime.civil()?,
        )
        .await
        {
            Ok(list) => list,
            Err(_) => {
                failure(stream, sender, 9, None).await?;
                self.close_at = Some(tokio::time::Instant::now() + FAILURE_DISCONNECT);
                return Ok(());
            }
        };
        // Source schedules query before HomeCafe callback; retain old CafeType if SQL is unavailable.
        let _cafe = match query_admitted(&self.frontend, pool, revision, &runtime.content.policy, 0)
            .await
        {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!("laboratory HomeCafe callback unavailable; prior type retained");
                0
            }
        };
        let pc = PasswordConfig::recovered(
            &runtime.content.policy,
            runtime.profile.identity_options().profile,
        )
        .map_err(|_| RuntimeError::Profile)?;
        let mut ps = PasswordSession::new(&self.frontend, revision, pc)
            .map_err(|_| RuntimeError::Operation)?;
        match ps
            .query_with_source_catch(&self.frontend, pool, runtime.clock()?)
            .await
        {
            Ok(q) => {
                if q.has_password {
                    send_password(stream, sender, PasswordMessage::Ask).await?;
                }
                send_password(
                    stream,
                    sender,
                    PasswordMessage::Has(HasPassword {
                        first: q.first,
                        has_password: q.has_password,
                        passed: ps.state().passed(true),
                        fail_count: q.fail_count,
                        retry_seconds: q.retry_seconds,
                    }),
                )
                .await?;
            }
            Err(_) => {
                ps.lock_after_failure();
                send_password(
                    stream,
                    sender,
                    PasswordMessage::Has(HasPassword {
                        first: false,
                        has_password: true,
                        passed: false,
                        fail_count: 0,
                        retry_seconds: -1,
                    }),
                )
                .await?;
            }
        }
        self.password = Some(ps);
        send(stream, sender, empty_character_list::CATEGORY, &list).await?;
        if let UserLoginReturn::Success(info) = feedback.feedback() {
            if info.is_dormant {
                let bytes = ctl::encode(
                    ControlKind::System,
                    Some(&ControlMessage::System {
                        category: 4,
                        text: Some(HeroesText {
                            format: Some("GameUI_Heroes_DormantAccount_Alarm_Text"),
                            parameters: Some(Vec::new()),
                        }),
                    }),
                    DEFAULT_MAX_BODY,
                )
                .map_err(|_| RuntimeError::Message)?;
                send(stream, sender, ControlKind::System.category(), &bytes).await?;
            }
            if info.is_return {
                let bytes = ctl::encode(
                    ControlKind::Cafe,
                    Some(&ControlMessage::Cafe {
                        home: 0,
                        account: 0,
                        cafe: 4,
                    }),
                    DEFAULT_MAX_BODY,
                )
                .map_err(|_| RuntimeError::Message)?;
                send(stream, sender, ControlKind::Cafe.category(), &bytes).await?;
            }
        }
        tracing::info!(
            empty_character_list_sent = true,
            game_entry_verified = false,
            "laboratory frontend login/lobby responses sent"
        );
        Ok(())
    }
    async fn password(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        kind: PasswordKind,
        frame: (i32, &[u8]),
    ) -> Result<(), RuntimeError> {
        let (category, body) = frame;
        let Some(ps) = self.password.as_mut() else {
            return Ok(());
        };
        let Some(message) = pwd::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
            .map_err(|_| RuntimeError::Message)?
        else {
            return Ok(());
        };
        let (operation, result) = match message {
            PasswordMessage::Check(value) => (
                0,
                ps.check_nullable(&self.frontend, pool, value, runtime.clock()?)
                    .await,
            ),
            PasswordMessage::Set(value) => {
                if !ps.state().passed(true)
                    || (value == Some("")
                        && !runtime
                            .content
                            .policy
                            .is_enabled(Some("SecondPasswordClear"))?)
                {
                    return Ok(());
                }
                let result = ps
                    .change_nullable(&self.frontend, pool, value, runtime.clock()?)
                    .await;
                (if value == Some("") { 2 } else { 1 }, result)
            }
            _ => return Err(RuntimeError::Message),
        };
        match result {
            Ok(r) => {
                send_password(
                    stream,
                    sender,
                    PasswordMessage::Result(PasswordResult {
                        operation,
                        passed: r.passed,
                        fail_count: r.fail_count,
                        retry_seconds: r.retry_seconds,
                    }),
                )
                .await?
            }
            Err(PasswordSessionError::State) => return Ok(()),
            Err(_) => {
                ps.lock_after_failure();
                locked(stream, sender).await?;
            }
        }
        tracing::info!(
            operation,
            "laboratory second-password operation completed; values omitted"
        );
        Ok(())
    }
    async fn dispatch(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
    ) -> Result<(), RuntimeError> {
        if self.close_at.is_some() {
            return Ok(());
        }
        if matches(
            category,
            body,
            USER_LOGIN_MESSAGE_CATEGORY,
            USER_LOGIN_MESSAGE_TYPE_ID,
        ) {
            return self
                .login(runtime, pool, stream, sender, category, body)
                .await;
        }
        for kind in [PasswordKind::Check, PasswordKind::Set] {
            if matches(category, body, kind.category(), kind.type_id()) {
                return self
                    .password(runtime, pool, stream, sender, kind, (category, body))
                    .await;
            }
        }
        for kind in [ControlKind::Ng, ControlKind::Pong] {
            if matches(category, body, kind.category(), kind.type_id()) {
                ctl::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?;
                // NGSecurityConn is NULL after unavailable service. PingPong=false has no pending ping.
                return Ok(());
            }
        }
        self.ignored_messages = self.ignored_messages.saturating_add(1);
        if self.ignored_messages <= 8 {
            let registered_category = if category == 0 {
                crate::protocol::server_profile::REGISTERED_CLASSES
                    .iter()
                    .find(|entry| body.get(..16) == Some(entry.type_id.as_bytes().as_slice()))
                    .map(|entry| entry.category_id)
            } else {
                Some(category)
            };
            tracing::info!(
                category,
                registered_category,
                "laboratory frontend message has no implemented handler; no response/effect"
            );
        }
        Ok(())
    }
}
/// Retains cipher and all complete frames from each read. Cancellation cleans up before reservation release.
pub async fn connection(
    mut stream: TcpStream,
    runtime: Arc<Runtime>,
    pool: PgPool,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), RuntimeError> {
    if !stream.peer_addr()?.ip().is_loopback() {
        return Err(RuntimeError::Profile);
    }
    let peer = stream.peer_addr()?.ip();
    let mut state = Connection::new(&runtime, peer);
    let result=async {
  let b=uuid::Uuid::new_v4().into_bytes();let salt=u32::from_be_bytes([b[0],b[1],b[2],b[3]])&0x7fffffff;
  // The pinned original config sets NGM_Disable=1; the bootstrap sync contains only that override.
  let features=FeatureMatrix{features:Some(vec![FeatureEntry{key:"NGM_disable",value:Some("1")}])};
  let (mut sender,packets)=encode_initial_packets(salt,Some(&features),DEFAULT_MAX_BODY,1024).map_err(|_|RuntimeError::Message)?;
  for bytes in packets{tokio::time::timeout(WRITE_TIMEOUT,stream.write_all(&bytes)).await.map_err(|_|RuntimeError::Timeout)??;}
  let mut receiver=LegacyReceiver::with_profile(CipherProfile::VindictusFrontend,DEFAULT_MAX_BODY);let mut buffer=Zeroizing::new([0u8;4096]);
  loop {
   let deadline=state.close_at.unwrap_or(state.login_deadline);let waiting=state.close_at.is_some()||state.frontend.phase()!=SessionPhase::PoliciesAccepted;
   let count=tokio::select!{
    _=shutdown.changed()=>return Ok(()),
    _=tokio::time::sleep_until(deadline),if waiting=>return if state.close_at.is_some(){Ok(())}else{Err(RuntimeError::Timeout)},
    value=stream.read(&mut buffer[..])=>value?,
   };
   if count==0{return if receiver.buffered_len()==0{Ok(())}else{Err(RuntimeError::Message)}}
   let frames=receiver.push(&buffer[..count]).map_err(|_|RuntimeError::Message)?;
   for mut frame in frames{
    let body=Zeroizing::new(std::mem::take(&mut frame.body));
    let initial=state.frontend.phase()==SessionPhase::Initial;
    let deadline=state.login_deadline;
    let future=state.dispatch(&runtime,&pool,&mut stream,&mut sender,frame.category_id,&body);
    if initial {
     tokio::time::timeout_at(deadline,future).await.map_err(|_|RuntimeError::Timeout)??;
    }else{future.await?;}
   }
  }
 }.await;
    if tokio::time::timeout(Duration::from_secs(5), state.cleanup(&runtime, &pool))
        .await
        .is_err()
    {
        tracing::warn!("laboratory cleanup deadline exceeded");
    }
    state.frontend.close();
    let _ = stream.shutdown().await;
    result
}
pub async fn run(
    listener: TcpListener,
    runtime: Arc<Runtime>,
    pool: PgPool,
    shutdown: impl Future<Output = ()>,
) -> Result<(), RuntimeError> {
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(RuntimeError::Profile);
    }
    let (stop, sig) = watch::channel(false);
    let mut tasks = JoinSet::new();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
         ()=&mut shutdown=>break,
         Some(r)=tasks.join_next(),if !tasks.is_empty()=>{if r.is_err(){tracing::warn!("laboratory frontend task ended unexpectedly");}},
         accepted=listener.accept(),if tasks.len()<MAX_CONNECTIONS=>{
          let (stream,_)=accepted?;let r=runtime.clone();let p=pool.clone();let s=sig.clone();tasks.spawn(async move{if let Err(error)=connection(stream,r,p,s).await{tracing::warn!(error=%error,"laboratory frontend connection closed");}});
         }
        }
    }
    let _ = stop.send(true);
    let drained = tokio::time::timeout(Duration::from_secs(10), async {
        while tasks.join_next().await.is_some() {}
    })
    .await;
    if drained.is_err() {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
    Ok(())
}
