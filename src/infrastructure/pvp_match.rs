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
}

#[derive(Default)]
struct State {
    players: HashMap<i64, Player>,
    channels: HashMap<(String, i32), Channel>,
    plays: HashMap<i64, Play>,
    next_play: i64,
}

/// What an async player step needs: the store and the WhoIs profile.
#[derive(Clone)]
pub struct Binder {
    pub pool: PgPool,
    pub assets: Arc<Assets>,
    pub config: QueryConfig,
    pub arithmetic: Arithmetic,
}

/// The mode's PvpConfig and FeatureMatrix ArenaSpecatorLevelLimitMin.
pub struct Rules {
    pub config: Option<PvpConfig>,
    pub spectator_min: i32,
}
impl Rules {
    pub fn from_assets(assets: &Assets, mode: &str) -> Result<Self, MinimumError> {
        Ok(Self {
            config: assets.npc_pvp(mode).cloned(),
            spectator_min: assets
                .npc_policy()
                .map(|p| p.get_integer("ArenaSpecatorLevelLimitMin"))
                .transpose()
                .map_err(|_| fail())?
                .unwrap_or(0),
        })
    }
}

/// A registering character (RegisterPvpPlayer of one PvPPartyMemberInfo).
pub struct Entry<'a> {
    pub cid: i64,
    pub owner: &'a UserKey<'a>,
    pub level: i32,
}

pub struct PvpHub {
    state: Mutex<State>,
    party: Arc<PartyHub>,
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

/// ArenaGame.GameInfo outside InGame: TeamInfo {-1: "0, 0"}, every slot "0,0,0,0".
fn game_info(play: &Play) -> wire::PvpGameInfo {
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
    pub fn new(party: Arc<PartyHub>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            party,
            relay: std::sync::RwLock::new(None),
            binder: Mutex::new(None),
        }
    }
    pub fn set_relay(&self, relay: Option<Relay>) {
        *self.relay.write().unwrap_or_else(PoisonError::into_inner) = relay;
    }
    fn relay(&self) -> Option<Relay> {
        self.relay.read().unwrap_or_else(PoisonError::into_inner).clone()
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
        let Some(config) = rules.config else {
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
            if config.matching != Matching::Arena {
                // FullMatchingSystem (DeathMatch, GodMatch) is PV3.
                return canceled("GameUI_Heroes_PvpJoinFail_InvalidPvpMode");
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
                },
            );
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
        let canceled = |format: &str| register_result(&mode, register_state::CANCELED, true, false, format, &[]);
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
        let Ok(complete) = register_result(&mode, register_state::WAIT, true, false, "GameUI_Heroes_PvpJoin_WhoisComplete", &[]) else {
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
        if let Ok(p) = register_result(&mode, register_state::WAIT, true, false, "GameUI_Heroes_PvpJoin_P2PJoined", &[]) {
            self.deliver(vec![(cid, p)]);
        }
        self.join_complete(cid);
        // RelayClient Closed → Remove (a JoinP2pFail while still waiting for the host).
        loop {
            match events.recv().await {
                Ok(RelayEvent::Closed { cid: c }) if c == cid => {
                    let waiting_host = self
                        .lock()
                        .players
                        .get(&cid)
                        .is_some_and(|p| p.play == Some(play_id) && p.stage == Stage::WaitingHostInfo);
                    if waiting_host
                        && let Ok(p) = canceled("GameUI_Heroes_PvpJoinFail_JoinP2pFail")
                    {
                        self.deliver(vec![(cid, p)]);
                    }
                    if self.lock().players.get(&cid).is_some_and(|p| p.play == Some(play_id)) {
                        self.remove(cid);
                    }
                    return;
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if !self.lock().players.get(&cid).is_some_and(|p| p.play == Some(play_id)) {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    }

    /// PvpPlayer.JoinComplete.
    fn join_complete(&self, cid: i64) {
        let mut out = Vec::new();
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
            let team = wire::team::INVALID;
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
            } else {
                if let Ok(w) = register_result(&mode, register_state::WAIT, true, false, "GameUI_Heroes_PvpJoin_WaitingHost", &[]) {
                    out.push((cid, w));
                }
                if let Some(p) = state.players.get_mut(&cid) {
                    p.stage = Stage::WaitingHostInfo;
                }
            }
        }
        self.deliver(out);
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
                .filter(|(c, p)| p.play == Some(play_id) && p.stage == Stage::WaitingHostInfo && **c != cid)
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
        {
            let mut state = self.lock();
            let Some(p) = state.players.remove(&cid) else {
                return;
            };
            let key = (p.mode.clone(), p.channel);
            if let Some(play_id) = p.play {
                if let Ok(k) = packet(&wire::Message::Kicked(wire::Empty {})) {
                    out.push((cid, k));
                }
                let mut close = false;
                if let Some(play) = state.plays.get_mut(&play_id) {
                    if let Some(s) = play.slots.iter_mut().find(|s| **s == Some(cid)) {
                        *s = None;
                    }
                    if let Some(relay) = self.relay() {
                        relay.leave(play.group, cid);
                    }
                    if play.host == Some(cid) {
                        cascade.extend(play.slots.iter().flatten().copied());
                    }
                    close = play.slots.iter().all(Option::is_none);
                }
                if close
                    && let Some(play) = state.plays.remove(&play_id)
                {
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
                && let Ok(d) = register_result(&p.mode, register_state::CANCELED, true, false, "GameUI_Heroes_PvpJoinFail_Dodged", std::slice::from_ref(&p.owner.name))
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
        for other in cascade {
            if other != cid {
                self.remove(other);
            }
        }
        let binder = self.binder.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(binder) = binder {
            for next in binds {
                tokio::spawn(Arc::clone(self).bind(binder.clone(), next));
            }
        }
    }
}

/// ArenaTeam.AddPlayer: the first player of a hostless play is its host; the first free slot.
fn add_to_play(state: &mut State, play_id: i64, cid: i64) -> bool {
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
