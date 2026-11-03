//! The party authority of the frontend process (MicroPlayService parties and the
//! FrontendService operations they push): the single broadcast point for every
//! party-visible change. Each entered character has a seat with a mailbox; the caller of
//! an operation gets its own packets back in order, and every other member's packets go
//! to its mailbox (UpdateParty, SendInvitation, Kick, SendPacket).
use super::minimum_character::MinimumError;
use crate::domain::map_partition::OutgoingPacket;
use crate::domain::party::{
    JoinFail, Member, Party, RejectReason, Removal, ShipLimits, StateFail, party_state,
};
use crate::protocol::mission::{
    self as wire, BlessStones, GameInfo, Message, PaidCoin, PartyInfo, PartyInvitationRejected,
    PartyInvited, PayCoinCompleted, Ship, ShipOption, UpdateShip,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, MutexGuard, PoisonError};
use tokio::sync::mpsc;

/// Mailbox bound per entered character; a full mailbox drops the push (the member is
/// stalled or gone and its connection closes the seat).
pub const MAILBOX: usize = 256;

/// What a member's connection receives from the party authority.
pub enum Notice {
    /// SendPacket / UpdateParty / SendInvitation pushes, in order.
    Packets(Vec<OutgoingPacket>),
    /// The Kick operation: the party removed this member (kick or removal of the party).
    Kicked { party_id: i64 },
    /// A MicroPlayHost message the play applies to every player (each member keeps a
    /// replica of the MicroPlay for its own MicroPlayer).
    Play(Box<Message>),
    /// The host started the play: the replica starts from this snapshot.
    Started(Box<Snapshot>),
    /// An erg won by this player (ErgInfo.Winner): GiveItem of its reward.
    Give { class_ex: String, count: i32 },
    /// MicroPlay.ChangeHost: this player's replica becomes the game host.
    BecomeHost(Box<Snapshot>),
    /// Trigger_LeaveWithHost: this player follows the leaving host (GiveReward_Season2Success
    /// when the main goal was achieved, else GiveReward_QuestFail(GiveUpQuest)).
    FollowHost,
    /// FinishMicroPlayHost: the host's return takes this player back too
    /// (ReturnFromQuest of a Finished player).
    ReturnNow,
    /// HostElectionFinished: the new host restarted its server (HostInfo to this player,
    /// and this player's MemberInfo to the new host).
    HostRestarted { host: i64, info: Option<GameInfo> },
    /// NoReward_SinkShip: ReturnFromQuest(ToTown) and Leave.
    Sink,
    /// EntityCommunicator.NoticeMail: a mail to this character was created (NoticeMail
    /// operation → MailBox.NoticeMail).
    Mail { mail_id: i64 },
}

/// MicroPlayEntityInfo of players disconnected from a play in progress
/// (CombatRejoinSystem): CID → (party, slot). The original keeps it in the database and
/// trusts only rows newer than the service start, so a process-wide map is equivalent.
static REJOIN: Mutex<BTreeMap<i64, (i64, i32)>> = Mutex::new(BTreeMap::new());

/// CharacterList LoginPartyState: 1 when the character can rejoin its play.
pub fn rejoinable(cid: i64) -> bool {
    REJOIN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains_key(&cid)
}

/// The shared sector state of a play: the sectors the host loaded and the sequence as
/// it advances (without anyone's evil cores).
#[derive(Clone)]
pub struct Snapshot {
    pub sectors: crate::domain::mission::SectorInfos,
    pub sequence: crate::domain::mission::SectorSequence,
    pub game_time: i32,
    pub in_game: bool,
}

/// A MicroPlayer of the play: its CID and level (MonsterKillExpCalculator weight).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    pub cid: i64,
    pub level: i32,
}

/// The member's BattleServiceProxy state as the party sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Initial: no party.
    None,
    Master {
        party_id: i64,
        state: u8,
    },
    Member {
        party_id: i64,
        state: u8,
        slot: i32,
        ready: u8,
    },
}

/// Result of an invitation, for the inviting master.
#[derive(Debug, PartialEq, Eq)]
pub enum Invite {
    /// InvitePartyMember completed (the invitation may still have been refused by the
    /// target's frontend, which the master learns from PartyInvitationRejected).
    Sent(Vec<OutgoingPacket>),
    /// The operation failed: PartyInvitationRejected to the master, and whether the party
    /// was removed with it.
    Failed(Vec<OutgoingPacket>),
}

/// The party's MicroPlay as its members join it (StartMicroPlay2 to the end of the play).
#[derive(Clone)]
pub struct Play {
    pub quest: String,
    pub title: Option<String>,
    pub option: ShipOption,
    pub swear_id: i32,
    /// The relay P2P group of the play.
    pub group: i32,
    /// MicroPlay.GameHost: the host's CID.
    pub host: i64,
    /// MicroPlay.HostInfo (RegisterServer of the host).
    pub host_info: Option<GameInfo>,
    /// MicroPlay.Players: slot → player.
    pub players: BTreeMap<i32, Player>,
    /// The sector state for players joining or rejoining a started play.
    pub snapshot: Option<Snapshot>,
    /// PlayState.Restarting: a host election waits for the new host's RegisterServer.
    pub restarting: bool,
    /// LeaveWithHostPlayers: CID → followHost (None while undecided; empty once triggered).
    pub leave_with_host: Option<BTreeMap<i64, Option<bool>>>,
    /// The players who followed the leaving host.
    pub followers: Vec<i64>,
}

struct Seat {
    name: String,
    /// The latest party entry of the character (for a reserved party).
    member: Option<Member>,
    mailbox: mpsc::Sender<Notice>,
    party: Option<i64>,
    /// Initial.InvitedPartyID.
    invited_party: Option<i64>,
}
struct Entry {
    party: Party,
    /// ShipInfo fields the master's RegisterShipInfo fixed; members, counts, state and the
    /// host's ping are refreshed on every broadcast (RefreshShipInfo).
    ship: Option<Ship>,
    play: Option<Play>,
    /// !HideTownShip: RegisterShipInfo with UpdateShipList (OpenPartyWithShipInfo).
    town_ship_listed: bool,
}
impl Entry {
    /// Party.RefreshShipInfo: the fixed ship fields with the current members, counts,
    /// state and the master's ping. None without a ship.
    fn refreshed_ship(&self) -> Option<Ship> {
        let party = &self.party;
        if !party.has_ship() {
            return None;
        }
        let mut ship = self.ship.clone()?;
        let master = party.master();
        ship.members = Some(party.members().map(|m| Some(info(m))).collect());
        ship.member_count = party.member_count() as i32;
        ship.state = party.state();
        ship.max_ship_member_count = party.party_size();
        ship.host_ping = master.map_or(-1, |m| m.latest_ping);
        ship.host_frame_rate = master.map_or(255, |m| m.latest_frame_rate);
        ship.is_return = master.is_some_and(|m| m.is_return);
        Some(ship)
    }
    /// MicroPlayService.UpdateShipList: a ship is listed while InShip, or in town when
    /// OpenPartyWithShipInfo registered it (not HideTownShip); a hunting quest while
    /// InGame; never a party-only ship or a giant raid.
    fn listed(&self) -> Option<Ship> {
        let ship = self.refreshed_ship()?;
        let state = self.party.state();
        let visible = if ship.is_hunting_quest {
            state == party_state::IN_GAME
        } else {
            state == party_state::IN_SHIP
                || (state == party_state::IN_TOWN_WITH_SHIP_INFO && self.town_ship_listed)
        };
        (visible && !ship.is_party_only && !ship.is_giant_raid).then_some(ship)
    }
}

#[derive(Default)]
struct State {
    seats: HashMap<i64, Seat>,
    parties: HashMap<i64, Entry>,
    /// ReserveInfo of the party opened for the players who left with the host.
    reserved: HashMap<i64, i64>,
}

