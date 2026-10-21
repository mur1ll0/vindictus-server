//! Restricted loopback candidate frontend. Login/lobby only; character services remain pending.
use crate::{
    application::{
        character_lobby::{mark_prologue_played_admitted, prepare_empty_character_list},
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
        character_setup::{self as setup, Kind as SetupKind, Message as SetupMessage},
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
struct MinimumRuntime {
    assets: Arc<super::minimum_assets::Assets>,
    creation: crate::application::creation_lifecycle::CreationRuntime,
}
pub struct Runtime {
    profile: LabRuntimeConfig,
    content: RewardPolicyPackage,
    registry: SessionRegistry,
    name_proof: Option<super::character_name_proof::NameProof>,
    minimum: Option<MinimumRuntime>,
    channel: Option<(std::net::SocketAddr, Arc<super::channel_server::Runtime>)>,
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
            name_proof: None,
            minimum: None,
            channel: None,
            registry: SessionRegistry::with_capacity(MAX_CONNECTIONS)
                .map_err(|_| RuntimeError::Operation)?,
        })
    }
    pub fn with_name_check(
        mut self,
        proof: Option<super::character_name_proof::NameProof>,
    ) -> Self {
        self.name_proof = proof;
        self
    }
    pub fn with_minimum_creation(
        mut self,
        pool: PgPool,
        assets: super::minimum_assets::Assets,
        proof_file: &std::path::Path,
    ) -> Result<Self, RuntimeError> {
        let creation = crate::application::creation_lifecycle::CreationRuntime::new(
            pool,
            &self.content,
            std::fs::File::open(proof_file)?,
            assets.entity_service_id,
            MAX_CONNECTIONS,
        )
        .map_err(|_| RuntimeError::Profile)?;
        self.channel = assets.channel.as_ref().map(|(bind, topology)| {
            (
                *bind,
                Arc::new(super::channel_server::Runtime::new(topology.clone())),
            )
        });
        self.minimum = Some(MinimumRuntime {
            assets: Arc::new(assets),
            creation,
        });
        Ok(self)
    }
    pub fn channel(&self) -> Option<(std::net::SocketAddr, Arc<super::channel_server::Runtime>)> {
        self.channel.as_ref().map(|(a, r)| (*a, r.clone()))
    }
    pub async fn shutdown_minimum(&self) -> Result<(), RuntimeError> {
        if let Some(m) = &self.minimum {
            m.creation
                .shutdown()
                .await
                .map_err(|_| RuntimeError::Operation)?
        }
        Ok(())
    }
    pub fn minimum_creation_available(&self) -> bool {
        self.minimum.is_some()
    }
    async fn character_list(
        &self,
        frontend: &FrontendSession,
        pool: &PgPool,
        revision: i64,
    ) -> Result<Zeroizing<Vec<u8>>, RuntimeError> {
        let id = frontend
            .accepted_identity()
            .map_err(|_| RuntimeError::Operation)?;
        let present: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_character_info WHERE uid=$1)")
                .bind(id.nexon_sn)
                .fetch_one(pool)
                .await
                .map_err(|_| RuntimeError::Operation)?;
        if present {
            let m = self.minimum.as_ref().ok_or(RuntimeError::Profile)?;
            let config = self.minimum_header()?;
            return super::minimum_character_list::query(
                pool,
                &super::character_lobby::UserKey {
                    uid: id.nexon_sn,
                    name: &id.nexon_id,
                    revision,
                },
                &m.assets.initializer,
                config,
                m.assets
                    .initializer
                    .policy()
                    .get_integer("DeleteCharacterDelay")?,
                || {
                    lab_clock::utc_now()
                        .map_err(|_| super::minimum_character::MinimumError::Conflict)
                },
            )
            .await
            .map_err(|_| RuntimeError::Operation);
        }
        Ok(Zeroizing::new(
            prepare_empty_character_list(
                frontend,
                pool,
                revision,
                &self.content.policy,
                self.civil()?,
            )
            .await
            .map_err(|_| RuntimeError::Operation)?,
        ))
    }
    fn minimum_header(&self) -> Result<super::character_lobby::QueryConfig, RuntimeError> {
        Ok(super::character_lobby::QueryConfig {
            max_free: self.content.policy.get_integer("MaxCharacterSlot")?,
            care_clock: if self.content.policy.is_enabled(Some("UserCareSystem"))? {
                Some(
                    self.civil()?
                        .sql_parameter()
                        .map_err(|_| RuntimeError::Operation)?,
                )
            } else {
                None
            },
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
    fn stat_arithmetic(&self) -> crate::domain::character_stats::Arithmetic {
        use crate::{
            domain::character_stats::Arithmetic, protocol::identity::FrameworkArchitecture,
        };
        match self.profile.identity_options().profile.architecture {
            FrameworkArchitecture::X86 => Arithmetic::FrameworkX86,
            FrameworkArchitecture::X64 => Arithmetic::FrameworkX64,
        }
    }
    fn item_log_context(
        &self,
        assets: &super::minimum_assets::Assets,
    ) -> Result<super::minimum_item_logs::Context, RuntimeError> {
        Ok(super::minimum_item_logs::Context {
            flags: super::minimum_item_logs::flags(assets).map_err(|_| RuntimeError::Profile)?,
            civil_offset_seconds: self.profile.civil_offset_seconds(),
        })
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
/// ServiceCore.EndPointNetwork.LogOutMessage (fieldless).
const LOG_OUT_CATEGORY: i32 = 493;
const LOG_OUT_TYPE_ID: uuid::Uuid = uuid::Uuid::from_u128(0x312f4152_ab0d_3ac2_99fa_fffa1e3f7e5e);
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
    selected: Option<super::minimum_selection::Selected>,
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
            selected: None,
        }
    }
    /// PlayerConn.Close: the character leaves the channel and its session ends.
    async fn release_selected(&mut self, runtime: &Runtime, pool: &PgPool) {
        if let Some(selected) = self.selected.take() {
            if let Some((_, channel)) = &runtime.channel
                && channel.remove(selected.cid, pool).await.is_err()
            {
                tracing::warn!("minimum channel cleanup failed; values omitted");
            }
            if super::minimum_selection::disconnect(pool, selected.cid)
                .await
                .is_err()
            {
                tracing::warn!("minimum selection cleanup failed; values omitted");
            }
        }
    }
    async fn cleanup(&mut self, runtime: &Runtime, pool: &PgPool) {
        self.release_selected(runtime, pool).await;
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
        // Existing legacy/partial characters remain refused;only explicit initialized minimum rows can be listed.
        let list = match runtime.character_list(&self.frontend, pool, revision).await {
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
        {
            use crate::protocol::minimum_map::{self as wire, Kind, Message};
            for kind in [Kind::SelectCharacter, Kind::EnterChannel] {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                if !self
                    .password
                    .as_ref()
                    .is_some_and(|p| p.selection_allowed(&self.frontend))
                {
                    locked(stream, sender).await?;
                    return Ok(());
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                    .ok_or(RuntimeError::Message)?;
                match message {
                    Message::SelectCharacter { index } => {
                        if self.selected.is_some() || runtime.channel.is_none() {
                            return Err(RuntimeError::Profile);
                        }
                        let id = self
                            .frontend
                            .accepted_identity()
                            .map_err(|_| RuntimeError::Operation)?;
                        let owner = super::character_lobby::UserKey {
                            uid: id.nexon_sn,
                            name: &id.nexon_id,
                            revision: self
                                .password
                                .as_ref()
                                .ok_or(RuntimeError::Operation)?
                                .revision(),
                        };
                        let assets = &runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .assets;
                        let selected = super::minimum_selection::select(
                            pool,
                            &owner,
                            assets,
                            index,
                            runtime.minimum_header()?,
                            runtime.stat_arithmetic(),
                        )
                        .await
                        .map_err(|_| RuntimeError::Operation)?;
                        self.selected = Some(selected);
                        for packet in &self
                            .selected
                            .as_ref()
                            .ok_or(RuntimeError::Operation)?
                            .packets
                        {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        tracing::info!(
                            "minimum character selected; optional services deferred; values omitted"
                        );
                        // StoryEntity construction: login auto-give and SyncStoryProgress.
                        if assets.npc.is_some() {
                            let cid = self.selected.as_ref().ok_or(RuntimeError::Operation)?.cid;
                            let story = async {
                                let n =
                                    super::minimum_story::login_openings(pool, &owner, assets, cid)
                                        .await?;
                                let ids = runtime
                                    .minimum
                                    .as_ref()
                                    .ok_or(super::minimum_character::MinimumError::Conflict)?
                                    .creation
                                    .entity_ids(n)
                                    .await
                                    .map_err(|_| {
                                        super::minimum_character::MinimumError::Conflict
                                    })?;
                                super::minimum_story::login(pool, &owner, assets, cid, ids).await
                            };
                            match story.await {
                                Ok(packets) => {
                                    for packet in &packets {
                                        send(stream, sender, packet.category, &packet.body).await?;
                                    }
                                    tracing::info!(
                                        synchronized = !packets.is_empty(),
                                        "minimum story login processed; values omitted"
                                    );
                                }
                                Err(_) => {
                                    tracing::warn!("minimum story login rejected; values omitted")
                                }
                            }
                        }
                    }
                    Message::EnterChannel {
                        channel_id,
                        partition_id,
                        action,
                    } => {
                        let selected = self.selected.as_ref().ok_or(RuntimeError::Message)?;
                        let (bind, channel) =
                            runtime.channel.as_ref().ok_or(RuntimeError::Profile)?;
                        let key = channel
                            .enter(
                                selected.cid,
                                channel_id,
                                partition_id,
                                action,
                                selected.look.clone(),
                            )
                            .await?;
                        let address = bind.ip().to_string();
                        let message = Message::ChannelServerAddress {
                            channel_id: 1,
                            address: Some(&address),
                            port: i32::from(bind.port()),
                            key,
                        };
                        let body = wire::encode(
                            Kind::ChannelServerAddress,
                            Some(&message),
                            DEFAULT_MAX_BODY,
                        )
                        .map_err(|_| RuntimeError::Message)?;
                        send(stream, sender, Kind::ChannelServerAddress.category(), &body).await?;
                    }
                    _ => return Err(RuntimeError::Message),
                }
                return Ok(());
            }
        }
        {
            use crate::protocol::inventory::{self as wire, Kind, Message};
            if matches(
                category,
                body,
                Kind::QueryInventory.category(),
                Kind::QueryInventory.type_id(),
            ) {
                if !self
                    .password
                    .as_ref()
                    .is_some_and(|p| p.selection_allowed(&self.frontend))
                {
                    // Before selection there is no PlayerConn: the source ignores item
                    // messages; the Check/Set SecondPassword_Locked dialog does not apply.
                    self.log_unhandled(category, body);
                    return Ok(());
                }
                let Some(Message::QueryInventory) =
                    wire::decode_ingress(Kind::QueryInventory, category, body, DEFAULT_MAX_BODY)
                        .map_err(|_| RuntimeError::Message)?
                else {
                    return Err(RuntimeError::Message);
                };
                let Some(selected) = self.selected.as_ref() else {
                    self.log_unhandled(category, body);
                    return Ok(());
                };
                let id = self
                    .frontend
                    .accepted_identity()
                    .map_err(|_| RuntimeError::Operation)?;
                let owner = super::character_lobby::UserKey {
                    uid: id.nexon_sn,
                    name: &id.nexon_id,
                    revision: self
                        .password
                        .as_ref()
                        .ok_or(RuntimeError::Operation)?
                        .revision(),
                };
                let assets = &runtime
                    .minimum
                    .as_ref()
                    .ok_or(RuntimeError::Profile)?
                    .assets;
                let packet = super::minimum_inventory::query(pool, &owner, assets, selected.cid)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                send(stream, sender, packet.category, &packet.body).await?;
                tracing::info!("minimum inventory snapshot sent; values omitted");
                return Ok(());
            }
        }
        {
            use crate::protocol::inventory::{self as wire, Kind, Message};
            if matches(
                category,
                body,
                Kind::MoveInventoryItem.category(),
                Kind::MoveInventoryItem.type_id(),
            ) {
                let Some(Message::MoveInventoryItem(request)) =
                    wire::decode_ingress(Kind::MoveInventoryItem, category, body, DEFAULT_MAX_BODY)
                        .map_err(|_| RuntimeError::Message)?
                else {
                    return Err(RuntimeError::Message);
                };
                // Without a selected character there is no PlayerConn/ItemServiceProxy.
                let (Some(selected), true) = (
                    self.selected.as_ref(),
                    self.password
                        .as_ref()
                        .is_some_and(|p| p.selection_allowed(&self.frontend)),
                ) else {
                    self.log_unhandled(category, body);
                    return Ok(());
                };
                let cid = selected.cid;
                let id = self
                    .frontend
                    .accepted_identity()
                    .map_err(|_| RuntimeError::Operation)?;
                let owner = super::character_lobby::UserKey {
                    uid: id.nexon_sn,
                    name: &id.nexon_id,
                    revision: self
                        .password
                        .as_ref()
                        .ok_or(RuntimeError::Operation)?
                        .revision(),
                };
                let assets = &runtime
                    .minimum
                    .as_ref()
                    .ok_or(RuntimeError::Profile)?
                    .assets;
                let started = std::time::Instant::now();
                match super::minimum_move::move_item(
                    pool,
                    &owner,
                    assets,
                    cid,
                    request,
                    runtime.item_log_context(assets)?,
                )
                .await
                {
                    Ok(super::minimum_move::Outcome::Applied(packet)) => {
                        send(stream, sender, packet.category, &packet.body).await?;
                        tracing::info!(
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "minimum inventory move saved and synchronized"
                        );
                    }
                    Ok(super::minimum_move::Outcome::Refused(packet)) => {
                        send(stream, sender, packet.category, &packet.body).await?;
                        tracing::info!("minimum inventory move refused by source rules");
                    }
                    Ok(super::minimum_move::Outcome::Unchanged) => {}
                    Err(_) => tracing::warn!("minimum inventory move rejected; values omitted"),
                }
                return Ok(());
            }
        }
        {
            use crate::protocol::inventory::{self as wire, Kind, Message};
            if matches(
                category,
                body,
                Kind::EquipBundle.category(),
                Kind::EquipBundle.type_id(),
            ) {
                let Some(Message::EquipBundle(request)) =
                    wire::decode_ingress(Kind::EquipBundle, category, body, DEFAULT_MAX_BODY)
                        .map_err(|_| RuntimeError::Message)?
                else {
                    return Err(RuntimeError::Message);
                };
                let (Some(selected), true) = (
                    self.selected.as_ref(),
                    self.password
                        .as_ref()
                        .is_some_and(|p| p.selection_allowed(&self.frontend)),
                ) else {
                    self.log_unhandled(category, body);
                    return Ok(());
                };
                // ItemServiceProxy: fishing classes are accepted without an operation.
                if request
                    .class
                    .as_deref()
                    .is_some_and(|c| c.starts_with("fishing_"))
                {
                    return Ok(());
                }
                let cid = selected.cid;
                let id = self
                    .frontend
                    .accepted_identity()
                    .map_err(|_| RuntimeError::Operation)?;
                let owner = super::character_lobby::UserKey {
                    uid: id.nexon_sn,
                    name: &id.nexon_id,
                    revision: self
                        .password
                        .as_ref()
                        .ok_or(RuntimeError::Operation)?
                        .revision(),
                };
                let assets = &runtime
                    .minimum
                    .as_ref()
                    .ok_or(RuntimeError::Profile)?
                    .assets;
                match super::minimum_quickslot::equip_bundle(
                    pool,
                    &owner,
                    assets,
                    cid,
                    request,
                    runtime.item_log_context(assets)?,
                )
                .await
                {
                    Ok(super::minimum_quickslot::Outcome::Applied(packet)) => {
                        send(stream, sender, packet.category, &packet.body).await?;
                        tracing::info!("minimum quick slot saved and synchronized");
                    }
                    Ok(super::minimum_quickslot::Outcome::Refused(packet)) => {
                        send(stream, sender, packet.category, &packet.body).await?;
                        tracing::info!("minimum quick slot refused by source rules");
                    }
                    Ok(super::minimum_quickslot::Outcome::Unchanged) => {}
                    Err(_) => tracing::warn!("minimum quick slot rejected; values omitted"),
                }
                return Ok(());
            }
        }
        {
            use crate::protocol::inventory::{self as wire, Kind, Message};
            for kind in [
                Kind::QueryAp,
                Kind::UseInventoryItem,
                Kind::QuerySkillList,
                Kind::LearnSkill,
                Kind::LearnNewSkill,
                Kind::QueryNpcList,
                Kind::QueryNpcTalk,
                Kind::SelectButton,
                Kind::ExecuteNpcServerCommand,
                Kind::BuyItem,
                Kind::QueryShopItemInfo,
                Kind::SellItem,
                Kind::QueryBuybackList,
                Kind::RepairItem,
                Kind::QueryCharacterCommonInfo,
            ] {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                    .ok_or(RuntimeError::Message)?;
                // QueryAP is an InGame message; UseInventoryItem passes UseSecondPassword.
                let (Some(selected), true) = (
                    self.selected.as_ref(),
                    self.password
                        .as_ref()
                        .is_some_and(|p| p.selection_allowed(&self.frontend)),
                ) else {
                    self.log_unhandled(category, body);
                    return Ok(());
                };
                let cid = selected.cid;
                let id = self
                    .frontend
                    .accepted_identity()
                    .map_err(|_| RuntimeError::Operation)?;
                let owner = super::character_lobby::UserKey {
                    uid: id.nexon_sn,
                    name: &id.nexon_id,
                    revision: self
                        .password
                        .as_ref()
                        .ok_or(RuntimeError::Operation)?
                        .revision(),
                };
                let assets = &runtime
                    .minimum
                    .as_ref()
                    .ok_or(RuntimeError::Profile)?
                    .assets;
                match message {
                    Message::QueryAp => {
                        match super::minimum_ap::query(pool, &owner, assets, cid).await {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?;
                                tracing::info!("minimum AP sent; values omitted");
                            }
                            Err(_) => tracing::warn!("minimum AP query rejected; values omitted"),
                        }
                    }
                    Message::LearnSkill(request) => {
                        let skill = request.skill.as_deref().unwrap_or("");
                        match super::minimum_skills::learn(
                            pool,
                            &owner,
                            assets,
                            cid,
                            skill,
                            request.value,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    changed = !packets.is_empty(),
                                    "minimum skill training processed; values omitted"
                                );
                            }
                            Err(_) => {
                                tracing::warn!("minimum skill training rejected; values omitted")
                            }
                        }
                    }
                    Message::QueryNpcTalk(_)
                    | Message::SelectButton(_)
                    | Message::ExecuteNpcServerCommand => {
                        use super::minimum_story::TalkRequest;
                        let request = match &message {
                            Message::QueryNpcTalk(q) => TalkRequest::TalkTo {
                                location: q.location.as_deref(),
                                npc: q.npc.as_deref().unwrap_or(""),
                                story_line: q.story_line.as_deref(),
                                command: q.command.as_deref(),
                            },
                            Message::SelectButton(index) => TalkRequest::SelectButton(*index),
                            _ => TalkRequest::ExecuteServerCommand,
                        };
                        let session =
                            &mut self.selected.as_mut().ok_or(RuntimeError::Operation)?.story;
                        match super::minimum_story::talk(
                            pool, &owner, assets, cid, session, request,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    messages = packets.len(),
                                    "minimum NPC talk processed; values omitted"
                                );
                            }
                            Err(_) => tracing::warn!("minimum NPC talk refused; values omitted"),
                        }
                    }
                    Message::BuyItem(request)
                        if request
                            .shop_id
                            .as_deref()
                            .is_some_and(|s| s.eq_ignore_ascii_case("buyback")) =>
                    {
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(1)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        let list = &mut self
                            .selected
                            .as_mut()
                            .ok_or(RuntimeError::Operation)?
                            .buyback;
                        match super::minimum_shop::buy_back(
                            pool,
                            &owner,
                            assets,
                            cid,
                            request.order,
                            request.count,
                            list,
                            ids,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum buyback processed; values omitted");
                            }
                            Err(_) => tracing::warn!("minimum buyback refused; values omitted"),
                        }
                    }
                    Message::SellItem(request) => {
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(1)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        let list = &mut self
                            .selected
                            .as_mut()
                            .ok_or(RuntimeError::Operation)?
                            .buyback;
                        match super::minimum_shop::sell(
                            pool,
                            &owner,
                            assets,
                            cid,
                            request.item_id,
                            request.count,
                            list,
                            ids,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum sale processed; values omitted");
                            }
                            Err(_) => tracing::warn!("minimum sale refused; values omitted"),
                        }
                    }
                    Message::RepairItem(request) => {
                        let ids = request.item_ids.clone().unwrap_or_default();
                        match super::minimum_repair::repair_item(
                            pool,
                            &owner,
                            assets,
                            cid,
                            super::minimum_repair::Selection {
                                item_ids: &ids,
                                all_equipped: request.all_equipped,
                                all_broken: request.all_broken,
                                all_repairable: request.all_repairable,
                            },
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum repair processed; values omitted");
                            }
                            Err(_) => tracing::warn!("minimum repair refused; values omitted"),
                        }
                    }
                    Message::QueryBuybackList => {
                        let list = &self
                            .selected
                            .as_ref()
                            .ok_or(RuntimeError::Operation)?
                            .buyback;
                        match super::minimum_shop::buyback_list(assets, list) {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?
                            }
                            Err(_) => {
                                tracing::warn!("minimum buyback list rejected; values omitted")
                            }
                        }
                    }
                    Message::BuyItem(request) => {
                        let shop = request.shop_id.as_deref().unwrap_or("");
                        let needed = super::minimum_shop::entity_ids_needed(
                            assets,
                            shop,
                            request.order,
                            request.count,
                        );
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(needed)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        match super::minimum_shop::buy(
                            pool,
                            &owner,
                            assets,
                            cid,
                            shop,
                            request.order,
                            request.count,
                            ids,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum shop purchase processed; values omitted");
                            }
                            Err(_) => {
                                tracing::warn!("minimum shop purchase refused; values omitted")
                            }
                        }
                    }
                    Message::QueryShopItemInfo(shop) => {
                        // ShopItemInfoResult is only sent for buy-restricted rows, which are
                        // not migrated; shops without them answer nothing, as the source.
                        if super::minimum_shop::has_restricted_rows(
                            assets,
                            shop.as_deref().unwrap_or(""),
                        ) {
                            tracing::warn!(
                                "minimum shop restriction query not migrated; values omitted"
                            );
                        }
                    }
                    Message::QueryNpcList => {
                        match super::minimum_story::npc_list(pool, &owner, assets, cid).await {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?;
                                tracing::info!("minimum NPC list sent; values omitted");
                            }
                            Err(_) => tracing::warn!("minimum NPC list rejected; values omitted"),
                        }
                    }
                    Message::LearnNewSkill(skill) => {
                        // InGame: not in battle; one entity ID for the new Skill row.
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(1)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        match super::minimum_skills::learn_new(
                            pool,
                            &owner,
                            assets,
                            cid,
                            skill.as_deref().unwrap_or(""),
                            ids,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    "minimum new skill request processed; values omitted"
                                );
                            }
                            Err(_) => {
                                tracing::warn!("minimum new skill request rejected; values omitted")
                            }
                        }
                    }
                    Message::QueryCharacterCommonInfo(query) => {
                        if query.cid != cid {
                            // Other characters are answered by their PlayerService entity.
                            self.log_unhandled(category, body);
                            return Ok(());
                        }
                        match super::minimum_selection::common_info(
                            pool,
                            &owner,
                            assets,
                            cid,
                            query.query_id,
                            runtime.minimum_header()?,
                            runtime.stat_arithmetic(),
                        )
                        .await
                        {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?;
                                tracing::info!("minimum character info sent; values omitted");
                            }
                            Err(_) => {
                                tracing::warn!("minimum character info rejected; values omitted")
                            }
                        }
                    }
                    Message::QuerySkillList => {
                        match super::minimum_skills::query(pool, &owner, assets, cid).await {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?;
                                tracing::info!("minimum skill list sent; values omitted");
                            }
                            Err(_) => {
                                tracing::warn!("minimum skill list rejected; values omitted")
                            }
                        }
                    }
                    Message::UseInventoryItem(request) => {
                        match super::minimum_use::use_item(
                            pool,
                            &owner,
                            assets,
                            cid,
                            request,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(super::minimum_use::Outcome::Applied(packets)) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum item use saved and synchronized");
                            }
                            Ok(super::minimum_use::Outcome::Unchanged) => {}
                            Err(_) => {
                                tracing::warn!("minimum item use rejected; values omitted")
                            }
                        }
                    }
                    _ => return Err(RuntimeError::Message),
                }
                return Ok(());
            }
        }
        if matches(category, body, LOG_OUT_CATEGORY, LOG_OUT_TYPE_ID) {
            // LogOutMessage has no fields; a null message is not `is LogOutMessage`.
            let fields = if category == 0 { &body[16..] } else { body };
            match fields {
                [1] => {}
                [0] => return Ok(()),
                _ => return Err(RuntimeError::Message),
            }
            if self.selected.is_none() {
                // CleanUpConnections without a PlayerConn has no effect.
                return Ok(());
            }
            // GameClient: CleanUpConnections(disconnectLoginConnection: false). The
            // PlayerConn.Closed handler of CharacterSelecting enters
            // CharacterList(sendList: true) while the client stays connected.
            self.release_selected(runtime, pool).await;
            let revision = self
                .password
                .as_ref()
                .ok_or(RuntimeError::Operation)?
                .revision();
            // InitSecondPassword sends nothing once the password has passed.
            let list = runtime
                .character_list(&self.frontend, pool, revision)
                .await?;
            send(stream, sender, empty_character_list::CATEGORY, &list).await?;
            tracing::info!("minimum character logged out; character list sent");
            return Ok(());
        }
        {
            use crate::protocol::equipment::{self as wire, Kind, Message};
            if matches(
                category,
                body,
                Kind::EquipItem.category(),
                Kind::EquipItem.type_id(),
            ) {
                if !self
                    .password
                    .as_ref()
                    .is_some_and(|p| p.selection_allowed(&self.frontend))
                {
                    // Before selection there is no PlayerConn: the source ignores item
                    // messages; the Check/Set SecondPassword_Locked dialog does not apply.
                    self.log_unhandled(category, body);
                    return Ok(());
                }
                let Some(Message::EquipItem { item_id, part }) =
                    wire::decode_ingress(Kind::EquipItem, category, body, DEFAULT_MAX_BODY)
                        .map_err(|_| RuntimeError::Message)?
                else {
                    return Err(RuntimeError::Message);
                };
                let Some(selected) = self.selected.as_ref() else {
                    self.log_unhandled(category, body);
                    return Ok(());
                };
                let cid = selected.cid;
                let id = self
                    .frontend
                    .accepted_identity()
                    .map_err(|_| RuntimeError::Operation)?;
                let owner = super::character_lobby::UserKey {
                    uid: id.nexon_sn,
                    name: &id.nexon_id,
                    revision: self
                        .password
                        .as_ref()
                        .ok_or(RuntimeError::Operation)?
                        .revision(),
                };
                let assets = &runtime
                    .minimum
                    .as_ref()
                    .ok_or(RuntimeError::Profile)?
                    .assets;
                let started = std::time::Instant::now();
                let outcome = super::minimum_equip::equip(
                    pool,
                    &owner,
                    assets,
                    cid,
                    item_id,
                    part,
                    runtime.minimum_header()?,
                    runtime.stat_arithmetic(),
                    runtime.item_log_context(assets)?,
                )
                .await;
                match outcome {
                    Ok(super::minimum_equip::Outcome::Applied { packets, look }) => {
                        for packet in &packets {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        if let Some(selected) = self.selected.as_mut() {
                            selected.look = look;
                        }
                        tracing::info!(
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "minimum equipment change saved and synchronized"
                        );
                    }
                    Ok(super::minimum_equip::Outcome::Refused(packet)) => {
                        send(stream, sender, packet.category, &packet.body).await?;
                        tracing::info!(
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "minimum equipment change refused by source rules"
                        );
                    }
                    Ok(super::minimum_equip::Outcome::QuickSlots(packet)) => {
                        send(stream, sender, packet.category, &packet.body).await?;
                        tracing::info!("minimum quick slot cleared and synchronized");
                    }
                    Ok(super::minimum_equip::Outcome::Unchanged) => {}
                    // Unsupported states and storage failures keep the session; nothing changed.
                    Err(_) => tracing::warn!("minimum equipment change rejected; values omitted"),
                }
                return Ok(());
            }
        }
        if self.selected.is_some() {
            self.log_unhandled(category, body);
            return Ok(());
        }
        {
            use crate::protocol::character_creation::{self as wire, Kind, Message};
            let kind = Kind::Create;
            if matches(category, body, kind.category(), kind.type_id()) {
                if !self
                    .password
                    .as_ref()
                    .is_some_and(|p| p.selection_allowed(&self.frontend))
                {
                    locked(stream, sender).await?;
                    return Ok(());
                }
                let Some(
                    message @ Message::Create {
                        name: Some(name), ..
                    },
                ) = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                else {
                    return Err(RuntimeError::Message);
                };
                let bytes = wire::encode(Kind::Create, Some(&message), DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?;
                let m = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
                let revision = self
                    .password
                    .as_ref()
                    .map(PasswordSession::revision)
                    .or(self.revision)
                    .ok_or(RuntimeError::Operation)?;
                let outcome = m
                    .creation
                    .run_minimum_admitted(
                        &self.frontend,
                        crate::application::creation_lifecycle::MinimumRequest {
                            revision,
                            name,
                            assets: Arc::clone(&m.assets),
                            body: bytes,
                            offset: runtime.profile.civil_offset_seconds(),
                            config: runtime.minimum_header()?,
                            delete_delay: m
                                .assets
                                .initializer
                                .policy()
                                .get_integer("DeleteCharacterDelay")?,
                        },
                    )
                    .await;
                use crate::application::creation_lifecycle::MinimumOutcome;
                match outcome {
                    Ok(MinimumOutcome::Created(created)) => {
                        send(
                            stream,
                            sender,
                            crate::protocol::character_list::CATEGORY,
                            &created.body,
                        )
                        .await?;
                        tracing::info!(
                            "native minimum character creation completed;optional services deferred;values omitted"
                        );
                    }
                    result => {
                        let code = match result {
                            Ok(MinimumOutcome::Rejected(r)) => r,
                            Ok(MinimumOutcome::InitializationFailed(r)) => r.source_result(),
                            _ => {
                                crate::domain::character_creation::CreateResult::InitCharacterFailed
                            }
                        };
                        let failure = wire::encode(
                            Kind::Fail,
                            Some(&Message::Fail(code as i32)),
                            DEFAULT_MAX_BODY,
                        )
                        .map_err(|_| RuntimeError::Message)?;
                        send(stream, sender, Kind::Fail.category(), &failure).await?;
                        tracing::warn!(
                            result = code as i32,
                            "native minimum creation rejected or compensated;values omitted"
                        );
                    }
                }
                return Ok(());
            }
        }
        let kind = SetupKind::Check;
        if matches(category, body, kind.category(), kind.type_id()) {
            let message = setup::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                .map_err(|_| RuntimeError::Message)?;
            if let Some(SetupMessage::Check { name, name_change }) = message
                && self.frontend.phase() == SessionPhase::PoliciesAccepted
                && let Some(proof) = runtime.name_proof.as_ref()
            {
                let revision = self
                    .password
                    .as_ref()
                    .map(PasswordSession::revision)
                    .or(self.revision)
                    .ok_or(RuntimeError::Operation)?;
                match crate::application::character_name_check::check_admitted(
                    &self.frontend,
                    pool,
                    revision,
                    proof,
                    name,
                )
                .await
                {
                    Ok(result) => {
                        use super::character_name_check::NameCheckResult;
                        if name_change {
                            let result_message = SetupMessage::Result {
                                valid: result == NameCheckResult::Success,
                                name,
                                error: result.error_key(),
                            };
                            let bytes = setup::encode(
                                SetupKind::Result,
                                Some(&result_message),
                                DEFAULT_MAX_BODY,
                            )
                            .map_err(|_| RuntimeError::Message)?;
                            send(stream, sender, SetupKind::Result.category(), &bytes).await?;
                        } else {
                            let format = result
                                .error_key()
                                .unwrap_or("GameUI_Heroes_Create_Character_ValidName");
                            let bytes = ctl::encode(
                                ControlKind::System,
                                Some(&ControlMessage::System {
                                    category: 4,
                                    text: Some(HeroesText {
                                        format: Some(format),
                                        parameters: Some(Vec::new()),
                                    }),
                                }),
                                DEFAULT_MAX_BODY,
                            )
                            .map_err(|_| RuntimeError::Message)?;
                            send(stream, sender, ControlKind::System.category(), &bytes).await?;
                        }
                    }
                    Err(_) => tracing::warn!(
                        "laboratory name check unavailable or outside measured profile; values omitted"
                    ),
                }
            }
            return Ok(());
        }
        let kind = SetupKind::Query;
        if matches(category, body, kind.category(), kind.type_id()) {
            let message = setup::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                .map_err(|_| RuntimeError::Message)?;
            if matches!(message, Some(SetupMessage::Query))
                && self.frontend.phase() == SessionPhase::PoliciesAccepted
            {
                let revision = self
                    .password
                    .as_ref()
                    .map(PasswordSession::revision)
                    .or(self.revision)
                    .ok_or(RuntimeError::Operation)?;
                match crate::application::character_reservations::query_admitted(
                    &self.frontend,
                    pool,
                    revision,
                )
                .await
                {
                    Ok(info) => {
                        let names = info.names.iter().map(|n| Some(n.as_str())).collect();
                        let message = SetupMessage::Reserved {
                            names: Some(names),
                            titles: Some(info.titles),
                        };
                        let bytes =
                            setup::encode(SetupKind::Reserved, Some(&message), DEFAULT_MAX_BODY)
                                .map_err(|_| RuntimeError::Message)?;
                        send(stream, sender, SetupKind::Reserved.category(), &bytes).await?;
                    }
                    Err(_) => {
                        // Source request has OnComplete but no OnFail response.
                        tracing::warn!("laboratory reservation query unavailable; values omitted");
                    }
                }
            }
            return Ok(());
        }
        let kind = SetupKind::Play;
        if matches(category, body, kind.category(), kind.type_id()) {
            let message = setup::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                .map_err(|_| RuntimeError::Message)?;
            // CharacterList ignores the numeric State and has no password guard/response here.
            if matches!(message, Some(SetupMessage::Play(_)))
                && self.frontend.phase() == SessionPhase::PoliciesAccepted
            {
                let revision = self
                    .password
                    .as_ref()
                    .map(PasswordSession::revision)
                    .or(self.revision)
                    .ok_or(RuntimeError::Operation)?;
                if mark_prologue_played_admitted(&self.frontend, pool, revision)
                    .await
                    .is_err()
                {
                    // Source sends an asynchronous operation without an OnFail callback.
                    tracing::warn!("laboratory prologue persistence unavailable; values omitted");
                } else {
                    tracing::info!("laboratory prologue flag persisted; no character created");
                }
            }
            return Ok(());
        }
        for kind in [ControlKind::Ng, ControlKind::Pong] {
            if matches(category, body, kind.category(), kind.type_id()) {
                ctl::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?;
                // NGSecurityConn is NULL after unavailable service. PingPong=false has no pending ping.
                return Ok(());
            }
        }
        self.log_unhandled(category, body);
        Ok(())
    }
    /// Category identity only; message values are never logged.
    fn log_unhandled(&mut self, category: i32, body: &[u8]) {
        self.ignored_messages = self.ignored_messages.saturating_add(1);
        if self.ignored_messages <= 32 {
            tracing::info!(
                category,
                registered_category = super::channel_server::registered_category(category, body),
                selected = self.selected.is_some(),
                "laboratory frontend message has no implemented handler; no response/effect"
            );
        }
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
