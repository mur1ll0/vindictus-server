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
    /// The parties of this frontend (MicroPlayService party authority, B5).
    party: Arc<super::party_hub::PartyHub>,
    /// The online members of each guild (GuildEntity presence, B5 social).
    guilds: Arc<super::guild::GuildHub>,
    /// PvpService matches hosted by a player through the relay (B5 PV2c).
    pvp: Arc<super::pvp_match::PvpHub>,
    /// Passports of the accepted logins for the messenger's LoginAuth (B5 MS3b).
    passports: Arc<super::messenger::Passports>,
    /// RankTitleManager.highRankList (B5 RK3).
    rank_titles: Arc<super::rank_title::HighRanks>,
    web_shop: super::lab_web_shop::Sessions,
    maintenance: super::maintenance::Maintenance,
    content_output: Option<std::path::PathBuf>,
    /// The panel's event schedule (AdminClient EventDataContext): overrides, drop
    /// modifiers and running events, applied on every content reload.
    schedule: Arc<std::sync::Mutex<crate::domain::event_schedule::State>>,
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
        let party = Arc::new(
            super::party_hub::PartyHub::new(
                p.is_enabled(Some("ForceClosePartyInShip"))?,
                p.is_enabled(Some("CombatRejoinSystem"))?,
            )
            .with_rejoin_limit_minutes(
                p.get_integer("CombatRejoinSystem_LimitMinutes")
                    .unwrap_or(0),
            ),
        );
        let guilds = Arc::new(super::guild::GuildHub::new(
            super::guild::GuildConfig::from_policy(p).map_err(|_| RuntimeError::Profile)?,
        ));
        let passports = Arc::new(super::messenger::Passports::default());
        let pvp = Arc::new(super::pvp_match::PvpHub::new(
            Arc::clone(&party),
            Arc::clone(&guilds),
        ));
        Ok(Self {
            profile,
            content,
            name_proof: None,
            minimum: None,
            channel: None,
            relay: None,
            party,
            guilds,
            pvp,
            passports,
            rank_titles: Arc::new(super::rank_title::HighRanks::new()),
            web_shop: super::lab_web_shop::Sessions::default(),
            maintenance: super::maintenance::Maintenance::default(),
            content_output: None,
            schedule: Default::default(),
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
        self.pvp.set_relay(self.relay.clone());
        let assets = assets.with_item_arithmetic(self.stat_arithmetic());
        // The guild values with the panel's published tuning (Ajustes de jogo → Guilda).
        if let Some(policy) = assets.npc_policy() {
            self.guilds.set_config(
                super::guild::GuildConfig::from_policy(policy)
                    .map_err(|_| RuntimeError::Profile)?,
            );
        }
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
        let mut overlay = overlay;
        {
            let schedule = self
                .schedule
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            overlay.scheduled = schedule.overrides.clone();
            overlay.drop_modifiers = schedule.drop_modifiers.clone();
        }
        let assets = tokio::task::spawn_blocking(move || {
            super::minimum_assets::Assets::read_with_overlay(&source, &overlay)
                .map(|a| a.with_item_arithmetic(arithmetic))
        })
        .await
        .map_err(|_| RuntimeError::Operation)?
        .map_err(|_| RuntimeError::Profile)?;
        if let Some(policy) = assets.npc_policy() {
            self.guilds.set_config(
                super::guild::GuildConfig::from_policy(policy)
                    .map_err(|_| RuntimeError::Profile)?,
            );
        }
        minimum.assets.replace(assets);
        Ok(())
    }
    /// The event schedule's state (the scheduler task and the panel).
    pub fn schedule(&self) -> Arc<std::sync::Mutex<crate::domain::event_schedule::State>> {
        Arc::clone(&self.schedule)
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
    /// The passports of accepted logins (the messenger listener's authentication).
    pub fn passports(&self) -> Arc<super::messenger::Passports> {
        Arc::clone(&self.passports)
    }
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
    pub(crate) fn quest_clock(&self) -> super::minimum_quest::QuestClock {
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
/// AddPlayer's relay side: AddClientWaiting, then (RelayClient Connected) P2PGroup.Join of
/// the play's group with the party as parent; the member's acknowledgement completes it.
fn relay_ticket(
    relay: &super::relay::Relay,
    group: i32,
    cid: i64,
    nexon_sn: i32,
    party_id: i64,
) -> super::minimum_mission::RelayTicket {
    use super::minimum_mission::MICRO_PLAY_CATEGORY;
    use super::relay::RelayEvent;
    // Subscribe before registering, so Connected cannot be missed.
    let mut events = relay.subscribe();
    let joiner = relay.clone();
    tokio::spawn(async move {
        let joined = tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                match events.recv().await {
                    Ok(RelayEvent::Connected { cid: c, .. }) if c == cid => {
                        // MicroPlayer.RelayClient_StateChanged → P2PGroup.Join.
                        if !joiner.join(group, cid, party_id, MICRO_PLAY_CATEGORY) {
                            return false;
                        }
                    }
                    Ok(RelayEvent::Joined { cid: c, .. }) if c == cid => return true,
                    Ok(RelayEvent::Closed { cid: c }) if c == cid => return false,
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
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
    super::minimum_mission::RelayTicket {
        address: relay.endpoint(),
        group,
        sn: waiting.sn,
        key: waiting.key,
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
/// The registered category, or category 0 with the type's GUID envelope; an unregistered
/// type (category 0) matches only by its GUID.
fn matches(category: i32, body: &[u8], typed: i32, type_id: uuid::Uuid) -> bool {
    (typed != 0 && category == typed)
        || (category == 0 && body.get(..16) == Some(type_id.as_bytes().as_slice()))
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
    /// PetManager.UpdateSchedule: the next period start of a pet's active time.
    pet_reset_at: Option<tokio::time::Instant>,
    /// MailBox.TimeLine of the selected character and its next 30-second polling.
    mail_timeline: super::minimum_mail::TimeLine,
    mail_at: Option<tokio::time::Instant>,
    /// PeriodicEventScheduler: the next BoostEventPeriod timeline of the character.
    boost_at: Option<tokio::time::Instant>,
    /// StoryEntity.AddToUpdateSchedule: the next StoryLinePeriodInfo boundary.
    story_at: Option<tokio::time::Instant>,
    /// AllUserGoalEventScheduler: the next AllUserGoalEventModify push and the last time
    /// (Unix seconds) pushes were sent up to.
    goal_at: Option<tokio::time::Instant>,
    goal_sent: i64,
    /// ItemOwner.TradeSearchOverheadChecker of the selected character.
    trade_search: crate::domain::marketplace::SearchOverhead,
    /// ItemOwner.TradeProxy: created (GetMyThings) by the first ProcessDailyItem.
    trade_proxy: bool,
    /// The party authority's pushes to the entered character (UpdateParty, Kick, ...).
    party_notices: Option<tokio::sync::mpsc::Receiver<super::party_hub::Notice>>,
    /// A host message replayed into this member's MicroPlay replica.
    replaying: bool,
    /// The GuildServiceProxy state of the entered character.
    guild: super::guild::State,
    /// TitleManager.RankTitles of the entered character (B5 RK3).
    rank_titles: Vec<i32>,
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
            pet_reset_at: None,
            mail_timeline: super::minimum_mail::TimeLine::default(),
            mail_at: None,
            boost_at: None,
            story_at: None,
            goal_at: None,
            goal_sent: 0,
            trade_search: Default::default(),
            trade_proxy: false,
            party_notices: None,
            guild: super::guild::State::Initial,
            rank_titles: Vec::new(),
            replaying: false,
        }
    }
    /// PlayerConn.Close: the character leaves the channel and its session ends.
    async fn release_selected(&mut self, runtime: &Runtime, pool: &PgPool) {
        // LeaveParty: the party and MicroPlay of this connection end with it and it leaves
        // the relay group. A host leaving a play in progress hands it to another player
        // (host election); a member keeps its slot for CombatRejoinSystem.
        if let Some(mut m) = self.mission.take() {
            self.leave_pets(runtime, pool, &mut m).await;
            let elected = self.leave_play(runtime, &m);
            if super::minimum_mission::play_open(&m) && !elected && runtime.party.combat_rejoin() {
                runtime
                    .party
                    .remember_disconnect(m.completion.cid, m.party_id, m.member.slot);
                let hub = runtime.party.clone();
                let (party_id, cid) = (m.party_id, m.completion.cid);
                tokio::spawn(async move {
                    tokio::time::sleep(hub.rejoin_limit()).await;
                    hub.expire_rejoin(party_id, cid);
                });
            }
        }
        self.effect_expiry = None;
        self.daily_at = None;
        self.pet_reset_at = None;
        self.mail_at = None;
        // PeriodicEventScheduler.OnLogOut.
        self.boost_at = None;
        self.story_at = None;
        self.goal_at = None;
        self.goal_sent = 0;
        self.mail_timeline.clear();
        self.trade_search = Default::default();
        self.trade_proxy = false;
        self.party_notices = None;
        if let Some(selected) = self.selected.as_ref() {
            // PvpServiceProxy.Close: UnregisterPvpPlayer.
            runtime.pvp.remove(selected.cid);
            // The frontend connection closed: RemoveMember("FrontendDisconnected").
            runtime.party.unregister(selected.cid);
            // The PlayerConn closed: OnlineGuildMember leaves its GuildEntity.
            super::guild::leave(pool, &runtime.party, &runtime.guilds, selected.cid).await;
        }
        self.guild = super::guild::State::Initial;
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
        // reserves no session and touches no game state. A launcher ticket (trilha L, the client
        // started with -launcher_ticket) is spent once and enters its account's game name; a
        // ticket that does not redeem is refused, never read as a typed name.
        let ticket = message
            .passport
            .and_then(crate::domain::admin::LauncherTicket::parse);
        let account_game = match (&ticket, message.passport) {
            (Some(ticket), _) => {
                super::game_links::redeem_launcher_ticket(pool, &ticket.digest(), self.peer)
                    .await
                    .ok()
                    .flatten()
                    .map(Zeroizing::new)
            }
            (None, Some(passport)) => match crate::domain::Username::parse(passport) {
                Ok(name) => super::game_links::game_name_for_login(pool, name.normalized())
                    .await
                    .ok()
                    .flatten()
                    .map(Zeroizing::new),
                Err(_) => None,
            },
            (None, None) => None,
        };
        let passport = account_game
            .as_ref()
            .map_or(message.passport, |g| Some(g.as_str()));
        let admitted = account_game.is_some()
            || (ticket.is_none()
                && runtime.profile.permits(passport)
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
        // The messenger's LoginAuth carries the passport the client logged in with.
        if let (Some(raw), Ok(id)) = (message.passport, self.frontend.accepted_identity()) {
            runtime.passports.remember(
                raw,
                super::messenger::Account {
                    nexon_sn: id.nexon_sn,
                    nexon_id: id.nexon_id.to_string(),
                },
                Instant::now(),
            );
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
        use super::minimum_mission::{self as mission, Accept};
        use crate::protocol::mission::Message;
        // The summary pet after RemovePlayer's EndBattle, for PushUpdatePetStatus.
        let mut left_pet = None;
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
        // PvpServiceProxy Hosting/Member: RegisterServerMessage is the PvP host's game info
        // (RegisterHostGameInfo); a failure unregisters (UnRegistering).
        if let (Message::RegisterServer(register), Some(cid)) =
            (&message, self.selected.as_ref().map(|s| s.cid))
            && let Some(ok) = runtime.pvp.register_server(cid, register.info.clone())
        {
            if !ok {
                runtime.pvp.remove(cid);
            }
            tracing::info!(ok, "pvp host game info processed; values omitted");
            return Ok(());
        }
        // LockPartyProxy: while registered in PvP the party stays as it is.
        if matches!(
            message,
            Message::RequestJoinParty(_)
                | Message::PartyInvite(_)
                | Message::PartyInvitationAccept(_)
                | Message::Kick(_)
                | Message::LeaveParty
        ) && self
            .selected
            .as_ref()
            .is_some_and(|s| runtime.pvp.locks_party(s.cid))
        {
            tracing::info!("party operation during pvp ignored; values omitted");
            return Ok(());
        }
        // MicroPlayHost/ShipHost messages: a MicroPlayMember's frontend does not forward them.
        let host_only = matches!(
            message,
            Message::RegisterServer(_)
                | Message::StartGame
                | Message::MoveToNextSector(_)
                | Message::AllUserJoinComplete
                | Message::MonsterKilled(_)
                | Message::PlayerKilled(_)
                | Message::RagdollKicked(_)
                | Message::SectorPropList(_)
                | Message::PropBroken(_)
                | Message::PickErg(_)
                | Message::MonsterDamageReport(_)
                | Message::CombatRecord(_)
                | Message::UseConsumable(_)
                | Message::PlayerRevived(_)
                | Message::MicroPlayEvent(_)
        );
        if host_only && !self.replaying && self.mission.as_ref().is_some_and(|m| !m.host) {
            tracing::info!("host message from a MicroPlay member ignored; values omitted");
            return Ok(());
        }
        // The host's play messages that every MicroPlayer applies: the other players'
        // replicas get them (EXP, cores, goals, deaths and consumables of each player).
        if !self.replaying
            && matches!(
                message,
                Message::MoveToNextSector(_)
                    | Message::MonsterKilled(_)
                    | Message::RagdollKicked(_)
                    | Message::CombatRecord(_)
                    | Message::UseConsumable(_)
                    | Message::PlayerKilled(_)
                    | Message::PlayerRevived(_)
                    | Message::MicroPlayEvent(_)
            )
            && let Some(m) = self.mission.as_ref().filter(|m| m.host)
        {
            runtime.party.replicate(m.party_id, cid, &message);
        }
        let packets = match message {
            Message::AnswerFinishQuest(follow) => {
                // MicroPlayMember AnswerFinishQuest → DecideLeaveWithHost.
                let Some(m) = self.mission.as_ref().filter(|m| !m.host) else {
                    return Ok(());
                };
                runtime
                    .party
                    .decide_leave_with_host(m.party_id, cid, follow)
                    .map_err(|_| RuntimeError::Operation)?
            }
            Message::OpenPartyWithShipInfo(open) => {
                // Initial: OpenNewParty then RegisterShipInfo(UpdateShipList); PartyMaster:
                // RegisterShipInfo of its party. GeneralFail("OpenPartyWithShipInfo") leaves
                // the party Initial opened.
                if self.mission.is_some() {
                    return Ok(());
                }
                let hub = runtime.party.clone();
                let mut packets = Vec::new();
                let (party_id, initial) = match hub.role(cid) {
                    super::party_hub::Role::Member { .. } => return Ok(()),
                    super::party_hub::Role::Master { party_id, .. } => (party_id, false),
                    super::party_hub::Role::None => {
                        let me = super::minimum_party::member(
                            pool,
                            &owner,
                            assets,
                            cid,
                            runtime.minimum_header()?,
                            runtime.stat_arithmetic(),
                        )
                        .await
                        .map_err(|_| RuntimeError::Operation)?;
                        let party_id = mission::entity_id().map_err(|_| RuntimeError::Operation)?;
                        packets.extend(
                            hub.open(party_id, me)
                                .map_err(|_| RuntimeError::Operation)?,
                        );
                        (party_id, true)
                    }
                };
                match mission::register_town_ship(
                    pool,
                    &owner,
                    assets,
                    cid,
                    &open,
                    hub.clone(),
                    party_id,
                    runtime.minimum_header()?,
                    runtime.stat_arithmetic(),
                )
                .await
                {
                    Ok(Ok(more)) => {
                        packets.extend(more);
                        tracing::info!("town ship opened; values omitted");
                    }
                    Ok(Err(error)) => {
                        if initial {
                            hub.leave(cid).map_err(|_| RuntimeError::Operation)?;
                            packets.push(
                                mission::packet(&Message::Kicked)
                                    .map_err(|_| RuntimeError::Message)?,
                            );
                        }
                        packets.push(
                            super::minimum_party::error_message(
                                true,
                                "OpenPartyWithShipInfo",
                                error,
                            )
                            .map_err(|_| RuntimeError::Message)?,
                        );
                    }
                    Err(_) => {
                        if initial {
                            hub.end_for(cid);
                        }
                        tracing::warn!("town ship rejected; values omitted");
                        return Ok(());
                    }
                }
                packets
            }
            Message::PartyChat(chat)
                if runtime
                    .pvp
                    .proxy(cid)
                    .is_some_and(|p| p != super::pvp_match::Proxy::Waiting) =>
            {
                // InGame: IsInPvp → PvpChat(IsTeamChat = !isFakeTownChat).
                runtime.pvp.chat(cid, chat.message.as_deref().unwrap_or(""), !chat.is_fake_town_chat);
                tracing::info!("pvp chat processed; values omitted");
                Vec::new()
            }
            Message::PartyChat(chat) => {
                // InGame PartyChatMessage → BattleServiceProxy.SendPartyChat (not Initial).
                let packets = runtime
                    .party
                    .chat(cid, chat.message.as_deref().unwrap_or(""))
                    .map_err(|_| RuntimeError::Operation)?;
                tracing::info!(
                    sent = !packets.is_empty(),
                    "party chat processed; values omitted"
                );
                packets
            }
            Message::TeacherRequest => {
                // MicroPlayHost TeacherRequest: a level-1 solo host asks for a teacher. The
                // recovered TeacherFinder never has a candidate (AddCandidate is never
                // called), so the session fails at once with TeacherRequestResult(false).
                let Some(m) = self.mission.as_ref().filter(|m| m.host && m.in_game) else {
                    return Ok(());
                };
                let text = if m.member.level > 1 {
                    Some("#TEACHER_ASSIST_LEVEL_EXCEED")
                } else if runtime.party.member_count(m.party_id) > 1 {
                    Some("#TEACHER_ASSIST_CANNOT_MULTIPLAY")
                } else {
                    None
                };
                let packet = match text {
                    Some(text) => super::minimum_shop::system_message(4, text, &[]),
                    None => mission::packet(&Message::TeacherRequestResult(
                        crate::protocol::mission::TeacherRequestResult {
                            accepted: false,
                            accepted_user_name: Some(String::new()),
                        },
                    )),
                }
                .map_err(|_| RuntimeError::Message)?;
                vec![packet]
            }
            Message::TeacherRequestRespond(_) => {
                // InGame TeacherRequestRespond without a WaitingSession.
                vec![
                    super::minimum_shop::system_message(4, "#TEACHER_ASSIST_NO_REQUEST", &[])
                        .map_err(|_| RuntimeError::Message)?,
                ]
            }
            Message::AcceptAssist(slot) => {
                // MicroPlayHost AcceptAssist → SetMemberState(AssistAccepted); the town
                // master states ignore it.
                let Some(m) = self.mission.as_ref().filter(|m| m.host && m.in_game) else {
                    return Ok(());
                };
                match runtime
                    .party
                    .set_member_state(
                        m.party_id,
                        slot,
                        crate::domain::party::ready_state::ASSIST_ACCEPTED,
                        cid,
                    )
                    .map_err(|_| RuntimeError::Operation)?
                {
                    Ok(packets) => packets,
                    Err(_) => return Ok(()),
                }
            }
            Message::RequestJoinParty(request) => {
                // Initial.RejoinCombat (CombatRejoinSystem).
                if !runtime.party.combat_rejoin()
                    || runtime.party.role(cid) != super::party_hub::Role::None
                {
                    return Ok(());
                }
                if request <= 0 {
                    // DeleteDisconnectedUserInfo + RemoveMicroPlayEntityInfo.
                    runtime.party.forget_disconnect(cid);
                    return Ok(());
                }
                if !super::party_hub::rejoinable(cid) {
                    return Ok(());
                }
                let me = super::minimum_party::member(
                    pool,
                    &owner,
                    assets,
                    cid,
                    runtime.minimum_header()?,
                    runtime.stat_arithmetic(),
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
                let mut packets = match runtime.party.rejoin(me) {
                    Ok(Some(packets)) => packets,
                    _ => {
                        runtime.party.forget_disconnect(cid);
                        vec![
                            super::minimum_party::error_message(
                                true,
                                "RequestJoinParty",
                                "ProcessingOtherOperation",
                            )
                            .map_err(|_| RuntimeError::Message)?,
                        ]
                    }
                };
                packets.push(
                    mission::packet(&Message::RejoinCombatSuccess)
                        .map_err(|_| RuntimeError::Message)?,
                );
                tracing::info!("combat rejoin requested; values omitted");
                packets
            }
            Message::JoinShip(join) => {
                // Initial (the boat's list): JoinParty(ShipID), then JoinMicroPlay unless
                // IsInTownWithShipInfo. PartyMember: JoinMicroPlay. Assist joins are
                // unreachable in the recovered server (no teacher is ever found).
                let mut joined_party = Vec::new();
                if runtime.party.role(cid) == super::party_hub::Role::None {
                    if self.mission.is_some() || join.is_assist {
                        return Ok(());
                    }
                    let me = super::minimum_party::member(
                        pool,
                        &owner,
                        assets,
                        cid,
                        runtime.minimum_header()?,
                        runtime.stat_arithmetic(),
                    )
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                    match runtime
                        .party
                        .join_ship_party(join.ship_id, me)
                        .map_err(|_| RuntimeError::Operation)?
                    {
                        Ok(packets) => joined_party = packets,
                        Err(_) => {
                            // GeneralFail("JoinShip", leaveParty): PartyMember → KickedMessage.
                            let packets = [
                                mission::packet(&Message::Kicked)
                                    .map_err(|_| RuntimeError::Message)?,
                                super::minimum_party::error_message(
                                    true,
                                    "JoinShip",
                                    super::minimum_party::error::JOIN_PARTY_FAIL,
                                )
                                .map_err(|_| RuntimeError::Message)?,
                            ];
                            for packet in &packets {
                                send(stream, sender, packet.category, &packet.body).await?;
                            }
                            tracing::info!("boat list JoinShip refused; values omitted");
                            return Ok(());
                        }
                    }
                    if join.is_in_town_with_ship_info {
                        for packet in &joined_party {
                            send(stream, sender, packet.category, &packet.body).await?;
                        }
                        return Ok(());
                    }
                }
                let super::party_hub::Role::Member {
                    party_id,
                    slot,
                    ready,
                    ..
                } = runtime.party.role(cid)
                else {
                    tracing::info!("JoinShip outside a party member state ignored; values omitted");
                    return Ok(());
                };
                let rejoin = ready == crate::domain::party::ready_state::REJOIN;
                if self.mission.is_some()
                    || join.is_assist
                    || join.is_in_town_with_ship_info
                    || join.ship_id != party_id
                    || !(rejoin || ready == crate::domain::party::ready_state::IN_TOWN)
                {
                    tracing::info!("JoinShip not admitted; values omitted");
                    return Ok(());
                }
                let nexon_sn = id.nexon_sn;
                // isRejoin without a GameHost: isHostCandidate, a new P2P group.
                let candidate = rejoin && runtime.party.hostless(party_id);
                if candidate {
                    runtime.party.set_group(party_id, relay.make_group());
                }
                let joined = mission::join_ship(
                    pool,
                    &owner,
                    assets,
                    cid,
                    party_id,
                    slot,
                    runtime.party.clone(),
                    rejoin,
                    candidate,
                    |group| relay_ticket(&relay, group, cid, nexon_sn, party_id),
                    runtime.minimum_header()?,
                    runtime.quest_clock(),
                )
                .await;
                match joined {
                    Ok(mission::Join::Joined { mission, packets }) => {
                        if rejoin {
                            runtime.party.forget_disconnect(cid);
                        }
                        self.mission = Some(mission);
                        if candidate {
                            runtime
                                .party
                                .adopt_host(party_id, cid)
                                .map_err(|_| RuntimeError::Operation)?;
                        }
                        tracing::info!("party member joined the ship; values omitted");
                        joined_party.extend(packets);
                        joined_party
                    }
                    Ok(mission::Join::Failed(reason)) => {
                        // GeneralFail("JoinShip", unlock, leaveParty): LeaveParty, then the dialog.
                        let mut out = joined_party;
                        out.extend(
                            super::minimum_party::leave(&runtime.party, cid)
                                .map_err(|_| RuntimeError::Operation)?
                                .packets,
                        );
                        out.push(
                            super::minimum_party::error_message(true, "JoinShip", reason)
                                .map_err(|_| RuntimeError::Message)?,
                        );
                        tracing::info!("party member JoinShip refused; values omitted");
                        out
                    }
                    Err(_) => {
                        tracing::warn!("party member JoinShip rejected; values omitted");
                        return Ok(());
                    }
                }
            }
            Message::Ready(ready) => {
                // MicroPlayMember ReadyMessage → SetMemberState(Ready or InShip).
                let (Some(m), super::party_hub::Role::Member { party_id, slot, .. }) =
                    (self.mission.as_ref(), runtime.party.role(cid))
                else {
                    return Ok(());
                };
                if m.host {
                    return Ok(());
                }
                let state = if ready == 1 {
                    crate::domain::party::ready_state::READY
                } else {
                    crate::domain::party::ready_state::IN_SHIP
                };
                match runtime
                    .party
                    .set_member_state(party_id, slot, state, cid)
                    .map_err(|_| RuntimeError::Operation)?
                {
                    Ok(packets) => packets,
                    Err(_) => {
                        tracing::info!("member ready state refused; values omitted");
                        return Ok(());
                    }
                }
            }
            Message::AcceptQuest(accept) => {
                if self.mission.is_some() {
                    tracing::warn!("AcceptQuest while already in a MicroPlay; ignored");
                    return Ok(());
                }
                // Initial opens the party (OpenNewParty → JoinParty); PartyMaster registers
                // the ship of its party; a party member is NotMaster.
                let hub = runtime.party.clone();
                let (party_id, opened, initial) = match hub.role(cid) {
                    super::party_hub::Role::Member { .. } => {
                        let packet = super::minimum_party::error_message(
                            true,
                            "AcceptQuest",
                            super::minimum_party::error::NOT_MASTER,
                        )
                        .map_err(|_| RuntimeError::Message)?;
                        send(stream, sender, packet.category, &packet.body).await?;
                        return Ok(());
                    }
                    super::party_hub::Role::Master { party_id, .. } => {
                        (party_id, Vec::new(), false)
                    }
                    super::party_hub::Role::None => {
                        let opened = async {
                            let me = super::minimum_party::member(
                                pool,
                                &owner,
                                assets,
                                cid,
                                runtime.minimum_header()?,
                                runtime.stat_arithmetic(),
                            )
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                            let party_id =
                                mission::entity_id().map_err(|_| RuntimeError::Operation)?;
                            let packets = hub
                                .open(party_id, me)
                                .map_err(|_| RuntimeError::Operation)?;
                            Ok::<_, RuntimeError>((party_id, packets))
                        }
                        .await;
                        let Ok((party_id, packets)) = opened else {
                            tracing::warn!("mission party could not be opened; values omitted");
                            return Ok(());
                        };
                        (party_id, packets, true)
                    }
                };
                let nexon_sn = id.nexon_sn;
                let outcome = mission::accept_quest(
                    pool,
                    &owner,
                    assets,
                    cid,
                    &accept,
                    mission::MasterParty {
                        hub: hub.clone(),
                        party_id,
                        opened,
                    },
                    |party_id| relay_ticket(&relay, relay.make_group(), cid, nexon_sn, party_id),
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
                        // GeneralFail(leavePartyOnFinished) of Initial: the opened party ends.
                        if initial {
                            hub.end_for(cid);
                        }
                        tracing::info!("mission refused by its digest checks; values omitted");
                        packets
                    }
                    Err(_) => {
                        if initial {
                            hub.end_for(cid);
                        }
                        tracing::warn!("mission accept rejected; values omitted");
                        return Ok(());
                    }
                }
            }
            Message::PartyInvite(invite) => {
                let out = super::minimum_party::invite(
                    &runtime.party,
                    pool,
                    &owner,
                    assets,
                    cid,
                    invite.invited_name.as_deref().unwrap_or(""),
                    runtime.minimum_header()?,
                    runtime.stat_arithmetic(),
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
                tracing::info!(
                    left = out.left,
                    "party invitation processed; values omitted"
                );
                out.packets
            }
            Message::PartyInvitationAccept(party_id) => {
                let out = super::minimum_party::accept(
                    &runtime.party,
                    pool,
                    &owner,
                    assets,
                    cid,
                    party_id,
                    runtime.minimum_header()?,
                    runtime.stat_arithmetic(),
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
                tracing::info!(
                    failed = out.left,
                    "party invitation accepted; values omitted"
                );
                out.packets
            }
            Message::PartyInvitationReject(party_id) => {
                super::minimum_party::reject(&runtime.party, cid, party_id)
                    .map_err(|_| RuntimeError::Operation)?;
                tracing::info!("party invitation rejected; values omitted");
                Vec::new()
            }
            Message::Kick(kick) => {
                let out = super::minimum_party::kick(&runtime.party, cid, kick.slot, kick.nexon_sn)
                    .map_err(|_| RuntimeError::Operation)?;
                tracing::info!("party kick processed; values omitted");
                out.packets
            }
            Message::RegisterServer(register) => {
                let Some(m) = self.mission.as_mut() else {
                    return Ok(());
                };
                if m.host && runtime.party.play(m.party_id).is_some_and(|p| p.restarting) {
                    // MicroPlayMember RegisterServer → HostRestarted → HostElectionFinished.
                    let restarted =
                        runtime
                            .party
                            .host_restarted(m.party_id, cid, register.info.clone());
                    m.host_info = register.info;
                    tracing::info!(restarted, "mission host restarted; values omitted");
                    return Ok(());
                }
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
                if moved
                    && state.host
                    && let Some(snapshot) = state.snapshot()
                {
                    runtime.party.update_snapshot(state.party_id, snapshot);
                }
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
                if erg.winner != state.member.slot {
                    // Another player won it: its frontend gives the reward.
                    let given =
                        runtime
                            .party
                            .give(state.party_id, erg.winner, &erg.kind, erg.amount);
                    tracing::info!(given, "mission erg won by another player; values omitted");
                    let put = mission::put_erg(erg_id, erg.winner)
                        .map_err(|_| RuntimeError::Operation)?;
                    send(stream, sender, put.category, &put.body).await?;
                    return Ok(());
                }
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
                let ships = match query.restriction.as_ref() {
                    Some(restriction) => super::minimum_party::recommended(
                        &runtime.party,
                        pool,
                        &owner,
                        assets,
                        cid,
                        restriction,
                    )
                    .await
                    .unwrap_or_else(|_| {
                        tracing::warn!("ship recommendation unavailable; values omitted");
                        Vec::new()
                    }),
                    None => Vec::new(),
                };
                let packets = mission::recommend_ships(
                    &query,
                    ships,
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
                    // BattleServiceProxy.LeaveParty outside the MicroPlay.
                    let out = super::minimum_party::leave(&runtime.party, cid)
                        .map_err(|_| RuntimeError::Operation)?;
                    tracing::info!(left = out.left, "party left; values omitted");
                    for packet in &out.packets {
                        send(stream, sender, packet.category, &packet.body).await?;
                    }
                    return Ok(());
                };
                // FinishMicroPlay(GiveUpQuest): nobody leaves while the host's leave is being
                // decided; the host of an InGame play with others asks them to follow it.
                if runtime.party.leaving_with_host(state.party_id, cid) {
                    let refused = super::minimum_shop::system_message(
                        0,
                        "GameUI_Heroes_Cannot_Leave_While_Host_Leaving",
                        &[],
                    )
                    .map_err(|_| RuntimeError::Message)?;
                    send(stream, sender, refused.category, &refused.body).await?;
                    return Ok(());
                }
                if state.host
                    && state.in_game
                    && !state.completion.finished
                    && !state.digest.is_hunting_quest
                    && let Some(own) = runtime
                        .party
                        .start_leave_with_host(state.party_id, cid)
                        .map_err(|_| RuntimeError::Operation)?
                {
                    let hub = runtime.party.clone();
                    let party_id = state.party_id;
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(20)).await;
                        hub.trigger_leave_with_host(party_id);
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        hub.return_with_host(party_id, &[]);
                    });
                    for packet in &own {
                        send(stream, sender, packet.category, &packet.body).await?;
                    }
                    tracing::info!("mission host leave decision started; values omitted");
                    return Ok(());
                }
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
                    Return::Town(mut packets) => {
                        if let Some(mut m) = self.mission.take() {
                            if let Some(out) = self.leave_pets(runtime, pool, &mut m).await {
                                packets.extend(out.packets);
                                left_pet = out.current;
                            }
                            // FinishMicroPlay(ReturnToTown) of the host with others in the
                            // play: everyone hears the 10 s countdown, then the finished
                            // players return with it.
                            let others = runtime.party.others(m.party_id, cid);
                            if m.host && !others.is_empty() {
                                if let Ok(own) = super::minimum_shop::system_message(
                                    0,
                                    "GameUI_Heroes_ReturnToTown_Remain_Time_Host",
                                    &["10".to_owned()],
                                ) {
                                    packets.insert(0, own);
                                }
                                if let Ok(notice) = super::minimum_shop::system_message(
                                    0,
                                    "GameUI_Heroes_ReturnToTown_Remain_Time",
                                    &["10".to_owned()],
                                ) {
                                    for other in &others {
                                        runtime.party.send(*other, vec![notice.clone()]);
                                    }
                                }
                                let hub = runtime.party.clone();
                                let party_id = m.party_id;
                                tokio::spawn(async move {
                                    tokio::time::sleep(Duration::from_secs(10)).await;
                                    hub.return_with_host(party_id, &others);
                                });
                            }
                            self.leave_play(runtime, &m);
                            // The proxy sent its own Kicked: the party only drops the member.
                            packets.extend(runtime.party.end_for(cid));
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
        Self::share_with_play(runtime, self.mission.as_deref(), &packets);
        for packet in &packets {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        let changed = left_pet.or_else(|| {
            self.mission
                .as_deref_mut()
                .and_then(|m| m.pets.take_summary())
        });
        if let Some(pet) = changed {
            let cid = self.selected.as_ref().ok_or(RuntimeError::Operation)?.cid;
            let assets = &runtime
                .minimum
                .as_ref()
                .ok_or(RuntimeError::Profile)?
                .assets
                .current();
            self.pet_changed(runtime, pool, assets, cid, pet, stream, sender)
                .await?;
        }
        Ok(())
    }
    /// InGame pet messages (FeatureMatrix "Pet"; the shared food also "Pet_food_V2") and the
    /// host's PetKilled/PetRevived, which BattleServiceProxy hands to the MicroPlay.
    #[allow(clippy::too_many_arguments)]
    /// ItemServiceProxy: CouponShopItemInfoQuery → QueryCouponShopItemInfo and
    /// CouponShopItemBuy → BuyCouponShopItem for the entered character.
    #[allow(clippy::too_many_arguments)]
    async fn coupon_shop(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: crate::protocol::coupon_shop::Message,
    ) -> Result<(), RuntimeError> {
        use crate::protocol::coupon_shop::Message;
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
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
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
        let outcome = match message {
            Message::ItemInfoQuery(version) => {
                super::minimum_coupon_shop::query(pool, &owner, assets, cid, version).await
            }
            Message::ItemBuy(request) => {
                let ids = minimum
                    .creation
                    .entity_ids(64)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                super::minimum_coupon_shop::buy(
                    pool,
                    &owner,
                    assets,
                    cid,
                    request,
                    ids,
                    runtime.item_log_context(assets)?,
                )
                .await
            }
            Message::ItemInfoResult(_) => {
                self.log_unhandled(category, body);
                return Ok(());
            }
        };
        match outcome {
            Ok(packets) => {
                for packet in &packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                tracing::info!("coupon shop request processed; values omitted");
            }
            Err(_) => tracing::warn!("coupon shop request rejected; values omitted"),
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    async fn pet(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: crate::protocol::pet::Message,
    ) -> Result<(), RuntimeError> {
        use crate::protocol::pet::Message;
        match &message {
            Message::PetKilled(k) => {
                if let Some(m) = self.mission.as_deref_mut() {
                    let counted = m.pets.killed(k.pet_id, std::time::Instant::now());
                    tracing::info!(counted, "mission pet killed; values omitted");
                }
                return Ok(());
            }
            Message::PetRevived(r) => {
                if let Some(m) = self.mission.as_deref_mut() {
                    let counted = m.pets.revived(r.pet_id, std::time::Instant::now());
                    tracing::info!(counted, "mission pet revived; values omitted");
                }
                return Ok(());
            }
            _ => {}
        }
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
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        let Ok(settings) = super::minimum_pet::settings(assets) else {
            tracing::warn!("pet feature policy unreadable; values omitted");
            return Ok(());
        };
        let food = settings.enabled && settings.food_v2;
        // BattleServiceProxy.IsEquipAllowed(-1) for PetCreate.
        let equip_allowed = self.mission.as_deref().is_none_or(|m| m.equip_allowed());
        let ids = match &message {
            Message::RequestPetFoodUnshare(_) => minimum
                .creation
                .entity_ids(4)
                .await
                .map_err(|_| RuntimeError::Operation)?,
            _ => Vec::new(),
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
        use super::minimum_pet as pets;
        let outcome = match message {
            Message::PetCreateNameCheck(m) if settings.enabled => {
                pets::check_name(pool, &owner, assets, cid, m.pet_name.as_deref())
                    .await
                    .and_then(|r| pets::create_name_answer(m.item_id, m.pet_name.as_deref(), r))
                    .map(|packets| pets::Outcome {
                        packets,
                        current: None,
                    })
            }
            Message::PetCreate(m) if equip_allowed => {
                pets::create(pool, &owner, assets, cid, m.item_id, m.pet_name.as_deref()).await
            }
            Message::PetChangeNameCheck(m) if settings.enabled => {
                pets::check_name(pool, &owner, assets, cid, m.pet_name.as_deref())
                    .await
                    .and_then(|r| {
                        pets::change_name_answer(m.item_id, m.pet_id, m.pet_name.as_deref(), r)
                    })
                    .map(|p| pets::Outcome {
                        packets: vec![p],
                        current: None,
                    })
            }
            Message::PetChangeName(m) if settings.enabled => {
                pets::change_name(
                    pool,
                    &owner,
                    assets,
                    cid,
                    m.item_id,
                    m.pet_id,
                    m.pet_name.as_deref(),
                )
                .await
            }
            Message::MovePetSlot(m) if settings.enabled => {
                pets::move_slot(pool, &owner, assets, cid, m.pet_id, m.dest_slot_no).await
            }
            Message::RemovePetSkill(m) if settings.enabled => {
                pets::remove_skill(pool, &owner, assets, cid, m.pet_id, m.pet_skill_id).await
            }
            Message::QueryPetList if settings.enabled => {
                pets::query_list(pool, &owner, assets, cid)
                    .await
                    .map(|packets| pets::Outcome {
                        packets,
                        current: None,
                    })
            }
            Message::SelectPet(pet_id) if settings.enabled => {
                pets::select(pool, &owner, assets, cid, pet_id).await
            }
            Message::RemovePet(pet_id) if settings.enabled => {
                pets::remove(pool, &owner, assets, cid, pet_id).await
            }
            Message::RequestPetFoodShare(item_id) if food => {
                pets::food_share(pool, &owner, assets, cid, item_id).await
            }
            Message::RequestPetFoodUnshare(class) if food => {
                pets::food_unshare(
                    pool,
                    &owner,
                    assets,
                    cid,
                    class.as_deref(),
                    &mut ids.into_iter(),
                )
                .await
            }
            // PetOperationMessage needs IsCheat (no GM session in the laboratory); the
            // feature-gated messages are ignored while the feature is off.
            _ => {
                tracing::info!(category, "pet message ignored; values omitted");
                return Ok(());
            }
        };
        match outcome {
            Ok(out) => {
                for packet in &out.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                tracing::info!(
                    category,
                    messages = out.packets.len(),
                    "minimum pet request processed; values omitted"
                );
                if let Some(pet) = out.current {
                    self.pet_changed(runtime, pool, assets, cid, pet, stream, sender)
                        .await?;
                }
            }
            Err(_) => tracing::warn!(category, "minimum pet request refused; values omitted"),
        }
        Ok(())
    }
    /// PetManager.Sync → PushUpdatePetStatus: the new summary pet reaches the town look and
    /// the MicroPlayer's PetStatus observer (QueryCurrentPet: member info in the ship,
    /// UpdateCurrentPetMessage to the host in and after the game).
    #[allow(clippy::too_many_arguments)]
    async fn pet_changed(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        assets: &super::minimum_assets::Assets,
        cid: i64,
        pet: Option<crate::protocol::pet::PetStatusInfo>,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
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
        let config = runtime.minimum_header()?;
        let arithmetic = runtime.stat_arithmetic();
        match super::minimum_pet::look(pool, &owner, assets, cid, config, arithmetic).await {
            Ok(look) => {
                if let Some((_, channel)) = runtime.channel.as_ref() {
                    channel.update_look(cid, look.clone()).await;
                }
                if let Some(selected) = self.selected.as_mut() {
                    selected.look = look;
                }
            }
            Err(_) => tracing::warn!("pet look refresh rejected; values omitted"),
        }
        let Some(mission) = self.mission.as_deref_mut() else {
            return Ok(());
        };
        let packet = if mission.state == super::minimum_mission::party_state::IN_SHIP {
            match super::minimum_mission::refresh_member(
                pool, &owner, assets, cid, mission, config, arithmetic,
            )
            .await
            {
                Ok(packet) => packet,
                Err(_) => {
                    tracing::warn!("mission member info refresh rejected; values omitted");
                    None
                }
            }
        } else if mission.in_game {
            if mission.completion.player_state
                == super::minimum_mission_finish::PlayerState::Finished
            {
                mission.pets.set_force_current(pet);
            } else {
                mission.pets.change_current(pet, std::time::Instant::now());
            }
            Some(
                super::minimum_pet::update_current_packet(
                    mission.member.slot,
                    mission.pets.current(),
                )
                .map_err(|_| RuntimeError::Message)?,
            )
        } else {
            None
        };
        if let Some(packet) = packet {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        Ok(())
    }
    /// ItemServiceProxy mail messages (InGame) → the item service mail processors. The
    /// laboratory has no second-password lock in game, so UseSecondPassword passes.
    #[allow(clippy::too_many_arguments)]
    async fn mail(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: crate::protocol::mail::Message,
    ) -> Result<(), RuntimeError> {
        use crate::protocol::mail::Message;
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
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        let ids = match &message {
            Message::SendMail(_) => super::minimum_mail::SEND_ENTITY_IDS,
            Message::GetMailItem(_) => super::minimum_mail::GET_ENTITY_IDS,
            _ => 0,
        };
        let ids = minimum
            .creation
            .entity_ids(ids)
            .await
            .map_err(|_| RuntimeError::Operation)?;
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
        use super::minimum_mail as mails;
        let outcome = match message {
            Message::QueryMailList => mails::mail_list(pool, &owner, assets, cid).await,
            Message::QueryMailInfo(mail_id) => mails::mail_info(pool, &owner, cid, mail_id).await,
            Message::SendMail(m) => mails::send_mail(pool, &owner, assets, cid, &m, ids).await,
            Message::GetMailItem(mail_id) => {
                mails::get_mail_item(pool, &owner, assets, cid, mail_id, ids).await
            }
            Message::DeleteMail(list) => {
                mails::delete_mail(pool, &owner, assets, cid, &list.unwrap_or_default()).await
            }
            Message::ReturnMail(mail_id) => {
                mails::return_mail_request(pool, &owner, assets, cid, mail_id).await
            }
            _ => {
                tracing::info!(category, "mail message ignored; values omitted");
                return Ok(());
            }
        };
        match outcome {
            Ok(out) => {
                for packet in &out.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                for (to, mail_id) in &out.notices {
                    runtime.party.notice_mail(*to, *mail_id);
                }
                tracing::info!(
                    category,
                    messages = out.packets.len(),
                    notices = out.notices.len(),
                    "minimum mail request processed; values omitted"
                );
            }
            Err(_) => tracing::warn!(category, "minimum mail request refused; values omitted"),
        }
        Ok(())
    }
    /// InGuild.GuildKicked: GuildResultMessage(result), then Initial (QueryGuildID).
    async fn guild_kicked(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        cid: i64,
        code: i32,
    ) -> Result<Vec<crate::domain::map_partition::OutgoingPacket>, RuntimeError> {
        let mut packets =
            vec![super::guild::result_packet(code, "", 0).map_err(|_| RuntimeError::Operation)?];
        let (next, out) = super::guild::enter(pool, &runtime.party, &runtime.guilds, cid).await;
        self.guild = next;
        packets.extend(out);
        Ok(packets)
    }
    /// The caller's part of a guild sync, with its GuildKicked(LeaveGuildSuccess).
    async fn guild_synced(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        cid: i64,
        synced: super::guild::Synced,
    ) -> Result<Vec<crate::domain::map_partition::OutgoingPacket>, RuntimeError> {
        let mut packets = synced.packets;
        if synced.kicked {
            packets.extend(
                self.guild_kicked(
                    runtime,
                    pool,
                    cid,
                    crate::protocol::guild::result::LEAVE_GUILD_SUCCESS,
                )
                .await?,
            );
        }
        Ok(packets)
    }
    /// FreeMatchReportMessage: a MicroPlayHost message (a member's frontend does not forward
    /// it) → FreeMatchReport; nothing is sent back.
    async fn pvp_message(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        category: i32,
        body: &[u8],
        message: Option<crate::protocol::pvp::Message>,
    ) -> Result<(), RuntimeError> {
        use crate::protocol::pvp::Message as Pvp;
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
            self.log_unhandled(category, body);
            return Ok(());
        };
        let report = match message {
            Some(Pvp::FreeMatchReport(report)) => report,
            Some(Pvp::QueryChannelList(query)) => {
                let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
                let assets = minimum.assets.current();
                // PvpServiceProxy.Initial: an empty list when the query fails.
                let mode = query.pvp_mode.clone().unwrap_or_default();
                match runtime.pvp.channel_list(assets.npc_pvp(&mode), &query) {
                    Ok(p) => {
                        runtime.party.deliver(cid, vec![p]);
                    }
                    Err(_) => tracing::warn!("pvp channel list failed; values omitted"),
                }
                tracing::info!(category, "pvp channel list processed; values omitted");
                return Ok(());
            }
            Some(Pvp::Register(request)) => {
                return self.pvp_register(runtime, pool, cid, &request).await;
            }
            Some(Pvp::Unregister(_)) => {
                runtime.pvp.unregister(cid);
                tracing::info!("pvp unregister processed; values omitted");
                return Ok(());
            }
            Some(Pvp::RegisterGameWaitingQueue(request)) => {
                let refused = runtime.pvp.waiting_queue(cid, request.game_index);
                if !refused.is_empty() {
                    runtime.party.deliver(cid, refused);
                }
                tracing::info!("pvp waiting queue processed; values omitted");
                return Ok(());
            }
            Some(Pvp::UnregisterGameWaitingQueue(_)) => {
                runtime.pvp.leave_waiting_queue(cid);
                tracing::info!("pvp waiting queue left; values omitted");
                return Ok(());
            }
            Some(Pvp::ConfirmJoin(confirm)) => {
                let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
                let assets = minimum.assets.current();
                let binder = self.pvp_binder(runtime, pool, &assets)?;
                let rules = super::pvp_match::Rules::from_assets(
                    &assets,
                    confirm.pvp_mode.as_deref().unwrap_or_default(),
                )
                .map_err(|_| RuntimeError::Profile)?;
                runtime.pvp.confirm_join(binder, cid, rules);
                tracing::info!("pvp confirm join processed; values omitted");
                return Ok(());
            }
            Some(Pvp::Penalty(penalty)) => {
                runtime.pvp.penalty(cid, penalty.player_index);
                tracing::info!("pvp penalty processed; values omitted");
                return Ok(());
            }
            Some(Pvp::Report(report)) => {
                runtime.pvp.report(cid, &report);
                tracing::info!(event = report.event, "pvp report processed; values omitted");
                return Ok(());
            }
            Some(_) => {
                // Confirm join and penalties belong to the Full matching (DeathMatch, GodMatch).
                tracing::info!(category, "pvp message not migrated; values omitted");
                return Ok(());
            }
            None => {
                self.log_unhandled(category, body);
                return Ok(());
            }
        };
        // BattleServiceProxy: only the MicroPlay host forwards it; no play is a LogicalFail.
        let Some((party_id, slot)) = self
            .mission
            .as_ref()
            .filter(|m| m.host)
            .map(|m| (m.party_id, m.member.slot))
        else {
            tracing::info!("free match report outside a hosted play ignored; values omitted");
            return Ok(());
        };
        let mut players: Vec<(i32, i64)> = runtime
            .party
            .players(party_id)
            .into_iter()
            .map(|(s, p)| (s, p.cid))
            .collect();
        if players.is_empty() {
            players.push((slot, cid));
        }
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        match super::pvp::resolve(&players, report.winner_tag, report.loser_tag) {
            Some((winner, loser)) => {
                for (target, win) in [(winner, true), (loser, false)] {
                    if !matches!(
                        super::pvp::increase(pool, assets, target, win).await,
                        Ok(true)
                    ) {
                        tracing::warn!("free match count not increased; values omitted");
                    }
                }
            }
            None => tracing::warn!("free match report with an invalid player tag; values omitted"),
        }
        tracing::info!(category, "free match report processed; values omitted");
        Ok(())
    }
    /// The PvP hub's store, WhoIs profile, MakeEntityID and guild levels.
    fn pvp_binder(
        &self,
        runtime: &Runtime,
        pool: &PgPool,
        assets: &Arc<super::minimum_assets::Assets>,
    ) -> Result<super::pvp_match::Binder, RuntimeError> {
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let guild_levels = assets
            .economy
            .as_ref()
            .and_then(|e| super::guild::levels(e.guild_levels(), &runtime.content.policy).ok())
            .map(Arc::new);
        Ok(super::pvp_match::Binder {
            pool: pool.clone(),
            assets: Arc::clone(assets),
            config: runtime.minimum_header()?,
            arithmetic: runtime.stat_arithmetic(),
            ids: Some(minimum.creation.entity_id_source()),
            guild_levels,
        })
    }
    /// GuildServiceProxy: FindNewbieRecommendGuild and RequestNewbieRecommendGuild (OutGuild),
    /// DirectPurchaseGuildItem (InGuild; IsNotMaster for others; UseSecondPassword passes).
    async fn guild_newbie_message(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        category: i32,
        body: &[u8],
        message: Option<crate::protocol::guild_newbie::Message>,
    ) -> Result<(), RuntimeError> {
        use crate::protocol::guild_newbie::{self as wire, Message};
        let (Some(message), Some(cid)) = (message, self.selected.as_ref().map(|s| s.cid)) else {
            self.log_unhandled(category, body);
            return Ok(());
        };
        let reply = |m: Message| -> Option<crate::domain::map_partition::OutgoingPacket> {
            wire::packet(&m, DEFAULT_MAX_BODY)
                .ok()
                .map(|(category, body)| crate::domain::map_partition::OutgoingPacket { category, body })
        };
        match (message, self.guild) {
            (Message::FindNewbieRecommendGuild(_), super::guild::State::OutGuild) => {
                let found = super::guild::find_newbie_recommend(pool, &runtime.guilds)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                if let Some(info) = found
                    && let Some(p) = reply(Message::FindNewbieRecommendGuildResult(wire::FindResult { info: Some(info) }))
                {
                    runtime.party.deliver(cid, vec![p]);
                }
            }
            (Message::RequestNewbieRecommendGuild(request), super::guild::State::OutGuild) => {
                let name: Vec<u8> = sqlx::query_scalar("SELECT name_utf16 FROM game_character_info WHERE id=$1")
                    .bind(cid)
                    .fetch_one(pool)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                let name = String::from_utf16_lossy(
                    &name.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect::<Vec<_>>(),
                );
                // ShipID: BattleServiceProxy.PartyID.
                let ship_id = match runtime.party.role(cid) {
                    super::party_hub::Role::Master { party_id, .. }
                    | super::party_hub::Role::Member { party_id, .. } => party_id,
                    super::party_hub::Role::None => 0,
                };
                super::guild::request_newbie_recommend(
                    pool,
                    &runtime.party,
                    &runtime.guilds,
                    cid,
                    &name,
                    ship_id,
                    request.guild_sn,
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
            }
            (Message::DirectPurchaseGuildItem(request), super::guild::State::InGuild { guild_sn }) => {
                let item_class = request.item_class.clone().unwrap_or_default();
                let master = super::guild::membership(pool, cid)
                    .await
                    .map_err(|_| RuntimeError::Operation)?
                    .is_some_and(|(sn, r)| sn == guild_sn && r == crate::protocol::guild::rank::MASTER);
                let result = if master {
                    let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
                    let assets = minimum.assets.current();
                    let id = self
                        .frontend
                        .accepted_identity()
                        .map_err(|_| RuntimeError::Operation)?;
                    super::guild::direct_purchase(
                        pool,
                        &runtime.party,
                        &runtime.guilds,
                        &assets,
                        id.nexon_sn,
                        cid,
                        guild_sn,
                        request.product_no,
                        &item_class,
                    )
                    .await
                    .map_err(|_| RuntimeError::Operation)?
                } else {
                    wire::purchase_result::IS_NOT_MASTER
                };
                if let Some(p) = reply(Message::DirectPurchaseGuildItemResult(wire::PurchaseResult {
                    item_class: Some(item_class),
                    result,
                })) {
                    runtime.party.deliver(cid, vec![p]);
                }
            }
            _ => {
                // Another state (or a server message): the proxy ignores it.
                tracing::info!(category, "guild newbie message ignored in this state; values omitted");
                return Ok(());
            }
        }
        tracing::info!(category, "guild newbie message processed; values omitted");
        Ok(())
    }
    /// PvpServiceProxy.Initial(PvpRegisterMessage): DSWaiting does not exist without a DS;
    /// InMicroPlay in a mission; BattleServiceProxy.QueryPartyMembers (a party member is
    /// InvalidMaster); PVPPartyCountLimit (InvalidMembercount; the IgnoreRestriction cheat
    /// needs a GM, not migrated); then RegisterPvpPlayer for the solo player (only the Arena
    /// is migrated, one player per registration).
    async fn pvp_register(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        cid: i64,
        request: &crate::protocol::pvp::Register,
    ) -> Result<(), RuntimeError> {
        use super::party_hub::Role;
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = minimum.assets.current();
        let members = match runtime.party.role(cid) {
            Role::None => 1,
            Role::Master { party_id, .. } => runtime.party.member_count(party_id),
            Role::Member { .. } => 0,
        };
        let limit = assets
            .npc_policy()
            .map(|p| p.get_integer("PVPPartyCountLimit"))
            .transpose()
            .map_err(|_| RuntimeError::Profile)?
            .unwrap_or(0);
        let refusal = if self.mission.is_some() {
            Some("GameUI_Heroes_PvpJoinFail_InMicroPlay")
        } else if members == 0 {
            Some("GameUI_Heroes_PvpJoinFail_InvalidMaster")
        } else if limit > 0 && members > limit as usize {
            Some("GameUI_Heroes_PvpJoinFail_InvalidMembercount")
        } else {
            None
        };
        // QueryPartyMembers of a master: the whole party registers (RegisterPvpPlayer IDList).
        if refusal.is_none()
            && members > 1
            && let super::party_hub::Role::Master { party_id, .. } = runtime.party.role(cid)
        {
            let mut team = Vec::new();
            for member in runtime.party.members(party_id) {
                let row: Option<(i32, i32, Vec<u8>, String, i64)> = sqlx::query_as(
                    "SELECT c.uid,c.level,c.name_utf16,u.name_key,u.local_revision FROM game_character_info c JOIN game_users u ON u.id=c.uid WHERE c.id=$1",
                )
                .bind(member)
                .fetch_optional(pool)
                .await
                .map_err(|_| RuntimeError::Operation)?;
                let Some((uid, level, name_utf16, account, revision)) = row else {
                    continue;
                };
                let guild_id = super::guild::membership(pool, member)
                    .await
                    .map_err(|_| RuntimeError::Operation)?
                    .map_or(0, |(sn, _)| sn);
                team.push(super::pvp_match::TeamEntry {
                    cid: member,
                    uid,
                    account,
                    revision,
                    level,
                    name: String::from_utf16_lossy(
                        &name_utf16
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|c| u16::from_le_bytes(*c))
                            .collect::<Vec<_>>(),
                    ),
                    guild_id,
                });
            }
            let binder = self.pvp_binder(runtime, pool, &assets)?;
            let rules = super::pvp_match::Rules::from_assets(
                &assets,
                request.pvp_mode.as_deref().unwrap_or_default(),
            )
            .map_err(|_| RuntimeError::Profile)?;
            match runtime.pvp.register_team(binder, team, rules, request) {
                Ok(packets) if !packets.is_empty() => {
                    runtime.party.deliver(cid, packets);
                }
                Ok(_) => {}
                Err(_) => tracing::warn!("pvp party register failed; values omitted"),
            }
            tracing::info!("pvp party register processed; values omitted");
            return Ok(());
        }
        let (level, name_utf16): (i32, Vec<u8>) =
            sqlx::query_as("SELECT level,name_utf16 FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_one(pool)
                .await
                .map_err(|_| RuntimeError::Operation)?;
        let name = String::from_utf16_lossy(
            &name_utf16
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect::<Vec<_>>(),
        );
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
        let binder = self.pvp_binder(runtime, pool, &assets)?;
        // GuildID of the registering character (0 without a guild).
        let guild_id = super::guild::membership(pool, cid)
            .await
            .map_err(|_| RuntimeError::Operation)?
            .map_or(0, |(sn, _)| sn);
        let entry = super::pvp_match::Entry {
            cid,
            owner: &owner,
            level,
            name: &name,
            guild_id,
        };
        let rules = super::pvp_match::Rules::from_assets(
            &assets,
            request.pvp_mode.as_deref().unwrap_or_default(),
        )
        .map_err(|_| RuntimeError::Profile)?;
        match runtime.pvp.register(binder, entry, refusal, rules, request) {
            Ok(packets) => {
                if !packets.is_empty() {
                    runtime.party.deliver(cid, packets);
                }
            }
            Err(_) => tracing::warn!("pvp register failed; values omitted"),
        }
        tracing::info!("pvp register processed; values omitted");
        Ok(())
    }
    /// SelectTitleMessage (InGame): PlayerConn SelectTitle, then the frontend's echo of the
    /// message when it completes (nothing when it fails).
    async fn title_message(
        &mut self,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: Option<crate::protocol::title::Message>,
    ) -> Result<(), RuntimeError> {
        let in_game = self.selected.is_some()
            && self
                .password
                .as_ref()
                .is_some_and(|p| p.selection_allowed(&self.frontend));
        let (Some(crate::protocol::title::Message::SelectTitle(title)), Some((cid, journal)), true) = (
            message,
            self.selected.as_ref().map(|s| (s.cid, s.log_journal)),
            in_game,
        ) else {
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
        match super::rank_title::select(pool, &owner, journal, cid, &self.rank_titles, title).await
        {
            Ok(Some(packet)) => send(stream, sender, packet.category, &packet.body).await?,
            Ok(None) => {}
            Err(_) => tracing::warn!("title selection rejected; values omitted"),
        }
        tracing::info!(category, "title selection processed; values omitted");
        Ok(())
    }
    /// InGame ranking messages (RankConn with CharacterRank on): the published results of the
    /// D-S3 ranking. Requests answered with nothing in the source (a failed or null result,
    /// RequestSetRankFavorite, RequestSetRankGoal) send nothing.
    #[allow(clippy::too_many_arguments)]
    async fn rank_message(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: Option<crate::protocol::rank::Message>,
    ) -> Result<(), RuntimeError> {
        use super::rank as ranks;
        use crate::protocol::rank::{self as wire, Message, PeriodResults};
        let in_game = self.selected.is_some()
            && self
                .password
                .as_ref()
                .is_some_and(|p| p.selection_allowed(&self.frontend));
        let (Some(message), Some(cid), true) =
            (message, self.selected.as_ref().map(|s| s.cid), in_game)
        else {
            self.log_unhandled(category, body);
            return Ok(());
        };
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        let enabled = assets
            .npc_policy()
            .map(|p| p.is_enabled(Some("CharacterRank")))
            .transpose()
            .map_err(|_| RuntimeError::Profile)?
            .unwrap_or(false);
        if !enabled {
            self.log_unhandled(category, body);
            return Ok(());
        }
        let op = |_| RuntimeError::Operation;
        let reply = match message {
            Message::QueryRankCharacter(q) => Some(Message::RankCharacter(PeriodResults {
                rank_result: Some(
                    ranks::by_cid(pool, q.cid, q.period_type)
                        .await
                        .map_err(op)?,
                ),
                period_type: q.period_type,
            })),
            Message::QueryRankAll(period_type) => Some(Message::RankAll(PeriodResults {
                rank_result: Some(ranks::firsts(pool, period_type).await.map_err(op)?),
                period_type,
            })),
            Message::QueryRank(q) => {
                match ranks::event(assets, q.rank_id.as_deref().unwrap_or("")).map_err(op)? {
                    Some((event_id, ..)) => Some(Message::RankInfo(PeriodResults {
                        rank_result: Some(
                            ranks::by_event(pool, event_id, q.period_type)
                                .await
                                .map_err(op)?,
                        ),
                        period_type: q.period_type,
                    })),
                    None => None,
                }
            }
            Message::QueryRankFavorites(q) => Some(Message::RankFavorites(PeriodResults {
                rank_result: Some(
                    ranks::favorites(pool, q.cid, q.period_type)
                        .await
                        .map_err(op)?,
                ),
                period_type: q.period_type,
            })),
            Message::QueryRankHeavyFavorites => Some(Message::RankHeavyFavorites(Some(
                ranks::heavy_favorites(pool).await.map_err(op)?,
            ))),
            Message::QueryRankAlarm(alarm_cid) => Some(Message::RankAlarm(Some(
                ranks::alarms(pool, alarm_cid)
                    .await
                    .map_err(op)?
                    .into_iter()
                    .map(|(period_type, event_id, rank)| {
                        Some(wire::Alarm {
                            period_type,
                            event_id,
                            rank,
                        })
                    })
                    .collect(),
            ))),
            // GetRankGoal's guild rival is a stub in the source: the answer has no rival.
            Message::QueryRankGoal(q) => Some(Message::RankGoal(wire::Goal {
                event_id: q.event_id,
                rival_name: Some(String::new()),
                rival_score: 0,
                rank: 0,
                rank_prev: 0,
            })),
            Message::QueryRankOther(q) if q.requester_cid != 0 => {
                let name = q.character_name.clone().unwrap_or_default();
                let target = super::minimum_party::character_info_by_name(pool, &name)
                    .await
                    .map_err(op)?;
                let own = target.is_some_and(|(t, _)| t == cid);
                let (level, base_character) = match target {
                    Some((t, _)) => sqlx::query_as::<_, (i32, i32)>(
                        "SELECT level,class::integer FROM game_character_info WHERE id=$1",
                    )
                    .bind(t)
                    .fetch_optional(pool)
                    .await
                    .map_err(|_| RuntimeError::Operation)?
                    .unwrap_or((0, 0)),
                    None => (0, 0),
                };
                let results = match target {
                    Some((t, _)) => ranks::by_cid(pool, t, q.period_type).await.map_err(op)?,
                    None => Vec::new(),
                };
                let requester = if own {
                    Vec::new()
                } else {
                    ranks::by_cid(pool, q.requester_cid, q.period_type)
                        .await
                        .map_err(op)?
                };
                Some(Message::RankOther(wire::Other {
                    level,
                    base_character,
                    character_name: Some(name),
                    rank_result: Some(results),
                    requester_rank_result: Some(requester),
                    period_type: q.period_type,
                }))
            }
            Message::SetRankFavorite(f) => {
                ranks::set_favorite(
                    pool,
                    assets,
                    cid,
                    f.rank_id.as_deref().unwrap_or(""),
                    f.is_addition,
                )
                .await
                .map_err(op)?;
                None
            }
            // QueryRandomRank: the banner draw needs InGameBannerPosition/PeriodType, not
            // exported yet; the answer has no banner.
            Message::QueryRandomRank => Some(Message::RandomRank(Some(Vec::new()))),
            _ => None,
        };
        if let Some(reply) = reply {
            let kind = reply.kind();
            let body = wire::encode(kind, Some(&reply), DEFAULT_MAX_BODY)
                .map_err(|_| RuntimeError::Message)?;
            send(stream, sender, kind.category(), &body).await?;
        }
        tracing::info!(category, "rank message processed; values omitted");
        Ok(())
    }
    /// GuildServiceProxy guild storage messages (InGuild): HandleGuildStorageSession and
    /// UpdateGuildStorageSetting (B5 G6b) and PurchaseGuildStorage (G6e); deposits, picks
    /// and arranging are G6c–G6d and are only logged.
    #[allow(clippy::too_many_arguments)]
    async fn guild_storage_message(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: Option<crate::protocol::guild_storage::Message>,
    ) -> Result<(), RuntimeError> {
        use crate::protocol::guild_storage::Message;
        let in_game = self.selected.is_some()
            && self
                .password
                .as_ref()
                .is_some_and(|p| p.selection_allowed(&self.frontend));
        let (Some(message), Some(cid), true, super::guild::State::InGuild { guild_sn }) = (
            message,
            self.selected.as_ref().map(|s| s.cid),
            in_game,
            self.guild,
        ) else {
            self.log_unhandled(category, body);
            return Ok(());
        };
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        let member_rank = super::guild::membership(pool, cid)
            .await
            .map_err(|_| RuntimeError::Operation)?
            .filter(|(sn, _)| *sn == guild_sn)
            .map_or(crate::protocol::guild::rank::UNKNOWN, |(_, r)| r);
        let config = super::guild_storage::StorageConfig::from_assets(assets)
            .map_err(|_| RuntimeError::Profile)?;
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
        let member = super::guild_storage::Member {
            owner: &owner,
            cid,
            rank: member_rank,
        };
        let outcome = match message {
            Message::HandleGuildStorageSession(starting) => {
                super::guild_storage::session(
                    pool,
                    assets,
                    config,
                    &runtime.guilds,
                    guild_sn,
                    cid,
                    member_rank,
                    starting,
                )
                .await
            }
            Message::UpdateGuildStorageSetting(setting) => {
                super::guild_storage::update_setting(
                    pool,
                    assets,
                    config,
                    &runtime.guilds,
                    &runtime.party,
                    guild_sn,
                    cid,
                    member_rank,
                    setting.gold_limit,
                    setting.access_limit_tag,
                )
                .await
            }
            Message::PurchaseGuildStorage(request) => {
                // GuildServiceProxy: GuildStorageSecondPassworded is off in EN-EU.
                super::guild_storage::purchase(
                    pool,
                    assets,
                    config,
                    &runtime.guilds,
                    &runtime.party,
                    guild_sn,
                    super::guild_storage::Buyer {
                        owner: &owner,
                        cid,
                        log: runtime.item_log_context(assets)?,
                    },
                    request.product_no,
                )
                .await
            }
            Message::AddGuildStorageItem(request) => {
                // TransferToSystem makes one system item of the deposit.
                let ids = minimum
                    .creation
                    .entity_ids(2)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                super::guild_storage::add_item(
                    pool,
                    assets,
                    config,
                    &runtime.guilds,
                    &runtime.party,
                    guild_sn,
                    member,
                    &request,
                    ids,
                )
                .await
            }
            Message::PickGuildStorageItem(request) => {
                let ids = minimum
                    .creation
                    .entity_ids(4)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                super::guild_storage::pick_item(
                    pool,
                    assets,
                    config,
                    &runtime.guilds,
                    &runtime.party,
                    guild_sn,
                    member,
                    &request,
                    ids,
                )
                .await
            }
            Message::ArrangeGuildStorageItem(request) => {
                super::guild_storage::arrange_item(
                    pool,
                    assets,
                    config,
                    &runtime.guilds,
                    &runtime.party,
                    guild_sn,
                    cid,
                    member_rank,
                    request.item_id,
                    request.slot,
                )
                .await
            }
            _ => {
                tracing::info!("guild storage message not handled by the server; values omitted");
                Ok(Vec::new())
            }
        };
        match outcome {
            Ok(packets) => {
                for packet in &packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
            }
            Err(_) => tracing::warn!("guild storage message rejected; values omitted"),
        }
        tracing::info!(category, "guild storage message processed; values omitted");
        Ok(())
    }
    /// GuildServiceProxy messages of the entered character. QueryGuildList works in every
    /// state; the others follow the proxy state (Initial: only ReconnectGuild; OutGuild:
    /// OpenGuild, GuildStoryRequest and the failures of member operations; InGuild: the
    /// JoinGuild and the HasGuild refusals; InGuild: leave, close, operations, notice,
    /// invite and master change).
    #[allow(clippy::too_many_arguments)]
    async fn guild_message(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: Option<crate::protocol::guild::Message>,
    ) -> Result<(), RuntimeError> {
        use super::guild::{self as guilds, State};
        use crate::domain::guild::{self as rules, open_error};
        use crate::protocol::guild::{Message, result};
        let in_game = self.selected.is_some()
            && self
                .password
                .as_ref()
                .is_some_and(|p| p.selection_allowed(&self.frontend));
        let (Some(message), Some(cid), true) =
            (message, self.selected.as_ref().map(|s| s.cid), in_game)
        else {
            self.log_unhandled(category, body);
            return Ok(());
        };
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
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
        let mut packets = Vec::new();
        let state = self.guild;
        match (state, message) {
            (_, Message::QueryGuildList(query)) => {
                match guilds::search(pool, &runtime.guilds, &query).await {
                    Ok(Some(list)) => packets.push(
                        guilds::packet(&Message::GuildList(list))
                            .map_err(|_| RuntimeError::Operation)?,
                    ),
                    Ok(None) | Err(_) => packets.push(
                        guilds::result_packet(result::GUILD_SEARCH_FAIL, "", 0)
                            .map_err(|_| RuntimeError::Operation)?,
                    ),
                }
            }
            (State::Initial, Message::ReconnectGuild) => {
                let (next, out) = guilds::enter(pool, &runtime.party, &runtime.guilds, cid).await;
                self.guild = next;
                packets = out;
            }
            (State::OutGuild, Message::OpenGuild(open)) => {
                // OpenGuild(GuildMemberKey, GuildName.Trim(), GuildNameID.Trim(), GuildIntro):
                // a null name or ID throws in the FE before the operation.
                let (Some(name), Some(guild_id)) = (open.guild_name, open.guild_name_id) else {
                    self.log_unhandled(category, body);
                    return Ok(());
                };
                let opened = guilds::open(
                    pool,
                    cid,
                    name.trim(),
                    guild_id.trim(),
                    open.guild_intro.as_deref(),
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
                match opened {
                    Ok(sn) => {
                        packets.push(
                            guilds::result_packet(result::OPEN_GUILD_SUCCESS, "", i64::from(sn))
                                .map_err(|_| RuntimeError::Operation)?,
                        );
                        let (next, out) =
                            guilds::enter(pool, &runtime.party, &runtime.guilds, cid).await;
                        self.guild = next;
                        packets.extend(out);
                        match super::minimum_story::delete_story_line(
                            pool,
                            &owner,
                            assets,
                            cid,
                            rules::CREATE_STORY,
                        )
                        .await
                        {
                            Ok(out) => packets.extend(out),
                            Err(_) => tracing::warn!(
                                "guild creation story deletion rejected; values omitted"
                            ),
                        }
                    }
                    Err(arg) => packets.push(
                        guilds::result_packet(result::OPEN_GUILD_FAIL, arg, 0)
                            .map_err(|_| RuntimeError::Operation)?,
                    ),
                }
            }
            (State::OutGuild, Message::GuildStoryRequest) => {
                let ids = minimum
                    .creation
                    .entity_ids(1)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                match super::minimum_story::give_story_line(
                    pool,
                    &owner,
                    assets,
                    cid,
                    rules::CREATE_STORY,
                    ids,
                )
                .await
                {
                    Ok(Some(out)) => {
                        packets.extend(out);
                        packets.push(
                            guilds::result_packet(result::GIVE_STORY_SUCCESS, "", 0)
                                .map_err(|_| RuntimeError::Operation)?,
                        );
                    }
                    Ok(None) | Err(_) => packets.push(
                        guilds::result_packet(result::GIVE_STORY_FAIL, "", 0)
                            .map_err(|_| RuntimeError::Operation)?,
                    ),
                }
            }
            (State::OutGuild, Message::CloseGuild) => packets.push(
                guilds::result_packet(result::CLOSE_GUILD_FAIL, open_error::NO_GUILD, 0)
                    .map_err(|_| RuntimeError::Operation)?,
            ),
            (State::OutGuild, Message::LeaveGuild) => packets.push(
                guilds::result_packet(result::LEAVE_GUILD_FAIL, "", 0)
                    .map_err(|_| RuntimeError::Operation)?,
            ),
            (
                State::OutGuild,
                Message::GuildOperation(_) | Message::InviteGuild(_) | Message::GuildNoticeEdit(_),
            ) => packets.push(
                guilds::result_packet(result::GUILD_OPERATION_FAIL, "", 0)
                    .map_err(|_| RuntimeError::Operation)?,
            ),
            (State::OutGuild, Message::JoinGuild(guild_sn)) => {
                let joined = guilds::join(pool, &runtime.party, &runtime.guilds, cid, guild_sn)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                match joined {
                    Ok(sn) => {
                        packets.push(
                            guilds::result_packet(result::JOIN_GUILD_SUCCESS, "", i64::from(sn))
                                .map_err(|_| RuntimeError::Operation)?,
                        );
                        let (next, out) =
                            guilds::enter(pool, &runtime.party, &runtime.guilds, cid).await;
                        self.guild = next;
                        packets.extend(out);
                    }
                    Err(arg) => {
                        packets.push(
                            guilds::result_packet(result::JOIN_GUILD_FAIL, arg, 0)
                                .map_err(|_| RuntimeError::Operation)?,
                        );
                        // HasGuild: the proxy goes back to Initial (QueryGuildID again).
                        if arg == open_error::HAS_GUILD {
                            let (next, out) =
                                guilds::enter(pool, &runtime.party, &runtime.guilds, cid).await;
                            self.guild = next;
                            packets.extend(out);
                        }
                    }
                }
            }
            (State::InGuild { guild_sn }, Message::CloseGuild) => {
                let closed = guilds::close(pool, &runtime.party, &runtime.guilds, cid, guild_sn)
                    .await
                    .map_err(|_| RuntimeError::Operation)?;
                match closed {
                    Ok(()) => {
                        packets = self
                            .guild_kicked(runtime, pool, cid, result::CLOSE_GUILD_SUCCESS)
                            .await?;
                    }
                    Err(arg) => packets.push(
                        guilds::result_packet_arg(result::CLOSE_GUILD_FAIL, arg, 0)
                            .map_err(|_| RuntimeError::Operation)?,
                    ),
                }
            }
            (State::InGuild { guild_sn }, Message::LeaveGuild) => {
                let left =
                    guilds::leave_guild(pool, &runtime.party, &runtime.guilds, cid, guild_sn)
                        .await
                        .map_err(|_| RuntimeError::Operation)?;
                match left {
                    Some(synced) => packets = self.guild_synced(runtime, pool, cid, synced).await?,
                    None => packets.push(
                        guilds::result_packet(result::LEAVE_GUILD_FAIL, "", 0)
                            .map_err(|_| RuntimeError::Operation)?,
                    ),
                }
            }
            (State::InGuild { guild_sn }, Message::GuildOperation(operations)) => {
                let (all, synced) = guilds::operate(
                    pool,
                    &runtime.party,
                    &runtime.guilds,
                    cid,
                    guild_sn,
                    &operations.unwrap_or_default(),
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
                let kicked = synced.kicked;
                packets = self.guild_synced(runtime, pool, cid, synced).await?;
                if !kicked {
                    let code = if all {
                        result::GUILD_OPERATION_SUCCESS
                    } else {
                        result::GUILD_OPERATION_FAIL
                    };
                    packets.push(
                        guilds::result_packet(code, "", 0).map_err(|_| RuntimeError::Operation)?,
                    );
                }
            }
            (State::InGuild { guild_sn }, Message::JoinGuildChatRoom(key)) => {
                // InGuild: only the own guild's key starts JoinGuildChatRoom.
                if key != i64::from(guild_sn) {
                    self.log_unhandled(category, body);
                    return Ok(());
                }
                let joined =
                    guilds::join_chat(pool, &runtime.party, &runtime.guilds, cid, guild_sn)
                        .await
                        .map_err(|_| RuntimeError::Operation)?;
                let code = if joined {
                    result::JOIN_GUILD_CHAT_SUCCESS
                } else {
                    result::JOIN_GUILD_CHAT_FAIL
                };
                packets.push(
                    guilds::result_packet(code, "", key).map_err(|_| RuntimeError::Operation)?,
                );
            }
            (State::InGuild { guild_sn }, Message::GuildChat(chat)) => {
                let (failed, out) = guilds::chat(
                    pool,
                    &runtime.party,
                    &runtime.guilds,
                    cid,
                    guild_sn,
                    chat.message.as_deref(),
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
                packets = out;
                if let Some(code) = failed {
                    packets.push(
                        super::minimum_shop::system_message(
                            0,
                            &format!("GameUI_Heroes_InGameGuildChat_Fail_{code}"),
                            &[],
                        )
                        .map_err(|_| RuntimeError::Operation)?,
                    );
                }
            }
            // InviteGuildProcessor answers OkMessage only.
            (State::InGuild { .. }, Message::InviteGuild(_)) => {}
            (State::InGuild { guild_sn }, Message::GuildNoticeEdit(text)) => {
                let synced = guilds::edit_notice(
                    pool,
                    &runtime.party,
                    &runtime.guilds,
                    cid,
                    guild_sn,
                    text.as_deref(),
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
                packets = self.guild_synced(runtime, pool, cid, synced).await?;
            }
            (State::InGuild { guild_sn }, Message::ChangeMaster(new_master)) => {
                let (code, synced) = guilds::change_master(
                    pool,
                    &runtime.party,
                    &runtime.guilds,
                    cid,
                    guild_sn,
                    new_master.as_deref(),
                )
                .await
                .map_err(|_| RuntimeError::Operation)?;
                packets = self.guild_synced(runtime, pool, cid, synced).await?;
                packets.push(
                    guilds::change_master_message(code).map_err(|_| RuntimeError::Operation)?,
                );
            }
            (State::InGuild { .. }, Message::OpenGuild(_)) => packets.push(
                guilds::result_packet(result::OPEN_GUILD_FAIL, open_error::HAS_GUILD, 0)
                    .map_err(|_| RuntimeError::Operation)?,
            ),
            (State::InGuild { .. }, Message::JoinGuild(_)) => packets.push(
                guilds::result_packet(result::JOIN_GUILD_FAIL, open_error::HAS_GUILD, 0)
                    .map_err(|_| RuntimeError::Operation)?,
            ),
            (State::InGuild { .. }, Message::GuildStoryRequest) => packets.push(
                guilds::result_packet(result::GIVE_STORY_FAIL, "", 0)
                    .map_err(|_| RuntimeError::Operation)?,
            ),
            _ => {
                self.log_unhandled(category, body);
                return Ok(());
            }
        }
        for packet in &packets {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        tracing::info!(
            category,
            messages = packets.len(),
            "guild message processed; values omitted"
        );
        Ok(())
    }
    /// GameClient RequestWhisperMessage: the TalkService whisper once InGame (TalkConn),
    /// Whisper_Error before. InGame UpdateWhisperFilterMessage: the PlayerService
    /// WhisperFilter (ignored in other states, as InGame alone handles it).
    #[allow(clippy::too_many_arguments)]
    async fn whisper(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: Option<crate::protocol::whisper::Message>,
    ) -> Result<(), RuntimeError> {
        use super::minimum_whisper as whispers;
        use crate::protocol::whisper::Message;
        let in_game = self.selected.is_some()
            && self
                .password
                .as_ref()
                .is_some_and(|p| p.selection_allowed(&self.frontend));
        let cid = self.selected.as_ref().filter(|_| in_game).map(|s| s.cid);
        let packets = match (message, cid) {
            (Some(Message::Request(request)), Some(cid)) => {
                whispers::whisper(pool, &runtime.party, cid, &request).await
            }
            (Some(Message::Request(request)), None) => {
                whispers::fail(request.receiver.as_deref(), whispers::ERROR).map(|p| vec![p])
            }
            (Some(Message::UpdateFilter(request)), Some(cid)) => {
                let uid = self
                    .frontend
                    .accepted_identity()
                    .map_err(|_| RuntimeError::Operation)?
                    .nexon_sn;
                whispers::update_filter(pool, cid, uid, &request).await
            }
            _ => {
                self.log_unhandled(category, body);
                return Ok(());
            }
        }
        .map_err(|_| RuntimeError::Operation)?;
        for packet in &packets {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        tracing::info!(
            category,
            messages = packets.len(),
            "whisper message processed; values omitted"
        );
        Ok(())
    }
    /// ItemServiceProxy trade messages (InGame) -> the item service TradeSystem processors.
    /// UseSecondPassword passes (enEU, no in-game lock in the laboratory); the trading
    /// punishment check of the source is not migrated.
    #[allow(clippy::too_many_arguments)]
    async fn marketplace(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        category: i32,
        body: &[u8],
        message: crate::protocol::marketplace::Message,
    ) -> Result<(), RuntimeError> {
        use super::minimum_marketplace as market;
        use crate::protocol::marketplace::Message;
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
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        let ids = match &message {
            Message::TradePurchaseItem(_) | Message::TradeCancelMyItem(_) => {
                market::GIVE_ENTITY_IDS
            }
            Message::EndTradeSession => market::COLLECT_ENTITY_IDS,
            _ => 0,
        };
        let ids = minimum
            .creation
            .entity_ids(ids)
            .await
            .map_err(|_| RuntimeError::Operation)?;
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
        let outcome = match message {
            Message::TradeCategorySearch(m) => {
                market::category_search(pool, assets, &mut self.trade_search, &m).await
            }
            Message::TradeItemClassListSearch(m) => {
                market::class_search(pool, assets, &mut self.trade_search, cid, &m).await
            }
            Message::TradeRequestMyItem => market::my_items(pool, assets, cid).await,
            Message::TradePurchaseItem(m) => {
                market::purchase(pool, &owner, assets, cid, &m, ids).await
            }
            Message::TradeCancelMyItem(m) => {
                market::cancel(pool, &owner, assets, cid, &m, ids).await
            }
            Message::InsertTradeOrder(m) => {
                market::insert_order(pool, &owner, assets, cid, &m).await
            }
            Message::InsertTradeTirOrder(_) => market::insert_tir_order(),
            // EndTradeSessionMessage -> TradeGetMyItems -> TradeManager.GetMyThings.
            Message::EndTradeSession => market::get_my_things(pool, &owner, assets, cid, ids).await,
            // "Receive Deprecated Message StartTradeSessionMessage": no answer.
            _ => {
                tracing::info!(category, "trade message ignored; values omitted");
                return Ok(());
            }
        };
        match outcome {
            Ok(out) => {
                for packet in &out.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                tracing::info!(
                    category,
                    messages = out.packets.len(),
                    "minimum trade request processed; values omitted"
                );
            }
            Err(_) => tracing::warn!(category, "minimum trade request refused; values omitted"),
        }
        Ok(())
    }
    /// ItemTradeProxy construction (the first ProcessDailyItem of the selected character):
    /// TradeManager.GetMyThings mails the expired registrations and the sales' gold.
    async fn trade_things(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        if self.trade_proxy {
            return Ok(());
        }
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
            return Ok(());
        };
        self.trade_proxy = true;
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
            .entity_ids(super::minimum_marketplace::COLLECT_ENTITY_IDS)
            .await
            .map_err(|_| RuntimeError::Operation)?;
        match super::minimum_marketplace::get_my_things(pool, &owner, assets, cid, ids).await {
            Ok(out) => {
                for packet in &out.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
            }
            Err(_) => tracing::warn!("minimum trade collection rejected; values omitted"),
        }
        Ok(())
    }
    /// MailBox.PushMailList: ProcessMailList and MailListMessage to this connection.
    async fn push_mail_list(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
            return Ok(());
        };
        let assets = &runtime
            .minimum
            .as_ref()
            .ok_or(RuntimeError::Profile)?
            .assets
            .current();
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
        match super::minimum_mail::mail_list(pool, &owner, assets, cid).await {
            Ok(out) => {
                for packet in &out.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                for (to, mail_id) in out.notices {
                    runtime.party.notice_mail(to, mail_id);
                }
            }
            Err(_) => tracing::warn!("minimum mail push rejected; values omitted"),
        }
        Ok(())
    }
    /// TimeLine.ScheduleTimeLine: every 30 s the due jobs run (each one PushMailList).
    async fn mail_jobs(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        self.mail_at = Some(
            tokio::time::Instant::now()
                + Duration::from_millis(crate::domain::mail::TIMELINE_INTERVAL_MS),
        );
        let Ok(now) = super::minimum_mail::now_ticks() else {
            return Ok(());
        };
        if self.mail_timeline.poll(now) > 0 {
            self.push_mail_list(runtime, pool, stream, sender).await?;
        }
        Ok(())
    }
    /// NoticeMailProcessor → MailBox.NoticeMail(mail): ScheduleMail(isJustSent: true); an
    /// arrived mail's job runs at once.
    async fn mail_notice(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        mail_id: i64,
    ) -> Result<(), RuntimeError> {
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
            return Ok(());
        };
        let assets = &runtime
            .minimum
            .as_ref()
            .ok_or(RuntimeError::Profile)?
            .assets
            .current();
        let (Ok(times), Ok(now)) = (
            super::minimum_mail::schedule_notice(pool, assets, cid, mail_id).await,
            super::minimum_mail::now_ticks(),
        ) else {
            tracing::warn!("mail notice not scheduled; values omitted");
            return Ok(());
        };
        let mut due = false;
        for at in times {
            due |= self.mail_timeline.add(at, now);
        }
        if due {
            self.push_mail_list(runtime, pool, stream, sender).await?;
        }
        Ok(())
    }
    /// Comm.NoticeMail(own CID) of a ForceMail give in this connection's last request: the
    /// express system mail is due at once.
    async fn own_mail_notice(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
            return Ok(());
        };
        if !super::minimum_mail::take_notice(cid) {
            return Ok(());
        }
        let assets = &runtime
            .minimum
            .as_ref()
            .ok_or(RuntimeError::Profile)?
            .assets
            .current();
        if let (Ok(times), Ok(now)) = (
            super::minimum_mail::schedule_all(pool, assets, cid).await,
            super::minimum_mail::now_ticks(),
        ) {
            for at in times {
                let _ = self.mail_timeline.add(at, now);
            }
        }
        self.push_mail_list(runtime, pool, stream, sender).await
    }
    /// PetManager.AddToUpdateSchedule: wake at the next period start (ticks).
    fn schedule_pets(&mut self, next: Option<i64>) {
        self.pet_reset_at = next.and_then(|at| {
            let now = super::minimum_status_effect::now_ticks().ok()?;
            let ticks = u64::try_from(at.saturating_sub(now)).ok()?;
            Some(
                tokio::time::Instant::now()
                    + Duration::from_nanos(ticks.saturating_mul(100))
                    + Duration::from_secs(1),
            )
        });
    }
    /// PetManager.ScheduledUpdate → DailyActiveTimeResetProcess of the selected account.
    async fn pet_jobs(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        self.pet_reset_at = None;
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
            return Ok(());
        };
        let assets = &runtime
            .minimum
            .as_ref()
            .ok_or(RuntimeError::Profile)?
            .assets
            .current();
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
        let reset = super::minimum_pet::scheduled_reset(pool, &owner, assets, cid).await;
        match reset {
            Ok((out, next)) => {
                for packet in &out.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                self.schedule_pets(next);
                if let Some(pet) = out.current {
                    self.pet_changed(runtime, pool, assets, cid, pet, stream, sender)
                        .await?;
                }
                tracing::info!("pet active time period processed; values omitted");
            }
            Err(_) => tracing::warn!("pet active time reset rejected; values omitted"),
        }
        Ok(())
    }
    /// A push of the party authority: UpdateParty/SendPacket packets, or the Kick operation
    /// (KickProcessor: LeaveParty and KickedMessage; the MicroPlay of this member ends).
    async fn party_notice(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        notice: super::party_hub::Notice,
    ) -> Result<(), RuntimeError> {
        let packets = match notice {
            super::party_hub::Notice::Packets(packets) => {
                let mut packets = packets;
                // A MicroPlayMember follows the party state; a party sent back to town
                // (the host left the ship) ends its ship.
                if let Some(m) = self.mission.as_mut()
                    && !m.host
                {
                    match runtime.party.role(m.completion.cid) {
                        super::party_hub::Role::Member { state, slot, .. }
                            if state != crate::domain::party::party_state::IN_TOWN =>
                        {
                            m.state = state;
                            m.member.slot = slot;
                        }
                        super::party_hub::Role::Master { state, .. }
                            if state != crate::domain::party::party_state::IN_TOWN => {}
                        _ => {
                            if let Some(mut m) = self.mission.take() {
                                if let Some(out) = self.leave_pets(runtime, pool, &mut m).await {
                                    packets.extend(out.packets);
                                }
                                self.leave_relay(runtime, &m);
                            }
                        }
                    }
                }
                packets
            }
            super::party_hub::Notice::Play(message) => {
                if self.mission.as_ref().is_some_and(|m| !m.host) {
                    self.replaying = true;
                    let applied = self.mission(runtime, pool, stream, sender, *message).await;
                    self.replaying = false;
                    applied?;
                }
                return Ok(());
            }
            super::party_hub::Notice::Started(snapshot) => {
                if let Some(m) = self.mission.as_mut().filter(|m| !m.host) {
                    super::minimum_mission::replica_start(m, *snapshot);
                    tracing::info!("member replica of the play started; values omitted");
                }
                return Ok(());
            }
            super::party_hub::Notice::Give { class_ex, count } => {
                return self
                    .give_erg(runtime, pool, stream, sender, &class_ex, count)
                    .await;
            }
            super::party_hub::Notice::BecomeHost(_) => {
                return self.become_host(runtime, pool, stream, sender).await;
            }
            super::party_hub::Notice::HostRestarted { host, info } => {
                return self
                    .host_restarted(runtime, pool, stream, sender, host, info)
                    .await;
            }
            super::party_hub::Notice::FollowHost => {
                if let Some(m) = self.mission.as_deref_mut() {
                    super::minimum_mission_finish::follow_host(m, std::time::Instant::now());
                    tracing::info!("mission player follows the leaving host; values omitted");
                }
                return Ok(());
            }
            super::party_hub::Notice::ReturnNow => {
                if self.mission.is_some() {
                    return self
                        .mission(
                            runtime,
                            pool,
                            stream,
                            sender,
                            crate::protocol::mission::Message::ReturnToTown,
                        )
                        .await;
                }
                return Ok(());
            }
            super::party_hub::Notice::Sink => {
                // NoReward_SinkShip: ReturnFromQuest(ToTown), then MicroPlayer.Leave.
                let mut packets = vec![
                    super::minimum_mission_finish::to_town().map_err(|_| RuntimeError::Message)?,
                ];
                if let Some(mut m) = self.mission.take() {
                    if let Some(out) = self.leave_pets(runtime, pool, &mut m).await {
                        packets.extend(out.packets);
                    }
                    self.leave_relay(runtime, &m);
                    packets.extend(runtime.party.end_for(m.completion.cid));
                }
                tracing::info!("mission ship sunk after a failed host election; values omitted");
                packets
            }
            super::party_hub::Notice::Mail { mail_id } => {
                return self
                    .mail_notice(runtime, pool, stream, sender, mail_id)
                    .await;
            }
            super::party_hub::Notice::Push(packets) => packets,
            super::party_hub::Notice::GuildKicked { result } => {
                // InGuild.GuildKicked: GuildResultMessage(value), then Initial
                // (QueryGuildID → OutGuild or a new connection).
                let mut packets = vec![
                    super::guild::result_packet(result, "", 0)
                        .map_err(|_| RuntimeError::Operation)?,
                ];
                if let Some(cid) = self.selected.as_ref().map(|s| s.cid) {
                    let (next, out) =
                        super::guild::enter(pool, &runtime.party, &runtime.guilds, cid).await;
                    self.guild = next;
                    packets.extend(out);
                }
                packets
            }
            super::party_hub::Notice::AllUserGoalReward { goal_id } => {
                return self
                    .all_user_goal_reward(runtime, pool, stream, sender, Some(goal_id))
                    .await;
            }
            super::party_hub::Notice::ProcessQueue => {
                return self
                    .all_user_goal_reward(runtime, pool, stream, sender, None)
                    .await;
            }
            super::party_hub::Notice::Announce {
                packet,
                town_only,
                level,
            } => {
                // BroadcastToAllClients: IsInTown with IsTownOnly, then AnnounceLevel.
                if town_only && self.mission.is_some() {
                    return Ok(());
                }
                if level != 0 {
                    let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
                        return Ok(());
                    };
                    let own: Option<i32> =
                        sqlx::query_scalar("SELECT level FROM game_character_info WHERE id=$1")
                            .bind(cid)
                            .fetch_optional(pool)
                            .await
                            .map_err(|_| RuntimeError::Operation)?;
                    if own.is_none_or(|l| l < level) {
                        return Ok(());
                    }
                }
                vec![packet]
            }
            super::party_hub::Notice::Kicked { .. } => {
                let mut packets = Vec::new();
                if let Some(mut m) = self.mission.take() {
                    if let Some(out) = self.leave_pets(runtime, pool, &mut m).await {
                        packets.extend(out.packets);
                    }
                    self.leave_relay(runtime, &m);
                }
                packets.extend(
                    super::minimum_party::kicked_by_party().map_err(|_| RuntimeError::Message)?,
                );
                tracing::info!("party removed the entered character; values omitted");
                packets
            }
        };
        for packet in &packets {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        Ok(())
    }
    /// MicroPlay.ChangeHost: this player's replica takes over as the game host; the
    /// election fails after HOST_RESTART_TIMEOUT (120 s) without its RegisterServer.
    async fn become_host(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        let (Some(selected), Some(password)) = (self.selected.as_ref(), self.password.as_ref())
        else {
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
            revision: password.revision(),
        };
        let Some(m) = self.mission.as_deref_mut() else {
            return Ok(());
        };
        let assets = &runtime
            .minimum
            .as_ref()
            .ok_or(RuntimeError::Profile)?
            .assets
            .current();
        let packets = super::minimum_mission::host_takeover(
            pool,
            &owner,
            assets,
            cid,
            m,
            runtime.minimum_header()?,
            runtime.stat_arithmetic(),
        )
        .await
        .map_err(|_| RuntimeError::Operation)?;
        let hub = runtime.party.clone();
        let party_id = m.party_id;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(120)).await;
            hub.election_timeout(party_id, cid);
        });
        for packet in &packets {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        tracing::info!("mission replica took over as host; values omitted");
        Ok(())
    }
    /// HostElectionFinished for this player: HostInfoMessage of the new host's server,
    /// and this player's MemberInfo to the new host.
    async fn host_restarted(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        host: i64,
        info: Option<crate::protocol::mission::GameInfo>,
    ) -> Result<(), RuntimeError> {
        let (Some(selected), Some(password)) = (self.selected.as_ref(), self.password.as_ref())
        else {
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
            revision: password.revision(),
        };
        let Some(m) = self.mission.as_deref_mut() else {
            return Ok(());
        };
        if let Some(slot) = m.party.slot_of(m.party_id, cid) {
            m.member.slot = slot;
        }
        m.host_info = info.clone();
        let assets = &runtime
            .minimum
            .as_ref()
            .ok_or(RuntimeError::Profile)?
            .assets
            .current();
        match super::minimum_mission::member_info_packet(
            pool,
            &owner,
            assets,
            cid,
            m,
            runtime.minimum_header()?,
            runtime.stat_arithmetic(),
        )
        .await
        {
            Ok(member) => runtime.party.send(host, vec![member]),
            Err(_) => tracing::warn!("member info for the new host unavailable; values omitted"),
        }
        let packet = super::minimum_mission::packet(&crate::protocol::mission::Message::HostInfo(
            crate::protocol::mission::HostInfo {
                game_info: info,
                uid: m.member.slot,
                key: m.member_key,
                skip_lobby: false,
                is_transfer_to_ds: false,
            },
        ))
        .map_err(|_| RuntimeError::Message)?;
        send(stream, sender, packet.category, &packet.body).await?;
        tracing::info!("mission member reached the new host; values omitted");
        Ok(())
    }
    /// MicroPlay.RemovePlayer of this player: the relay group, and CheckHostDisconnected
    /// (a host leaving a play in progress starts the host election).
    fn leave_play(&self, runtime: &Runtime, mission: &super::minimum_mission::Mission) -> bool {
        let mut elected = false;
        if mission.host && super::minimum_mission::play_open(mission) {
            match runtime
                .party
                .elect(mission.party_id, mission.completion.cid)
            {
                Ok(Some(_)) => {
                    elected = true;
                    tracing::info!("mission host election started; values omitted");
                }
                Ok(None) => {}
                Err(_) => tracing::warn!("mission host election failed; values omitted"),
            }
        }
        self.leave_relay(runtime, mission);
        elected
    }
    /// MicroPlayer.SendMessage to every player (SendMessageAllPlayers) for this player's
    /// own GetItem and personal QuestTargetResult: the other players see them too.
    fn share_with_play(
        runtime: &Runtime,
        mission: Option<&super::minimum_mission::Mission>,
        packets: &[crate::domain::map_partition::OutgoingPacket],
    ) {
        use crate::protocol::mission::{self as wire, Kind, Message};
        let Some(m) = mission else {
            return;
        };
        let shared: Vec<_> = packets
            .iter()
            .filter(|p| {
                p.category == Kind::GetItem.category()
                    || (p.category == Kind::QuestTargetResult.category()
                        && matches!(
                            wire::decode(Kind::QuestTargetResult, &p.body, DEFAULT_MAX_BODY),
                            Ok(Some(Message::QuestTargetResult(r)))
                                if r.character_name.as_deref().is_some_and(|n| !n.is_empty())
                        ))
            })
            .cloned()
            .collect();
        if shared.is_empty() {
            return;
        }
        for other in runtime.party.others(m.party_id, m.completion.cid) {
            runtime.party.send(other, shared.clone());
        }
    }
    /// An erg this player won in another player's pick: MicroPlayer GiveItem.
    async fn give_erg(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        class_ex: &str,
        count: i32,
    ) -> Result<(), RuntimeError> {
        let (Some(selected), Some(password)) = (self.selected.as_ref(), self.password.as_ref())
        else {
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
            revision: password.revision(),
        };
        let Some(state) = self.mission.as_deref_mut() else {
            return Ok(());
        };
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        let ids = minimum
            .creation
            .entity_ids(1)
            .await
            .map_err(|_| RuntimeError::Operation)?;
        let mut ids = ids.into_iter();
        let packets = match super::minimum_mission::give_reward(
            pool,
            &owner,
            assets,
            state,
            cid,
            class_ex,
            count,
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
                tracing::warn!("mission erg give refused by the give planner; values omitted");
                return Ok(());
            }
        };
        Self::share_with_play(runtime, self.mission.as_deref(), &packets);
        for packet in &packets {
            send(stream, sender, packet.category, &packet.body).await?;
        }
        tracing::info!("mission erg of another pick given; values omitted");
        Ok(())
    }
    /// The relay side of leaving a MicroPlay: P2PGroup.Leave of this player; the group
    /// closes once empty.
    fn leave_relay(&self, runtime: &Runtime, mission: &super::minimum_mission::Mission) {
        if let Some(relay) = runtime.relay() {
            relay.leave(mission.group, mission.completion.cid);
        }
    }
    /// MicroPlay.RemovePlayer: PetManager.EndBattle(includeExpInfo: false) when the player
    /// leaves the play while its pets still count.
    async fn leave_pets(
        &self,
        runtime: &Runtime,
        pool: &PgPool,
        mission: &mut super::minimum_mission::Mission,
    ) -> Option<super::minimum_pet::Outcome> {
        if !mission.pet_usable() {
            return None;
        }
        let rewards = mission.pets.end_battle(false, std::time::Instant::now());
        if rewards.is_empty() {
            return None;
        }
        let assets = &runtime.minimum.as_ref()?.assets.current();
        let id = self.frontend.accepted_identity().ok()?;
        let owner = super::character_lobby::UserKey {
            uid: id.nexon_sn,
            name: &id.nexon_id,
            revision: self.password.as_ref()?.revision(),
        };
        match super::minimum_pet::reward_end_battle(
            pool,
            &owner,
            assets,
            mission.completion.cid,
            &rewards,
        )
        .await
        {
            Ok(out) => Some(out),
            Err(_) => {
                tracing::warn!("pet battle reward rejected; values omitted");
                None
            }
        }
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
            // MicroPlayInformer: AllUserGoalEvent for the events of this player.
            let mut party = runtime.party.others(state.party_id, cid);
            party.push(cid);
            for (actor, event) in &events {
                if *actor != cid {
                    continue;
                }
                match super::minimum_all_user_goal::inform(pool, assets, cid, event, &party).await {
                    Ok(Some(informed)) => {
                        for p in informed.broadcast {
                            runtime.party.announce_all(p, false, 0);
                        }
                        if let Some(goal_id) = informed.reward {
                            runtime.party.all_user_goal_reward(goal_id);
                        }
                    }
                    Ok(None) => {}
                    Err(_) => tracing::warn!("all user goal count rejected; values omitted"),
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
        // ProcessDailyItemProcessor creates the missing ItemOwner.TradeProxy before the
        // daily procedures.
        self.trade_things(runtime, pool, stream, sender).await?;
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
        // ProcessDailyItemProcessor: QueuedItemController.ProcessQueuedItem before the
        // daily procedures.
        match super::minimum_queued_item::process(pool, &owner, assets, cid, &mut || {
            ids.next()
                .ok_or(super::minimum_character::MinimumError::Conflict)
        })
        .await
        {
            Ok(done) => {
                for packet in &done.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
            }
            Err(_) => tracing::warn!("queued items rejected; values omitted"),
        }
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
        // DailyItemProcedure UpdateCharacterRankTitle / UpdateRankTitle (B5 RK3).
        let loaded = !matches!(self.guild, super::guild::State::Initial);
        let journal = self.selected.as_ref().map_or(0, |s| s.log_journal);
        match super::minimum_daily_item::rank_title_processes(pool, &owner, assets, cid, loaded)
            .await
        {
            Ok(kinds) => {
                for kind in kinds {
                    let packets = match kind {
                        crate::domain::daily_item::ProcessKind::UpdateCharacterRankTitle => {
                            match rank_title_update(
                                runtime,
                                pool,
                                &owner,
                                assets,
                                journal,
                                cid,
                                &mut self.rank_titles,
                                crate::domain::rank::TitleUpdate::Character,
                            )
                            .await
                            {
                                Ok(mut packets) => super::rank_title::updated_telepathy(
                                    assets,
                                    &self.rank_titles,
                                    false,
                                )
                                .map(|t| {
                                    packets.extend(t);
                                    packets
                                }),
                                Err(e) => Err(e),
                            }
                        }
                        _ => super::rank_title::updated_telepathy(assets, &self.rank_titles, true),
                    };
                    match packets {
                        Ok(packets) => {
                            for packet in &packets {
                                send(stream, sender, packet.category, &packet.body).await?;
                            }
                        }
                        Err(_) => tracing::warn!("rank title process rejected; values omitted"),
                    }
                }
            }
            Err(_) => tracing::warn!("rank title processes rejected; values omitted"),
        }
        if let Ok(Some(after)) = super::minimum_daily_item::next_range(assets) {
            // DailyItemScheduler schedules three seconds after the range start.
            self.daily_at = Some(tokio::time::Instant::now() + after + Duration::from_secs(3));
        }
        Ok(())
    }
    /// PeriodicEventScheduler.OnScheduledTimeLine → ItemOwnerEvent: the next period is
    /// added first, then the BoostEvent reward and its announce to every client.
    async fn boost_event(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        self.boost_at = None;
        let Some((cid, journal)) = self.selected.as_ref().map(|s| (s.cid, s.log_journal)) else {
            return Ok(());
        };
        let minimum = runtime.minimum.as_ref().ok_or(RuntimeError::Profile)?;
        let assets = &minimum.assets.current();
        self.boost_at =
            super::minimum_periodic_event::period(assets).map(|p| tokio::time::Instant::now() + p);
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
        let ids = minimum
            .creation
            .entity_ids(super::minimum_story::TALK_ENTITY_IDS)
            .await
            .map_err(|_| RuntimeError::Operation)?;
        match super::minimum_periodic_event::boost(
            pool,
            &owner,
            assets,
            cid,
            runtime.growth_context(journal)?,
            ids,
        )
        .await
        {
            Ok(boost) => {
                for packet in &boost.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                if let Some(a) = boost.announce {
                    runtime.party.announce_all(a.packet, a.town_only, a.level);
                }
                if !boost.packets.is_empty() {
                    tracing::info!("periodic event reward given; values omitted");
                }
            }
            Err(_) => tracing::warn!("periodic event reward rejected; values omitted"),
        }
        Ok(())
    }
    /// AllUserGoalEventScheduler for this client: the next push after the ones sent.
    fn schedule_goals(
        goal_at: &mut Option<tokio::time::Instant>,
        goal_sent: &mut i64,
        assets: &super::minimum_assets::Assets,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
        if *goal_sent == 0 {
            *goal_sent = now;
        }
        *goal_at = super::minimum_all_user_goal::schedule(assets, now)
            .into_iter()
            .map(|(t, _, _)| t)
            .find(|t| *t > *goal_sent)
            .map(|t| {
                tokio::time::Instant::now()
                    + Duration::from_secs(u64::try_from(t - now).unwrap_or(0))
            });
    }
    /// The AllUserGoalEventModify pushes due now, then the next schedule.
    async fn goal_pushes(
        &mut self,
        runtime: &Runtime,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
    ) -> Result<(), RuntimeError> {
        self.goal_at = None;
        let Some(minimum) = runtime.minimum.as_ref() else {
            return Ok(());
        };
        let assets = &minimum.assets.current();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
        // The pushes computed before the boundary, due between the last send and now.
        let since = self.goal_sent;
        for (_, goal_id, count) in super::minimum_all_user_goal::schedule(assets, since)
            .into_iter()
            .filter(|(t, _, _)| *t > since && *t <= now)
        {
            let message = crate::protocol::all_user_goal::Message::EventModify { goal_id, count };
            if let Ok(p) = super::minimum_all_user_goal::packet(&message) {
                send(stream, sender, p.category, &p.body).await?;
            }
        }
        self.goal_sent = now;
        Self::schedule_goals(&mut self.goal_at, &mut self.goal_sent, assets);
        Ok(())
    }
    /// AllUserGoalRewardProcessor.RefreshQueuedItemAction: the queued items, then
    /// AllUserGoalRewardMessage when event rows were met.
    async fn all_user_goal_reward(
        &mut self,
        runtime: &Runtime,
        pool: &PgPool,
        stream: &mut TcpStream,
        sender: &mut LegacySender,
        goal_id: Option<i32>,
    ) -> Result<(), RuntimeError> {
        let (Some(cid), Some(password), Some(minimum)) = (
            self.selected.as_ref().map(|s| s.cid),
            self.password.as_ref(),
            runtime.minimum.as_ref(),
        ) else {
            return Ok(());
        };
        let id = self
            .frontend
            .accepted_identity()
            .map_err(|_| RuntimeError::Operation)?;
        let owner = super::character_lobby::UserKey {
            uid: id.nexon_sn,
            name: &id.nexon_id,
            revision: password.revision(),
        };
        let assets = &minimum.assets.current();
        let ids = minimum
            .creation
            .entity_ids(64)
            .await
            .map_err(|_| RuntimeError::Operation)?;
        let mut ids = ids.into_iter();
        match super::minimum_queued_item::process(pool, &owner, assets, cid, &mut || {
            ids.next()
                .ok_or(super::minimum_character::MinimumError::Conflict)
        })
        .await
        {
            Ok(done) => {
                for packet in &done.packets {
                    send(stream, sender, packet.category, &packet.body).await?;
                }
                if done.event_rows > 0
                    && let Some(goal_id) = goal_id
                    && let Ok(p) = super::minimum_all_user_goal::packet(
                        &crate::protocol::all_user_goal::Message::Reward(goal_id),
                    )
                {
                    send(stream, sender, p.category, &p.body).await?;
                }
                tracing::info!("all user goal reward processed; values omitted");
            }
            Err(_) => tracing::warn!("all user goal reward rejected; values omitted"),
        }
        Ok(())
    }
    /// StoryEntity.ScheduledUpdate → DailyProcess of the entered character.
    async fn story_periods(&mut self, runtime: &Runtime, pool: &PgPool) {
        self.story_at = None;
        let Some(cid) = self.selected.as_ref().map(|s| s.cid) else {
            return;
        };
        let (Ok(id), Some(password), Some(minimum)) = (
            self.frontend.accepted_identity(),
            self.password.as_ref(),
            runtime.minimum.as_ref(),
        ) else {
            return;
        };
        let owner = super::character_lobby::UserKey {
            uid: id.nexon_sn,
            name: &id.nexon_id,
            revision: password.revision(),
        };
        let assets = &minimum.assets.current();
        match super::minimum_periodic_event::daily_story(pool, &owner, assets, cid).await {
            // AddToUpdateSchedule rounds a boundary already passed to the next one.
            Ok(next) => {
                self.story_at =
                    next.map(|n| tokio::time::Instant::now() + n.max(Duration::from_secs(1)))
            }
            Err(_) => tracing::warn!("minimum daily story process rejected; values omitted"),
        }
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
        let ap_before = state.completion.given.2;
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
        // RewardCharacterProcessor → APManager.GiveAPWithGuild for the AP just given.
        let ap = state.completion.given.2.wrapping_sub(ap_before);
        if ap > 0
            && let Some(economy) = assets.economy.as_ref()
        {
            let gp = match super::guild::levels(economy.guild_levels(), &runtime.content.policy) {
                Ok(levels) => {
                    super::guild::give_ap_gp(
                        pool,
                        &runtime.party,
                        &runtime.guilds,
                        &levels,
                        state.completion.cid,
                        ap,
                    )
                    .await
                }
                Err(e) => Err(e),
            };
            match gp {
                Ok(out) => packets.extend(out),
                Err(_) => tracing::warn!("guild GP of the mission AP rejected; values omitted"),
            }
        }
        packets.extend(Self::mission_titles(state, runtime, pool, &owner, journal).await?);
        // RewardPetEndBattle changed the current pet (PushUpdatePetStatus).
        let pet_summary = state.pets.take_summary();
        // GiveReward_Null → Leave: the proxy left the party with the MicroPlay.
        Self::share_with_play(runtime, Some(&*state), &packets);
        if state.completion.left
            && let Some(m) = self.mission.take()
        {
            self.leave_play(runtime, &m);
            packets.extend(runtime.party.end_for(m.completion.cid));
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
        if let Some(pet) = pet_summary {
            self.pet_changed(runtime, pool, assets, cid, pet, stream, sender)
                .await?;
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
                        let cid = selected.cid;
                        self.selected = Some(selected);
                        // The entered character becomes reachable by the party authority
                        // (PlayerService QueryFrontendID).
                        match super::minimum_party::name_of(pool, cid).await {
                            Ok(name) => {
                                let (tx, rx) =
                                    tokio::sync::mpsc::channel(super::party_hub::MAILBOX);
                                runtime.party.register(cid, &name, tx);
                                self.party_notices = Some(rx);
                            }
                            Err(_) => tracing::warn!(
                                "party seat of the entered character unavailable; values omitted"
                            ),
                        }
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
                        // (DailyQuestProcess), TriggerDailyStory (StoryLinePeriodInfo),
                        // QueryStoryProgress and QueryStoryGuide.
                        let mut entered = Vec::new();
                        let mut mail_list = Vec::new();
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
                            self.story_at = None;
                            match super::minimum_periodic_event::daily_story(
                                pool, &owner, assets, cid,
                            )
                            .await
                            {
                                Ok(next) => {
                                    self.story_at = next.map(|n| tokio::time::Instant::now() + n)
                                }
                                Err(_) => tracing::warn!(
                                    "minimum daily story process rejected; values omitted"
                                ),
                            }
                            // InGame.OnEnter: FirstIncomeMessage, one Notice a second from 20 s.
                            let first = runtime
                                .schedule()
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .first_income();
                            for (n, line) in first.into_iter().enumerate() {
                                if let Ok(p) = super::minimum_shop::system_message(1, &line, &[]) {
                                    let hub = runtime.party.clone();
                                    tokio::spawn(async move {
                                        tokio::time::sleep(Duration::from_secs(20 + n as u64))
                                            .await;
                                        hub.send(cid, vec![p]);
                                    });
                                }
                            }
                            // SelectCharacterProcessor: RefreshAllUserGoalEvent.
                            match super::minimum_all_user_goal::login_message(pool, assets).await {
                                Ok(Some(p)) => entered.push(p),
                                Ok(None) => {}
                                Err(_) => {
                                    tracing::warn!("all user goal refresh rejected; values omitted")
                                }
                            }
                            Self::schedule_goals(&mut self.goal_at, &mut self.goal_sent, assets);
                            match super::minimum_story::in_game_sync(pool, &owner, assets, cid)
                                .await
                            {
                                Ok(packets) => entered.extend(packets),
                                Err(_) => tracing::warn!(
                                    "minimum in-game story sync rejected; values omitted"
                                ),
                            }
                        }
                        let cid = self.selected.as_ref().ok_or(RuntimeError::Operation)?.cid;
                        match super::minimum_mail::mail_list(pool, &owner, assets, cid).await {
                            Ok(out) => {
                                mail_list = out.packets;
                                for (to, mail_id) in out.notices {
                                    runtime.party.notice_mail(to, mail_id);
                                }
                            }
                            Err(_) => tracing::warn!(
                                "minimum mail list rejected at selection; values omitted"
                            ),
                        }
                        // ItemOwner load: PetManager.Load (PetList and the shared food).
                        {
                            let cid = self.selected.as_ref().ok_or(RuntimeError::Operation)?.cid;
                            match super::minimum_pet::enter(pool, &owner, assets, cid).await {
                                Ok((packets, next)) => {
                                    entered.extend(packets);
                                    self.schedule_pets(next);
                                }
                                Err(_) => {
                                    tracing::warn!("minimum pet load rejected; values omitted")
                                }
                            }
                        }
                        // ItemOwner activation: TimeLine(Service.Thread, this, 30000) and
                        // ScheduleAllMail; ItemServiceProxy.Open then sends UpdateMailList.
                        let mail_times = super::minimum_mail::schedule_all(pool, assets, cid).await;
                        let mail_now = super::minimum_mail::now_ticks();
                        self.mail_timeline.clear();
                        self.mail_at = Some(
                            tokio::time::Instant::now()
                                + Duration::from_millis(crate::domain::mail::TIMELINE_INTERVAL_MS),
                        );
                        if let (Ok(times), Ok(now)) = (mail_times, mail_now) {
                            for at in times {
                                // A past job is the list the proxy sends right after.
                                let _ = self.mail_timeline.add(at, now);
                            }
                        }
                        entered.extend(mail_list);
                        // InGame entry: QueryWhisperFilter → WhisperFilterListMessage.
                        match super::minimum_whisper::filter_list_message(pool, cid).await {
                            Ok(p) => entered.push(p),
                            Err(_) => {
                                tracing::warn!("whisper filter list rejected; values omitted")
                            }
                        }
                        // GuildServiceProxy attached on InGame entry.
                        let (guild, packets) =
                            super::guild::enter(pool, &runtime.party, &runtime.guilds, cid).await;
                        self.guild = guild;
                        entered.extend(packets);
                        // GuildInfoManager.UpdateBriefGuildInfo on the first login:
                        // UpdateRankTitles once the guild info is known (B5 RK3).
                        self.rank_titles.clear();
                        let journal = self.selected.as_ref().map_or(0, |s| s.log_journal);
                        let id = self
                            .frontend
                            .accepted_identity()
                            .map_err(|_| RuntimeError::Operation)?;
                        let titled = super::character_lobby::UserKey {
                            uid: id.nexon_sn,
                            name: &id.nexon_id,
                            revision: self
                                .password
                                .as_ref()
                                .ok_or(RuntimeError::Operation)?
                                .revision(),
                        };
                        match rank_title_update(
                            runtime,
                            pool,
                            &titled,
                            assets,
                            journal,
                            cid,
                            &mut self.rank_titles,
                            crate::domain::rank::TitleUpdate::Both,
                        )
                        .await
                        {
                            Ok(packets) => entered.extend(packets),
                            Err(_) => tracing::warn!("rank titles rejected; values omitted"),
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
                        // PeriodicEventScheduler.OnLogOn: AddEvent when PeriodicEventWaiting.
                        self.boost_at = super::minimum_periodic_event::period(assets)
                            .map(|p| tokio::time::Instant::now() + p);
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
                Kind::PartyInvite,
                Kind::PartyInvitationAccept,
                Kind::PartyInvitationReject,
                Kind::Kick,
                Kind::JoinShip,
                Kind::Ready,
                Kind::RequestJoinParty,
                Kind::PartyChat,
                Kind::TeacherRequest,
                Kind::TeacherRequestRespond,
                Kind::AcceptAssist,
                Kind::AnswerFinishQuest,
                Kind::OpenPartyWithShipInfo,
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
            use crate::protocol::mail::{self as wire, Kind};
            for kind in Kind::INGRESS {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                    .ok_or(RuntimeError::Message)?;
                return self
                    .mail(runtime, pool, stream, sender, category, body, message)
                    .await;
            }
        }
        {
            use crate::protocol::pvp::{self as wire, Kind};
            for kind in Kind::INGRESS {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?;
                return self
                    .pvp_message(runtime, pool, category, body, message)
                    .await;
            }
        }
        {
            use crate::protocol::title::{self as wire, Kind};
            for kind in Kind::INGRESS {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?;
                return self
                    .title_message(pool, stream, sender, category, body, message)
                    .await;
            }
        }
        {
            use crate::protocol::rank::{self as wire, Kind};
            for kind in Kind::INGRESS {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?;
                return self
                    .rank_message(runtime, pool, stream, sender, category, body, message)
                    .await;
            }
        }
        // Newbie guild recommendation and direct guild purchase: unregistered (GUID envelope).
        if let Some(kind) = crate::protocol::guild_newbie::ingress_kind(category, body) {
            let message = crate::protocol::guild_newbie::decode_ingress(
                kind,
                category,
                body,
                DEFAULT_MAX_BODY,
            )
            .map_err(|_| RuntimeError::Message)?;
            return self.guild_newbie_message(runtime, pool, category, body, message).await;
        }
        // Guild storage messages: unregistered, so only their GUID envelope is theirs.
        if let Some(kind) = crate::protocol::guild_storage::ingress_kind(category, body) {
            let message = crate::protocol::guild_storage::decode_ingress(
                kind,
                category,
                body,
                DEFAULT_MAX_BODY,
            )
            .map_err(|_| RuntimeError::Message)?;
            return self
                .guild_storage_message(runtime, pool, stream, sender, category, body, message)
                .await;
        }
        {
            use crate::protocol::guild::{self as wire, Kind};
            for kind in Kind::INGRESS {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?;
                return self
                    .guild_message(runtime, pool, stream, sender, category, body, message)
                    .await;
            }
        }
        {
            use crate::protocol::whisper::{self as wire, Kind};
            for kind in Kind::INGRESS {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?;
                return self
                    .whisper(runtime, pool, stream, sender, category, body, message)
                    .await;
            }
        }
        {
            use crate::protocol::marketplace::{self as wire, Kind};
            for kind in Kind::INGRESS {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                    .ok_or(RuntimeError::Message)?;
                return self
                    .marketplace(runtime, pool, stream, sender, category, body, message)
                    .await;
            }
        }
        {
            use crate::protocol::coupon_shop as wire;
            if let Some(kind) = wire::ingress_kind(category, body) {
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                    .ok_or(RuntimeError::Message)?;
                return self
                    .coupon_shop(runtime, pool, stream, sender, category, body, message)
                    .await;
            }
        }
        {
            use crate::protocol::pet::{self as wire, Kind};
            for kind in Kind::INGRESS {
                if !matches(category, body, kind.category(), kind.type_id()) {
                    continue;
                }
                let message = wire::decode_ingress(kind, category, body, DEFAULT_MAX_BODY)
                    .map_err(|_| RuntimeError::Message)?
                    .ok_or(RuntimeError::Message)?;
                return self
                    .pet(runtime, pool, stream, sender, category, body, message)
                    .await;
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
                Kind::DestroyItem,
                Kind::DestroySlotItem,
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
                    Message::DestroyItem(request) => {
                        // ItemServiceProxy: UseSecondPassword passes (not refused in a micro play).
                        let class = request.item_class.as_deref().unwrap_or_default();
                        match super::minimum_shop::destroy_class(
                            pool,
                            &owner,
                            assets,
                            cid,
                            class,
                            request.num,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!(
                                    "minimum item class destroy processed; values omitted"
                                );
                            }
                            Err(_) => {
                                tracing::warn!("minimum item class destroy refused; values omitted")
                            }
                        }
                    }
                    Message::DestroySlotItem(item_id) => {
                        // ItemServiceProxy: refused inside a micro play (the selected
                        // character here is in town); UseSecondPassword passes.
                        match super::minimum_shop::destroy_slot(
                            pool,
                            &owner,
                            assets,
                            cid,
                            item_id,
                            runtime.item_log_context(assets)?,
                        )
                        .await
                        {
                            Ok(packets) => {
                                for packet in &packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                tracing::info!("minimum item destroy processed; values omitted");
                            }
                            Err(_) => {
                                tracing::warn!("minimum item destroy refused; values omitted")
                            }
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
                            Ok(super::minimum_use::Outcome::Pet(out)) => {
                                for packet in &out.packets {
                                    send(stream, sender, packet.category, &packet.body).await?;
                                }
                                if let Some(pet) = out.current {
                                    self.pet_changed(
                                        runtime, pool, assets, cid, pet, stream, sender,
                                    )
                                    .await?;
                                }
                                tracing::info!("minimum pet item used; values omitted");
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
  // The panel's event switches (laboratory extension) reach the client as overrides too.
  // The running scheduled events' keys follow (FeatureMatrix.OverrideFeature of the AdminClient).
  let event_switches:Vec<(String,String)>=runtime.minimum_assets().map(|a|a.event_overrides.iter().map(|(k,on)|(k.clone(),crate::domain::content_overlay::Overlay::event_value(*on).to_owned())).chain(a.scheduled_features.iter().map(|(k,v)|(k.clone(),v.clone()))).collect()).unwrap_or_default();
  let features=FeatureMatrix{features:Some(vec![FeatureEntry{key:"NGM_disable",value:Some("1")},FeatureEntry{key:"CashShop",value:Some("1.04.08")},FeatureEntry{key:"ingame_cashshop_shopListURL",value:Some(LAB_SHOP_LIST_URL)},FeatureEntry{key:"ingame_cashshop_purchaseURL",value:Some(super::lab_web_shop::PURCHASE_URL)},FeatureEntry{key:"S3_CharacterCreation",value:Some("3.09.00")}].into_iter().chain(event_switches.iter().map(|(k,v)|FeatureEntry{key:k.as_str(),value:Some(v.as_str())})).collect())};
  let (mut sender,packets)=encode_initial_packets(salt,Some(&features),super::minimum_cash_shop::CATALOG_MAX_BODY,1024).map_err(|_|RuntimeError::Message)?;
  for bytes in packets{tokio::time::timeout(WRITE_TIMEOUT,stream.write_all(&bytes)).await.map_err(|_|RuntimeError::Timeout)??;}
  let mut receiver=LegacyReceiver::with_profile(CipherProfile::VindictusFrontend,DEFAULT_MAX_BODY);let mut buffer=Zeroizing::new([0u8;4096]);
  loop {
   // Comm.NoticeMail of this character from the last request, timer or notice.
   if state.close_at.is_none(){state.own_mail_notice(&runtime,&pool,&mut stream,&mut sender).await?;}
   let deadline=state.close_at.unwrap_or(state.login_deadline);let waiting=state.close_at.is_some()||state.frontend.phase()!=SessionPhase::PoliciesAccepted;
   let timer=state.mission_timer();
   let effects=state.effect_expiry;
   let daily=state.daily_at;
   let pets=state.pet_reset_at;
   let mails=state.mail_at;
   let boost=state.boost_at;
   let story=state.story_at;
   let goal=state.goal_at;
   let party_notice=async{match state.party_notices.as_mut(){Some(rx)=>rx.recv().await,None=>std::future::pending().await}};
   let count=tokio::select!{
    _=shutdown.changed()=>return Ok(()),
    _=tokio::time::sleep_until(deadline),if waiting=>return if state.close_at.is_some(){Ok(())}else{Err(RuntimeError::Timeout)},
    _=tokio::time::sleep_until(timer.unwrap_or(deadline)),if timer.is_some()&&state.close_at.is_none()=>{state.mission_jobs(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    _=tokio::time::sleep_until(effects.unwrap_or(deadline)),if effects.is_some()&&state.close_at.is_none()=>{state.effect_jobs(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    _=tokio::time::sleep_until(daily.unwrap_or(deadline)),if daily.is_some()&&state.close_at.is_none()=>{state.daily_items(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    _=tokio::time::sleep_until(pets.unwrap_or(deadline)),if pets.is_some()&&state.close_at.is_none()=>{state.pet_jobs(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    _=tokio::time::sleep_until(mails.unwrap_or(deadline)),if mails.is_some()&&state.close_at.is_none()=>{state.mail_jobs(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    _=tokio::time::sleep_until(boost.unwrap_or(deadline)),if boost.is_some()&&state.close_at.is_none()=>{state.boost_event(&runtime,&pool,&mut stream,&mut sender).await?;continue;}
    _=tokio::time::sleep_until(story.unwrap_or(deadline)),if story.is_some()&&state.close_at.is_none()=>{state.story_periods(&runtime,&pool).await;continue;}
    _=tokio::time::sleep_until(goal.unwrap_or(deadline)),if goal.is_some()&&state.close_at.is_none()=>{state.goal_pushes(&runtime,&mut stream,&mut sender).await?;continue;}
    Some(notice)=party_notice=>{state.party_notice(&runtime,&pool,&mut stream,&mut sender,notice).await?;continue;}
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
/// Laboratory extension: the server's own marketplace listings, checked every minute
/// against the panel's settings (off by default) until shutdown.
async fn auto_listing(runtime: Arc<Runtime>, pool: PgPool, mut stop: watch::Receiver<bool>) {
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = stop.changed() => return,
            _ = tick.tick() => {}
        }
        if *stop.borrow() {
            return;
        }
        let Some(assets) = runtime.minimum_assets() else {
            continue;
        };
        match super::minimum_marketplace::auto_tick(&pool, &assets).await {
            Ok(0) => {}
            Ok(listed) => tracing::info!(listed, "marketplace auto listing made"),
            Err(_) => tracing::warn!("marketplace auto listing refused; values omitted"),
        }
    }
}
/// The panel's event schedule: a ProcessEvent pass at start and at each minute boundary.
/// The ranking publication (decision D-S3): every minute, publish when a 07:00 UTC boundary
/// passed since the last publication (also right after the start).
async fn rank_publication(runtime: Arc<Runtime>, pool: PgPool, mut stop: watch::Receiver<bool>) {
    let mut wait = Duration::from_secs(10);
    let mut titles_at: Option<i64> = None;
    loop {
        tokio::select! {
            _ = stop.changed() => return,
            _ = tokio::time::sleep(wait) => {}
        }
        if *stop.borrow() {
            return;
        }
        wait = Duration::from_secs(60);
        let Some(minimum) = runtime.minimum.as_ref() else {
            continue;
        };
        let assets = minimum.assets.current();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        match super::rank::events(&assets) {
            Ok(events) if !events.is_empty() => {
                match super::rank::publish(&pool, &events, now).await {
                    Ok(true) => tracing::info!(events = events.len(), "ranking published"),
                    Ok(false) => {}
                    Err(_) => tracing::warn!("ranking publication failed; values omitted"),
                }
            }
            Ok(_) => {}
            Err(_) => tracing::warn!("ranking events unreadable; values omitted"),
        }
        // RankTitleManager.UpdateHighRank: at start, then three seconds after each weekly
        // period start (after that boundary's publication); retried while it fails.
        if titles_at.is_none_or(|at| now >= at) {
            match runtime.rank_titles.refresh(&pool, &assets).await {
                Ok(()) => titles_at = Some(crate::domain::rank::next_week_start(now) + 3),
                Err(_) => tracing::warn!("rank title list unreadable; values omitted"),
            }
        }
    }
}
/// GuildInfoManager.UpdateRankTitles(kind) on a connection's RankTitles (`held`).
#[allow(clippy::too_many_arguments)]
async fn rank_title_update(
    runtime: &Runtime,
    pool: &PgPool,
    owner: &super::character_lobby::UserKey<'_>,
    assets: &super::minimum_assets::Assets,
    journal: i64,
    cid: i64,
    held: &mut Vec<i32>,
    kind: crate::domain::rank::TitleUpdate,
) -> Result<Vec<crate::domain::map_partition::OutgoingPacket>, super::minimum_character::MinimumError>
{
    let growth = runtime
        .growth_context(journal)
        .map_err(|_| super::minimum_character::MinimumError::Conflict)?;
    let high = runtime.rank_titles.get();
    super::rank_title::update(pool, owner, assets, &growth, cid, &high, held, kind).await
}
async fn event_schedule(runtime: Arc<Runtime>, pool: PgPool, mut stop: watch::Receiver<bool>) {
    let mut wait = Duration::from_secs(5);
    loop {
        tokio::select! {
            _ = stop.changed() => return,
            _ = tokio::time::sleep(wait) => {}
        }
        if *stop.borrow() {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let millis = i64::try_from(now.as_millis()).unwrap_or(0);
        wait = Duration::from_millis(super::event_scheduler::until_next_minute(millis));
        let offset = runtime.quest_clock().civil_offset_seconds;
        let schedule = runtime.schedule();
        let pass =
            match super::event_scheduler::process(&pool, &schedule, millis / 1000, offset).await {
                Ok(pass) => pass,
                Err(_) => {
                    tracing::warn!("event schedule pass refused; values omitted");
                    continue;
                }
            };
        for command in &pass.unsupported {
            tracing::warn!(command = %command, "event schedule command not migrated");
        }
        if pass.reload {
            match super::content_edits::loaded_overlay(&pool).await {
                Ok(overlay) => match runtime.reload_assets(overlay).await {
                    Ok(()) => tracing::info!("event schedule applied to the content"),
                    Err(_) => tracing::warn!("event schedule content reload failed"),
                },
                Err(_) => tracing::warn!("event schedule overlay unreadable"),
            }
        }
        // BroadcastMessage: NotifyClient(Notice) to every client.
        for notice in &pass.notices {
            if let Ok(p) = super::minimum_shop::system_message(1, notice, &[]) {
                runtime.party.announce_all(p, false, 0);
            }
        }
    }
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
    let auto_listing = tokio::spawn(auto_listing(runtime.clone(), pool.clone(), sig.clone()));
    let schedule = tokio::spawn(event_schedule(runtime.clone(), pool.clone(), sig.clone()));
    let ranking = tokio::spawn(rank_publication(runtime.clone(), pool.clone(), sig.clone()));
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
    let _ = auto_listing.await;
    let _ = schedule.await;
    let _ = ranking.await;
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