pub struct PartyHub {
    state: Mutex<State>,
    /// FeatureMatrix ForceClosePartyInShip.
    force_close_in_ship: bool,
    /// FeatureMatrix CombatRejoinSystem.
    combat_rejoin: bool,
    /// CombatRejoinSystem_LimitMinutes plus 30 s (RemovePartyScheduleID).
    rejoin_limit: std::time::Duration,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn packet(message: &Message) -> Result<OutgoingPacket, MinimumError> {
    let kind = message.kind();
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(
            kind,
            Some(message),
            super::minimum_mission::MAX_MISSION_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    })
}
/// InsertBlessStoneComplete of a slot without stones (SlotDic order: ALL, AP, EXP, LUCK).
pub fn stones(slot: i32) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::InsertBlessStoneComplete(BlessStones {
        slot,
        owners: Some(vec![-1; 4]),
        types: Some(vec![0, 2, 1, 3]),
    }))
}
/// AskFinishQuestMessage(pending, message, LeaveWithHostDecisionTimeOut = 20).
fn ask_finish(pending: bool, text: &str) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::AskFinishQuest(wire::AskFinishQuest {
        show_pending_dialog: pending,
        message: Some(text.to_owned()),
        time_out: 20,
    }))
}
fn info(m: &Member) -> wire::PartyMember {
    wire::PartyMember {
        nexon_sn: m.nexon_sn,
        character: m.character,
        character_id: Some(m.name.clone()),
        slot: m.slot,
        level: m.level,
        state: m.state,
        latest_ping: m.latest_ping,
        latest_frame_rate: m.latest_frame_rate,
        is_return: m.is_return,
        is_event_jumping: m.is_event_jumping,
    }
}
/// PartyMember.SendInvitationRejectedMessage.
fn rejected(name: &str, reason: RejectReason) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::PartyInvitationRejected(PartyInvitationRejected {
        invited_name: Some(name.to_owned()),
        reason: reason as i32,
    }))
}

impl State {
    /// PartyMember.SendPartyInfo for every member (Party.BroadCastPartyInfo); nothing while
    /// EnteringShip (IsLoadingState). Returns the packets of `caller`.
    fn broadcast(&self, party_id: i64, caller: i64) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let Some(entry) = self.parties.get(&party_id) else {
            return Ok(Vec::new());
        };
        let party = &entry.party;
        if party.state() == party_state::ENTERING_SHIP {
            return Ok(Vec::new());
        }
        let members: Vec<Option<wire::PartyMember>> =
            party.members().map(|m| Some(info(m))).collect();
        let mut out = vec![packet(&Message::PartyInfo(PartyInfo {
            party_id,
            party_size: party.party_size(),
            state: party.state(),
            members: Some(members),
        }))?];
        if let Some(ship) = entry.refreshed_ship() {
            out.push(packet(&Message::UpdateShip(UpdateShip {
                info: Some(ship),
            }))?);
        }
        let mut own = Vec::new();
        for m in party.members() {
            if m.cid == caller {
                own = out.clone();
            } else {
                self.push(m.cid, Notice::Packets(out.clone()));
            }
        }
        Ok(own)
    }
    fn push(&self, cid: i64, notice: Notice) {
        if let Some(seat) = self.seats.get(&cid)
            && seat.mailbox.try_send(notice).is_err()
        {
            tracing::warn!("party notice dropped by a full or closed mailbox; values omitted");
        }
    }
    /// MicroPlayService.RemoveParty → Party.Close → BroadCastKicked.
    fn dissolve(&mut self, party_id: i64, caller: i64) -> bool {
        let Some(entry) = self.parties.remove(&party_id) else {
            return false;
        };
        let mut caller_kicked = false;
        for m in entry.party.members() {
            if let Some(seat) = self.seats.get_mut(&m.cid) {
                seat.party = None;
            }
            if m.cid == caller {
                caller_kicked = true;
            } else {
                self.push(m.cid, Notice::Kicked { party_id });
            }
        }
        for seat in self.seats.values_mut() {
            if seat.invited_party == Some(party_id) {
                seat.invited_party = None;
            }
        }
        caller_kicked
    }
    /// RemoveMember of `cid` and what follows it; the removed member gets nothing (its own
    /// frontend already left). Returns the packets of `caller` (when another member).
    fn remove(
        &mut self,
        party_id: i64,
        cid: i64,
        caller: i64,
        force_close_in_ship: bool,
    ) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let keep_for_rejoin = REJOIN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&cid)
            .is_some_and(|(p, _)| *p == party_id);
        let Some(entry) = self.parties.get_mut(&party_id) else {
            return Ok(Vec::new());
        };
        let removal = entry.party.remove(cid, force_close_in_ship);
        if let Some(seat) = self.seats.get_mut(&cid)
            && seat.party == Some(party_id)
        {
            seat.party = None;
        }
        let waits_for_rejoin = keep_for_rejoin
            && self.parties.get(&party_id).is_some_and(|e| {
                e.party.member_count() == 0 && e.play.as_ref().is_some_and(|p| p.snapshot.is_some())
            });
        match removal {
            None => Ok(Vec::new()),
            Some(Removal::Dissolved) if waits_for_rejoin => {
                // RemoveParty under CombatRejoinSystem: the play waits for its players
                // without a host (GameHost = null) until the rejoin limit.
                if let Some(play) = self
                    .parties
                    .get_mut(&party_id)
                    .and_then(|e| e.play.as_mut())
                {
                    play.players.retain(|_, p| p.cid != cid);
                    play.host = 0;
                    play.restarting = false;
                }
                Ok(Vec::new())
            }
            Some(Removal::Dissolved) => {
                self.dissolve(party_id, caller);
                Ok(Vec::new())
            }
            Some(Removal::Continues) => {
                if let Some(entry) = self.parties.get_mut(&party_id) {
                    if !entry.party.has_ship() {
                        entry.ship = None;
                    }
                    if !entry.party.has_micro_play() {
                        entry.play = None;
                    }
                    if let Some(play) = entry.play.as_mut() {
                        play.players.retain(|_, p| p.cid != cid);
                    }
                }
                self.broadcast(party_id, caller)
            }
        }
    }
}

