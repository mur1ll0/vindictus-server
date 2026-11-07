//! PvpService matches hosted by a player through the relay (B5 PV2c, decision D-S2): the
//! source's PvpPlay without DSInfo (a relay P2PGroup, the first player as IPvpHost), with the
//! DS request of the matching systems removed. Arena (ArenaMatchingSystem): channels of
//! MaxSize players, the PvpPlay opened by the first registration, the queue refilling the
//! play. PvpPlayer: BindPlay (WhoIs → GameJoinMemberInfo), JoinP2P (AddClientWaiting,
//! ConnectionRequestMessage, P2PGroup.Join) and JoinComplete (PvpGameHosted to the host;
//! after the host's RegisterServer, PvpMemberInfo to the host and PvpHostInfo to the
//! member). Every packet goes out through the party hub (the single delivery point).
//! The Arena game rules (bouts, commands, rewards) and the Full matching of DeathMatch and
//! GodMatch are the next steps (PV3); the PvP party lock, chat and kicks too.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
    party_hub::PartyHub,
    relay::{Relay, RelayEvent},
};
use crate::{
    domain::{
        character_stats::Arithmetic,
        map_partition::OutgoingPacket,
        pvp::{Matching, PvpConfig},
        pvp_arena::{self as arena, Arena, Effect, Timer},
        pvp_team::{self as team_rules, TeamGame, TeamState, TeamTimer},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::HeroesString,
        mission::{self as mwire, GameInfo, JoinMember, StrDict},
        pvp::{self as wire, register_state},
    },
};
use rand_core::RngCore;
use sqlx::PgPool;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

/// The relay category of a PvP player (PvpService.Category).
pub const PVP_CATEGORY: &str = "PvpService.PvpService";
/// PvpPlay.SetHostGameInfo: the members waiting for the host join it after 10 s.
const HOST_INFO_DELAY: Duration = Duration::from_secs(10);
/// RelayServer: the client connects and sends its NMInfo within the waiting timeouts.
const RELAY_JOIN_TIMEOUT: Duration = Duration::from_secs(120);

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// The PvpServiceProxy state of the player's frontend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proxy {
    Waiting,
    Hosting,
    Host,
    Member,
}

/// PvpPlayer.PvpPlayerState.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Created,
    Querying,
    Joining,
    WaitingHostInfo,
    Ready,
}

struct Owner {
    uid: i32,
    name: String,
    revision: i64,
}

struct Player {
    mode: String,
    channel: i32,
    play: Option<i64>,
    slot: i32,
    stage: Stage,
    proxy: Proxy,
    /// PvpPlayer.Key (KeyRandom.Next).
    key: i32,
    join: Option<JoinMember>,
    owner: Owner,
    /// The character's name and level (PlayerInfo).
    name: String,
    level: i32,
    guild_id: i32,
}

/// ArenaChannel: PlayerDic in registration order, InGamePlayers, WaitingPlayers.
#[derive(Default)]
struct Channel {
    players: Vec<i64>,
    in_game: Vec<i64>,
    waiting: VecDeque<i64>,
    play: Option<i64>,
}

struct Play {
    mode: String,
    channel: i32,
    group: i32,
    host: Option<i64>,
    /// Host.GameInfo (RegisterServer).
    host_info: Option<GameInfo>,
    slots: Vec<Option<i64>>,
    config: PvpConfig,
    /// ArenaGame of an Arena play.
    arena: Option<Arena>,
    /// DeathMatchGame or GodMatchGame of a Full matching play.
    team: Option<TeamGame>,
    /// Play.GameEndConfirm.
    end_confirm: Option<std::collections::BTreeSet<i64>>,
}

/// FullMatchingSystem.StartingGameInfo: TeamA (red), TeamB (blue) and the players still to
/// confirm the join.
struct Starting {
    a: Vec<i64>,
    b: Vec<i64>,
    pending: std::collections::BTreeSet<i64>,
}

/// FullMatchingSystem of a mode: MakingTeams, WaitingTeams and StartingGame.
#[derive(Default)]
struct Full {
    /// The mode's config of the last registration.
    config: Option<PvpConfig>,
    making: Vec<Vec<i64>>,
    waiting: Vec<Vec<i64>>,
    starting: Vec<Starting>,
}

#[derive(Default)]
struct State {
    players: HashMap<i64, Player>,
    channels: HashMap<(String, i32), Channel>,
    plays: HashMap<i64, Play>,
    next_play: i64,
    full: HashMap<String, Full>,
}

/// What an async player step needs: the store and the WhoIs profile.
#[derive(Clone)]
pub struct Binder {
    pub pool: PgPool,
    pub assets: Arc<Assets>,
    pub config: QueryConfig,
    pub arithmetic: Arithmetic,
    /// MakeEntityID for the rewards.
    pub ids: Option<crate::application::creation_lifecycle::EntityIds>,
    /// GuildContents.GuildLevelUpInfoDic for GiveGuildGP.
    pub guild_levels: Option<Arc<crate::domain::guild::Levels>>,
}

/// The mode's PvpConfig and FeatureMatrix ArenaSpecatorLevelLimitMin.
pub struct Rules {
    pub config: Option<PvpConfig>,
    pub spectator_min: i32,
    /// FeatureMatrix ArenaDoubleEvent and CharacterRank.
    pub double_event: bool,
    pub character_rank: bool,
}
impl Rules {
    pub fn from_assets(assets: &Assets, mode: &str) -> Result<Self, MinimumError> {
        let policy = assets.npc_policy();
        let enabled = |key: &str| -> Result<bool, MinimumError> {
            policy
                .map(|p| p.is_enabled(Some(key)))
                .transpose()
                .map_err(|_| fail())
                .map(|v| v.unwrap_or(false))
        };
        // The panel's PvP knobs: PvpTeamSize_<mode> (> 0) and PvpNotGuild_<mode>.
        let mut config = assets.npc_pvp(mode).cloned();
        if let (Some(c), Some(p)) = (config.as_mut(), policy) {
            let size = p.get_integer(&format!("PvpTeamSize_{mode}")).unwrap_or(0);
            if size > 0 {
                c.team_size = size;
            }
            if p.is_enabled(Some(&format!("PvpNotGuild_{mode}")))
                .unwrap_or(false)
            {
                c.is_guild = false;
            }
        }
        Ok(Self {
            config,
            spectator_min: policy
                .map(|p| p.get_integer("ArenaSpecatorLevelLimitMin"))
                .transpose()
                .map_err(|_| fail())?
                .unwrap_or(0),
            double_event: enabled("ArenaDoubleEvent")?,
            character_rank: enabled("CharacterRank")?,
        })
    }
}

