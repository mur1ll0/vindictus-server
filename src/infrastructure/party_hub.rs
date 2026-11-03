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
    self as wire, Message, PartyInfo, PartyInvitationRejected, PartyInvited, Ship, UpdateShip,
};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use tokio::sync::mpsc;

/// Mailbox bound per entered character; a full mailbox drops the push (the member is
/// stalled or gone and its connection closes the seat).
pub const MAILBOX: usize = 256;

/// What a member's connection receives from the party authority.
#[derive(Debug)]
pub enum Notice {
    /// SendPacket / UpdateParty / SendInvitation pushes, in order.
    Packets(Vec<OutgoingPacket>),
    /// The Kick operation: the party removed this member (kick or removal of the party).
    Kicked { party_id: i64 },
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

struct Seat {
    name: String,
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
}
#[derive(Default)]
struct State {
    seats: HashMap<i64, Seat>,
    parties: HashMap<i64, Entry>,
}

pub struct PartyHub {
    state: Mutex<State>,
    /// FeatureMatrix ForceClosePartyInShip.
    force_close_in_ship: bool,
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
            members: Some(members.clone()),
        }))?];
        if party.has_ship()
            && let Some(ship) = &entry.ship
        {
            let master = party.master();
            let mut ship = ship.clone();
            ship.members = Some(members);
            ship.member_count = party.member_count() as i32;
            ship.state = party.state();
            ship.max_ship_member_count = party.party_size();
            ship.host_ping = master.map_or(-1, |m| m.latest_ping);
            ship.host_frame_rate = master.map_or(255, |m| m.latest_frame_rate);
            ship.is_return = master.is_some_and(|m| m.is_return);
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
        let Some(entry) = self.parties.get_mut(&party_id) else {
            return Ok(Vec::new());
        };
        let removal = entry.party.remove(cid, force_close_in_ship);
        if let Some(seat) = self.seats.get_mut(&cid)
            && seat.party == Some(party_id)
        {
            seat.party = None;
        }
        match removal {
            None => Ok(Vec::new()),
            Some(Removal::Dissolved) => {
                self.dissolve(party_id, caller);
                Ok(Vec::new())
            }
            Some(Removal::Continues) => {
                if let Some(entry) = self.parties.get_mut(&party_id)
                    && !entry.party.has_ship()
                {
                    entry.ship = None;
                }
                self.broadcast(party_id, caller)
            }
        }
    }
}

impl PartyHub {
    pub fn new(force_close_in_ship: bool) -> Self {
        Self {
            state: Mutex::new(State::default()),
            force_close_in_ship,
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
                mailbox,
                party: None,
                invited_party: None,
            },
        );
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
            Some(m) if m.slot == 0 => Role::Master { party_id, state },
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
        let mut party = Party::new(party_id);
        party.join(master).map_err(|_| fail())?;
        s.parties.insert(party_id, Entry { party, ship: None });
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
        let seat = s.seats.get(&cid).ok_or_else(fail)?;
        if seat.invited_party != Some(party_id) || seat.party.is_some() {
            return Ok(None);
        }
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
    ) -> Result<(), MinimumError> {
        let mut s = self.lock();
        let entry = s.parties.get_mut(&party_id).ok_or_else(fail)?;
        if !entry.party.set_ship(limits, story_quest) {
            return Err(fail());
        }
        entry.ship = Some(ship);
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
    pub fn end_for(&self, cid: i64) {
        let mut s = self.lock();
        let Some(party_id) = s.seats.get(&cid).and_then(|seat| seat.party) else {
            return;
        };
        if s.remove(party_id, cid, cid, self.force_close_in_ship)
            .is_err()
        {
            tracing::warn!("party removal after the mission failed; values omitted");
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
        let hub = PartyHub::new(false);
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
        let hub = PartyHub::new(false);
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
    fn kick_and_disconnect_notify_the_remaining_members() {
        let hub = PartyHub::new(false);
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