impl PartyHub {
    pub fn new(force_close_in_ship: bool, combat_rejoin: bool) -> Self {
        Self {
            state: Mutex::new(State::default()),
            force_close_in_ship,
            combat_rejoin,
            rejoin_limit: std::time::Duration::from_secs(30),
        }
    }
    /// CombatRejoinSystem_LimitMinutes of the feature policy.
    pub fn with_rejoin_limit_minutes(mut self, minutes: i32) -> Self {
        let minutes = u64::try_from(minutes).unwrap_or(0);
        self.rejoin_limit = std::time::Duration::from_secs(minutes * 60 + 30);
        self
    }
    pub fn rejoin_limit(&self) -> std::time::Duration {
        self.rejoin_limit
    }
    pub fn combat_rejoin(&self) -> bool {
        self.combat_rejoin
    }
    /// StartHostElection → QueryHostCID → ChangeHost when the host leaves a play in
    /// progress: the first other player becomes the party master and the game host; every
    /// other player gets HostRestarting and the new host its takeover. None when nobody
    /// is left (the play ends with its last player).
    pub fn elect(&self, party_id: i64, leaving: i64) -> Result<Option<i64>, MinimumError> {
        let mut s = self.lock();
        let Some(entry) = s.parties.get_mut(&party_id) else {
            return Ok(None);
        };
        let Some(play) = entry.play.as_mut() else {
            return Ok(None);
        };
        if play.host != leaving {
            return Ok(None);
        }
        let Some((&old_slot, &new_host)) = play.players.iter().find(|(_, p)| p.cid != leaving)
        else {
            return Ok(None);
        };
        play.players.retain(|_, p| p.cid != leaving);
        play.players.remove(&old_slot);
        play.players.insert(0, new_host);
        play.host = new_host.cid;
        play.host_info = None;
        play.restarting = true;
        let snapshot = play.snapshot.clone();
        let others: Vec<i64> = play.players.values().map(|p| p.cid).collect();
        if !entry.party.transfer_master(leaving, new_host.cid) {
            tracing::warn!("host election could not move the party master; values omitted");
        }
        let restarting = packet(&Message::HostRestarting)?;
        for cid in others {
            s.push(cid, Notice::Packets(vec![restarting.clone()]));
        }
        if let Some(snapshot) = snapshot {
            s.push(new_host.cid, Notice::BecomeHost(Box::new(snapshot)));
        }
        Ok(Some(new_host.cid))
    }
    /// HostRestarted → HostElectionFinished: the play's HostInfo; the other players learn it.
    pub fn host_restarted(&self, party_id: i64, host: i64, info: Option<GameInfo>) -> bool {
        let mut s = self.lock();
        let Some(play) = s.parties.get_mut(&party_id).and_then(|e| e.play.as_mut()) else {
            return false;
        };
        if !play.restarting || play.host != host {
            return false;
        }
        play.restarting = false;
        play.host_info = info.clone();
        let others: Vec<i64> = play
            .players
            .values()
            .filter(|p| p.cid != host)
            .map(|p| p.cid)
            .collect();
        for cid in others {
            s.push(
                cid,
                Notice::HostRestarted {
                    host,
                    info: info.clone(),
                },
            );
        }
        true
    }
    /// ElectionFailed: the new host never restarted; NoReward_SinkShip for every player.
    pub fn election_timeout(&self, party_id: i64, host: i64) {
        let s = self.lock();
        let Some(play) = s.parties.get(&party_id).and_then(|e| e.play.as_ref()) else {
            return;
        };
        if !play.restarting || play.host != host {
            return;
        }
        for p in play.players.values() {
            s.push(p.cid, Notice::Sink);
        }
    }
    /// A player disconnected from a play in progress keeps its party and slot for
    /// CombatRejoinSystem (MicroPlayEntityInfo, DisconnectedMemberCID).
    pub fn remember_disconnect(&self, cid: i64, party_id: i64, slot: i32) {
        if self.combat_rejoin {
            REJOIN
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(cid, (party_id, slot));
        }
    }
    /// RemoveMicroPlayEntityInfo / DeleteDisconnectedUserInfo.
    pub fn forget_disconnect(&self, cid: i64) {
        REJOIN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&cid);
    }
    /// RequestJoinParty(RequestType > 0): JoinParty(REJOIN) of the remembered party; its
    /// PartyInfo for `member`. None without a rejoinable play.
    pub fn rejoin(&self, member: Member) -> Result<Option<Vec<OutgoingPacket>>, MinimumError> {
        let cid = member.cid;
        let Some((party_id, slot)) = REJOIN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&cid)
            .copied()
        else {
            return Ok(None);
        };
        let mut s = self.lock();
        if s.seats.get(&cid).is_none_or(|seat| seat.party.is_some()) {
            return Ok(None);
        }
        let joined = s
            .parties
            .get_mut(&party_id)
            .filter(|e| e.play.as_ref().is_some_and(|p| p.snapshot.is_some()))
            .map(|e| e.party.rejoin(member, slot));
        match joined {
            Some(Ok(_)) => {
                if let Some(seat) = s.seats.get_mut(&cid) {
                    seat.party = Some(party_id);
                    seat.invited_party = None;
                }
                s.broadcast(party_id, cid).map(Some)
            }
            _ => Ok(None),
        }
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    /// The entered character becomes reachable (QueryFrontendID succeeds for it).
    pub fn register(&self, cid: i64, name: &str, mailbox: mpsc::Sender<Notice>) {
        self.lock().seats.insert(
            cid,
            Seat {
                name: name.to_owned(),
                member: None,
                mailbox,
                party: None,
                invited_party: None,
            },
        );
    }
    /// EntityCommunicator.NoticeMail(cid, mailID) with makeEntity false: delivered only to an
    /// entered character.
    pub fn notice_mail(&self, cid: i64, mail_id: i64) {
        self.lock().push(cid, Notice::Mail { mail_id });
    }
    /// The character's frontend closed: RemoveMember("FrontendDisconnected") of its party
    /// and InvitationRejected(NotAvailable) of a pending invitation.
    pub fn unregister(&self, cid: i64) {
        let mut s = self.lock();
        let Some(seat) = s.seats.remove(&cid) else {
            return;
        };
        if let Some(party_id) = seat.invited_party {
            let _ = Self::reject_in(
                &mut s,
                &seat.name,
                cid,
                party_id,
                RejectReason::NotAvailable,
            );
        }
        if let Some(party_id) = seat.party
            && s.remove(party_id, cid, cid, self.force_close_in_ship)
                .is_err()
        {
            tracing::warn!("party removal of a closed frontend failed; values omitted");
        }
    }
    /// The BattleServiceProxy state of `cid`.
    pub fn role(&self, cid: i64) -> Role {
        let s = self.lock();
        let Some(party_id) = s.seats.get(&cid).and_then(|seat| seat.party) else {
            return Role::None;
        };
        let Some(entry) = s.parties.get(&party_id) else {
            return Role::None;
        };
        let state = entry.party.state();
        match entry.party.member(cid) {
            Some(m) if m.state == crate::domain::party::ready_state::HOST => {
                Role::Master { party_id, state }
            }
            Some(m) => Role::Member {
                party_id,
                state,
                slot: m.slot,
                ready: m.state,
            },
            None => Role::None,
        }
    }
    /// QueryFrontendID: the CID of an entered character (name comparison as SQL Server's
    /// case-insensitive collation).
    pub fn entered(&self, name: &str) -> Option<i64> {
        self.lock()
            .seats
            .iter()
            .find(|(_, seat)| seat.name.eq_ignore_ascii_case(name))
            .map(|(cid, _)| *cid)
    }
    /// OpenNewParty → OpenParty + JoinParty of the master in slot 0; its PartyInfo.
    pub fn open(&self, party_id: i64, master: Member) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let mut s = self.lock();
        let cid = master.cid;
        let seat = s.seats.get_mut(&cid).ok_or_else(fail)?;
        if seat.party.is_some() {
            return Err(fail());
        }
        seat.party = Some(party_id);
        seat.invited_party = None;
        seat.member = Some(master.clone());
        let mut party = Party::new(party_id);
        party.join(master).map_err(|_| fail())?;
        s.parties.insert(
            party_id,
            Entry {
                party,
                ship: None,
                play: None,
                town_ship_listed: false,
            },
        );
        s.broadcast(party_id, cid)
    }
    /// InvitePartyMember (MicroPlay) → SendInvitation (the target's frontend).
    pub fn invite(
        &self,
        party_id: i64,
        inviting: i64,
        invited: i64,
        invited_name: &str,
    ) -> Result<Invite, MinimumError> {
        let mut s = self.lock();
        let entry = s.parties.get_mut(&party_id).ok_or_else(fail)?;
        let master_name = entry
            .party
            .master()
            .map(|m| m.name.clone())
            .unwrap_or_default();
        if let Err(reason) = entry.party.check_invite(inviting, invited) {
            let out = vec![rejected(invited_name, reason)?];
            if invited == inviting && entry.party.is_to_remove() {
                s.dissolve(party_id, inviting);
            }
            return Ok(Invite::Failed(out));
        }
        // QueryFrontendID: the invited character must be entered.
        if !s.seats.contains_key(&invited) {
            let out = vec![rejected(invited_name, RejectReason::ClientNotLoggedOn)?];
            if s.parties
                .get(&party_id)
                .is_some_and(|e| e.party.is_to_remove())
            {
                s.dissolve(party_id, inviting);
            }
            return Ok(Invite::Failed(out));
        }
        if let Some(e) = s.parties.get_mut(&party_id) {
            e.party.add_invitation(invited);
        }
        // SendInvitationProcessor: the target must be in Initial without another invitation.
        let target = s.seats.get_mut(&invited).ok_or_else(fail)?;
        if target.party.is_some() || target.invited_party.is_some() {
            if let Some(e) = s.parties.get_mut(&party_id) {
                e.party.remove_invitation(invited);
            }
            let out = vec![rejected(invited_name, RejectReason::AlreadyInParty)?];
            if s.parties
                .get(&party_id)
                .is_some_and(|e| e.party.is_to_remove())
            {
                s.dissolve(party_id, inviting);
            }
            return Ok(Invite::Sent(out));
        }
        target.invited_party = Some(party_id);
        let invitation = packet(&Message::PartyInvited(PartyInvited {
            host_name: Some(master_name),
            party_id,
        }))?;
        s.push(invited, Notice::Packets(vec![invitation]));
        Ok(Invite::Sent(Vec::new()))
    }
    /// InvitationRejected (MicroPlay) for the invited character's reject or a failed accept.
    fn reject_in(
        s: &mut State,
        name: &str,
        cid: i64,
        party_id: i64,
        reason: RejectReason,
    ) -> Result<(), MinimumError> {
        if let Some(seat) = s.seats.get_mut(&cid)
            && seat.invited_party == Some(party_id)
        {
            seat.invited_party = None;
        }
        let Some(entry) = s.parties.get_mut(&party_id) else {
            return Ok(());
        };
        entry.party.remove_invitation(cid);
        let Some(master) = entry.party.master().map(|m| m.cid) else {
            return Ok(());
        };
        s.push(master, Notice::Packets(vec![rejected(name, reason)?]));
        if s.parties
            .get(&party_id)
            .is_some_and(|e| e.party.is_to_remove())
        {
            s.dissolve(party_id, cid);
        }
        Ok(())
    }
    /// PartyInvitationRejectMessage: InvitationRejected(Rejected); InvitedPartyID = 0.
    pub fn reject(&self, cid: i64, party_id: i64) -> Result<(), MinimumError> {
        let mut s = self.lock();
        let name = s
            .seats
            .get(&cid)
            .map(|seat| seat.name.clone())
            .unwrap_or_default();
        Self::reject_in(&mut s, &name, cid, party_id, RejectReason::Rejected)
    }
    /// PartyInvitationAcceptMessage: JoinParty(NORMAL) of the invited party. None when the
    /// accepted party is not the invitation (logged and ignored by the original).
    pub fn accept(
        &self,
        party_id: i64,
        member: Member,
    ) -> Result<Option<Result<Vec<OutgoingPacket>, JoinFail>>, MinimumError> {
        let mut s = self.lock();
        let cid = member.cid;
        let name = member.name.clone();
        let seat = s.seats.get_mut(&cid).ok_or_else(fail)?;
        if seat.invited_party != Some(party_id) || seat.party.is_some() {
            return Ok(None);
        }
        seat.member = Some(member.clone());
        let joined = match s.parties.get_mut(&party_id) {
            Some(entry) => entry.party.join(member),
            None => Err(JoinFail::NotInvited),
        };
        match joined {
            Ok(_) => {
                if let Some(seat) = s.seats.get_mut(&cid) {
                    seat.party = Some(party_id);
                    seat.invited_party = None;
                }
                s.broadcast(party_id, cid).map(|p| Some(Ok(p)))
            }
            Err(e) => {
                // InvitationRejected(NotAvailable) to the party, then LeaveParty.
                Self::reject_in(&mut s, &name, cid, party_id, RejectReason::NotAvailable)?;
                Ok(Some(Err(e)))
            }
        }
    }
    /// BattleServiceProxy.LeaveParty: the frontend closes its party connection and the
    /// party removes the member. False when `cid` was not in a party.
    pub fn leave(&self, cid: i64) -> Result<bool, MinimumError> {
        let mut s = self.lock();
        let Some(party_id) = s.seats.get(&cid).and_then(|seat| seat.party) else {
            return Ok(false);
        };
        s.remove(party_id, cid, cid, self.force_close_in_ship)?;
        Ok(true)
    }
    /// KickMessage of the master: KickMember; the kicked member's frontend gets Kick.
    pub fn kick(
        &self,
        master: i64,
        slot: i32,
        nexon_sn: i32,
    ) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let mut s = self.lock();
        let Some(party_id) = s.seats.get(&master).and_then(|seat| seat.party) else {
            return Ok(Vec::new());
        };
        let Some(kicked) = s
            .parties
            .get(&party_id)
            .and_then(|e| e.party.kick(master, slot, nexon_sn))
        else {
            return Ok(Vec::new());
        };
        s.push(kicked, Notice::Kicked { party_id });
        s.remove(party_id, kicked, master, self.force_close_in_ship)
    }
    /// TransferMaster of the current master.
    pub fn transfer_master(
        &self,
        master: i64,
        new_master: i64,
    ) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let mut s = self.lock();
        let Some(party_id) = s.seats.get(&master).and_then(|seat| seat.party) else {
            return Ok(Vec::new());
        };
        let moved = s
            .parties
            .get_mut(&party_id)
            .is_some_and(|e| e.party.transfer_master(master, new_master));
        if !moved {
            return Ok(Vec::new());
        }
        s.broadcast(party_id, master)
    }
    /// RegisterShipInfo / SetShipInfo: the ship's fixed fields and the party size.
    pub fn set_ship(
        &self,
        party_id: i64,
        ship: Ship,
        limits: ShipLimits,
        story_quest: bool,
        listed_in_town: bool,
    ) -> Result<(), MinimumError> {
        let mut s = self.lock();
        let entry = s.parties.get_mut(&party_id).ok_or_else(fail)?;
        if !entry.party.set_ship(limits, story_quest) {
            return Err(fail());
        }
        entry.ship = Some(ship);
        entry.town_ship_listed = listed_in_town;
        Ok(())
    }
    /// StartMicroPlay2: the party's MicroPlay exists.
    pub fn start_micro_play(&self, party_id: i64) {
        if let Some(e) = self.lock().parties.get_mut(&party_id) {
            e.party.start_micro_play();
        }
    }
    /// Party.ChangeState.
    pub fn change_state(&self, party_id: i64, state: u8) {
        if let Some(e) = self.lock().parties.get_mut(&party_id) {
            e.party.change_state(state);
        }
    }
    /// BroadCastPartyInfo; the packets of `caller`.
    pub fn broadcast(
        &self,
        party_id: i64,
        caller: i64,
    ) -> Result<Vec<OutgoingPacket>, MinimumError> {
        self.lock().broadcast(party_id, caller)
    }
    /// SetMemberState of a slot, broadcasting a change.
    pub fn set_member_state(
        &self,
        party_id: i64,
        slot: i32,
        state: u8,
        caller: i64,
    ) -> Result<Result<Vec<OutgoingPacket>, StateFail>, MinimumError> {
        let mut s = self.lock();
        let Some(entry) = s.parties.get_mut(&party_id) else {
            return Ok(Err(StateFail::InvalidSlot));
        };
        match entry.party.set_member_state(slot, state) {
            Ok(true) => s.broadcast(party_id, caller).map(Ok),
            Ok(false) => Ok(Ok(Vec::new())),
            Err(e) => Ok(Err(e)),
        }
    }
    /// StartMicroPlay2 → AddPlayer of the host: the play of the party.
    pub fn set_play(&self, party_id: i64, play: Play) {
        if let Some(e) = self.lock().parties.get_mut(&party_id) {
            e.play = Some(play);
        }
    }
    /// HostStarted: MicroPlay.HostInfo.
    pub fn set_host_info(&self, party_id: i64, info: Option<GameInfo>) {
        if let Some(play) = self
            .lock()
            .parties
            .get_mut(&party_id)
            .and_then(|e| e.play.as_mut())
        {
            play.host_info = info;
        }
    }
    pub fn play(&self, party_id: i64) -> Option<Play> {
        self.lock()
            .parties
            .get(&party_id)
            .and_then(|e| e.play.clone())
    }
    /// MicroPlay.AddPlayer of a joining member: BroadCastCoinInfo (every player gets
    /// the paid coins of every slot) and BlessStone.SendStoneInfo (every player gets its
    /// own slot). The packets of `cid`; None when the play is gone or the slot taken.
    pub fn add_player(
        &self,
        party_id: i64,
        slot: i32,
        player: Player,
    ) -> Result<Option<Vec<OutgoingPacket>>, MinimumError> {
        let cid = player.cid;
        let mut s = self.lock();
        let Some(play) = s.parties.get_mut(&party_id).and_then(|e| e.play.as_mut()) else {
            return Ok(None);
        };
        if play.players.get(&slot).is_some_and(|p| p.cid != cid) {
            return Ok(None);
        }
        play.players.retain(|_, p| p.cid != cid);
        play.players.insert(slot, player);
        let players = play.players.clone();
        let coins = packet(&Message::PayCoinCompleted(PayCoinCompleted {
            coins: Some(
                players
                    .keys()
                    .map(|slot| {
                        Some(PaidCoin {
                            slot: *slot,
                            silver_coin: Some(Vec::new()),
                            platinum_coin_owner: -1,
                            platinum_coin_type: 0,
                        })
                    })
                    .collect(),
            ),
        }))?;
        let mut own = Vec::new();
        for (player_slot, player) in &players {
            let packets = vec![coins.clone(), stones(*player_slot)?];
            if player.cid == cid {
                own = packets;
            } else {
                s.push(player.cid, Notice::Packets(packets));
            }
        }
        Ok(Some(own))
    }
    /// The players of the play in slot order.
    pub fn players(&self, party_id: i64) -> Vec<(i32, Player)> {
        self.lock()
            .parties
            .get(&party_id)
            .and_then(|e| e.play.as_ref())
            .map(|p| p.players.iter().map(|(s, p)| (*s, *p)).collect())
            .unwrap_or_default()
    }
    /// The slot of a player in the play.
    pub fn slot_of(&self, party_id: i64, cid: i64) -> Option<i32> {
        self.players(party_id)
            .into_iter()
            .find(|(_, p)| p.cid == cid)
            .map(|(s, _)| s)
    }
    /// A host message every other player's replica applies.
    pub fn replicate(&self, party_id: i64, from: i64, message: &Message) {
        let s = self.lock();
        let Some(play) = s.parties.get(&party_id).and_then(|e| e.play.as_ref()) else {
            return;
        };
        for p in play.players.values().filter(|p| p.cid != from) {
            s.push(p.cid, Notice::Play(Box::new(message.clone())));
        }
    }
    /// QueryMicroPlayInfo at StartGame: the snapshot every other player starts from.
    pub fn start_play(&self, party_id: i64, from: i64, snapshot: Snapshot) {
        let mut s = self.lock();
        let Some(play) = s.parties.get_mut(&party_id).and_then(|e| e.play.as_mut()) else {
            return;
        };
        play.snapshot = Some(snapshot.clone());
        let others: Vec<i64> = play
            .players
            .values()
            .filter(|p| p.cid != from)
            .map(|p| p.cid)
            .collect();
        for cid in others {
            s.push(cid, Notice::Started(Box::new(snapshot.clone())));
        }
    }
    /// The host's sector state after a sector move (for later joiners).
    pub fn update_snapshot(&self, party_id: i64, snapshot: Snapshot) {
        if let Some(play) = self
            .lock()
            .parties
            .get_mut(&party_id)
            .and_then(|e| e.play.as_mut())
        {
            play.snapshot = Some(snapshot);
        }
    }
    /// The play's snapshot.
    pub fn snapshot_of(&self, party_id: i64) -> Option<Snapshot> {
        self.lock()
            .parties
            .get(&party_id)
            .and_then(|e| e.play.as_ref())
            .and_then(|p| p.snapshot.clone())
    }
    /// GiveItem of an erg to the player in `slot`.
    pub fn give(&self, party_id: i64, slot: i32, class_ex: &str, count: i32) -> bool {
        let s = self.lock();
        let Some(player) = s
            .parties
            .get(&party_id)
            .and_then(|e| e.play.as_ref())
            .and_then(|p| p.players.get(&slot).copied())
        else {
            return false;
        };
        s.push(
            player.cid,
            Notice::Give {
                class_ex: class_ex.to_owned(),
                count,
            },
        );
        true
    }
    /// MicroPlayService.ShipList: the listed ships in party order.
    pub fn open_ships(&self) -> Vec<Ship> {
        let s = self.lock();
        let mut ids: Vec<&i64> = s.parties.keys().collect();
        ids.sort_unstable();
        ids.into_iter()
            .filter_map(|id| s.parties.get(id).and_then(Entry::listed))
            .collect()
    }
    /// JoinParty(NORMAL) of a character in Initial into a ship (JoinShip from the boat's
    /// list): a ship admits without an invitation. Its PartyInfo for the joiner.
    pub fn join_ship_party(
        &self,
        party_id: i64,
        member: Member,
    ) -> Result<Result<Vec<OutgoingPacket>, JoinFail>, MinimumError> {
        let mut s = self.lock();
        let cid = member.cid;
        match s.seats.get_mut(&cid) {
            Some(seat) if seat.party.is_none() => seat.member = Some(member.clone()),
            _ => return Ok(Err(JoinFail::InternalError)),
        }
        let joined = match s.parties.get_mut(&party_id) {
            Some(entry) => entry.party.join(member),
            None => Err(JoinFail::NotInvited),
        };
        if let Err(e) = joined {
            return Ok(Err(e));
        }
        if let Some(seat) = s.seats.get_mut(&cid) {
            seat.party = Some(party_id);
            seat.invited_party = None;
        }
        s.broadcast(party_id, cid).map(Ok)
    }
    /// PartyChat: every member gets PartyChatMessage with the sender's name; the packets
    /// of the sender.
    pub fn chat(&self, cid: i64, text: &str) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let s = self.lock();
        let Some(party_id) = s.seats.get(&cid).and_then(|seat| seat.party) else {
            return Ok(Vec::new());
        };
        let Some(entry) = s.parties.get(&party_id) else {
            return Ok(Vec::new());
        };
        let Some(sender) = entry.party.member(cid) else {
            return Ok(Vec::new());
        };
        let message = packet(&Message::PartyChat(wire::PartyChat {
            sender_name: Some(sender.name.clone()),
            message: Some(text.to_owned()),
            is_fake_town_chat: false,
        }))?;
        for m in entry.party.members().filter(|m| m.cid != cid) {
            s.push(m.cid, Notice::Packets(vec![message.clone()]));
        }
        Ok(vec![message])
    }
    /// The party's member count.
    pub fn member_count(&self, party_id: i64) -> usize {
        self.lock()
            .parties
            .get(&party_id)
            .map_or(0, |e| e.party.member_count())
    }
    /// Party.IsAllReady.
    pub fn all_ready(&self, party_id: i64) -> bool {
        self.lock()
            .parties
            .get(&party_id)
            .is_some_and(|e| e.party.is_all_ready())
    }
    /// The members' CIDs in slot order.
    pub fn members(&self, party_id: i64) -> Vec<i64> {
        self.lock()
            .parties
            .get(&party_id)
            .map(|e| e.party.members().map(|m| m.cid).collect())
            .unwrap_or_default()
    }
    /// SendPacket to one entered character (MemberInfo to the host, ...).
    pub fn send(&self, cid: i64, packets: Vec<OutgoingPacket>) {
        self.lock().push(cid, Notice::Packets(packets));
    }
    /// The party ends for `cid` without a Kick of its own (its frontend already sent the
    /// return); the remaining members follow RemoveMember.
    pub fn end_for(&self, cid: i64) -> Vec<OutgoingPacket> {
        let mut s = self.lock();
        if let Some(party_id) = s.seats.get(&cid).and_then(|seat| seat.party)
            && s.remove(party_id, cid, cid, self.force_close_in_ship)
                .is_err()
        {
            tracing::warn!("party removal after the mission failed; values omitted");
        }
        // A player who left with the host joins the party reserved for them
        // (ReturnFromQuest_Peer_NewParty / Host_NewParty_ToTown).
        let Some(new_party) = s.reserved.remove(&cid) else {
            return Vec::new();
        };
        let Some(member) = s.seats.get(&cid).and_then(|seat| seat.member.clone()) else {
            return Vec::new();
        };
        let entry = s.parties.entry(new_party).or_insert_with(|| Entry {
            party: Party::new(new_party),
            ship: None,
            play: None,
            town_ship_listed: false,
        });
        entry.party.add_invitation(cid);
        if entry.party.join(member).is_err() {
            return Vec::new();
        }
        if let Some(seat) = s.seats.get_mut(&cid) {
            seat.party = Some(new_party);
        }
        s.broadcast(new_party, cid).unwrap_or_default()
    }
    /// The party's play, if any player leaves with the host, is deciding.
    pub fn leaving_with_host(&self, party_id: i64, cid: i64) -> bool {
        self.lock()
            .parties
            .get(&party_id)
            .and_then(|e| e.play.as_ref())
            .and_then(|p| p.leave_with_host.as_ref())
            .is_some_and(|m| m.contains_key(&cid))
    }
    /// MicroPlay.StartLeaveWithHostDecision: the host waits; every other player is asked
    /// whether to follow it (20 s). The host's packets; None with fewer than two players.
    pub fn start_leave_with_host(
        &self,
        party_id: i64,
        host: i64,
    ) -> Result<Option<Vec<OutgoingPacket>>, MinimumError> {
        let mut s = self.lock();
        let Some(play) = s.parties.get_mut(&party_id).and_then(|e| e.play.as_mut()) else {
            return Ok(None);
        };
        if play.players.len() < 2 || play.leave_with_host.is_some() {
            return Ok(None);
        }
        let mut decisions = BTreeMap::new();
        let mut asked = Vec::new();
        for p in play.players.values() {
            if p.cid == host {
                decisions.insert(p.cid, Some(true));
            } else {
                decisions.insert(p.cid, None);
                asked.push(p.cid);
            }
        }
        play.leave_with_host = Some(decisions);
        let ask = ask_finish(false, "GameUI_Heroes_Peer_LeaveWithHost")?;
        for cid in asked {
            s.push(cid, Notice::Packets(vec![ask.clone()]));
        }
        Ok(Some(vec![ask_finish(
            true,
            "GameUI_Heroes_Host_WaitOtherPlayers",
        )?]))
    }
    /// AnswerFinishQuest → DecideLeaveWithHost; the packets of `cid`.
    pub fn decide_leave_with_host(
        &self,
        party_id: i64,
        cid: i64,
        follow: bool,
    ) -> Result<Vec<OutgoingPacket>, MinimumError> {
        let complete = {
            let mut s = self.lock();
            let Some(decisions) = s
                .parties
                .get_mut(&party_id)
                .and_then(|e| e.play.as_mut())
                .and_then(|p| p.leave_with_host.as_mut())
            else {
                return Ok(Vec::new());
            };
            if decisions.get(&cid) != Some(&None) {
                return Ok(Vec::new());
            }
            decisions.insert(cid, Some(follow));
            decisions.values().all(Option::is_some)
        };
        if complete {
            self.trigger_leave_with_host(party_id);
            return Ok(Vec::new());
        }
        if follow {
            return Ok(vec![ask_finish(
                true,
                "GameUI_Heroes_Peer_WaitOtherPlayers",
            )?]);
        }
        Ok(Vec::new())
    }
    /// Trigger_LeaveWithHost (all decided or the 20 s timeout): the undecided follow;
    /// every follower gets its reward; with more than one follower and someone staying,
    /// the followers get a reserved party in town. Returns the followers.
    pub fn trigger_leave_with_host(&self, party_id: i64) -> Vec<i64> {
        let mut s = self.lock();
        let Some(play) = s.parties.get_mut(&party_id).and_then(|e| e.play.as_mut()) else {
            return Vec::new();
        };
        let Some(decisions) = play.leave_with_host.as_mut() else {
            return Vec::new();
        };
        // An empty map: already triggered.
        if decisions.is_empty() {
            return Vec::new();
        }
        for d in decisions.values_mut() {
            if d.is_none() {
                *d = Some(true);
            }
        }
        let followers: Vec<i64> = decisions
            .iter()
            .filter(|(_, d)| **d == Some(true))
            .map(|(c, _)| *c)
            .collect();
        let stayers = decisions.values().any(|d| *d == Some(false));
        decisions.clear();
        play.followers = followers.clone();
        if followers.len() > 1 && stayers {
            let new_party = super::minimum_mission::entity_id().unwrap_or(party_id ^ 1);
            for cid in &followers {
                s.reserved.insert(*cid, new_party);
            }
        }
        for cid in &followers {
            s.push(*cid, Notice::FollowHost);
        }
        followers
    }
    /// FinishMicroPlayHost (60 s after the trigger, or 10 s after the host's return):
    /// the given players (the followers when empty) still in the play return with the host.
    pub fn return_with_host(&self, party_id: i64, players: &[i64]) {
        let s = self.lock();
        let Some(play) = s.parties.get(&party_id).and_then(|e| e.play.as_ref()) else {
            return;
        };
        let players = if players.is_empty() {
            play.followers.as_slice()
        } else {
            players
        };
        for p in play.players.values().filter(|p| players.contains(&p.cid)) {
            s.push(p.cid, Notice::ReturnNow);
        }
    }
    /// The other players of the play (for the host's return countdown).
    pub fn others(&self, party_id: i64, cid: i64) -> Vec<i64> {
        self.players(party_id)
            .into_iter()
            .map(|(_, p)| p.cid)
            .filter(|c| *c != cid)
            .collect()
    }
    /// The play waits without a host (all players disconnected under CombatRejoinSystem).
    pub fn hostless(&self, party_id: i64) -> bool {
        self.lock()
            .parties
            .get(&party_id)
            .and_then(|e| e.play.as_ref())
            .is_some_and(|p| p.host == 0)
    }
    /// A new P2P group for a play that waited without players.
    pub fn set_group(&self, party_id: i64, group: i32) {
        if let Some(play) = self
            .lock()
            .parties
            .get_mut(&party_id)
            .and_then(|e| e.play.as_mut())
        {
            play.group = group;
        }
    }
    /// AddPlayer's host candidate (isRejoin without a GameHost): StartHostElection with
    /// the rejoined player, who becomes master and host (ChangeHost).
    pub fn adopt_host(&self, party_id: i64, cid: i64) -> Result<(), MinimumError> {
        let mut s = self.lock();
        let Some(entry) = s.parties.get_mut(&party_id) else {
            return Ok(());
        };
        entry.party.promote(cid);
        let Some(play) = entry.play.as_mut() else {
            return Ok(());
        };
        if let Some((&slot, &player)) = play.players.iter().find(|(_, p)| p.cid == cid) {
            play.players.remove(&slot);
            play.players.insert(0, player);
        }
        play.host = cid;
        play.restarting = true;
        play.host_info = None;
        let snapshot = play.snapshot.clone();
        s.push(
            cid,
            Notice::Packets(vec![packet(&Message::HostRestarting)?]),
        );
        if let Some(snapshot) = snapshot {
            s.push(cid, Notice::BecomeHost(Box::new(snapshot)));
        }
        s.broadcast(party_id, -1)?;
        Ok(())
    }
    /// RemovePartyScheduleID: after CombatRejoinSystem_LimitMinutes (+30 s) a play still
    /// waiting for `cid` forgets it, and an empty party is removed.
    pub fn expire_rejoin(&self, party_id: i64, cid: i64) {
        let remembered = REJOIN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&cid)
            .is_some_and(|(p, _)| *p == party_id);
        if !remembered {
            return;
        }
        self.forget_disconnect(cid);
        let mut s = self.lock();
        if s.parties
            .get(&party_id)
            .is_some_and(|e| e.party.member_count() == 0)
        {
            s.dissolve(party_id, -1);
        }
    }
    /// Read-only counts for the admin panel: parties and their member counts.
    pub fn snapshot(&self) -> Vec<(i64, usize, u8)> {
        let s = self.lock();
        let mut out: Vec<(i64, usize, u8)> = s
            .parties
            .values()
            .map(|e| (e.party.id, e.party.member_count(), e.party.state()))
            .collect();
        out.sort_unstable();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::party::ready_state;

    fn member(cid: i64) -> Member {
        Member {
            cid,
            nexon_sn: cid as i32,
            character: 1,
            name: format!("Name{cid}"),
            level: 10,
            slot: -1,
            state: ready_state::IN_TOWN,
            latest_ping: 0,
            latest_frame_rate: 0,
            is_return: false,
            is_event_jumping: false,
        }
    }
    fn seat(hub: &PartyHub, cid: i64) -> mpsc::Receiver<Notice> {
        let (tx, rx) = mpsc::channel(MAILBOX);
        hub.register(cid, &format!("Name{cid}"), tx);
        rx
    }
    fn kinds(packets: &[OutgoingPacket]) -> Vec<i32> {
        packets.iter().map(|p| p.category).collect()
    }
    const PARTY_INFO: i32 = 511;
    const INVITED: i32 = 516;
    const REJECTED: i32 = 514;

    #[test]
    fn invitation_accept_broadcasts_to_both_and_leave_dissolves() {
        let hub = PartyHub::new(false, true);
        let _a = seat(&hub, 1);
        let mut b = seat(&hub, 2);
        assert_eq!(hub.entered("name2"), Some(2));
        let own = hub.open(100, member(1)).unwrap();
        assert_eq!(kinds(&own), [PARTY_INFO]);
        assert_eq!(
            hub.role(1),
            Role::Master {
                party_id: 100,
                state: 0
            }
        );
        assert_eq!(
            hub.invite(100, 1, 2, "Name2").unwrap(),
            Invite::Sent(Vec::new())
        );
        let Ok(Notice::Packets(p)) = b.try_recv() else {
            panic!()
        };
        assert_eq!(kinds(&p), [INVITED]);
        assert_eq!(hub.accept(999, member(2)).unwrap(), None);
        let own = hub.accept(100, member(2)).unwrap().unwrap().unwrap();
        assert_eq!(kinds(&own), [PARTY_INFO]);
        assert!(matches!(
            hub.role(2),
            Role::Member {
                party_id: 100,
                slot: 1,
                ..
            }
        ));
        assert!(hub.leave(1).unwrap());
        // The master left a two-member town party: the other member is alone → removed.
        assert!(matches!(b.try_recv(), Ok(Notice::Kicked { party_id: 100 })));
        assert_eq!(hub.role(2), Role::None);
        assert!(hub.snapshot().is_empty());
    }

    #[test]
    fn an_offline_or_rejecting_target_reports_to_the_master_and_removes_the_lone_party() {
        let hub = PartyHub::new(false, true);
        let mut a = seat(&hub, 1);
        let _b = seat(&hub, 2);
        hub.open(100, member(1)).unwrap();
        let Invite::Failed(p) = hub.invite(100, 1, 3, "Name3").unwrap() else {
            panic!()
        };
        assert_eq!(kinds(&p), [REJECTED]);
        assert!(hub.snapshot().is_empty());
        hub.open(101, member(1)).unwrap();
        hub.invite(101, 1, 2, "Name2").unwrap();
        hub.reject(2, 101).unwrap();
        let Ok(Notice::Packets(p)) = a.try_recv() else {
            panic!()
        };
        assert_eq!(kinds(&p), [REJECTED]);
        assert!(matches!(a.try_recv(), Ok(Notice::Kicked { party_id: 101 })));
    }

    #[test]
    fn a_joining_player_gets_coins_of_every_slot_and_the_host_its_own_stones() {
        let hub = PartyHub::new(false, true);
        let mut a = seat(&hub, 1);
        let mut b = seat(&hub, 2);
        hub.open(100, member(1)).unwrap();
        hub.invite(100, 1, 2, "Name2").unwrap();
        hub.accept(100, member(2)).unwrap().unwrap().unwrap();
        while a.try_recv().is_ok() {}
        while b.try_recv().is_ok() {}
        assert_eq!(
            hub.add_player(100, 0, Player { cid: 1, level: 10 })
                .unwrap(),
            None
        );
        hub.start_micro_play(100);
        hub.change_state(100, party_state::IN_SHIP);
        hub.set_play(
            100,
            Play {
                quest: "q".into(),
                title: None,
                option: crate::protocol::mission::ShipOption {
                    max_member_count: 4,
                    swear_member_limit: -1,
                    until_force_start: 1,
                    min_level: 1,
                    max_level: 99,
                    is_party_only: 0,
                    over18_only: 0,
                    difficulty: 0,
                    is_season2: false,
                    selected_boss_quest_ids: None,
                    is_practice_mode: false,
                    user_ds_mode: false,
                },
                swear_id: -1,
                group: 9,
                host: 1,
                host_info: None,
                players: BTreeMap::new(),
                snapshot: None,
                restarting: false,
                leave_with_host: None,
                followers: Vec::new(),
            },
        );
        let player = |cid| Player { cid, level: 10 };
        let own = hub.add_player(100, 0, player(1)).unwrap().unwrap();
        assert_eq!(kinds(&own), [432, 48]);
        let own = hub.add_player(100, 1, player(2)).unwrap().unwrap();
        assert_eq!(kinds(&own), [432, 48]);
        // The host hears the new coin list and its own stone slot again.
        let Ok(Notice::Packets(p)) = a.try_recv() else {
            panic!()
        };
        assert_eq!(kinds(&p), [432, 48]);
        assert_eq!(hub.add_player(100, 1, player(3)).unwrap(), None);
        // Host messages reach the other players' replicas; ergs reach their winner.
        hub.replicate(100, 1, &Message::AllUserJoinComplete);
        assert!(matches!(b.try_recv(), Ok(Notice::Play(_))));
        assert!(a.try_recv().is_err());
        assert!(hub.give(100, 1, "gold", 5));
        assert!(matches!(b.try_recv(), Ok(Notice::Give { count: 5, .. })));
        assert_eq!(hub.slot_of(100, 2), Some(1));
        // Without a started play nobody is elected; with one, the other player becomes
        // the master in slot 0 and the game host.
        assert_eq!(hub.elect(100, 2).unwrap(), None);
        hub.update_snapshot(
            100,
            Snapshot {
                sectors: crate::domain::mission::SectorInfos {
                    current_group: 0,
                    starting_group: 0,
                    sectors: Vec::new(),
                },
                sequence: crate::domain::mission::SectorSequence {
                    starting_group: 0,
                    sectors: Vec::new(),
                    current: None,
                    last_used_spawn_point: None,
                    latest_cleared: 0,
                    passed: Vec::new(),
                    cores: Vec::new(),
                },
                game_time: 0,
                in_game: true,
            },
        );
        while a.try_recv().is_ok() {}
        while b.try_recv().is_ok() {}
        assert_eq!(hub.elect(100, 1).unwrap(), Some(2));
        assert!(matches!(hub.role(2), Role::Master { .. }));
        assert_eq!(hub.slot_of(100, 2), Some(0));
        assert!(matches!(b.try_recv(), Ok(Notice::Packets(_))));
        assert!(matches!(b.try_recv(), Ok(Notice::BecomeHost(_))));
        assert!(!hub.host_restarted(100, 1, None));
        assert!(hub.host_restarted(100, 2, None));
        hub.election_timeout(100, 2);
        assert!(b.try_recv().is_err());
        // The old host's frontend then leaves the party; it can rejoin the play.
        hub.remember_disconnect(1, 100, 1);
        hub.unregister(1);
        assert!(rejoinable(1));
        let _a = seat(&hub, 1);
        assert!(hub.rejoin(member(1)).unwrap().is_some());
        assert!(matches!(hub.role(1), Role::Member { ready: 6, .. }));
        hub.forget_disconnect(1);
        assert!(!rejoinable(1));
        // A member leaving the party leaves the play.
        // The new master leaves the ship of a party in InShip: back to town, play gone.
        assert!(hub.leave(2).unwrap());
        assert!(hub.play(100).is_none());
    }

    #[test]
    fn party_chat_reaches_every_member_with_the_sender_name() {
        let hub = PartyHub::new(false, true);
        let _a = seat(&hub, 1);
        let mut b = seat(&hub, 2);
        assert!(hub.chat(1, "oi").unwrap().is_empty());
        hub.open(100, member(1)).unwrap();
        hub.invite(100, 1, 2, "Name2").unwrap();
        hub.accept(100, member(2)).unwrap().unwrap().unwrap();
        while b.try_recv().is_ok() {}
        assert_eq!(kinds(&hub.chat(1, "oi").unwrap()), [509]);
        let Ok(Notice::Packets(p)) = b.try_recv() else {
            panic!()
        };
        assert_eq!(kinds(&p), [509]);
        // A town party without a ship is not listed; a stranger cannot join it.
        assert!(hub.open_ships().is_empty());
        let _c = seat(&hub, 3);
        assert_eq!(
            hub.join_ship_party(100, member(3)).unwrap(),
            Err(JoinFail::NotInvited)
        );
    }

    fn play(host: i64) -> Play {
        Play {
            quest: "q".into(),
            title: None,
            option: crate::protocol::mission::ShipOption {
                max_member_count: 4,
                swear_member_limit: -1,
                until_force_start: 1,
                min_level: 1,
                max_level: 99,
                is_party_only: 0,
                over18_only: 0,
                difficulty: 0,
                is_season2: false,
                selected_boss_quest_ids: None,
                is_practice_mode: false,
                user_ds_mode: false,
            },
            swear_id: -1,
            group: 9,
            host,
            host_info: None,
            players: BTreeMap::new(),
            snapshot: Some(Snapshot {
                sectors: crate::domain::mission::SectorInfos {
                    current_group: 0,
                    starting_group: 0,
                    sectors: Vec::new(),
                },
                sequence: crate::domain::mission::SectorSequence {
                    starting_group: 0,
                    sectors: Vec::new(),
                    current: None,
                    last_used_spawn_point: None,
                    latest_cleared: 0,
                    passed: Vec::new(),
                    cores: Vec::new(),
                },
                game_time: 0,
                in_game: true,
            }),
            restarting: false,
            leave_with_host: None,
            followers: Vec::new(),
        }
    }

    #[test]
    fn players_decide_to_follow_the_leaving_host_and_followers_get_a_party() {
        let hub = PartyHub::new(false, true);
        let mut rx: Vec<_> = (1..=3).map(|c| seat(&hub, c)).collect();
        hub.open(200, member(1)).unwrap();
        for cid in [2, 3] {
            hub.invite(200, 1, cid, &format!("Name{cid}")).unwrap();
            hub.accept(200, member(cid)).unwrap().unwrap().unwrap();
        }
        hub.start_micro_play(200);
        hub.change_state(200, party_state::IN_GAME);
        hub.set_play(200, play(1));
        for (slot, cid) in [(0, 1), (1, 2), (2, 3)] {
            hub.add_player(200, slot, Player { cid, level: 10 })
                .unwrap();
        }
        for r in rx.iter_mut() {
            while r.try_recv().is_ok() {}
        }
        let own = hub.start_leave_with_host(200, 1).unwrap().unwrap();
        assert_eq!(kinds(&own), [127]);
        assert!(hub.leaving_with_host(200, 2));
        assert!(matches!(rx[1].try_recv(), Ok(Notice::Packets(_))));
        // B follows (waits), C stays: everyone decided → trigger.
        assert_eq!(
            kinds(&hub.decide_leave_with_host(200, 2, true).unwrap()),
            [127]
        );
        assert!(
            hub.decide_leave_with_host(200, 3, false)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(rx[0].try_recv(), Ok(Notice::FollowHost)));
        assert!(matches!(rx[1].try_recv(), Ok(Notice::FollowHost)));
        assert!(hub.trigger_leave_with_host(200).is_empty());
        // Two followers and a stayer: the followers meet again in a reserved party.
        hub.end_for(1);
        let joined = hub.end_for(2);
        assert_eq!(kinds(&joined), [PARTY_INFO]);
        assert!(matches!(hub.role(2), Role::Member { .. }));
        hub.return_with_host(200, &[]);
    }

    #[test]
    fn a_play_left_empty_waits_for_its_host_to_rejoin_and_take_over() {
        let hub = PartyHub::new(false, true);
        let mut a = seat(&hub, 11);
        hub.open(300, member(11)).unwrap();
        hub.start_micro_play(300);
        hub.change_state(300, party_state::IN_GAME);
        hub.set_play(300, play(11));
        hub.add_player(300, 0, Player { cid: 11, level: 10 })
            .unwrap();
        assert_eq!(hub.elect(300, 11).unwrap(), None);
        hub.remember_disconnect(11, 300, 0);
        hub.unregister(11);
        assert!(hub.hostless(300));
        let _a2 = seat(&hub, 11);
        assert!(hub.rejoin(member(11)).unwrap().is_some());
        assert!(matches!(
            hub.role(11),
            Role::Member {
                slot: 0,
                ready: 6,
                ..
            }
        ));
        hub.add_player(300, 0, Player { cid: 11, level: 10 })
            .unwrap();
        hub.adopt_host(300, 11).unwrap();
        assert!(matches!(hub.role(11), Role::Master { .. }));
        assert!(!hub.hostless(300));
        drop(a.try_recv());
        hub.expire_rejoin(300, 11);
    }

    #[test]
    fn kick_and_disconnect_notify_the_remaining_members() {
        let hub = PartyHub::new(false, true);
        let _a = seat(&hub, 1);
        let mut b = seat(&hub, 2);
        let mut c = seat(&hub, 3);
        hub.open(100, member(1)).unwrap();
        for cid in [2, 3] {
            hub.invite(100, 1, cid, &format!("Name{cid}")).unwrap();
            hub.accept(100, member(cid)).unwrap().unwrap().unwrap();
        }
        while b.try_recv().is_ok() {}
        while c.try_recv().is_ok() {}
        let own = hub.kick(1, 1, 2).unwrap();
        assert_eq!(kinds(&own), [PARTY_INFO]);
        assert!(matches!(b.try_recv(), Ok(Notice::Kicked { party_id: 100 })));
        assert!(matches!(c.try_recv(), Ok(Notice::Packets(_))));
        hub.unregister(1);
        // The master's frontend closed: member 3 is alone in town → removed.
        assert!(matches!(c.try_recv(), Ok(Notice::Kicked { party_id: 100 })));
    }
}
