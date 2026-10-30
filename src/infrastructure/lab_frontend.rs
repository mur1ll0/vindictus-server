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
        game_user::LegacyNameKey, legacy_user_data::SqlDateTime2,
        login_effects::EffectClockSamples, login_policy::ApplicationClockSamples,
        passport::LoginRejection, second_password::ClockSamples,
        user_login_composition::TokenProfile, user_login_return::UserLoginReturn,
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
/// Bound on a connection that has not finished login and the second password. The
/// original Initial state has no timeout, and the client connects as soon as its login
/// screen opens (it needs the NGM_disable feature sync), so the player's typing time
/// counts here; 30 s disconnected slow logins. The laboratory keeps a finite bound.
pub const LOGIN_WAIT: Duration = Duration::from_secs(600);
/// ShopList.aspx on the bootstrap HTTP of compose.yaml (127.0.0.1:8081, region folder).
const LAB_SHOP_LIST_URL: &str = "http://127.0.0.1:8081/en-eu-x-gm/ShopList.aspx";
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
    /// Swapped whole when the panel applies a tuning live (B4.5b).
    assets: Arc<super::minimum_assets::LiveAssets>,
    /// The assets profile, re-read with the new overlay on a live tuning.
    source: std::path::PathBuf,
    creation: crate::application::creation_lifecycle::CreationRuntime,
}
pub struct Runtime {
    profile: LabRuntimeConfig,
    content: RewardPolicyPackage,
    registry: SessionRegistry,
    name_proof: Option<super::character_name_proof::NameProof>,
    minimum: Option<MinimumRuntime>,
    channel: Option<(std::net::SocketAddr, Arc<super::channel_server::Runtime>)>,
    relay: Option<super::relay::Relay>,
    web_shop: super::lab_web_shop::Sessions,
    maintenance: super::maintenance::Maintenance,
    content_output: Option<std::path::PathBuf>,
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
            relay: None,
            web_shop: super::lab_web_shop::Sessions::default(),
            maintenance: super::maintenance::Maintenance::default(),
            content_output: None,
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
        source: &std::path::Path,
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
        self.relay = assets.relay.map(super::relay::Relay::new);
        let assets = assets.with_item_arithmetic(self.stat_arithmetic());
        self.minimum = Some(MinimumRuntime {
            assets: Arc::new(super::minimum_assets::LiveAssets::new(assets)),
            source: source.to_path_buf(),
            creation,
        });
        Ok(self)
    }
    /// Live tuning (B4.5b): re-read the assets profile with `overlay` and swap it in. Messages
    /// already running keep the snapshot they took; the next ones use the new values.
    pub async fn reload_assets(
        &self,
        overlay: crate::domain::content_overlay::Overlay,
    ) -> Result<(), RuntimeError> {
        let minimum = self.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let source = minimum.source.clone();
        let arithmetic = self.stat_arithmetic();
        let assets = tokio::task::spawn_blocking(move || {
            super::minimum_assets::Assets::read_with_overlay(&source, &overlay)
                .map(|a| a.with_item_arithmetic(arithmetic))
        })
        .await
        .map_err(|_| RuntimeError::Operation)?
        .map_err(|_| RuntimeError::Profile)?;
        minimum.assets.replace(assets);
        Ok(())
    }
    /// Where the overlay for the client launcher is written (`--content-edits-output`).
    pub fn with_content_output(mut self, path: Option<std::path::PathBuf>) -> Self {
        self.content_output = path;
        self
    }
    pub fn content_output(&self) -> Option<&std::path::Path> {
        self.content_output.as_deref()
    }
    /// Scheduled maintenance: the notices every connection sends and the frontend stop.
    pub fn maintenance(&self) -> &super::maintenance::Maintenance {
        &self.maintenance
    }
    /// Characters selected in live connections, the authority of the in-game web shop.
    pub fn web_shop(&self) -> super::lab_web_shop::Sessions {
        self.web_shop.clone()
    }
    /// The swappable contents, for a service that outlives one snapshot (the web shop).
    pub fn live_assets(&self) -> Option<Arc<super::minimum_assets::LiveAssets>> {
        self.minimum.as_ref().map(|m| Arc::clone(&m.assets))
    }
    /// The minimum creation assets, when configured.
    pub fn minimum_assets(&self) -> Option<Arc<super::minimum_assets::Assets>> {
        self.minimum.as_ref().map(|m| m.assets.current())
    }
    /// The level-up chain's runtime values for a change made outside a connection (the
    /// admin panel, B4.3c), with `journal` as the title log's journal.
    pub fn growth_context(
        &self,
        journal: i64,
    ) -> Result<super::minimum_growth::GrowthContext, RuntimeError> {
        Ok(super::minimum_growth::GrowthContext {
            clock: self.quest_clock(),
            arithmetic: self.stat_arithmetic(),
            config: self.minimum_header()?,
            journal,
        })
    }
    /// The RelayServer of mission entry, when the assets configure one.
    pub fn relay(&self) -> Option<super::relay::Relay> {
        self.relay.clone()
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
            let assets = m.assets.current();
            let config = self.minimum_header()?;
            return super::minimum_character_list::query(
                pool,
                &super::character_lobby::UserKey {
                    uid: id.nexon_sn,
                    name: &id.nexon_id,
                    revision,
                },
                assets.initializers(),
                assets.npc.as_ref().and_then(|n| n.progression.as_ref()),
                config,
                assets
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
    /// Item editor removals (B4 item editor, content overlay `removed_items`): every item of
    /// those classes leaves its owners, unequipped first. Run at start, before connections.
    pub async fn purge_removed_items(
        &self,
        pool: &sqlx::PgPool,
        classes: &[String],
    ) -> Result<Vec<(String, super::minimum_equip::Purged)>, RuntimeError> {
        let Some(assets) = self.minimum_assets() else {
            return Ok(Vec::new());
        };
        let mut done = Vec::new();
        for class in classes {
            let purged = super::minimum_equip::purge_class(
                pool,
                &assets,
                class,
                self.minimum_header()?,
                self.stat_arithmetic(),
            )
            .await
            .map_err(|_| RuntimeError::Operation)?;
            done.push((class.clone(), purged));
        }
        Ok(done)
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
    fn quest_clock(&self) -> super::minimum_quest::QuestClock {
        super::minimum_quest::QuestClock {
            arithmetic: self.stat_arithmetic(),
            civil_offset_seconds: i64::from(self.profile.civil_offset_seconds()),
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
/// Lowercase account-name key of an allowlist passport (the recovered NexonID derivation).
fn unclaimed_name_key(
    passport: Option<&str>,
    options: crate::protocol::identity::LabIdentityOptions,
) -> Option<String> {
    let identity = crate::protocol::identity::derive_lab_identity(passport, options).ok()?;
    LegacyNameKey::parse(&identity.nexon_id)
        .ok()
        .map(|key| key.as_str().to_owned())
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
    /// BattleServiceProxy party/MicroPlay of the selected character (solo host).
    mission: Option<Box<super::minimum_mission::Mission>>,
    /// InGame.LastQueryRecommendShip (2 s cooldown of the boat ship list).
    last_recommend_ship: Option<std::time::Instant>,
    /// RemoveScheduleID of the next stored status effect to expire.
    effect_expiry: Option<tokio::time::Instant>,
    /// DailyItemScheduler: the next range start of a periodic daily item procedure.
    daily_at: Option<tokio::time::Instant>,
    /// ItemOwner.LatestSort (30-second SortInventory cooldown).
    latest_sort: Option<std::time::Instant>,
}
impl Connection {
    fn new(runtime: &Runtime, peer: std::net::IpAddr) -> Self {
        Self {
            frontend: FrontendSession::new(&runtime.registry),
            password: None,
            revision: None,
            peer,
            close_at: None,
            login_deadline: tokio::time::Instant::now() + LOGIN_WAIT,
            ignored_messages: 0,
            selected: None,
            mission: None,
            last_recommend_ship: None,
            effect_expiry: None,
            daily_at: None,
            latest_sort: None,
        }
    }
    /// PlayerConn.Close: the character leaves the channel and its session ends.
    async fn release_selected(&mut self, runtime: &Runtime, pool: &PgPool) {
        // LeaveParty: the party and MicroPlay of this connection end with it; its relay
        // group closes (the disconnected player gets no reward, as without CombatRejoinSystem).
        if let Some(m) = self.mission.take()
            && let Some(relay) = runtime.relay()
        {
            relay.close_group(m.group);
        }
        self.effect_expiry = None;
        self.daily_at = None;
        if let Some(selected) = self.selected.take() {
            runtime.web_shop.remove(selected.cid);
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
        // B4.2 (operator decision 2026-10-26): the client never sends its password field (the
        // UserLogin lengths were identical with and without one), so the ID typed is the login
        // of an active account, which enters that account's game name; the password protects
        // the panel and the second password the characters. Otherwise the local allowlist
        // applies, only to names no account has claimed. Checked before reservation: a refusal
        // reserves no session and touches no game state.
        let account_game = match message
            .passport
            .and_then(|p| crate::domain::Username::parse(p).ok())
        {
            Some(name) => super::game_links::game_name_for_login(pool, name.normalized())
                .await
                .ok()
                .flatten()
                .map(Zeroizing::new),
            None => None,
        };
        let passport = account_game
            .as_ref()
            .map_or(message.passport, |g| Some(g.as_str()));
        let admitted = account_game.is_some()
            || (runtime.profile.permits(passport)
                && match unclaimed_name_key(passport, runtime.profile.identity_options()) {
                    Some(key) => {
                        matches!(super::game_links::claimed(pool, &key).await, Ok(false))
                    }
                    None => false,
                });
        if !admitted {
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
        if let Err(error) =
            self.frontend
                .admit_lab(token, Instant::now(), passport, self.peer, &admission)
        {
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
    /// BattleServiceProxy for a solo host: AcceptQuest in the Initial state, then
    /// RegisterServer and StartGame in ShipHost. The relay join of the host follows the
    /// relay's Connected event in a task of its own.
    async fn mission(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        message: crate::protocol::mission::Message,
    ) -> Result<(), RuntimeError> {
        use super::minimum_mission::{self as mission, Accept, RelayTicket};
        use crate::protocol::mission::Message;
        let (Some(selected), true, Some(relay)) = (
            self.selected.as_ref(),
            self.password
                .as_ref()
                .is_some_and(|p| p.selection_allowed(&self.frontend)),
            runtime.relay(),
        ) else {
            tracing::warn!("mission message outside an entered character or relay; ignored");
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
            .assets
            .current();
        let packets = match message {
            Message::AcceptQuest(accept) => {
                if self.mission.is_some() {
                    tracing::warn!("AcceptQuest while already in a party; ignored");
                    return Ok(());
                }
                let nexon_sn = id.nexon_sn;
                let outcome = mission::accept_quest(
                    pool,
                    &owner,
                    assets,
                    cid,
                    &accept,
                    |party_id| {
                        let group = relay.make_group();
                        // Subscribe before registering, so Connected cannot be missed.
                        let mut events = relay.subscribe();
                        let joiner = relay.clone();
                        tokio::spawn(async move {
                            let joined = tokio::time::timeout(Duration::from_secs(120), async {
                                loop {
                                    match events.recv().await {
                                        Ok(super::relay::RelayEvent::Connected {
                                            cid: c, ..
                                        }) if c == cid => {
                                            // MicroPlayer.RelayClient_StateChanged → P2PGroup.Join.
                                            if !joiner.join(
                                                group,
                                                cid,
                                                party_id,
                                                mission::MICRO_PLAY_CATEGORY,
                                            ) {
                                                return false;
                                            }
                                        }
                                        Ok(super::relay::RelayEvent::Joined { cid: c, .. })
                                            if c == cid =>
                                        {
                                            return true;
                                        }
                                        Ok(super::relay::RelayEvent::Closed { cid: c })
                                            if c == cid =>
                                        {
                                            return false;
                                        }
                                        Ok(_)
                                        | Err(tokio::sync::broadcast::error::RecvError::Lagged(
                                            _,
                                        )) => {}
                                        Err(_) => return false,
                                    }
                                }
                            })
                            .await;
                            tracing::info!(
                                joined = matches!(joined, Ok(true)),
                                "mission relay join finished; values omitted"
                            );
                        });
                        let waiting = relay.add_client_waiting(cid, nexon_sn);
                        RelayTicket {
                            address: relay.endpoint(),
                            group,
                            sn: waiting.sn,
                            key: waiting.key,
                        }
                    },
                    runtime.minimum_header()?,
                    runtime.quest_clock(),
                )
                .await;
                match outcome {
                    Ok(Accept::Started { mission, packets }) => {
                        self.mission = Some(mission);
                        tracing::info!("mission accepted; relay waiting; values omitted");
                        packets
                    }
                    Ok(Accept::Failed(packets)) => {
                        tracing::info!("mission refused by its digest checks; values omitted");
                        packets
                    }
                    Err(_) => {
                        tracing::warn!("mission accept rejected; values omitted");
                        return Ok(());
                    }
                }
            }
            Message::RegisterServer(register) => {
                let Some(m) = self.mission.as_mut() else {
                    return Ok(());
                };
                mission::register_server(m, register.info).map_err(|_| RuntimeError::Operation)?
            }
            Message::StartGame => {
                let Some(m) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                let packets = match mission::pay_required_item(pool, &owner, assets, cid, m).await {
                    Ok(Ok(mut paid)) => {
                        paid.extend(
                            mission::start_game(m, contents, &mut mission::OsMissionRandom)
                                .map_err(|_| RuntimeError::Operation)?,
                        );
                        paid
                    }
                    Ok(Err(failed)) => {
                        tracing::warn!("mission required item not paid; values omitted");
                        failed
                    }
                    Err(_) => return Err(RuntimeError::Operation),
                };
                tracing::info!(
                    granted = m.sectors.is_some(),
                    "mission start processed; values omitted"
                );
                packets
            }
            Message::MoveToNextSector(m) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                let moved = mission::move_sector(
                    state,
                    contents,
                    m.trigger_name.as_deref().unwrap_or(""),
                    m.target_group,
                )
                .map_err(|_| RuntimeError::Operation)?;
                tracing::info!(moved, "mission sector move processed; values omitted");
                Vec::new()
            }
            // AllUserJoinComplete → ConfirmCoinPaid: BlessStone.Consume with no stone
            // inserted and no fatigue system has no client-visible effect.
            Message::AllUserJoinComplete => {
                tracing::info!("mission all users joined; values omitted");
                Vec::new()
            }
            Message::MonsterKilled(k) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                let rate = assets
                    .npc_policy()
                    .and_then(|p| p.get_integer("ExpRate_Monster").ok())
                    .unwrap_or(100);
                let (kill, packets) = mission::monster_killed(
                    state,
                    contents,
                    k.attacker,
                    k.target,
                    k.action_type.as_deref().unwrap_or(""),
                    k.has_evil_core,
                    rate,
                    &mut mission::OsMissionRandom,
                )
                .map_err(|_| RuntimeError::Operation)?;
                tracing::info!(
                    killed = matches!(kill, crate::domain::mission::Kill::Killed { .. }),
                    "mission monster kill processed; values omitted"
                );
                packets
            }
            Message::RagdollKicked(kick) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                let Some(item) = mission::ragdoll_kicked(
                    state,
                    contents,
                    kick.tag,
                    kick.target_entity_name.as_deref().unwrap_or(""),
                    kick.core_type,
                    kick.is_rare_core,
                ) else {
                    tracing::info!("mission core kick without loot; values omitted");
                    return Ok(());
                };
                let ids = runtime
                    .minimum
                    .as_ref()
                    .ok_or(RuntimeError::Profile)?
                    .creation
                    .entity_ids(1)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                let mut ids = ids.into_iter();
                match mission::give_reward(
                    pool,
                    &owner,
                    assets,
                    state,
                    cid,
                    &item,
                    1,
                    kick.core_type,
                    &mut || {
                        ids.next()
                            .ok_or(super::minimum_character::MinimumError::Conflict)
                    },
                )
                .await
                {
                    Ok(packets) => {
                        tracing::info!("mission core looted; values omitted");
                        packets
                    }
                    Err(_) => {
                        tracing::warn!(
                            "mission core loot refused by the give planner; values omitted"
                        );
                        Vec::new()
                    }
                }
            }
            Message::SectorPropList(list) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                let props: Vec<(i32, i32)> = list.props.0.unwrap_or_default();
                mission::sector_props(state, contents, &props, &mut mission::OsMissionRandom)
                    .map_err(|_| RuntimeError::Operation)?;
                tracing::info!(
                    props = props.len(),
                    "mission sector props registered; values omitted"
                );
                Vec::new()
            }
            Message::PropBroken(broken) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                mission::prop_broken(
                    state,
                    contents,
                    broken.prop,
                    broken.entity_name.as_deref(),
                    broken.attacker,
                    &mut mission::OsMissionRandom,
                )
                .map_err(|_| RuntimeError::Operation)?
            }
            Message::PickErg(erg_id) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                let Some(erg) = mission::pick_erg(state, contents, erg_id) else {
                    tracing::info!("mission erg pick refused; values omitted");
                    return Ok(());
                };
                let ids = runtime
                    .minimum
                    .as_ref()
                    .ok_or(RuntimeError::Profile)?
                    .creation
                    .entity_ids(1)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                let mut ids = ids.into_iter();
                let mut packets = match mission::give_reward(
                    pool,
                    &owner,
                    assets,
                    state,
                    cid,
                    &erg.kind,
                    erg.amount,
                    0,
                    &mut || {
                        ids.next()
                            .ok_or(super::minimum_character::MinimumError::Conflict)
                    },
                )
                .await
                {
                    Ok(packets) => packets,
                    Err(_) => {
                        tracing::warn!(
                            "mission erg give refused by the give planner; values omitted"
                        );
                        Vec::new()
                    }
                };
                // PickErg completes once the erg is picked; PutErg tells the host who took it.
                packets.push(
                    mission::put_erg(erg_id, erg.winner).map_err(|_| RuntimeError::Operation)?,
                );
                tracing::info!("mission erg picked; values omitted");
                packets
            }
            // MDRSystem_V1 anti-cheat only marks CheaterType for rankings, which are not
            // migrated (MDRSystem_V2 and the action filter are off).
            Message::MonsterDamageReport(_) => Vec::new(),
            Message::CombatRecord(record) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                if record.player_number == state.member.slot {
                    state.combat_record = Some(record);
                }
                Vec::new()
            }
            Message::UseConsumable(used) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                let Some((destroy, mut packets)) =
                    mission::use_consumable(state, contents, used.tag, used.part, used.slot)
                        .map_err(|_| RuntimeError::Operation)?
                else {
                    tracing::info!("mission consumable use refused; values omitted");
                    return Ok(());
                };
                if let Some(class) = destroy {
                    match mission::destroy_consumable(pool, &owner, assets, cid, &class).await {
                        Ok(more) => packets.extend(more),
                        Err(_) => {
                            tracing::warn!("mission consumable destroy failed; values omitted")
                        }
                    }
                }
                tracing::info!("mission consumable used; values omitted");
                packets
            }
            Message::PlayerRevived(revived) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let contents = assets.mission().ok_or(RuntimeError::Profile)?;
                let packets = mission::apply_effect(
                    state,
                    contents,
                    revived.caster_tag,
                    revived.reviver_tag,
                    revived.method.as_deref().unwrap_or(""),
                    2,
                )
                .map_err(|_| RuntimeError::Operation)?;
                tracing::info!(
                    alive = state.alive,
                    "mission revive processed; values omitted"
                );
                packets
            }
            Message::QueryRecommendShip(query) => {
                let packets = mission::recommend_ships(
                    &query,
                    &mut self.last_recommend_ship,
                    std::time::Instant::now(),
                )
                .map_err(|_| RuntimeError::Operation)?;
                tracing::info!("mission ship list sent; values omitted");
                packets
            }
            Message::PlayerKilled(tag) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let dead = mission::player_killed(state, tag);
                tracing::info!(dead, "mission player death processed; values omitted");
                Vec::new()
            }
            Message::MicroPlayEvent(event) => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                let accepted = super::minimum_mission_finish::micro_play_event(
                    state,
                    event.slot,
                    event.event.as_deref().unwrap_or(""),
                );
                tracing::info!(accepted, "mission client event processed; values omitted");
                Vec::new()
            }
            Message::LeaveParty => {
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                super::minimum_mission_finish::give_up(state, std::time::Instant::now());
                tracing::info!("mission give-up requested; values omitted");
                Vec::new()
            }
            Message::ReturnToTown => {
                use super::minimum_mission_finish::{Return, return_to_town};
                let Some(state) = self.mission.as_mut() else {
                    return Ok(());
                };
                match return_to_town(state).map_err(|_| RuntimeError::Operation)? {
                    Return::Ignored => return Ok(()),
                    Return::StorySector(packets) => {
                        tracing::info!("mission returned to its story sector; values omitted");
                        packets
                    }
                    Return::Town(packets) => {
                        if let Some(m) = self.mission.take() {
                            relay.close_group(m.group);
                        }
                        tracing::info!("mission left; returning to town; values omitted");
                        packets
                    }
                }
            }
            _ => return Ok(()),
        };
        let mut packets = packets;
        let journal = self.selected.as_ref().map_or(0, |s| s.log_journal);
        packets
            .extend(Self::mission_after(&mut self.mission, runtime, pool, &owner, journal).await?);
        for packet in &packets {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        Ok(())
    }
    /// TitleGoalTracker → InformMicroplayEvent → Titles.Proceed for the queued informer
    /// events of a non-practice game, event by event.
    async fn mission_titles(
        state: &mut super::minimum_mission::Mission,
        runtime: &Runtime,
        pool: &PgPool,
        owner: &super::character_lobby::UserKey<'_>,
        journal: i64,
    ) -> Result<Vec<crate::domain::map_partition::OutgoingPacket>, RuntimeError> {
        let assets = &runtime
            .minimum
            .as_ref()
            .ok_or(RuntimeError::Profile)?
            .assets
            .current();
        // TitleGoalTracker → InformMicroplayEvent → Titles.Proceed, event by event.
        if state.title_events.is_empty() {
            return Ok(Vec::new());
        }
        let mut packets = Vec::new();
        {
            let cid = state.completion.cid;
            if state.title_goals.is_none() {
                match super::minimum_growth::tracking_goals(pool, assets, cid).await {
                    Ok(goals) => state.title_goals = Some(goals),
                    Err(_) => tracing::warn!("mission title tracker not loaded; values omitted"),
                }
            }
            let events = std::mem::take(&mut state.title_events);
            if let Some(goals) = state.title_goals.as_ref() {
                let growth = super::minimum_growth::GrowthContext {
                    clock: runtime.quest_clock(),
                    arithmetic: runtime.stat_arithmetic(),
                    config: runtime.minimum_header()?,
                    journal,
                };
                for (actor, event) in &events {
                    match super::minimum_growth::proceed_titles(
                        pool,
                        owner,
                        assets,
                        cid,
                        goals,
                        event,
                        *actor == cid,
                        &state.quest,
                        growth,
                    )
                    .await
                    {
                        Ok(more) => {
                            if !more.is_empty() {
                                tracing::info!("mission title goals proceeded; values omitted");
                            }
                            packets.extend(more);
                        }
                        Err(_) => tracing::warn!("mission title goal not applied; values omitted"),
                    }
                }
            }
        }
        Ok(packets)
    }
    /// After each mission message: the informer events through the goal tracker and a
    /// requested quest play count update.
    async fn mission_after(
        mission: &mut Option<Box<super::minimum_mission::Mission>>,
        runtime: &Runtime,
        pool: &PgPool,
        owner: &super::character_lobby::UserKey<'_>,
        journal: i64,
    ) -> Result<Vec<crate::domain::map_partition::OutgoingPacket>, RuntimeError> {
        let Some(state) = mission.as_mut() else {
            return Ok(Vec::new());
        };
        let assets = &runtime
            .minimum
            .as_ref()
            .ok_or(RuntimeError::Profile)?
            .assets
            .current();
        let ctx = super::minimum_mission_finish::FinishContext {
            contents: assets.mission().ok_or(RuntimeError::Profile)?,
            policy: assets.npc_policy().ok_or(RuntimeError::Profile)?,
            clock: runtime.quest_clock(),
        };
        let mut packets =
            super::minimum_mission_finish::drain(state, ctx, std::time::Instant::now())
                .map_err(|_| RuntimeError::Operation)?;
        packets.extend(Self::mission_titles(state, runtime, pool, owner, journal).await?);
        if super::minimum_mission_finish::update_play_count(state, pool, owner, assets)
            .await
            .is_err()
        {
            tracing::warn!("mission quest play count update failed; values omitted");
        }
        Ok(packets)
    }
    /// When the earliest MicroPlay job of the mission is due.
    /// StatusEffectManager.AddStatusEffectToMemory: schedules the next expiry of the stored
    /// effects of the selected character (none when it has none or they cannot load).
    /// The one authority for a new look of the selected character: the selection keeps it
    /// for later channel entries and the channel member's Look component takes it
    /// (ChannelMember.LookObserver → Player.UpdateLook), so the town shows it on return.
    async fn set_look(
        &mut self,
        runtime: &Runtime,
        look: crate::domain::map_partition::OutgoingPacket,
    ) {
        let Some(selected) = self.selected.as_mut() else {
            return;
        };
        if let Some((_, channel)) = runtime.channel.as_ref() {
            channel.update_look(selected.cid, look.clone()).await;
        }
        selected.look = look;
    }

    async fn schedule_effects(&mut self, runtime: &Runtime, pool: &PgPool) {
        self.effect_expiry = None;
        let (Some(selected), Some(minimum)) = (self.selected.as_ref(), runtime.minimum.as_ref())
        else {
            return;
        };
        let assets = minimum.assets.current();
        let Some(shop) = assets.cash_shop.as_ref() else {
            return;
        };
        let Ok(mut tx) = pool.begin().await else {
            return;
        };
        let next = super::minimum_status_effect::next_expiry(
            &mut tx,
            &shop.customize().status_effects,
            selected.cid,
        )
        .await;
        if tx.commit().await.is_err() {
            return;
        }
        if let Ok(Some(after)) = next {
            // Wake a second after the ExpireTime: IsExpired is strictly past it.
            self.effect_expiry = Some(tokio::time::Instant::now() + after + Duration::from_secs(1));
        }
    }
    /// RemoveExpiredStatusEffect: the expired effects leave and Save announces them.
    async fn effect_jobs(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        self.effect_expiry = None;
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
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
            .assets
            .current();
        match super::minimum_status_effect::expire_due(
            pool,
            &owner,
            assets,
            cid,
            runtime.stat_arithmetic(),
        )
        .await
        {
            Ok(Some(packets)) => {
                for packet in &packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                tracing::info!("minimum status effects expired; values omitted");
            }
            Ok(None) => {}
            Err(_) => tracing::warn!("minimum status effect expiry rejected; values omitted"),
        }
        self.schedule_effects(runtime, pool).await;
        Ok(())
    }
    /// DailyItemController.OnLogIn/ExecuteDailyItemProcess/OnSchedulerEvent: the due
    /// procedures for the selected character, then the next range is scheduled.
    async fn daily_items(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        self.daily_at = None;
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
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
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        let ids = minimum
            .creation
            .entity_ids(32)
            .await
            .map_err(|_| RuntimeError::Operation)?;
        let mut ids = ids.into_iter();
        match super::minimum_daily_item::process(pool, &owner, assets, cid, &mut || {
            ids.next()
                .ok_or(super::minimum_character::MinimumError::Conflict)
        })
        .await
        {
            Ok(packets) => {
                for packet in &packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                if !packets.is_empty() {
                    tracing::info!("minimum daily items processed; values omitted");
                }
            }
            Err(_) => tracing::warn!("minimum daily items rejected; values omitted"),
        }
        if let Ok(Some(after)) = super::minimum_daily_item::next_range(assets) {
            // DailyItemScheduler schedules three seconds after the range start.
            self.daily_at = Some(tokio::time::Instant::now() + after + Duration::from_secs(3));
        }
        Ok(())
    }
    fn mission_timer(&self) -> Option<tokio::time::Instant> {
        let at = super::minimum_mission_finish::next_job(self.mission.as_deref()?)?;
        Some(tokio::time::Instant::from_std(at))
    }
    /// Runs the due MicroPlay jobs (success scene end, rewards, per-minute job).
    async fn mission_jobs(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        let (Some(selected), Some(_)) = (self.selected.as_ref(), self.mission.as_ref()) else {
            return Ok(());
        };
        let cid = selected.cid;
        let journal = selected.log_journal;
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
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        let ctx = super::minimum_mission_finish::FinishContext {
            contents: assets.mission().ok_or(RuntimeError::Profile)?,
            policy: assets.npc_policy().ok_or(RuntimeError::Profile)?,
            clock: runtime.quest_clock(),
        };
        let rc = super::minimum_mission_finish::RewardContext {
            pool,
            owner: &owner,
            assets,
            growth: super::minimum_growth::GrowthContext {
                clock: runtime.quest_clock(),
                arithmetic: runtime.stat_arithmetic(),
                config: runtime.minimum_header()?,
                journal,
            },
        };
        let ids = minimum
            .creation
            .entity_ids(super::minimum_mission_finish::REWARD_ENTITY_IDS)
            .await
            .map_err(|_| RuntimeError::Operation)?;
        let mut ids = ids.into_iter();
        let state = self.mission.as_mut().ok_or(RuntimeError::Operation)?;
        let _ = cid;
        let packets = super::minimum_mission_finish::run_jobs(
            state,
            ctx,
            &rc,
            &mut ids,
            std::time::Instant::now(),
        )
        .await
        .map_err(|_| RuntimeError::Operation)?;
        let mut packets = packets;
        packets.extend(Self::mission_titles(state, runtime, pool, &owner, journal).await?);
        // GiveReward_Null → Leave: the proxy left the party with the MicroPlay.
        if state.completion.left
            && let Some(m) = self.mission.take()
        {
            if let Some(relay) = runtime.relay() {
                relay.close_group(m.group);
            }
            tracing::info!("mission left without reward; values omitted");
        }
        for packet in &packets {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        if !packets.is_empty() {
            tracing::info!(
                messages = packets.len(),
                "mission jobs processed; values omitted"
            );
        }
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
                        // CharacterList ignores a summary that IsDeleting, without a response.
                        if super::minimum_character_list::deleting_at(pool, &owner, index)
                            .await
                            .map_err(|_| RuntimeError::Operation)?
                        {
                            tracing::info!("selection of a character waiting for deletion ignored");
                            return Ok(());
                        }
                        let assets = &runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .assets
                            .current();
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
                        runtime.web_shop.insert(
                            selected.cid,
                            super::lab_web_shop::Owner {
                                uid: owner.uid,
                                name: owner.name.to_owned(),
                                revision: owner.revision,
                            },
                        );
                        self.selected = Some(selected);
                        // SelectCharacterProcessor: OpenLog (AddCharacterJournal "Login").
                        if let Some(selected) = self.selected.as_mut() {
                            selected.log_journal =
                                super::character_initialization_logs::open_character_journal(
                                    pool,
                                    selected.cid,
                                )
                                .await;
                        }
                        // The PlayerService pushes of SelectCharacter (StoryEntity
                        // construction, QuestOwner NotifyLevel) reach the client before
                        // CharacterSelecting sends LoginOk on the operation's completion.
                        let mut early = Vec::new();
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
                                    tracing::info!(
                                        synchronized = !packets.is_empty(),
                                        "minimum story login processed; values omitted"
                                    );
                                    early.extend(packets);
                                }
                                Err(_) => {
                                    tracing::warn!("minimum story login rejected; values omitted")
                                }
                            }
                            // QuestOwner construction and NotifyLevel: QuestProgress.
                            if assets.npc.as_ref().is_some_and(|n| n.quests.is_some()) {
                                let clock = runtime.quest_clock();
                                let quests = async {
                                    let n = super::minimum_quest::login_openings(
                                        pool, &owner, assets, cid,
                                    )
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
                                    super::minimum_quest::login(
                                        pool, &owner, assets, cid, ids, clock,
                                    )
                                    .await
                                };
                                match quests.await {
                                    Ok(packets) => {
                                        tracing::info!(
                                            "minimum quest login processed; values omitted"
                                        );
                                        early.extend(packets);
                                    }
                                    Err(_) => tracing::warn!(
                                        "minimum quest login rejected; values omitted"
                                    ),
                                }
                            }
                        }
                        for packet in &early {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        for packet in &self
                            .selected
                            .as_ref()
                            .ok_or(RuntimeError::Operation)?
                            .packets
                        {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        // InGame.OnEnter, after the observers: TriggerDailyQuest
                        // (DailyQuestProcess), TriggerDailyStory (StoryLinePeriodInfo is not
                        // in the profile), QueryStoryProgress and QueryStoryGuide.
                        let mut entered = Vec::new();
                        if assets.npc.is_some() {
                            let cid = self.selected.as_ref().ok_or(RuntimeError::Operation)?.cid;
                            if assets.npc.as_ref().is_some_and(|n| n.quests.is_some()) {
                                match super::minimum_quest::daily_process(
                                    pool,
                                    &owner,
                                    assets,
                                    cid,
                                    runtime.quest_clock(),
                                )
                                .await
                                {
                                    Ok(packets) => entered.extend(packets),
                                    Err(_) => tracing::warn!(
                                        "minimum daily quest process rejected; values omitted"
                                    ),
                                }
                            }
                            match super::minimum_story::in_game_sync(pool, &owner, assets, cid)
                                .await
                            {
                                Ok(packets) => entered.extend(packets),
                                Err(_) => tracing::warn!(
                                    "minimum in-game story sync rejected; values omitted"
                                ),
                            }
                        }
                        for packet in &entered {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        tracing::info!(
                            entered_categories = ?entered.iter().map(|p| p.category).collect::<Vec<_>>(),
                            early_messages = early.len(),
                            early_categories = ?early.iter().map(|p| p.category).collect::<Vec<_>>(),
                            "minimum character selected; optional services deferred; values omitted"
                        );
                        self.schedule_effects(runtime, pool).await;
                        self.daily_items(runtime, pool, stream, sender).await?;
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
                    .assets
                    .current();
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
                    .assets
                    .current();
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
                    .assets
                    .current();
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
                        // PushQuickSlotInfoMessage → UpdateEquipmentObservable: on the ship
                        // the MicroPlayer reloads the battle inventory for the host.
                        if let Some(mission) = self.mission.as_deref_mut() {
                            match super::minimum_mission::refresh_member(
                                pool,
                                &owner,
                                assets,
                                cid,
                                mission,
                                runtime.minimum_header()?,
                                runtime.stat_arithmetic(),
                            )
                            .await
                            {
                                Ok(Some(member)) => {
                                    send(stream, sender, member.category, &member.body).await?
                                }
                                Ok(None) => {}
                                Err(_) => tracing::warn!(
                                    "mission member info refresh rejected; values omitted"
                                ),
                            }
                        }
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
            use crate::protocol::mission::Kind;
            for kind in [
                Kind::AcceptQuest,
                Kind::QueryRecommendShip,
                Kind::RegisterServer,
                Kind::StartGame,
                Kind::MoveToNextSector,
                Kind::AllUserJoinComplete,
                Kind::MonsterKilled,
                Kind::PlayerKilled,
                Kind::RagdollKicked,
                Kind::SectorPropList,
                Kind::PropBroken,
                Kind::PickErg,
                Kind::MonsterDamageReport,
                Kind::CombatRecord,
                Kind::UseConsumable,
                Kind::PlayerRevived,
                Kind::MicroPlayEvent,
                Kind::ReturnToTown,
                Kind::LeaveParty,
            ] {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = crate::protocol::mission::decode_ingress(
                    kind,
                    category,
                    body,
                    DEFAULT_MAX_BODY,
                )
                .map_err(|_| RuntimeError::Message)?
                .ok_or(RuntimeError::Message)?;
                return self.mission(runtime, pool, stream, sender, message).await;
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
                Kind::SetStoryLineStatus,
                Kind::BuyItem,
                Kind::RequestCraft,
                Kind::QueryCashShopProductList,
                Kind::QueryCashShopBalance,
                Kind::QueryCashShopInventory,
                Kind::QueryCashShopItemPickUp,
                Kind::QueryCashShopPurchaseItem,
                Kind::WishListSelect,
                Kind::WishListInsert,
                Kind::WishListDelete,
                Kind::BeautyShopCustomize,
                Kind::RequestSecuredOperation,
                Kind::UseExtraStorage,
                Kind::RequestSortInventory,
                Kind::HotSpringRequestInfo,
                Kind::HotSpringAddPotion,
                Kind::HotSpringRequestPotionEffect,
                Kind::QueryShopItemInfo,
                Kind::SellItem,
                Kind::QueryBuybackList,
                Kind::RepairItem,
                Kind::MaxDurabilityRepairItem,
                Kind::QueryQuestProgress,
                Kind::QueryCharacterCommonInfo,
                Kind::QueryRestoreItemList,
            ] {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                    .ok_or(RuntimeError::Message)?;
                // CashShopServiceProxy answers the catalog and the balance from login on
                // (CashShopPeer needs only the NexonID); the client asks both in the lobby.
                if matches!(
                    message,
                    Message::QueryCashShopProductList | Message::QueryCashShopBalance
                ) {
                    let Ok(uid) = self.frontend.accepted_identity().map(|id| id.nexon_sn) else {
                        self.log_unhandled(category, body);
                        return Ok(());
                    };
                    let assets = &runtime
                        .minimum
                        .as_ref()
                        .ok_or(RuntimeError::Profile)?
                        .assets
                        .current();
                    let packets = match message {
                        Message::QueryCashShopProductList => {
                            super::minimum_cash_shop::product_list(assets)
                        }
                        _ => super::minimum_cash_shop::balance(pool, uid)
                            .await
                            .map(|p| vec![p]),
                    };
                    match packets {
                        Ok(packets) => {
                            for packet in &packets {
                                send(stream, sender, packet.category, &packet.body).await?;
                            }
                            tracing::info!(
                                categories = ?packets.iter().map(|p| p.category).collect::<Vec<_>>(),
                                "minimum cash shop account query answered; values omitted"
                            );
                        }
                        Err(_) => tracing::warn!(
                            "minimum cash shop account query refused; values omitted"
                        ),
                    }
                    return Ok(());
                }
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
                    .assets
                    .current();
                match message {
                    Message::QueryAp => {
                        match super::minimum_ap::query(
                            pool,
                            &owner,
                            assets,
                            cid,
                            runtime.stat_arithmetic(),
                        )
                        .await
                        {
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
                            runtime.stat_arithmetic(),
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
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(super::minimum_story::TALK_ENTITY_IDS)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        let growth = super::minimum_growth::GrowthContext {
                            clock: runtime.quest_clock(),
                            arithmetic: runtime.stat_arithmetic(),
                            config: runtime.minimum_header()?,
                            journal: self.selected.as_ref().map_or(0, |s| s.log_journal),
                        };
                        let session =
                            &mut self.selected.as_mut().ok_or(RuntimeError::Operation)?.story;
                        match super::minimum_story::talk(
                            pool, &owner, assets, cid, session, request, ids, growth,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    messages = packets.len(),
                                    categories = ?packets.iter().map(|p| p.category).collect::<Vec<_>>(),
                                    "minimum NPC talk processed; values omitted"
                                );
                            }
                            Err(_) => tracing::warn!("minimum NPC talk refused; values omitted"),
                        }
                    }
                    // BuyItemProcessor: Count <= 0 fails first; "Buyback_byAP" with
                    // RestoreItem restores from the RestoreStorage.
                    Message::BuyItem(request)
                        if request.count > 0
                            && request
                                .shop_id
                                .as_deref()
                                .is_some_and(|s| s.eq_ignore_ascii_case("buyback_byap"))
                            && assets
                                .enhance
                                .as_ref()
                                .and_then(|c| c.policy())
                                .is_some_and(|p| {
                                    p.is_enabled(Some("RestoreItem")).unwrap_or(false)
                                }) =>
                    {
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(1)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        match super::minimum_restore::restore(
                            pool,
                            &owner,
                            assets,
                            cid,
                            request.order,
                            request.count,
                            ids,
                            runtime.stat_arithmetic(),
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum restore processed; values omitted");
                            }
                            Err(_) => tracing::warn!("minimum restore refused; values omitted"),
                        }
                    }
                    Message::QueryRestoreItemList => {
                        match super::minimum_restore::list(pool, &owner, assets, cid).await {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?;
                                tracing::info!("minimum restore list sent; values omitted");
                            }
                            Err(_) => {
                                tracing::warn!("minimum restore list refused; values omitted")
                            }
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
                    Message::MaxDurabilityRepairItem(request) => {
                        match super::minimum_repair::max_durability_repair(
                            pool,
                            &owner,
                            assets,
                            cid,
                            &request,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    "minimum max durability repair processed; values omitted"
                                );
                            }
                            Err(_) => {
                                tracing::warn!(
                                    "minimum max durability repair refused; values omitted"
                                )
                            }
                        }
                    }
                    Message::QueryQuestProgress => {
                        match super::minimum_quest::query_progress(
                            pool,
                            &owner,
                            assets,
                            cid,
                            runtime.quest_clock(),
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum quest progress pushed; values omitted");
                            }
                            Err(_) => {
                                tracing::warn!("minimum quest progress refused; values omitted")
                            }
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
                    Message::RequestSecuredOperation(operation) => {
                        match super::minimum_cash_shop::secured_operation(
                            pool, &owner, assets, cid, operation,
                        )
                        .await
                        {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?;
                                tracing::info!(
                                    category = packet.category,
                                    operation,
                                    "minimum secured operation answered; values omitted"
                                );
                            }
                            Err(_) => {
                                tracing::warn!("minimum secured operation refused; values omitted")
                            }
                        }
                    }
                    Message::BeautyShopCustomize(request) => {
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(16)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        let outcome = super::minimum_beauty_shop::customize(
                            pool,
                            &owner,
                            assets,
                            cid,
                            &request,
                            ids,
                            runtime.item_log_context(assets)?,
                            runtime.minimum_header()?,
                            runtime.stat_arithmetic(),
                        )
                        .await;
                        // OnFail of the operation: ItemFailMessage "GameUI_Heroes_BeautyShopFail".
                        let customized = match outcome {
                            Ok(c) => c,
                            Err(_) => {
                                tracing::warn!("minimum beauty shop refused; values omitted");
                                super::minimum_beauty_shop::Customized {
                                    packets: vec![
                                        super::minimum_equip::item_fail("BeautyShopFail")
                                            .map_err(|_| RuntimeError::Message)?,
                                    ],
                                    look: None,
                                }
                            }
                        };
                        for packet in &customized.packets {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        if let Some(look) = customized.look {
                            self.set_look(runtime, look).await;
                        }
                        tracing::info!(categories = ?customized.packets.iter().map(|p| p.category).collect::<Vec<_>>(), "minimum beauty shop processed; values omitted");
                    }
                    Message::QueryCashShopInventory => {
                        match super::minimum_cash_shop::inventory(pool, &owner, cid).await {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?
                            }
                            Err(_) => tracing::warn!(
                                "minimum cash shop inventory refused; values omitted"
                            ),
                        }
                    }
                    Message::QueryCashShopPurchaseItem(request) => {
                        match super::minimum_cash_shop::purchase(
                            pool, &owner, assets, cid, &request,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(categories = ?packets.iter().map(|p| p.category).collect::<Vec<_>>(), "minimum cash shop purchase processed; values omitted");
                            }
                            Err(_) => {
                                tracing::warn!("minimum cash shop purchase refused; values omitted")
                            }
                        }
                    }
                    Message::QueryCashShopItemPickUp(request) => {
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(super::minimum_cash_shop::entity_ids_needed(&request))
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        match super::minimum_cash_shop::pick_up(
                            pool,
                            &owner,
                            assets,
                            cid,
                            &request,
                            ids,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(categories = ?packets.iter().map(|p| p.category).collect::<Vec<_>>(), "minimum cash shop pick-up processed; values omitted");
                            }
                            Err(_) => {
                                tracing::warn!("minimum cash shop pick-up refused; values omitted")
                            }
                        }
                    }
                    Message::WishListSelect(request_cid) => {
                        match super::minimum_cash_shop::wish_list(pool, &owner, cid, request_cid)
                            .await
                        {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?
                            }
                            Err(_) => tracing::warn!("minimum wish list refused; values omitted"),
                        }
                    }
                    Message::WishListInsert(request) => {
                        match super::minimum_cash_shop::wish_insert(
                            pool, &owner, assets, cid, &request,
                        )
                        .await
                        {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?
                            }
                            Err(_) => {
                                tracing::warn!("minimum wish list insert refused; values omitted")
                            }
                        }
                    }
                    Message::WishListDelete(request) => {
                        match super::minimum_cash_shop::wish_delete(pool, &owner, cid, &request)
                            .await
                        {
                            Ok(packet) => {
                                send(stream, sender, packet.category, &packet.body).await?
                            }
                            Err(_) => {
                                tracing::warn!("minimum wish list delete refused; values omitted")
                            }
                        }
                    }
                    Message::RequestCraft(request) => {
                        let needed = super::minimum_craft::entity_ids_needed(assets, &request);
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(needed)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        match super::minimum_craft::craft(
                            pool,
                            &owner,
                            assets,
                            cid,
                            &request,
                            ids,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum craft processed; values omitted");
                            }
                            Err(_) => tracing::warn!("minimum craft refused; values omitted"),
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
                    Message::SetStoryLineStatus(request) => {
                        match super::minimum_story::set_story_line_status(
                            pool,
                            &owner,
                            assets,
                            cid,
                            request.story_line.as_deref().unwrap_or(""),
                            request.status,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    synchronized = !packets.is_empty(),
                                    "minimum storyline status processed; values omitted"
                                );
                            }
                            Err(_) => {
                                tracing::warn!("minimum storyline status refused; values omitted")
                            }
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
                            self.selected.as_ref().map_or(0, |s| s.log_journal),
                            runtime.stat_arithmetic(),
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
                        let buyback: Vec<String> =
                            selected.buyback.iter().map(|b| b.class.clone()).collect();
                        let ids = runtime
                            .minimum
                            .as_ref()
                            .ok_or(RuntimeError::Profile)?
                            .creation
                            .entity_ids(16)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                        match super::minimum_use::use_item(
                            pool,
                            &owner,
                            assets,
                            cid,
                            request,
                            runtime.item_log_context(assets)?,
                            runtime.quest_clock(),
                            &buyback,
                            ids,
                        )
                        .await
                        {
                            Ok(super::minimum_use::Outcome::Applied(packets)) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum item use saved and synchronized");
                            }
                            Ok(super::minimum_use::Outcome::AppliedWithSummary(packets)) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                // HomeCafeBonus.OnUseItem: PushCharacterInfo (QueryID 0).
                                let common = super::minimum_selection::common_info(
                                    pool,
                                    &owner,
                                    assets,
                                    cid,
                                    0,
                                    runtime.minimum_header()?,
                                    runtime.stat_arithmetic(),
                                )
                                .await
                                .map_err(|_| RuntimeError::Operation)?;
                                send(stream, sender, common.category, &common.body).await?;
                                tracing::info!("minimum home cafe bonus applied; values omitted");
                                self.schedule_effects(runtime, pool).await;
                                // HomeCafeBonus.OnUseItem: ExecuteDailyItemProcess.
                                self.daily_items(runtime, pool, stream, sender).await?;
                            }
                            Ok(super::minimum_use::Outcome::DyeOpened(packets, session)) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                // ItemOwner.DyeAmpleSession = DyeAmpleSession.From(item).
                                if let Some(selected) = self.selected.as_mut() {
                                    selected.dye = Some(session);
                                }
                                tracing::info!(
                                    "minimum dye ampoule session opened; values omitted"
                                );
                            }
                            Ok(super::minimum_use::Outcome::Unchanged) => {}
                            Err(_) => {
                                tracing::warn!("minimum item use rejected; values omitted")
                            }
                        }
                    }
                    Message::HotSpringRequestInfo(_)
                    | Message::HotSpringAddPotion(_)
                    | Message::HotSpringRequestPotionEffect(_)
                        if !assets
                            .catalog
                            .policy
                            .is_enabled(Some("HotSpring"))
                            .unwrap_or(false) => {}
                    Message::HotSpringRequestInfo(request) => {
                        match super::minimum_hot_spring::request_info(
                            pool,
                            request.channel,
                            request.town_id,
                        )
                        .await
                        {
                            Ok(Some(packet)) => {
                                send(stream, sender, packet.category, &packet.body).await?;
                                tracing::info!("minimum hot spring info sent; values omitted");
                            }
                            Ok(None) => {}
                            Err(_) => {
                                tracing::warn!("minimum hot spring info rejected; values omitted")
                            }
                        }
                    }
                    Message::HotSpringAddPotion(request) => {
                        match super::minimum_hot_spring::add_potion(
                            pool, &owner, assets, cid, &request,
                        )
                        .await
                        {
                            Ok(added) => {
                                // ChannelHotSpringInfo → MMOChannelService: every member of
                                // the channel sees the new potion through the channel runtime.
                                if let Some(packet) = added.channel {
                                    let broadcast = match &runtime.channel {
                                        Some((_, channel)) => {
                                            channel
                                                .broadcast_from(request.channel, cid, packet)
                                                .await
                                        }
                                        None => false,
                                    };
                                    if !broadcast {
                                        tracing::warn!(
                                            "minimum hot spring member not in channel; values omitted"
                                        );
                                    }
                                }
                                // NotifyClient: every frontend client in town (only this
                                // session until B5 adds the client registry).
                                if let Some(packet) = added.announce {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                for packet in added.reply.iter().chain(&added.refresh) {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    "minimum hot spring potion processed; values omitted"
                                );
                            }
                            Err(_) => {
                                tracing::warn!("minimum hot spring potion rejected; values omitted")
                            }
                        }
                    }
                    Message::HotSpringRequestPotionEffect(request) => {
                        match super::minimum_hot_spring::request_effect(
                            pool,
                            &owner,
                            assets,
                            cid,
                            &request,
                            runtime.stat_arithmetic(),
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                if !packets.is_empty() {
                                    self.schedule_effects(runtime, pool).await;
                                }
                                tracing::info!(
                                    "minimum hot spring effect processed; values omitted"
                                );
                            }
                            Err(_) => {
                                tracing::warn!("minimum hot spring effect rejected; values omitted")
                            }
                        }
                    }
                    Message::RequestSortInventory(target) => {
                        // ItemOwner.SortBagInventory/SortCashInventory/SortStoryInventory.
                        if self
                            .latest_sort
                            .is_some_and(|t| t.elapsed() < Duration::from_secs(30))
                        {
                            let dialog = super::minimum_shop::system_message(
                                super::minimum_shop::DIALOG,
                                "GameUI_Heroes_SortFail_Cooling",
                                &[],
                            )
                            .map_err(|_| RuntimeError::Message)?;
                            send(stream, sender, dialog.category, &dialog.body).await?;
                            return Ok(());
                        }
                        match super::minimum_sort::sort(pool, &owner, assets, cid, target).await {
                            Ok(Some(packets)) => {
                                self.latest_sort = Some(std::time::Instant::now());
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(target, "minimum inventory sorted; values omitted");
                            }
                            Ok(None) => {
                                tracing::info!(
                                    target,
                                    "minimum inventory sort refused; values omitted"
                                )
                            }
                            Err(_) => {
                                tracing::warn!("minimum inventory sort rejected; values omitted")
                            }
                        }
                    }
                    Message::UseExtraStorage(request) => {
                        match super::minimum_extra_storage::use_extra_storage(
                            pool, &owner, assets, cid, request,
                        )
                        .await
                        {
                            Ok(super::minimum_extra_storage::Outcome::Applied(packets)) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    storage = request.storage_id,
                                    "minimum extra storage opened; values omitted"
                                );
                            }
                            Ok(super::minimum_extra_storage::Outcome::Refused) => {
                                tracing::info!(
                                    storage = request.storage_id,
                                    "minimum extra storage refused; values omitted"
                                )
                            }
                            Err(_) => {
                                tracing::warn!("minimum extra storage rejected; values omitted")
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
            self.reenter_character_list(runtime, pool, stream, sender)
                .await?;
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
                // ItemServiceProxy: BattleServiceProxy.IsEquipAllowed refuses inside a stage.
                if self.mission.as_ref().is_some_and(|m| !m.equip_allowed()) {
                    let refused = super::minimum_shop::system_message(
                        super::minimum_shop::SYSTEM,
                        "Heroes_System_Item_Error_Cannot_Equip_In_Dungeon",
                        &[],
                    )
                    .map_err(|_| RuntimeError::Message)?;
                    send(stream, sender, refused.category, &refused.body).await?;
                    tracing::info!("minimum equipment change refused inside a stage");
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
                    .assets
                    .current();
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
                        if let Some(mission) = self.mission.as_deref_mut() {
                            match super::minimum_mission::refresh_member(
                                pool,
                                &owner,
                                assets,
                                cid,
                                mission,
                                runtime.minimum_header()?,
                                runtime.stat_arithmetic(),
                            )
                            .await
                            {
                                Ok(Some(member)) => {
                                    send(stream, sender, member.category, &member.body).await?
                                }
                                Ok(None) => {}
                                Err(_) => tracing::warn!(
                                    "mission member info refresh rejected; values omitted"
                                ),
                            }
                        }
                        self.set_look(runtime, look).await;
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
        {
            use crate::protocol::inventory::{self as wire, Kind, Message};
            if matches(
                category,
                body,
                Kind::HideCostume.category(),
                Kind::HideCostume.type_id(),
            ) {
                let Some(Message::HideCostume(request)) =
                    wire::decode_ingress(Kind::HideCostume, category, body, DEFAULT_MAX_BODY)
                        .map_err(|_| RuntimeError::Message)?
                else {
                    return Err(RuntimeError::Message);
                };
                // InGame forwards it only outside a battle; the dungeon SystemMessage
                // is not migrated, so a request there changes nothing.
                let (Some(selected), true, false) = (
                    self.selected.as_ref(),
                    self.password
                        .as_ref()
                        .is_some_and(|p| p.selection_allowed(&self.frontend)),
                    self.mission.is_some(),
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
                    .assets
                    .current();
                match super::minimum_costume::hide_costume(
                    pool,
                    &owner,
                    assets,
                    cid,
                    request,
                    runtime.minimum_header()?,
                    runtime.stat_arithmetic(),
                )
                .await
                {
                    Ok(Some(applied)) => {
                        for packet in &applied.packets {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        self.set_look(runtime, applied.look).await;
                        tracing::info!(
                            hide_head = request.hide_head,
                            "minimum costume visibility saved and synchronized"
                        );
                    }
                    Ok(None) => tracing::info!("minimum costume visibility unchanged"),
                    Err(_) => {
                        tracing::warn!("minimum costume visibility rejected; nothing written")
                    }
                }
                return Ok(());
            }
            for kind in [
                Kind::SkillEnhanceSessionStart,
                Kind::SkillEnhanceSessionEnd,
                Kind::SkillEnhanceUseErg,
                Kind::SkillEnhance,
                Kind::SkillEnhanceUseDurability,
                Kind::SkillEnhanceRestoreDurability,
            ] {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                    .ok_or(RuntimeError::Message)?;
                // InGame forwards the SkillEnhance operations without a second password.
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
                let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
                let assets = &minimum.assets.current();
                let log = runtime.item_log_context(assets)?;
                let ids = minimum
                    .creation
                    .entity_ids(2)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                // SkillEnhanceUseDurabilityFromHost: a MicroPlay outside practice mode
                // forwards it to the player of the slot (only this player here).
                let battle_slot = self
                    .mission
                    .as_ref()
                    .filter(|m| !m.option.is_practice_mode)
                    .map(|m| m.member.slot);
                let state = &mut self
                    .selected
                    .as_mut()
                    .ok_or(RuntimeError::Operation)?
                    .skill_enhance;
                let done = match message {
                    Message::SkillEnhanceSessionStart(request) => {
                        super::minimum_skill_enhance::start(
                            pool, &owner, assets, cid, state, &request,
                        )
                        .await
                    }
                    Message::SkillEnhanceSessionEnd(_) => {
                        super::minimum_skill_enhance::end(state);
                        Ok(Vec::new())
                    }
                    Message::SkillEnhanceUseErg(request) => {
                        super::minimum_skill_enhance::use_erg(
                            pool, &owner, assets, cid, state, &request, log,
                        )
                        .await
                    }
                    Message::SkillEnhanceUseDurability(request) => {
                        if battle_slot != Some(request.slot) {
                            tracing::info!(
                                "minimum skill enhance durability ignored outside the player's battle"
                            );
                            return Ok(());
                        }
                        let uses = request.use_durability.0.unwrap_or_default();
                        super::minimum_skill_enhance::use_durability(
                            pool, &owner, cid, state, &uses, assets,
                        )
                        .await
                    }
                    Message::SkillEnhanceRestoreDurability(request) => {
                        super::minimum_skill_enhance::restore_durability(
                            pool, &owner, assets, cid, state, &request, log,
                        )
                        .await
                    }
                    Message::SkillEnhance(additional) => {
                        super::minimum_skill_enhance::roll(
                            pool,
                            &owner,
                            assets,
                            cid,
                            state,
                            additional.as_deref(),
                            ids,
                            log,
                        )
                        .await
                    }
                    _ => return Err(RuntimeError::Message),
                };
                match done {
                    Ok(packets) => {
                        for packet in &packets {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        tracing::info!(
                            categories = ?packets.iter().map(|p| p.category).collect::<Vec<_>>(),
                            "minimum skill enhance processed; values omitted"
                        );
                    }
                    Err(_) => tracing::warn!("minimum skill enhance refused; nothing written"),
                }
                return Ok(());
            }
            if matches(
                category,
                body,
                Kind::ManufactureCraft.category(),
                Kind::ManufactureCraft.type_id(),
            ) {
                let Some(Message::ManufactureCraft(request)) =
                    wire::decode_ingress(Kind::ManufactureCraft, category, body, DEFAULT_MAX_BODY)
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
                // ItemServiceProxy forwards it directly: no second password and no
                // dungeon check.
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
                let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
                let assets = &minimum.assets.current();
                let ids = minimum
                    .creation
                    .entity_ids(8)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                let log = runtime.item_log_context(assets)?;
                match super::minimum_manufacture::craft(
                    pool, &owner, assets, cid, &request, ids, log,
                )
                .await
                {
                    Ok(packets) => {
                        for packet in &packets {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        tracing::info!(
                            categories = ?packets.iter().map(|p| p.category).collect::<Vec<_>>(),
                            "minimum manufacture craft processed; values omitted"
                        );
                    }
                    Err(_) => tracing::warn!("minimum manufacture craft refused; nothing written"),
                }
                return Ok(());
            }
            if matches(
                category,
                body,
                Kind::EnchantItem.category(),
                Kind::EnchantItem.type_id(),
            ) {
                let Some(Message::EnchantItem(request)) =
                    wire::decode_ingress(Kind::EnchantItem, category, body, DEFAULT_MAX_BODY)
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
                // ItemServiceProxy: only in town (hunting quests are not migrated).
                if self.mission.is_some() {
                    let packet = super::minimum_shop::system_message(
                        0,
                        "Heroes_System_Item_Error_Cannot_Craft_In_Dungeon",
                        &[],
                    )
                    .map_err(|_| RuntimeError::Operation)?;
                    send(stream, sender, packet.category, &packet.body).await?;
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
                let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
                let assets = &minimum.assets.current();
                let ids = minimum
                    .creation
                    .entity_ids(2)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                let log = runtime.item_log_context(assets)?;
                let session = &mut self
                    .selected
                    .as_mut()
                    .ok_or(RuntimeError::Operation)?
                    .enchant;
                match super::minimum_enchant::enchant(
                    pool, &owner, assets, cid, session, &request, ids, log,
                )
                .await
                {
                    Ok(packets) => {
                        for packet in &packets {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        tracing::info!(
                            categories = ?packets.iter().map(|p| p.category).collect::<Vec<_>>(),
                            "minimum enchant processed; values omitted"
                        );
                    }
                    Err(_) => tracing::warn!("minimum enchant refused; nothing written"),
                }
                return Ok(());
            }
            if matches(
                category,
                body,
                Kind::EnhanceItem.category(),
                Kind::EnhanceItem.type_id(),
            ) {
                let Some(Message::EnhanceItem(request)) =
                    wire::decode_ingress(Kind::EnhanceItem, category, body, DEFAULT_MAX_BODY)
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
                // ItemServiceProxy: only in town (hunting quests are not migrated).
                if self.mission.is_some() {
                    let packet = super::minimum_shop::system_message(
                        0,
                        "Heroes_System_Item_Error_Cannot_Craft_In_Dungeon",
                        &[],
                    )
                    .map_err(|_| RuntimeError::Operation)?;
                    send(stream, sender, packet.category, &packet.body).await?;
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
                let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
                let assets = &minimum.assets.current();
                // Enhance_Ferghus_v2 selects EnhanceItemByData; EnhanceItem is not migrated.
                let by_data = assets
                    .enhance
                    .as_ref()
                    .and_then(|c| c.policy())
                    .map(|p| p.is_enabled(Some("Enhance_Ferghus_v2")))
                    .transpose()
                    .map_err(|_| RuntimeError::Operation)?;
                if by_data != Some(true) {
                    self.log_unhandled(category, body);
                    return Ok(());
                }
                let ids = minimum
                    .creation
                    .entity_ids(4)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                match super::minimum_enhance::enhance(
                    pool,
                    &owner,
                    assets,
                    cid,
                    &request,
                    ids,
                    runtime.item_log_context(assets)?,
                    runtime.minimum_header()?,
                )
                .await
                {
                    Ok(enhanced) => {
                        let packets = enhanced.packets;
                        for packet in &packets {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        if let Some(look) = enhanced.look {
                            self.set_look(runtime, look).await;
                        }
                        tracing::info!(
                            categories = ?packets.iter().map(|p| p.category).collect::<Vec<_>>(),
                            "minimum enhance processed; values omitted"
                        );
                    }
                    Err(_) => tracing::warn!("minimum enhance refused; nothing written"),
                }
                return Ok(());
            }
        }
        if self
            .dye_request(runtime, pool, stream, sender, category, body)
            .await?
            .is_some()
        {
            return Ok(());
        }
        if self.selected.is_some() {
            self.log_unhandled(category, body);
            return Ok(());
        }
        if self
            .character_list_change(runtime, pool, stream, sender, category, body)
            .await?
            .is_some()
        {
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
                let assets = m.assets.current();
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
                            assets: Arc::clone(&assets),
                            body: bytes,
                            offset: runtime.profile.civil_offset_seconds(),
                            config: runtime.minimum_header()?,
                            delete_delay: assets
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
    /// CharacterList(sendList: true): InitSecondPassword before CharacterListMessage. Once
    /// passed it sends HasSecondPasswordMessage(isFirstQuery false, State != NoPassword,
    /// IsSecondPasswordPassed, 0, null) and SendDifferentIPMessage (nothing with enEU). The
    /// client waits for it: without it Start does nothing (operator report of 2026-10-26).
    async fn reenter_character_list(
        &self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        let password = self.password.as_ref().ok_or(RuntimeError::Operation)?;
        let revision = password.revision();
        let state = password.state();
        send_password(
            stream,
            sender,
            PasswordMessage::Has(HasPassword {
                first: false,
                has_password: state != crate::domain::second_password::ClientState::NoPassword,
                passed: state.passed(true),
                fail_count: 0,
                retry_seconds: -1,
            }),
        )
        .await?;
        let list = runtime
            .character_list(&self.frontend, pool, revision)
            .await?;
        send(stream, sender, empty_character_list::CATEGORY, &list).await
    }
    /// InGame DyeAmpleRequestMessage → RequestDyeAmple on the ItemService. `None` when the
    /// message is not one.
    async fn dye_request(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
    ) -> Result<Option<()>, RuntimeError> {
        use crate::protocol::dye::{self as wire, Kind, Message};
        let kind = Kind::Request;
        if !matches(category, body, kind.category(), kind.type_id()) {
            return Ok(None);
        }
        let Some(Message::Request {
            x,
            y,
            is_avatar,
            color_value,
        }) = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
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
            return Ok(Some(()));
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
            .assets
            .current();
        let request = super::minimum_dye::Request {
            x,
            y,
            is_avatar,
            color_value,
        };
        match super::minimum_dye::request(
            pool,
            &owner,
            assets,
            cid,
            selected.dye.as_ref(),
            request,
            runtime.item_log_context(assets)?,
        )
        .await
        {
            Ok(packets) => {
                for packet in &packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                tracing::info!("minimum dye ampoule colored and synchronized; values omitted");
            }
            Err(_) => tracing::warn!("minimum dye ampoule request refused; nothing written"),
        }
        Ok(Some(()))
    }
    /// CharacterList DeleteCharacter / DeleteCharacterCancel / SelectOrdersChange. `None` when
    /// the message is not one of them.
    async fn character_list_change(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
    ) -> Result<Option<()>, RuntimeError> {
        use super::minimum_character_delete as delete;
        use crate::protocol::character_delete::{self as wire, DeleteResult, Kind, Message};
        let Some(kind) = [Kind::Delete, Kind::Cancel, Kind::SelectOrders]
            .into_iter()
            .find(|k| matches(category, body, k.category(), k.type_id()))
        else {
            return Ok(None);
        };
        // CharacterList.OnMessageReceive returns before any handler until the second password
        // passed; the frontend answers the Locked dialog as for SelectCharacter.
        if !self
            .password
            .as_ref()
            .is_some_and(|p| p.selection_allowed(&self.frontend))
        {
            locked(stream, sender).await?;
            return Ok(Some(()));
        }
        // A null message is not `is DeleteCharacterMessage`: no handler runs.
        let Some(message) = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
            .map_err(|_| RuntimeError::Message)?
        else {
            return Ok(Some(()));
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
        let reply = |m: Message| {
            wire::encode(m.kind(), Some(&m), DEFAULT_MAX_BODY).map_err(|_| RuntimeError::Message)
        };
        // CharacterProcessing.MoveToList(success) enters CharacterList(sendList: success).
        let relist = match message {
            Message::Delete { name, character_sn } => {
                let result = |result: DeleteResult, remain_time_sec: i32| {
                    reply(Message::DeleteResult {
                        character_sn,
                        result: result as i32,
                        remain_time_sec,
                    })
                };
                // GetCharacterSummary(Name); without it the source answers Fail_NoSuchCharacter
                // and its CashShop branch stops on the null summary.
                let Some(cid) = delete::listed_named(pool, &owner, name.as_deref())
                    .await
                    .map_err(|_| RuntimeError::Operation)?
                else {
                    let bytes = result(DeleteResult::FailNoSuchCharacter, 0)?;
                    send(stream, sender, Kind::DeleteResult.category(), &bytes).await?;
                    tracing::info!("character delete refused: not listed; values omitted");
                    return Ok(Some(()));
                };
                // The laboratory overrides CashShop on: QueryCashShopInventory of the character.
                match delete::remain_cash_items(pool, &owner, cid).await {
                    Ok(false) => {}
                    Ok(true) => {
                        let bytes = result(DeleteResult::FailRemainCashItems, 0)?;
                        send(stream, sender, Kind::DeleteResult.category(), &bytes).await?;
                        tracing::info!("character delete refused: cash items remain");
                        return Ok(Some(()));
                    }
                    Err(_) => {
                        let bytes = result(DeleteResult::FailExcuteOp, 0)?;
                        send(stream, sender, Kind::DeleteResult.category(), &bytes).await?;
                        tracing::warn!("character delete cash inventory query failed");
                        return Ok(Some(()));
                    }
                }
                let assets = runtime
                    .minimum
                    .as_ref()
                    .ok_or(RuntimeError::Profile)?
                    .assets
                    .current();
                let delay = assets
                    .initializer
                    .policy()
                    .get_integer("DeleteCharacterDelay")?;
                let (outcome, wait) =
                    delete::delete(pool, &owner, name.as_deref(), character_sn, delay, || {
                        lab_clock::utc_now()
                            .map_err(|_| super::minimum_character::MinimumError::Conflict)
                    })
                    .await;
                let bytes = result(outcome, wait)?;
                send(stream, sender, Kind::DeleteResult.category(), &bytes).await?;
                tracing::info!(
                    result = outcome as i32,
                    "character delete processed; values omitted"
                );
                true
            }
            Message::Cancel { name, character_sn } => {
                let outcome = delete::cancel(pool, &owner, name.as_deref(), character_sn).await;
                let bytes = reply(Message::CancelResult {
                    character_sn,
                    result: outcome as i32,
                })?;
                send(stream, sender, Kind::CancelResult.category(), &bytes).await?;
                tracing::info!(
                    result = outcome as i32,
                    "character delete cancel processed; values omitted"
                );
                true
            }
            Message::SelectOrders(orders) => {
                // SelectOrdersChange has no OnFail: a failed change sends nothing.
                match delete::select_orders(pool, &owner, orders.as_deref().unwrap_or_default())
                    .await
                {
                    Ok(true) if orders.is_some() => {
                        tracing::info!("character slot order changed; values omitted");
                        true
                    }
                    Ok(_) => {
                        tracing::info!("character slot order refused; nothing written");
                        false
                    }
                    Err(_) => {
                        tracing::warn!("character slot order failed; nothing written");
                        false
                    }
                }
            }
            _ => return Err(RuntimeError::Message),
        };
        if relist {
            self.reenter_character_list(runtime, pool, stream, sender)
                .await?;
        }
        Ok(Some(()))
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
    let peer =
        super::lab_peer::effective_peer(stream.peer_addr()?.ip()).ok_or(RuntimeError::Profile)?;
    let mut state = Connection::new(&runtime, peer);
    let mut notices = runtime.maintenance().notices();
    let result=async {
  let b=uuid::Uuid::new_v4().into_bytes();let salt=u32::from_be_bytes([b[0],b[1],b[2],b[3]])&0x7fffffff;
  // The pinned original config sets NGM_Disable=1. The laboratory replaces the NISMS cash
  // shop, so it also overrides CashShop (empty in en-EU-x-GM) with the en-EU value
  // (operator decision 2026-10-24), as a FeatureMatrix override of the original config would.
  // InGameCashShopDialog_V2 downloads its product list from ingame_cashshop_shopListURL; the
  // original server published a static ShopList.aspx, here served by the bootstrap HTTP
  // (scripts/dev/lab-export-shop-list.ps1) instead of the unreachable 192.168.0.200. Its
  // purchases go to ingame_cashshop_purchaseURL, the laboratory web shop of this process.
  // S3_CharacterCreation (NULL in every region) opens the NewCharacterDialogS3 class grid, which
  // lists Delia besides the ten classes of the V4 bar; the lab client.dll runs it as the NA's
  // CharacterCreation_UI_V3 flow (scripts/client/patch_class_routing.py, CREATION_GRID).
  let features=FeatureMatrix{features:Some(vec![FeatureEntry{key:"NGM_disable",value:Some("1")},FeatureEntry{key:"CashShop",value:Some("1.04.08")},FeatureEntry{key:"ingame_cashshop_shopListURL",value:Some(LAB_SHOP_LIST_URL)},FeatureEntry{key:"ingame_cashshop_purchaseURL",value:Some(super::lab_web_shop::PURCHASE_URL)},FeatureEntry{key:"S3_CharacterCreation",value:Some("3.09.00")}])};
  let (mut sender,packets)=encode_initial_packets(salt,Some(&features),super::minimum_cash_shop::CATALOG_MAX_BODY,1024).map_err(|_|RuntimeError::Message)?;
  for bytes in packets{tokio::time::timeout(WRITE_TIMEOUT,stream.write_all(&bytes)).await.map_err(|_|RuntimeError::Timeout)??;}
  let mut receiver=LegacyReceiver::with_profile(CipherProfile::VindictusFrontend,DEFAULT_MAX_BODY);let mut buffer=Zeroizing::new([0u8;4096]);
  loop {
   let deadline=state.close_at.unwrap_or(state.login_deadline);let waiting=state.close_at.is_some()||state.frontend.phase()!=SessionPhase::PoliciesAccepted;
   let timer=state.mission_timer();
   let effects=state.effect_expiry;
   let daily=state.daily_at;
   let count=tokio::select!{
    _=shutdown.changed()=>return Ok(()),
    _=tokio::time::sleep_until(deadline),if waiting=>return if state.close_at.is_some(){Ok(())}else{Err(RuntimeError::Timeout)},
    _=tokio::time::sleep_until(timer.unwrap_or(deadline)),if timer.is_some()&&state.close_at.is_none()=>{state.mission_jobs(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    _=tokio::time::sleep_until(effects.unwrap_or(deadline)),if effects.is_some()&&state.close_at.is_none()=>{state.effect_jobs(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    _=tokio::time::sleep_until(daily.unwrap_or(deadline)),if daily.is_some()&&state.close_at.is_none()=>{state.daily_items(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    notice=notices.recv()=>{
     // ShutDownProcessor.SendNoticeShutDownMessage: every client, NoticeShutDownMessage{Delay}.
     match notice{
      Ok(delay)=>{let body=ctl::encode(ControlKind::NoticeShutDown,Some(&ControlMessage::NoticeShutDown(delay)),DEFAULT_MAX_BODY).map_err(|_|RuntimeError::Message)?;send(&mut stream,&mut sender,ControlKind::NoticeShutDown.category(),&body).await?;}
      Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>{}
      Err(tokio::sync::broadcast::error::RecvError::Closed)=>return Ok(()),
     }
     continue;
    }
    value=stream.read(&mut buffer[..])=>value?,
   };
   if count==0{return if receiver.buffered_len()==0{Ok(())}else{Err(RuntimeError::Message)}}
   let frames=receiver.push(&buffer[..count]).map_err(|_|{tracing::warn!("laboratory frontend frame rejected by the packet layer");RuntimeError::Message})?;
   for mut frame in frames{
    let body=Zeroizing::new(std::mem::take(&mut frame.body));
    let initial=state.frontend.phase()==SessionPhase::Initial;
    let deadline=state.login_deadline;
    // Diagnostics for an in-game message the decoders reject: category and size only.
    let category=frame.category_id;
    let registered=super::channel_server::registered_category(category,&body);
    let in_game=state.selected.is_some();
    let length=body.len();
    // Bounded hex of a small rejected body; second-password messages never.
    let secret=[PasswordKind::Check,PasswordKind::Set].iter().any(|k|matches(category,&body,k.category(),k.type_id()));
    let prefix=if secret||length>64{String::new()}else{body.iter().map(|b|format!("{b:02x}")).collect()};
    let future=state.dispatch(&runtime,&pool,&mut stream,&mut sender,category,&body);
    if initial {
     tokio::time::timeout_at(deadline,future).await.map_err(|_|RuntimeError::Timeout)??;
    }else{
     match future.await{
      // One undecodable in-game message must not drop the character; no response is sent.
      Err(RuntimeError::Message) if in_game=>tracing::warn!(category,registered_category=registered,length,body_hex=%prefix,"laboratory frontend in-game message rejected; ignored"),
      other=>other?,
     }
    }
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
    if !super::lab_peer::admitted_listener(listener.local_addr()?) {
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