fn arena_rules(config: &PvpConfig, rules: &Rules) -> arena::Rules {
    let size = |v: i32| usize::try_from(v).unwrap_or(0);
    arena::Rules {
        pair_of_player: size(config.pair_of_player),
        team_size: size(config.team_size),
        max_size: size(config.max_size),
        game_length: config.game_length,
        wait_between_round: config.wait_between_round,
        round_to_win: config.round_to_win,
        win_signet: config.win_signet,
        lose_signet: config.lose_signet,
        reward_cool_down: config.same_player_reward_cool_down,
        has_event_reward: !config.event_reward_item.is_empty(),
        double_event: rules.double_event,
        character_rank: rules.character_rank,
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// A registering character (RegisterPvpPlayer of one PvPPartyMemberInfo).
pub struct Entry<'a> {
    pub cid: i64,
    pub owner: &'a UserKey<'a>,
    pub level: i32,
    /// The character's name.
    pub name: &'a str,
    /// GuildID (0 without a guild).
    pub guild_id: i32,
}

pub struct PvpHub {
    state: Mutex<State>,
    party: Arc<PartyHub>,
    guilds: Arc<super::guild::GuildHub>,
    /// The PvP relay (the frontend's relay, set when the minimum profile loads).
    relay: std::sync::RwLock<Option<Relay>>,
    /// The latest binder, for players a removal moves from a queue into a play.
    binder: Mutex<Option<Binder>>,
}

fn register_result(
    mode: &str,
    state: i32,
    pending: bool,
    timer: bool,
    format: &str,
    args: &[String],
) -> Result<OutgoingPacket, MinimumError> {
    packet(&wire::Message::RegisterResult(wire::RegisterResult {
        pvp_mode: Some(mode.to_owned()),
        use_pending_dialog: pending,
        start_waiting_timer: timer,
        state,
        message: Some(HeroesString {
            format: Some(format.to_owned()),
            parameters: Some(args.iter().cloned().map(Some).collect()),
        }),
    }))
}

pub fn packet(message: &wire::Message) -> Result<OutgoingPacket, MinimumError> {
    let (category, body) = wire::packet(message, DEFAULT_MAX_BODY).map_err(|_| fail())?;
    Ok(OutgoingPacket { category, body })
}

fn client_config(config: &PvpConfig) -> StrDict<Option<String>> {
    StrDict(Some(
        config
            .client_config
            .iter()
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect(),
    ))
}

/// IPvpGame.GameInfo: the Arena's, else TeamInfo {-1: "0, 0"} and every slot "0,0,0,0".
fn game_info(play: &Play) -> wire::PvpGameInfo {
    if let Some(a) = &play.arena {
        let (team, players) = a.game_info(now_ms());
        let dict = |v: Vec<(i32, String)>| {
            crate::protocol::inventory::Dict(Some(
                v.into_iter().map(|(k, v)| (k, Some(v))).collect(),
            ))
        };
        return wire::PvpGameInfo {
            team_info: dict(team),
            player_info: dict(players),
        };
    }
    wire::PvpGameInfo {
        team_info: crate::protocol::inventory::Dict(Some(vec![(-1, Some("0, 0".to_owned()))])),
        player_info: crate::protocol::inventory::Dict(Some(
            play.slots
                .iter()
                .enumerate()
                .filter(|(_, c)| c.is_some())
                .map(|(s, _)| (s as i32, Some("0,0,0,0".to_owned())))
                .collect(),
        )),
    }
}

impl PvpHub {
    pub fn new(party: Arc<PartyHub>, guilds: Arc<super::guild::GuildHub>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            party,
            guilds,
            relay: std::sync::RwLock::new(None),
            binder: Mutex::new(None),
        }
    }
    pub fn set_relay(&self, relay: Option<Relay>) {
        *self.relay.write().unwrap_or_else(PoisonError::into_inner) = relay;
    }
    fn relay(&self) -> Option<Relay> {
        self.relay
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn deliver(&self, out: Vec<(i64, OutgoingPacket)>) {
        let mut by: BTreeMap<i64, Vec<OutgoingPacket>> = BTreeMap::new();
        for (cid, p) in out {
            by.entry(cid).or_default().push(p);
        }
        for (cid, packets) in by {
            self.party.deliver(cid, packets);
        }
    }

    /// The PvpServiceProxy state of a registered character.
    pub fn proxy(&self, cid: i64) -> Option<Proxy> {
        self.lock().players.get(&cid).map(|p| p.proxy)
    }

    /// PvpQueryChannelListProcessor: the Arena channels ("{players}/{MaxSize}"); other
    /// matching systems return none.
    pub fn channel_list(
        &self,
        config: Option<&PvpConfig>,
        query: &wire::QueryChannelList,
    ) -> Result<OutgoingPacket, MinimumError> {
        let mode = query.pvp_mode.clone().unwrap_or_default();
        let state = self.lock();
        let channels = match config {
            Some(c) if c.matching == Matching::Arena => (0..c.arena_channel_count.max(0))
                .map(|id| {
                    let count = state
                        .channels
                        .get(&(mode.clone(), id))
                        .map_or(0, |ch| ch.players.len());
                    Some(wire::ChannelInfo {
                        desc: Some(format!("{count}/{}", c.max_size)),
                        channel_id: id,
                    })
                })
                .collect(),
            _ => Vec::new(),
        };
        packet(&wire::Message::ChannelList(wire::ChannelList {
            channels: Some(channels),
            pvp_mode: query.pvp_mode.clone(),
            key: query.key.clone(),
        }))
    }

    /// PvpServiceProxy.Initial(PvpRegisterMessage) and RegisterPvpPlayerProcessor for a solo
    /// player (`refusal` is the frontend's own check: InMicroPlay, InvalidMaster or
    /// InvalidMembercount); then the matching system. Returns the caller's packets.
    pub fn register(
        self: &Arc<Self>,
        binder: Binder,
        entry: Entry<'_>,
        refusal: Option<&'static str>,
        rules: Rules,
        request: &wire::Register,
    ) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let mode = request.pvp_mode.clone().unwrap_or_default();
        let canceled = |format: &str| -> Result<Vec<OutgoingPacket>, MinimumError> {
            Ok(vec![register_result(
                &mode,
                register_state::CANCELED,
                true,
                false,
                format,
                &[],
            )?])
        };
        if let Some(format) = refusal {
            return canceled(format);
        }
        *self.binder.lock().unwrap_or_else(PoisonError::into_inner) = Some(binder.clone());
        let Some(config) = rules.config.clone() else {
            return canceled("GameUI_Heroes_PvpJoinFail_InvalidPvpMode");
        };
        let spectator_min = rules.spectator_min;
        {
            let mut state = self.lock();
            if state.players.remove(&entry.cid).is_some() {
                return canceled("GameUI_Heroes_PvpJoinFail_PlayerDuplicated");
            }
            if config.level_min != -1 && entry.level < config.level_min {
                // The Arena takes a player below the minimum as a spectator.
                let spectator = config.matching == Matching::Arena && entry.level >= spectator_min;
                if !spectator {
                    return Ok(vec![register_result(
                        &mode,
                        register_state::CANCELED,
                        true,
                        false,
                        "GameUI_Heroes_PvpJoinFail_LevelConstraint_Min",
                        &[config.level_min.to_string()],
                    )?]);
                }
            } else if config.level_max != -1 && entry.level > config.level_max {
                return Ok(vec![register_result(
                    &mode,
                    register_state::CANCELED,
                    true,
                    false,
                    "GameUI_Heroes_PvpJoinFail_LevelConstraint_Max",
                    &[config.level_max.to_string()],
                )?]);
            }
            match config.matching {
                Matching::Arena => {}
                // A guild mode registers a whole guild team of TeamSize; a solo player only
                // fits a TeamSize of 1 (the party registration is not migrated).
                Matching::Full if config.is_guild && entry.guild_id == 0 => {
                    return canceled("GameUI_Heroes_PvpJoinFail_InvalidGuildTeam");
                }
                Matching::Full if config.is_guild && config.team_size != 1 => {
                    return Ok(vec![register_result(
                        &mode,
                        register_state::CANCELED,
                        true,
                        false,
                        "GameUI_Heroes_PvpJoinFail_InvalidPartySize",
                        &[config.team_size.to_string()],
                    )?]);
                }
                Matching::Full => {}
                _ => return canceled("GameUI_Heroes_PvpJoinFail_InvalidPvpMode"),
            }
            state.players.insert(
                entry.cid,
                Player {
                    mode: mode.clone(),
                    channel: request.channel_id,
                    play: None,
                    slot: -1,
                    stage: Stage::Created,
                    proxy: Proxy::Waiting,
                    key: (rand_core::OsRng.next_u32() & 0x7fff_ffff) as i32,
                    join: None,
                    owner: Owner {
                        uid: entry.owner.uid,
                        name: entry.owner.name.to_owned(),
                        revision: entry.owner.revision,
                    },
                    name: entry.name.to_owned(),
                    level: entry.level,
                    guild_id: entry.guild_id,
                },
            );
        }
        if config.matching == Matching::Full {
            self.full_register(&mode, &config, entry.cid);
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let mut binds = Vec::new();
        {
            let mut state = self.lock();
            if request.channel_id < 0 || request.channel_id >= config.arena_channel_count {
                state.players.remove(&entry.cid);
                return Ok(vec![register_result(
                    &mode,
                    register_state::CANCELED,
                    false,
                    false,
                    "GameUI_Heroes_PvpJoinFail_InvalidChannel",
                    &[],
                )?]);
            }
            out.push((
                entry.cid,
                register_result(
                    &mode,
                    register_state::WAIT,
                    true,
                    false,
                    "GameUI_Heroes_PvpJoin_WaitingHost",
                    &[],
                )?,
            ));
            let key = (mode.clone(), request.channel_id);
            let channel = state.channels.entry(key.clone()).or_default();
            channel.players.push(entry.cid);
            match channel.play {
                None => {
                    // StartDS → StartGame without a DS: the play opens at once.
                    channel.waiting.push_back(entry.cid);
                    let taken: Vec<i64> = channel
                        .waiting
                        .drain(..)
                        .take(usize::try_from(config.max_size).unwrap_or(0))
                        .collect();
                    let Some(relay) = self.relay() else {
                        state.players.remove(&entry.cid);
                        if let Some(c) = state.channels.get_mut(&key) {
                            c.players.retain(|c| *c != entry.cid);
                        }
                        return canceled("GameUI_Heroes_PvpJoinFail_StartGameFailed");
                    };
                    state.next_play += 1;
                    let id = state.next_play;
                    let size = usize::try_from(config.max_size).unwrap_or(0);
                    state.plays.insert(
                        id,
                        Play {
                            mode: mode.clone(),
                            channel: request.channel_id,
                            group: relay.make_group(),
                            host: None,
                            host_info: None,
                            slots: vec![None; size],
                            config: config.clone(),
                            arena: Some(Arena::new(arena_rules(&config, &rules), now_ms())),
                            team: None,
                            end_confirm: None,
                        },
                    );
                    if let Some(c) = state.channels.get_mut(&key) {
                        c.play = Some(id);
                        c.in_game.extend(taken.iter().copied());
                    }
                    for cid in taken {
                        if add_to_play(&mut state, id, cid) {
                            binds.push(cid);
                        }
                    }
                }
                Some(id) => {
                    if channel.players.len() > usize::try_from(config.max_size).unwrap_or(0) {
                        channel.waiting.push_back(entry.cid);
                        let waiting = channel.waiting.len().to_string();
                        out.push((
                            entry.cid,
                            register_result(
                                &mode,
                                register_state::WAIT,
                                false,
                                true,
                                "GameUI_Heroes_PvpJoin_WaitingEmptySpace",
                                &[waiting],
                            )?,
                        ));
                    } else {
                        channel.in_game.push(entry.cid);
                        if add_to_play(&mut state, id, entry.cid) {
                            binds.push(entry.cid);
                        }
                    }
                }
            }
        }
        self.deliver(out);
        for cid in binds {
            tokio::spawn(Arc::clone(self).bind(binder.clone(), cid));
        }
        Ok(Vec::new())
    }

    /// PvpPlayer.BindPlay (WhoIs and the member info) and JoinP2P (the relay group).
    async fn bind(self: Arc<Self>, binder: Binder, cid: i64) {
        let Some((owner, slot, key, mode, group, play_id)) = ({
            let mut state = self.lock();
            let snapshot = state.players.get(&cid).and_then(|p| {
                let play = state.plays.get(&p.play?)?;
                Some((
                    (p.owner.uid, p.owner.name.clone(), p.owner.revision),
                    p.slot,
                    p.key,
                    p.mode.clone(),
                    play.group,
                    p.play?,
                ))
            });
            if let Some(p) = state.players.get_mut(&cid) {
                p.stage = Stage::Querying;
            }
            snapshot
        }) else {
            return;
        };
        let user = UserKey {
            uid: owner.0,
            name: &owner.1,
            revision: owner.2,
        };
        let member = super::minimum_mission::member_info(
            &binder.pool,
            &user,
            &binder.assets,
            cid,
            slot,
            key,
            None,
            &[],
            binder.config,
            binder.arithmetic,
        )
        .await;
        let canceled = |format: &str| {
            register_result(&mode, register_state::CANCELED, true, false, format, &[])
        };
        let member = match member {
            Ok(m) => m,
            Err(_) => {
                tracing::warn!("pvp whois failed; values omitted");
                if let Ok(p) = canceled("GameUI_Heroes_PvpJoinFail_WhoisFail") {
                    self.deliver(vec![(cid, p)]);
                }
                self.remove(cid);
                return;
            }
        };
        {
            let mut state = self.lock();
            match state.players.get_mut(&cid) {
                Some(p) if p.play == Some(play_id) => p.join = Some(member.join),
                _ => return,
            }
        }
        let Ok(complete) = register_result(
            &mode,
            register_state::WAIT,
            true,
            false,
            "GameUI_Heroes_PvpJoin_WhoisComplete",
            &[],
        ) else {
            return;
        };
        self.deliver(vec![(cid, complete)]);
        let Some(relay) = self.relay() else {
            if let Ok(p) = canceled("GameUI_Heroes_PvpJoinFail_JoinP2pFail") {
                self.deliver(vec![(cid, p)]);
            }
            self.remove(cid);
            return;
        };
        // JoinP2P: subscribe before registering, so Connected cannot be missed.
        let mut events = relay.subscribe();
        let waiting = relay.add_client_waiting(cid, owner.0);
        let endpoint = relay.endpoint();
        let request = super::minimum_mission::packet(&mwire::Message::ConnectionRequest(
            mwire::ConnectionRequest {
                address: Some(endpoint.ip().to_string()),
                port: endpoint.port(),
                posix_time: waiting.sn,
                key: waiting.key,
                category: Some(PVP_CATEGORY.to_owned()),
                ping_host_cid: -1,
                group_id: 0,
            },
        ));
        let Ok(request) = request else {
            self.remove(cid);
            return;
        };
        if let Some(p) = self.lock().players.get_mut(&cid) {
            p.stage = Stage::Joining;
        }
        self.deliver(vec![(cid, request)]);
        let joined = tokio::time::timeout(RELAY_JOIN_TIMEOUT, async {
            loop {
                match events.recv().await {
                    Ok(RelayEvent::Connected { cid: c, .. }) if c == cid => {
                        if !relay.join(group, cid, play_id, PVP_CATEGORY) {
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
        if !matches!(joined, Ok(true)) {
            tracing::info!("pvp relay join failed; values omitted");
            if let Ok(p) = canceled("GameUI_Heroes_PvpJoinFail_JoinP2pFail") {
                self.deliver(vec![(cid, p)]);
            }
            self.remove(cid);
            return;
        }
        if let Ok(p) = register_result(
            &mode,
            register_state::WAIT,
            true,
            false,
            "GameUI_Heroes_PvpJoin_P2PJoined",
            &[],
        ) {
            self.deliver(vec![(cid, p)]);
        }
        self.join_complete(cid);
        // RelayClient Closed → Remove (a JoinP2pFail while still waiting for the host).
        loop {
            match events.recv().await {
                Ok(RelayEvent::Closed { cid: c }) if c == cid => {
                    let waiting_host = self.lock().players.get(&cid).is_some_and(|p| {
                        p.play == Some(play_id) && p.stage == Stage::WaitingHostInfo
                    });
                    if waiting_host && let Ok(p) = canceled("GameUI_Heroes_PvpJoinFail_JoinP2pFail")
                    {
                        self.deliver(vec![(cid, p)]);
                    }
                    if self
                        .lock()
                        .players
                        .get(&cid)
                        .is_some_and(|p| p.play == Some(play_id))
                    {
                        self.remove(cid);
                    }
                    return;
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if !self
                        .lock()
                        .players
                        .get(&cid)
                        .is_some_and(|p| p.play == Some(play_id))
                    {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    }

    /// PvpPlayer.JoinComplete.
    fn join_complete(self: &Arc<Self>, cid: i64) {
        let mut out = Vec::new();
        let mut ready = None;
        {
            let mut state = self.lock();
            let Some(p) = state.players.get(&cid) else {
                return;
            };
            let (Some(play_id), Some(join)) = (p.play, p.join.clone()) else {
                return;
            };
            let mode = p.mode.clone();
            let slot = p.slot;
            let Some(play) = state.plays.get(&play_id) else {
                return;
            };
            let config = client_config(&play.config);
            let team = match (&play.team, usize::try_from(slot)) {
                (Some(t), Ok(s)) => t.team_of(s),
                _ => wire::team::INVALID,
            };
            if let Some(info) = &play.host_info {
                if play.host == Some(cid) {
                    return;
                }
                if let Some(host) = play.host
                    && let Ok(m) = packet(&wire::Message::MemberInfo(wire::MemberInfo {
                        member_info: Some(join.clone()),
                        team_id: team,
                    }))
                {
                    out.push((host, m));
                }
                if let Ok(h) = packet(&wire::Message::HostInfo(wire::HostInfo {
                    game_info: Some(info.clone()),
                    member_info: Some(join),
                    config,
                })) {
                    out.push((cid, h));
                }
                if let Some(p) = state.players.get_mut(&cid) {
                    p.stage = Stage::Ready;
                    p.proxy = Proxy::Member;
                }
                ready = Some(play_id);
            } else if play.host == Some(cid) {
                if let Ok(h) = packet(&wire::Message::GameHosted(wire::GameHosted {
                    game_info: Some(game_info(play)),
                    host_info: Some(join),
                    team_id: team,
                    tag: slot,
                    config,
                })) {
                    out.push((cid, h));
                }
                if let Some(p) = state.players.get_mut(&cid) {
                    p.stage = Stage::Ready;
                    p.proxy = Proxy::Hosting;
                }
                ready = Some(play_id);
            } else {
                if let Ok(w) = register_result(
                    &mode,
                    register_state::WAIT,
                    true,
                    false,
                    "GameUI_Heroes_PvpJoin_WaitingHost",
                    &[],
                ) {
                    out.push((cid, w));
                }
                if let Some(p) = state.players.get_mut(&cid) {
                    p.stage = Stage::WaitingHostInfo;
                }
            }
        }
        self.deliver(out);
        if let Some(play_id) = ready {
            let effects = {
                let mut state = self.lock();
                let all_ready = state
                    .players
                    .values()
                    .filter(|p| p.play == Some(play_id))
                    .all(|p| p.stage == Stage::Ready);
                state
                    .plays
                    .get_mut(&play_id)
                    .and_then(|p| p.team.as_mut())
                    .map(|t| t.on_player_ready(all_ready))
            };
            if let Some(effects) = effects {
                self.apply(play_id, effects);
            }
        }
    }

    /// RegisterServerMessage in the Hosting or Member proxy state: RegisterHostGameInfo
    /// (the host only), then PvpPlay.SetHostGameInfo. None when the character is not in a
    /// PvP play (the message belongs to a mission); Some(false) for a non-host.
    pub fn register_server(self: &Arc<Self>, cid: i64, info: Option<GameInfo>) -> Option<bool> {
        let play_id = {
            let mut state = self.lock();
            let p = state.players.get(&cid)?;
            if !matches!(p.proxy, Proxy::Hosting | Proxy::Member | Proxy::Host) {
                return None;
            }
            let play_id = p.play?;
            let play = state.plays.get_mut(&play_id)?;
            if play.host != Some(cid) {
                return Some(false);
            }
            play.host_info = Some(info.unwrap_or(GameInfo {
                name: None,
                map: None,
                game_dir: None,
                description: None,
                host_id: 0,
                ds_ip: None,
                ds_port: 0,
            }));
            if let Some(p) = state.players.get_mut(&cid) {
                p.proxy = Proxy::Host;
            }
            play_id
        };
        let hub = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(HOST_INFO_DELAY).await;
            let waiting: Vec<i64> = hub
                .lock()
                .players
                .iter()
                .filter(|(c, p)| {
                    p.play == Some(play_id) && p.stage == Stage::WaitingHostInfo && **c != cid
                })
                .map(|(c, _)| *c)
                .collect();
            for member in waiting {
                hub.join_complete(member);
            }
        });
        Some(true)
    }

    /// PvpPlayer.Remove (PvpUnregister, a closed relay or frontend): PvpKicked to a bound
    /// player, ArenaTeam.RemovePlayer (the host's leaving removes everyone), the channel
    /// refill from its queue, and the play's close when it empties.
    pub fn remove(self: &Arc<Self>, cid: i64) {
        let mut out = Vec::new();
        let mut binds = Vec::new();
        let mut cascade = Vec::new();
        let mut removal_effects = None;
        let mut reselected = None;
        self.full_unregister(cid);
        {
            let mut state = self.lock();
            let Some(p) = state.players.get(&cid).map(|p| {
                (
                    p.mode.clone(),
                    p.channel,
                    p.play,
                    p.slot,
                    p.owner.name.clone(),
                )
            }) else {
                return;
            };
            let p = Removed {
                mode: p.0,
                channel: p.1,
                play: p.2,
                slot: p.3,
                name: p.4,
            };
            state.players.remove(&cid);
            let key = (p.mode.clone(), p.channel);
            if let Some(play_id) = p.play {
                if let Ok(k) = packet(&wire::Message::Kicked(wire::Empty {})) {
                    out.push((cid, k));
                }
                let mut close = false;
                if let Some(play) = state.plays.get_mut(&play_id) {
                    if let (Some(a), Ok(slot)) = (play.arena.as_mut(), usize::try_from(p.slot)) {
                        let effects = a.remove_player(slot);
                        removal_effects = Some((play_id, effects));
                    }
                    if let (Some(t), Ok(slot)) = (play.team.as_mut(), usize::try_from(p.slot)) {
                        let effects = t.remove_player(slot, now_ms());
                        removal_effects = Some((play_id, effects));
                    }
                    if let Some(confirm) = play.end_confirm.as_mut() {
                        confirm.remove(&cid);
                    }
                    if let Some(s) = play.slots.iter_mut().find(|s| **s == Some(cid)) {
                        *s = None;
                    }
                    if let Some(relay) = self.relay() {
                        relay.leave(play.group, cid);
                    }
                    if play.host == Some(cid) {
                        match &play.team {
                            // PvpPlayTeam.RemovePlayer: ReselectHost before the game ends.
                            Some(t) if t.state() != TeamState::GameEnded => {
                                play.host = play.slots.iter().flatten().copied().next();
                                play.host_info = None;
                                reselected = play.host.map(|_| play_id);
                            }
                            _ => cascade.extend(play.slots.iter().flatten().copied()),
                        }
                    }
                    close = play.slots.iter().all(Option::is_none);
                }
                if close && let Some(play) = state.plays.remove(&play_id) {
                    if let Some(relay) = self.relay() {
                        relay.close_group(play.group);
                    }
                    if let Some(c) = state.channels.get_mut(&key) {
                        c.play = None;
                    }
                }
            }
            // ArenaMatchingSystem.UnregisterPlayer → ArenaChannel.RemovePlayer.
            let mut dodged = false;
            let mut refill = None;
            if let Some(channel) = state.channels.get_mut(&key)
                && channel.players.contains(&cid)
            {
                dodged = !channel.in_game.contains(&cid);
                channel.players.retain(|c| *c != cid);
                let was_in_game = channel.in_game.contains(&cid);
                channel.in_game.retain(|c| *c != cid);
                channel.waiting.retain(|c| *c != cid);
                if was_in_game
                    && let Some(play_id) = channel.play
                    && let Some(next) = channel.waiting.pop_front()
                {
                    channel.in_game.push(next);
                    refill = Some((play_id, next));
                }
            }
            if dodged
                && let Ok(d) = register_result(
                    &p.mode,
                    register_state::CANCELED,
                    true,
                    false,
                    "GameUI_Heroes_PvpJoinFail_Dodged",
                    std::slice::from_ref(&p.name),
                )
            {
                out.push((cid, d));
            }
            if let Some((play_id, next)) = refill
                && add_to_play(&mut state, play_id, next)
            {
                binds.push(next);
            }
        }
        self.deliver(out);
        if let Some((play_id, effects)) = removal_effects {
            self.apply(play_id, effects);
        }
        if let Some(play_id) = reselected {
            self.host_changed(play_id);
        }
        for other in cascade {
            if other != cid {
                self.remove(other);
            }
        }
        let binder = self
            .binder
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(binder) = binder {
            for next in binds {
                tokio::spawn(Arc::clone(self).bind(binder.clone(), next));
            }
        }
    }
}

/// ArenaTeam.AddPlayer: the first player of a hostless play is its host; the first free slot.
fn add_to_play(state: &mut State, play_id: i64, cid: i64) -> bool {
    add_to_team(state, play_id, cid, wire::team::INVALID)
}

/// PvpPlayTeam.AddPlayer (TeamRed/TeamBlue) or ArenaTeam.AddPlayer (Invalid).
fn add_to_team(state: &mut State, play_id: i64, cid: i64, team: i32) -> bool {
    let Some(name) = state.players.get(&cid).map(|p| p.name.clone()) else {
        return false;
    };
    let Some(play) = state.plays.get_mut(&play_id) else {
        return false;
    };
    let Some(slot) = play.slots.iter().position(Option::is_none) else {
        return false;
    };
    play.slots[slot] = Some(cid);
    if play.host.is_none() {
        play.host = Some(cid);
    }
    if let Some(a) = play.arena.as_mut() {
        a.add_player(
            slot,
            arena::Member {
                cid,
                name: name.clone(),
            },
        );
    }
    if let Some(t) = play.team.as_mut() {
        t.add_player(slot, team, arena::Member { cid, name });
    }
    let _ = (&play.mode, play.channel);
    match state.players.get_mut(&cid) {
        Some(p) => {
            p.play = Some(play_id);
            p.slot = slot as i32;
            true
        }
        None => false,
    }
}

/// The removed player's fields remove() uses after the entry is gone.
struct Removed {
    mode: String,
    channel: i32,
    play: Option<i64>,
    slot: i32,
    name: String,
}

/// An effect's asynchronous part (PlayerConn and RankConn operations, the PvP DB).
enum Job {
    /// GiveRewardItem(class, count).
    Reward { class: String, count: i32 },
    /// UpdateRankBasis_Pvp.
    RankBasis,
    /// PvpResultLog, then a winner's PvpResultInfoGet → ArenaResultInfoMessage.
    Result { win: bool, successive: i32 },
    /// GiveGuildGP(ByPVP) to the player's guild.
    GuildGp { gp: i32 },
}
struct Work {
    cid: i64,
    owner: (i32, String, i64),
    mode: String,
    job: Job,
}

/// PvpPlay.SendCommand: the arg without a leading or trailing comma.
fn trim_arg(arg: &str) -> String {
    let arg = arg.strip_prefix(',').unwrap_or(arg);
    arg.strip_suffix(',').unwrap_or(arg).to_owned()
}

impl PvpHub {
    /// The game's effects: commands to the host and dialogs through the party hub, timers,
    /// and the reward/result operations run in the background.
    fn apply(self: &Arc<Self>, play_id: i64, effects: Vec<Effect>) {
        if effects.is_empty() {
            return;
        }
        let mut out = Vec::new();
        let mut work = Vec::new();
        let mut timers = Vec::new();
        let mut end_confirm = false;
        {
            let state = self.lock();
            let Some(play) = state.plays.get(&play_id) else {
                return;
            };
            let at = |slot: usize| play.slots.get(slot).copied().flatten();
            let owner = |cid: i64| {
                state
                    .players
                    .get(&cid)
                    .map(|p| (p.owner.uid, p.owner.name.clone(), p.owner.revision))
            };
            for e in effects {
                match e {
                    Effect::Command(command, arg) => {
                        let arg = trim_arg(&arg);
                        tracing::info!(command, "pvp command; values omitted");
                        if let (Some(host), Ok(p)) = (
                            play.host,
                            packet(&wire::Message::Command(wire::Command {
                                command,
                                arg: Some(arg),
                            })),
                        ) {
                            out.push((host, p));
                        }
                    }
                    Effect::Dialog { slot, format } => {
                        if let (Some(cid), Ok(p)) = (
                            at(slot),
                            super::minimum_shop::system_message(
                                super::minimum_shop::DIALOG,
                                format,
                                &[],
                            ),
                        ) {
                            out.push((cid, p));
                        }
                    }
                    Effect::Schedule { delay_ms, timer } => timers.push((delay_ms, timer)),
                    Effect::Reward { slot, count } => {
                        if let Some(cid) = at(slot)
                            && let Some(o) = owner(cid)
                        {
                            work.push(Work {
                                cid,
                                owner: o,
                                mode: play.mode.clone(),
                                job: Job::Reward {
                                    class: play.config.reward_item.clone(),
                                    count,
                                },
                            });
                        }
                    }
                    Effect::EventReward { slot } => {
                        if let Some(cid) = at(slot)
                            && let Some(o) = owner(cid)
                        {
                            work.push(Work {
                                cid,
                                owner: o,
                                mode: play.mode.clone(),
                                job: Job::Reward {
                                    class: play.config.event_reward_item.clone(),
                                    count: 1,
                                },
                            });
                        }
                    }
                    Effect::RankBasis { slot } => {
                        if let Some(cid) = at(slot)
                            && let Some(o) = owner(cid)
                        {
                            work.push(Work {
                                cid,
                                owner: o,
                                mode: play.mode.clone(),
                                job: Job::RankBasis,
                            });
                        }
                    }
                    // NotifyStoryEvent(PVP): story events are not migrated.
                    Effect::Story { .. } => {}
                    Effect::Telepathy { format, args } => {
                        for cid in play.slots.iter().flatten() {
                            if let Ok(p) = super::minimum_shop::system_message(2, format, &args) {
                                out.push((*cid, p));
                            }
                        }
                    }
                    Effect::Notice { slot, format } => {
                        if let (Some(cid), Ok(p)) = (
                            at(slot),
                            super::minimum_shop::system_message(
                                super::minimum_shop::SYSTEM,
                                format,
                                &[],
                            ),
                        ) {
                            out.push((cid, p));
                        }
                    }
                    Effect::GuildGp { slot, gp } => {
                        if let Some(cid) = at(slot)
                            && let Some(o) = owner(cid)
                        {
                            work.push(Work {
                                cid,
                                owner: o,
                                mode: play.mode.clone(),
                                job: Job::GuildGp { gp },
                            });
                        }
                    }
                    // UpdateRankBasis_Pvp of a guild: the guild ranking is off in EN-EU.
                    Effect::GuildRankBasis { .. } => {}
                    Effect::EndConfirm => end_confirm = true,
                    Effect::Result {
                        slot,
                        win,
                        successive,
                    } => {
                        if let Some(cid) = at(slot)
                            && let Some(o) = owner(cid)
                        {
                            work.push(Work {
                                cid,
                                owner: o,
                                mode: play.mode.clone(),
                                job: Job::Result { win, successive },
                            });
                        }
                    }
                }
            }
        }
        if end_confirm {
            let mut state = self.lock();
            if let Some(play) = state.plays.get_mut(&play_id) {
                play.end_confirm = Some(play.slots.iter().flatten().copied().collect());
            }
        }
        self.deliver(out);
        for (delay, timer) in timers {
            let hub = Arc::clone(self);
            tokio::spawn(async move {
                if delay > 0 {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                hub.fire(play_id, timer);
            });
        }
        if !work.is_empty() {
            let binder = self
                .binder
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(binder) = binder {
                let hub = Arc::clone(self);
                tokio::spawn(async move { hub.run_work(binder, work).await });
            }
        }
    }

    /// A Scheduler job of a play still open.
    fn fire(self: &Arc<Self>, play_id: i64, timer: Timer) {
        let effects = {
            let mut state = self.lock();
            let Some(play) = state.plays.get_mut(&play_id) else {
                return;
            };
            match (timer, play.arena.as_mut(), play.team.as_mut()) {
                (Timer::Team(TeamTimer::ForceRemove), _, Some(_)) => {
                    let left: Vec<i64> = play
                        .end_confirm
                        .as_ref()
                        .map(|c| c.iter().copied().collect())
                        .unwrap_or_default();
                    drop(state);
                    for cid in left {
                        self.remove(cid);
                    }
                    return;
                }
                (Timer::Team(t), _, Some(game)) => game.timer(t, now_ms()),
                (Timer::Team(_), _, None) => return,
                (_, Some(a), _) => a.timer(timer, now_ms()),
                _ => return,
            }
        };
        self.apply(play_id, effects);
    }

    async fn run_work(self: Arc<Self>, binder: Binder, work: Vec<Work>) {
        for w in work {
            let owner = UserKey {
                uid: w.owner.0,
                name: &w.owner.1,
                revision: w.owner.2,
            };
            let done = match &w.job {
                Job::Reward { class, count } => {
                    let wanted = usize::try_from(*count)
                        .unwrap_or(0)
                        .saturating_mul(2)
                        .min(512);
                    let ids = match &binder.ids {
                        Some(ids) => ids.take(wanted).await.map_err(|_| ()),
                        None => Err(()),
                    };
                    match ids {
                        Ok(ids) => {
                            match super::pvp_reward::reward_item(
                                &binder.pool,
                                &owner,
                                &binder.assets,
                                w.cid,
                                class,
                                *count,
                                ids,
                            )
                            .await
                            {
                                Ok((_, packets)) => {
                                    if !packets.is_empty() {
                                        self.party.deliver(w.cid, packets);
                                    }
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        Err(_) => Err(fail()),
                    }
                }
                Job::GuildGp { gp } => match (
                    &binder.guild_levels,
                    super::guild::membership(&binder.pool, w.cid).await,
                ) {
                    (Some(levels), Ok(Some((guild_sn, _)))) => super::guild::give_gp(
                        &binder.pool,
                        &self.party,
                        &self.guilds,
                        levels,
                        guild_sn,
                        w.cid,
                        *gp,
                        crate::domain::guild::gain::BY_PVP,
                    )
                    .await
                    .map(|packets| {
                        if let Some(p) = packets.filter(|p| !p.is_empty()) {
                            self.party.deliver(w.cid, p);
                        }
                    }),
                    (_, Err(e)) => Err(e),
                    _ => Ok(()),
                },
                Job::RankBasis => super::rank::update_rank_basis(
                    &binder.pool,
                    &binder.assets,
                    w.cid,
                    &format!("Pvp_{}", w.mode),
                    1,
                    0,
                )
                .await
                .map(|_| ()),
                Job::Result { win, successive } => {
                    let (wins, loses) = if *win { (1, 0) } else { (0, 1) };
                    let logged = super::pvp_reward::result_update(
                        &binder.pool,
                        w.cid,
                        &w.mode,
                        wins,
                        loses,
                        *successive,
                    )
                    .await;
                    match (logged, *win) {
                        (Ok(()), true) => {
                            match super::pvp_reward::result_info(&binder.pool, w.cid, &w.mode).await
                            {
                                Ok(Some((win, lose, max))) => {
                                    if let Ok(p) = packet(&wire::Message::ArenaResultInfo(
                                        wire::ArenaResultInfo {
                                            win,
                                            lose,
                                            successive_win: max,
                                        },
                                    )) {
                                        self.party.deliver(w.cid, vec![p]);
                                    }
                                    Ok(())
                                }
                                Ok(None) => Ok(()),
                                Err(e) => Err(e),
                            }
                        }
                        (r, _) => r,
                    }
                }
            };
            if done.is_err() {
                tracing::warn!("pvp reward or result failed; values omitted");
            }
        }
    }

    /// The play and slot of a ready player.
    fn seat(state: &State, cid: i64) -> Option<(i64, usize)> {
        let p = state.players.get(&cid)?;
        Some((p.play?, usize::try_from(p.slot).ok()?))
    }

    /// PvpRegisterGameWaitingQueue (Host or Member): Arena only, LevelLimitMin, then
    /// AddWaitingQueueGame; the failure is a ForceDialog of the frontend.
    pub fn waiting_queue(self: &Arc<Self>, cid: i64, index: i32) -> Vec<OutgoingPacket> {
        let refused = |format: &str, args: &[String]| {
            super::minimum_shop::system_message(5, format, args)
                .into_iter()
                .collect::<Vec<_>>()
        };
        let (play_id, effects) = {
            let mut state = self.lock();
            let Some((play_id, slot)) = Self::seat(&state, cid) else {
                return refused("GameUI_Heroes_PvpJoinFail_Unknown", &[]);
            };
            let level = state.players.get(&cid).map_or(0, |p| p.level);
            let Some(play) = state.plays.get_mut(&play_id) else {
                return refused("GameUI_Heroes_PvpJoinFail_Unknown", &[]);
            };
            if play.config.level_min > level {
                let min = play.config.level_min.to_string();
                return refused("GameUI_Heroes_Arena_Register_Level_Fail", &[min]);
            }
            let Some(a) = play.arena.as_mut() else {
                return refused("GameUI_Heroes_PvpJoinFail_Unknown", &[]);
            };
            match usize::try_from(index)
                .ok()
                .and_then(|i| a.add_waiting_queue(i, slot, now_ms()))
            {
                Some(effects) => (play_id, effects),
                None => return refused("GameUI_Heroes_PvpJoinFail_Unknown", &[]),
            }
        };
        self.apply(play_id, effects);
        Vec::new()
    }

    /// PvpUnregisterGameWaitingQueue.
    pub fn leave_waiting_queue(self: &Arc<Self>, cid: i64) {
        let found = {
            let mut state = self.lock();
            Self::seat(&state, cid).and_then(|(play_id, slot)| {
                let a = state.plays.get_mut(&play_id)?.arena.as_mut()?;
                Some((play_id, a.remove_waiting_queue(slot)))
            })
        };
        if let Some((play_id, effects)) = found {
            self.apply(play_id, effects);
        }
    }

    /// PvpReport of the host (Hosting or Host): Game.OnReport.
    pub fn report(self: &Arc<Self>, cid: i64, report: &wire::Report) {
        let found = {
            let mut state = self.lock();
            let Some((play_id, _)) = Self::seat(&state, cid) else {
                return;
            };
            match state.plays.get_mut(&play_id) {
                Some(play) if play.host == Some(cid) => {
                    let now = now_ms();
                    if let Some(a) = play.arena.as_mut() {
                        Some((
                            play_id,
                            a.report(report.event, report.subject, report.object, now),
                        ))
                    } else {
                        play.team.as_mut().map(|t| {
                            (
                                play_id,
                                t.report(report.event, report.subject, report.object, now),
                            )
                        })
                    }
                }
                _ => None,
            }
        };
        if let Some((play_id, effects)) = found {
            self.apply(play_id, effects);
        }
    }
}

impl PvpHub {
    fn make_message(mode: &str, team: &[i64], size: i32) -> Vec<(i64, OutgoingPacket)> {
        let args = [team.len().to_string(), size.to_string()];
        team.iter()
            .filter_map(|c| {
                register_result(
                    mode,
                    register_state::MAKE,
                    false,
                    false,
                    "GameUI_Heroes_PvpJoin_MakePartyMembers",
                    &args,
                )
                .ok()
                .map(|p| (*c, p))
            })
            .collect()
    }
    fn team_message(
        mode: &str,
        team: &[i64],
        state: i32,
        pending: bool,
        format: &str,
        args: &[String],
    ) -> Vec<(i64, OutgoingPacket)> {
        team.iter()
            .filter_map(|c| {
                register_result(mode, state, pending, false, format, args)
                    .ok()
                    .map(|p| (*c, p))
            })
            .collect()
    }

    /// FullMatchingSystem.RegisterPlayer of a solo player (AddMakingParty).
    fn full_register(self: &Arc<Self>, mode: &str, config: &PvpConfig, cid: i64) {
        self.full_register_team(mode, config, vec![cid]);
    }

    /// FullMatchingSystem.RegisterPlayer of a registered team (AddMakingParty).
    fn full_register_team(self: &Arc<Self>, mode: &str, config: &PvpConfig, team: Vec<i64>) {
        let mut out = Vec::new();
        {
            let mut state = self.lock();
            let guilds: HashMap<i64, i32> = state
                .players
                .iter()
                .map(|(c, p)| (*c, p.guild_id))
                .collect();
            let full = state.full.entry(mode.to_owned()).or_default();
            full.config = Some(config.clone());
            Self::add_making(full, mode, config, team, &guilds, &mut out);
        }
        self.deliver(out);
    }

    /// AddMakingParty: a full (or guild) team waits; else it joins a making team with room.
    fn add_making(
        full: &mut Full,
        mode: &str,
        config: &PvpConfig,
        team: Vec<i64>,
        guilds: &HashMap<i64, i32>,
        out: &mut Vec<(i64, OutgoingPacket)>,
    ) {
        let size = usize::try_from(config.team_size).unwrap_or(0);
        if config.is_guild || team.len() == size {
            Self::add_waiting(full, mode, config, team, guilds, out);
            return;
        }
        if let Some(i) = full
            .making
            .iter()
            .position(|m| m.len() + team.len() <= size)
        {
            full.making[i].extend(team);
            if full.making[i].len() == size {
                let ready = full.making.remove(i);
                Self::add_waiting(full, mode, config, ready, guilds, out);
            } else {
                out.extend(Self::make_message(mode, &full.making[i], config.team_size));
            }
            return;
        }
        out.extend(Self::make_message(mode, &team, config.team_size));
        full.making.push(team);
    }

    /// AddWaitingParty: the first waiting team (another guild in a guild mode) is the opponent;
    /// without a DS the game is joinable at once (DSReady).
    fn add_waiting(
        full: &mut Full,
        mode: &str,
        config: &PvpConfig,
        team: Vec<i64>,
        guilds: &HashMap<i64, i32>,
        out: &mut Vec<(i64, OutgoingPacket)>,
    ) {
        let guild = |t: &[i64]| t.first().and_then(|c| guilds.get(c)).copied().unwrap_or(0);
        let opponent = full
            .waiting
            .iter()
            .position(|w| !config.is_guild || guild(w) != guild(&team));
        match opponent {
            None => {
                out.extend(Self::team_message(
                    mode,
                    &team,
                    register_state::WAIT,
                    false,
                    "GameUI_Heroes_PvpJoin_WaitingOpponent",
                    &[],
                ));
                full.waiting.push(team);
            }
            Some(i) => {
                let a = full.waiting.remove(i);
                for t in [&a, &team] {
                    out.extend(Self::team_message(
                        mode,
                        t,
                        register_state::WAIT,
                        false,
                        "GameUI_Heroes_PvpJoin_OpponentFound",
                        &[],
                    ));
                    out.extend(Self::team_message(
                        mode,
                        t,
                        register_state::JOINABLE,
                        true,
                        "GameUI_Heroes_PvpJoin_DSReady",
                        &[],
                    ));
                }
                let pending = a.iter().chain(team.iter()).copied().collect();
                full.starting.push(Starting {
                    a,
                    b: team,
                    pending,
                });
            }
        }
    }

    /// FullMatchingSystem.UnregisterPlayer of a player not yet in a play: the making or
    /// waiting team loses it; a starting game's opponent goes back to making.
    fn full_unregister(self: &Arc<Self>, cid: i64) {
        let mut out = Vec::new();
        {
            let mut state = self.lock();
            let Some(mode) = state
                .players
                .get(&cid)
                .filter(|p| p.play.is_none())
                .map(|p| p.mode.clone())
            else {
                return;
            };
            let guilds: HashMap<i64, i32> = state
                .players
                .iter()
                .map(|(c, p)| (*c, p.guild_id))
                .collect();
            if let Some(full) = state.full.get_mut(&mode) {
                let config = full.config.clone();
                Self::drop_member(full, cid, &mode, config.as_ref(), &guilds, &mut out);
            }
        }
        self.deliver(out);
    }

    fn drop_member(
        full: &mut Full,
        cid: i64,
        mode: &str,
        config: Option<&PvpConfig>,
        guilds: &HashMap<i64, i32>,
        out: &mut Vec<(i64, OutgoingPacket)>,
    ) {
        let dodged = |out: &mut Vec<(i64, OutgoingPacket)>| {
            out.extend(Self::team_message(
                mode,
                &[cid],
                register_state::CANCELED,
                true,
                "GameUI_Heroes_PvpJoinFail_Dodged",
                &[],
            ));
        };
        for team in full.making.iter_mut().chain(full.waiting.iter_mut()) {
            if let Some(pos) = team.iter().position(|c| *c == cid) {
                team.remove(pos);
                dodged(out);
            }
        }
        full.making.retain(|t| !t.is_empty());
        full.waiting.retain(|t| !t.is_empty());
        if let Some(i) = full
            .starting
            .iter()
            .position(|g| g.a.contains(&cid) || g.b.contains(&cid))
        {
            let g = full.starting.remove(i);
            let (own, other) = if g.a.contains(&cid) {
                (g.a, g.b)
            } else {
                (g.b, g.a)
            };
            dodged(out);
            out.extend(Self::team_message(
                mode,
                &other,
                register_state::WAIT,
                false,
                "GameUI_Heroes_PvpJoinFail_OpponentDodged",
                &[String::new()],
            ));
            if let Some(config) = config {
                Self::add_making(full, mode, config, other, guilds, out);
                let rest: Vec<i64> = own.into_iter().filter(|c| *c != cid).collect();
                if !rest.is_empty() {
                    Self::add_making(full, mode, config, rest, guilds, out);
                }
            } else {
                full.making.push(other);
            }
        }
    }

    /// PvpConfirmJoin: the last confirmation of a starting game opens its play (TeamA red,
    /// TeamB blue) on a relay group, without a DS.
    pub fn confirm_join(self: &Arc<Self>, binder: Binder, cid: i64, rules: Rules) {
        *self.binder.lock().unwrap_or_else(PoisonError::into_inner) = Some(binder.clone());
        let mut binds = Vec::new();
        {
            let mut state = self.lock();
            let Some(mode) = state.players.get(&cid).map(|p| p.mode.clone()) else {
                return;
            };
            let Some(config) = rules.config.clone() else {
                return;
            };
            let Some(full) = state.full.get_mut(&mode) else {
                return;
            };
            let Some(i) = full.starting.iter().position(|g| g.pending.contains(&cid)) else {
                return;
            };
            full.starting[i].pending.remove(&cid);
            if !full.starting[i].pending.is_empty() {
                return;
            }
            let g = full.starting.remove(i);
            let Some(relay) = self.relay() else { return };
            state.next_play += 1;
            let id = state.next_play;
            let size = usize::try_from(config.team_size.max(0) * 2).unwrap_or(0);
            let rule = if config.rule == crate::domain::pvp::GameRule::GodMatch {
                team_rules::Rule::GodMatch
            } else {
                team_rules::Rule::DeathMatch
            };
            let game = TeamGame::new(
                team_rules::Rules {
                    rule,
                    game_length: config.game_length,
                    wait_before_start: config.wait_before_start,
                    wait_between_round: config.wait_between_round,
                    round_to_win: config.round_to_win,
                    win_signet: config.win_signet,
                    draw_signet: config.draw_signet,
                    lose_signet: config.lose_signet,
                    min_clear_time: config.min_clear_time,
                    is_guild: config.is_guild,
                    has_event_reward: !config.event_reward_item.is_empty(),
                    character_rank: rules.character_rank,
                },
                now_ms(),
            );
            state.plays.insert(
                id,
                Play {
                    mode: mode.clone(),
                    channel: -1,
                    group: relay.make_group(),
                    host: None,
                    host_info: None,
                    slots: vec![None; size],
                    config,
                    arena: None,
                    team: Some(game),
                    end_confirm: None,
                },
            );
            for (members, team) in [(g.a, arena::RED), (g.b, arena::BLUE)] {
                for member in members {
                    if add_to_team(&mut state, id, member, team) {
                        binds.push(member);
                    }
                }
            }
        }
        for member in binds {
            tokio::spawn(Arc::clone(self).bind(binder.clone(), member));
        }
    }

    /// PlayerConn HostChanged of every player after ReselectHost: the new host hosts again
    /// (PvpGameHosted), the others wait for its RegisterServer.
    fn host_changed(self: &Arc<Self>, play_id: i64) {
        let players: Vec<i64> = {
            let mut state = self.lock();
            let members: Vec<i64> = state
                .players
                .iter()
                .filter(|(_, p)| p.play == Some(play_id))
                .map(|(c, _)| *c)
                .collect();
            for c in &members {
                if let Some(p) = state.players.get_mut(c) {
                    p.stage = Stage::WaitingHostInfo;
                }
            }
            members
        };
        let host = self.lock().plays.get(&play_id).and_then(|p| p.host);
        if let Some(host) = host {
            self.join_complete(host);
        }
        for c in players.into_iter().filter(|c| Some(*c) != host) {
            self.join_complete(c);
        }
    }

    /// PvpUnregister: during the end confirmation it only confirms (everyone leaves when the
    /// last one does); otherwise Remove.
    pub fn unregister(self: &Arc<Self>, cid: i64) {
        let confirm = {
            let mut state = self.lock();
            let play_id = state.players.get(&cid).and_then(|p| p.play);
            match play_id.and_then(|id| state.plays.get_mut(&id).map(|p| (id, p))) {
                Some((_, play)) if play.end_confirm.is_some() => {
                    let set = play.end_confirm.get_or_insert_with(Default::default);
                    set.remove(&cid);
                    Some(if set.is_empty() {
                        play.slots.iter().flatten().copied().collect()
                    } else {
                        Vec::new()
                    })
                }
                _ => None,
            }
        };
        match confirm {
            Some(all) => {
                for c in all {
                    self.remove(c);
                }
            }
            None => self.remove(cid),
        }
    }

    /// PvpGivePanalty of the host: the player gets no team-game reward.
    pub fn penalty(self: &Arc<Self>, cid: i64, index: i32) {
        let mut state = self.lock();
        let Some((play_id, _)) = Self::seat(&state, cid) else {
            return;
        };
        if let Some(play) = state.plays.get_mut(&play_id)
            && play.host == Some(cid)
            && let (Some(t), Ok(slot)) = (play.team.as_mut(), usize::try_from(index))
        {
            t.penalty(slot);
        }
    }
}

impl PvpHub {
    /// PvpChatProcessor: a team chat goes to the sender's team (every player in the Arena),
    /// another chat to every player as a fake town chat.
    pub fn chat(self: &Arc<Self>, cid: i64, message: &str, team_chat: bool) {
        let out = {
            let state = self.lock();
            let Some((play_id, slot)) = Self::seat(&state, cid) else {
                return;
            };
            let (Some(play), Some(sender)) = (state.plays.get(&play_id), state.players.get(&cid))
            else {
                return;
            };
            let targets: Vec<i64> = match (&play.team, team_chat) {
                (Some(t), true) => {
                    let team = t.team_of(slot);
                    play.slots
                        .iter()
                        .enumerate()
                        .filter(|(s, c)| c.is_some() && t.team_of(*s) == team)
                        .filter_map(|(_, c)| *c)
                        .collect()
                }
                _ => play.slots.iter().flatten().copied().collect(),
            };
            let chat =
                super::minimum_mission::packet(&mwire::Message::PartyChat(mwire::PartyChat {
                    sender_name: Some(sender.name.clone()),
                    message: Some(message.to_owned()),
                    is_fake_town_chat: !team_chat,
                }));
            match chat {
                Ok(p) => targets
                    .into_iter()
                    .map(|c| (c, p.clone()))
                    .collect::<Vec<_>>(),
                Err(_) => return,
            }
        };
        self.deliver(out);
    }
}

/// A party member registered by its master (PvPPartyMemberInfo with the member's account).
pub struct TeamEntry {
    pub cid: i64,
    pub uid: i32,
    pub account: String,
    pub revision: i64,
    pub level: i32,
    pub name: String,
    pub guild_id: i32,
}

impl PvpHub {
    /// RegisterPvpPlayerProcessor of a party (the master's QueryPartyMembers): the members'
    /// duplicates, levels, the Full matching's party size and guild rules (the Arena takes one
    /// player), then one PvpPlayer per member and the team's AddMakingParty.
    pub fn register_team(
        self: &Arc<Self>,
        binder: Binder,
        members: Vec<TeamEntry>,
        rules: Rules,
        request: &wire::Register,
    ) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let mode = request.pvp_mode.clone().unwrap_or_default();
        let canceled =
            |format: &str, args: &[String]| -> Result<Vec<OutgoingPacket>, MinimumError> {
                Ok(vec![register_result(
                    &mode,
                    register_state::CANCELED,
                    true,
                    false,
                    format,
                    args,
                )?])
            };
        let Some(config) = rules.config.clone() else {
            return canceled("GameUI_Heroes_PvpJoinFail_InvalidPvpMode", &[]);
        };
        *self.binder.lock().unwrap_or_else(PoisonError::into_inner) = Some(binder);
        {
            let mut state = self.lock();
            let mut duplicated = false;
            for m in &members {
                duplicated |= state.players.remove(&m.cid).is_some();
            }
            if duplicated {
                return canceled("GameUI_Heroes_PvpJoinFail_PlayerDuplicated", &[]);
            }
        }
        for m in &members {
            if config.level_min != -1 && m.level < config.level_min {
                return canceled(
                    "GameUI_Heroes_PvpJoinFail_LevelConstraint_Min",
                    &[config.level_min.to_string()],
                );
            }
            if config.level_max != -1 && m.level > config.level_max {
                return canceled(
                    "GameUI_Heroes_PvpJoinFail_LevelConstraint_Max",
                    &[config.level_max.to_string()],
                );
            }
        }
        let size = usize::try_from(config.team_size).unwrap_or(0);
        match config.matching {
            Matching::Full if config.is_guild => {
                let guild = members.first().map_or(0, |m| m.guild_id);
                if members.len() != size {
                    return canceled(
                        "GameUI_Heroes_PvpJoinFail_InvalidPartySize",
                        &[config.team_size.to_string()],
                    );
                }
                if guild == 0 || members.iter().any(|m| m.guild_id != guild) {
                    return canceled("GameUI_Heroes_PvpJoinFail_InvalidGuildTeam", &[]);
                }
            }
            Matching::Full => {
                if members.len() > size {
                    return canceled(
                        "GameUI_Heroes_PvpJoinFail_InvalidPartySize",
                        &[config.team_size.to_string()],
                    );
                }
            }
            Matching::Arena => {
                return canceled(
                    "GameUI_Heroes_PvpJoinFail_InvalidPartySize",
                    &["1".to_owned()],
                );
            }
            _ => return canceled("GameUI_Heroes_PvpJoinFail_InvalidPvpMode", &[]),
        }
        {
            let mut state = self.lock();
            for m in &members {
                state.players.insert(
                    m.cid,
                    Player {
                        mode: mode.clone(),
                        channel: request.channel_id,
                        play: None,
                        slot: -1,
                        stage: Stage::Created,
                        proxy: Proxy::Waiting,
                        key: (rand_core::OsRng.next_u32() & 0x7fff_ffff) as i32,
                        join: None,
                        owner: Owner {
                            uid: m.uid,
                            name: m.account.clone(),
                            revision: m.revision,
                        },
                        name: m.name.clone(),
                        level: m.level,
                        guild_id: m.guild_id,
                    },
                );
            }
        }
        self.full_register_team(&mode, &config, members.iter().map(|m| m.cid).collect());
        Ok(Vec::new())
    }

    /// LockPartyProxy: a character registered in PvP keeps its party as it is.
    pub fn locks_party(&self, cid: i64) -> bool {
        self.lock().players.contains_key(&cid)
    }
}
