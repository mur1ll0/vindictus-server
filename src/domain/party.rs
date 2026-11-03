//! Party authority recovered from MicroPlayServiceCore._Party_ (Party, PartyMember and the
//! party processors): slots, invitations, the master in slot 0, reelection, transfer, kick
//! and the members' ready states. Pure state; `infrastructure::party_hub` delivers what it
//! decides to the members' frontends.
use std::collections::BTreeSet;

/// ReadyState (byte enum).
pub mod ready_state {
    pub const IN_TOWN: u8 = 0;
    pub const IN_SHIP: u8 = 1;
    pub const READY: u8 = 2;
    pub const HOST: u8 = 3;
    pub const ASSIST_WAIT: u8 = 4;
    pub const ASSIST_ACCEPTED: u8 = 5;
    pub const REJOIN: u8 = 6;
}
/// PartyInfoState (byte enum).
pub mod party_state {
    pub const IN_TOWN: u8 = 0;
    pub const IN_TOWN_WITH_SHIP_INFO: u8 = 1;
    pub const ENTERING_SHIP: u8 = 2;
    pub const IN_SHIP: u8 = 3;
    pub const STARTING: u8 = 4;
    pub const IN_GAME: u8 = 5;
}

/// InvitationRejectReason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum RejectReason {
    NoSuchCharacter = 0,
    ClientNotLoggedOn = 1,
    AlreadyInvited = 2,
    YouAreNotPartyMaster = 3,
    AlreadyInParty = 4,
    Rejected = 5,
    PartyIsFull = 6,
    GameStarted = 7,
    Captcha = 8,
    Punished = 9,
    NotAvailable = 10,
    PleaseWait = 11,
    SystemError = 12,
}

/// JoinParty.FailReason of the NORMAL join this server migrates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinFail {
    NotInvited,
    Loading,
    ShipAlreadyLaunched,
    LevelNotMatch,
    PartyIsFull,
    InternalError,
}

/// SetMemberState.FailReasonEnum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateFail {
    InvalidSlot,
    HostCannotChangeState,
    CannotReadyInTown,
}

/// PartyMember: the CharacterSummary fields of PartyMemberInfo plus the slot and state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub cid: i64,
    pub nexon_sn: i32,
    pub character: u8,
    pub name: String,
    pub level: i32,
    pub slot: i32,
    pub state: u8,
    pub latest_ping: i32,
    pub latest_frame_rate: i32,
    pub is_return: bool,
    pub is_event_jumping: bool,
}
impl Member {
    /// IsReady: the host or a Ready member.
    pub fn is_ready(&self) -> bool {
        self.state == ready_state::HOST || self.state == ready_state::READY
    }
}

/// What a removal left: the party goes on (its members get PartyInfo) or it is removed
/// (MicroPlayService.RemoveParty → Close → BroadCastKicked).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    Continues,
    Dissolved,
}

/// Party: MemberArray (slot 0 is the master), Invited and the PartyInfoState. A ship
/// (ShipInfo) fixes the party size at MaxShipMemberCount and the level constraint.
#[derive(Clone, Debug)]
pub struct Party {
    pub id: i64,
    slots: Vec<Option<Member>>,
    invited: BTreeSet<i64>,
    state: u8,
    /// ShipInfo.MaxShipMemberCount and ShipOptionInfo Min/MaxLevel while HasShipInfo.
    ship: Option<ShipLimits>,
    /// MicroPlay != null (from StartMicroPlay2).
    micro_play: bool,
    /// The ship's quest requires a story line (QuestInfo.RequiredStoryLineID).
    story_quest: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShipLimits {
    pub max_members: i32,
    pub min_level: i32,
    pub max_level: i32,
}

impl Party {
    /// Party constructor: four empty slots in town.
    pub fn new(id: i64) -> Self {
        Self {
            id,
            slots: vec![None; 4],
            invited: BTreeSet::new(),
            state: party_state::IN_TOWN,
            ship: None,
            micro_play: false,
            story_quest: false,
        }
    }
    pub fn state(&self) -> u8 {
        self.state
    }
    pub fn has_ship(&self) -> bool {
        self.ship.is_some()
    }
    pub fn has_micro_play(&self) -> bool {
        self.micro_play
    }
    /// Master: MemberArray[0].
    pub fn master(&self) -> Option<&Member> {
        self.slots.first().and_then(Option::as_ref)
    }
    pub fn member_count(&self) -> usize {
        self.slots.iter().flatten().count()
    }
    /// PartySize: ShipInfo.MaxShipMemberCount, else the slot array length.
    pub fn party_size(&self) -> i32 {
        self.ship.map_or(self.slots.len() as i32, |s| s.max_members)
    }
    /// Members in slot order (GetPartyMemberInfoList).
    pub fn members(&self) -> impl Iterator<Item = &Member> {
        self.slots.iter().flatten()
    }
    pub fn member(&self, cid: i64) -> Option<&Member> {
        self.members().find(|m| m.cid == cid)
    }
    fn member_mut(&mut self, cid: i64) -> Option<&mut Member> {
        self.slots.iter_mut().flatten().find(|m| m.cid == cid)
    }
    pub fn by_slot(&self, slot: i32) -> Option<&Member> {
        usize::try_from(slot)
            .ok()
            .and_then(|s| self.slots.get(s))
            .and_then(Option::as_ref)
    }
    pub fn is_invited(&self, cid: i64) -> bool {
        self.invited.contains(&cid)
    }
    pub fn is_all_ready(&self) -> bool {
        self.members().all(Member::is_ready)
    }
    /// SetPartySize: refused below the member count; members keep their slots.
    fn set_party_size(&mut self, max: usize) -> bool {
        if self.slots.len() == max {
            return true;
        }
        if max < self.member_count()
            || self.slots[max.min(self.slots.len())..]
                .iter()
                .any(Option::is_some)
        {
            return false;
        }
        self.slots.resize(max, None);
        true
    }
    /// ExtendPartySize: 4 → 6 → 8 without a ship.
    fn extend_party_size(&mut self) -> bool {
        if self.ship.is_some() {
            return false;
        }
        match self.slots.len() {
            4 => self.set_party_size(6),
            6 => self.set_party_size(8),
            _ => false,
        }
    }
    fn full(&mut self) -> bool {
        self.member_count() as i32 >= self.party_size() && !self.extend_party_size()
    }
    /// IsToRemove.
    pub fn is_to_remove(&self) -> bool {
        if self.micro_play {
            return self.member_count() == 0 && self.state == party_state::IN_TOWN;
        }
        self.state == party_state::IN_TOWN && self.member_count() <= 1 && self.invited.is_empty()
    }
    /// InvitePartyMemberProcessor checks before QueryFrontendID: only the master invites,
    /// a member or pending invitation is AlreadyInvited and a full party is PartyIsFull.
    pub fn check_invite(&mut self, inviting: i64, invited: i64) -> Result<(), RejectReason> {
        if self.master().map(|m| m.cid) != Some(inviting) {
            return Err(RejectReason::YouAreNotPartyMaster);
        }
        if self.invited.contains(&invited) || self.member(invited).is_some() {
            return Err(RejectReason::AlreadyInvited);
        }
        if self.full() {
            return Err(RejectReason::PartyIsFull);
        }
        Ok(())
    }
    /// Invited.Add after QueryFrontendID.
    pub fn add_invitation(&mut self, invited: i64) {
        self.invited.insert(invited);
    }
    /// Invited.Remove (InvitationRejected, SendInvitation failure).
    pub fn remove_invitation(&mut self, invited: i64) {
        self.invited.remove(&invited);
    }
    /// JoinPartyProcessor for JoinType.NORMAL, then AddMember: the first empty slot, Host
    /// in slot 0 and InTown elsewhere. `level` is the joiner's (ConstraintPassed while the
    /// MicroPlay exists).
    pub fn join(&mut self, mut member: Member) -> Result<i32, JoinFail> {
        match self.state {
            party_state::IN_TOWN => {
                if self.member_count() != 0 && !self.invited.contains(&member.cid) {
                    return Err(JoinFail::NotInvited);
                }
            }
            party_state::IN_TOWN_WITH_SHIP_INFO | party_state::IN_SHIP => {}
            party_state::IN_GAME => {
                // Assist (ASSIST/FORCE_ASSIST) and CombatRejoin are not migrated.
                return Err(JoinFail::ShipAlreadyLaunched);
            }
            _ => return Err(JoinFail::Loading),
        }
        if self.micro_play
            && let Some(s) = self.ship
            && (member.level < s.min_level || s.max_level < member.level)
        {
            return Err(JoinFail::LevelNotMatch);
        }
        if self.full() {
            return Err(JoinFail::PartyIsFull);
        }
        if self.member(member.cid).is_some() {
            return Err(JoinFail::InternalError);
        }
        self.invited.remove(&member.cid);
        let Some(slot) = self.slots.iter().position(Option::is_none) else {
            return Err(JoinFail::InternalError);
        };
        member.slot = slot as i32;
        member.state = if slot == 0 {
            ready_state::HOST
        } else {
            ready_state::IN_TOWN
        };
        self.slots[slot] = Some(member);
        Ok(slot as i32)
    }
    /// JoinParty(REJOIN) under CombatRejoinSystem: the disconnected member returns to its
    /// slot (DisconnectedMemberCID) when free, else to the first free slot, as Rejoin,
    /// also while the party is InGame.
    pub fn rejoin(&mut self, mut member: Member, slot: i32) -> Result<i32, JoinFail> {
        if self.member(member.cid).is_some() {
            return Err(JoinFail::InternalError);
        }
        if self.member_count() as i32 >= self.party_size() {
            return Err(JoinFail::PartyIsFull);
        }
        // Slot 0 only when the master is gone too (the play waits without a host).
        let master_gone = self.slots.first().is_some_and(Option::is_none);
        let free = usize::try_from(slot)
            .ok()
            .filter(|s| (*s > 0 || master_gone) && self.slots.get(*s).is_some_and(Option::is_none))
            .or_else(|| {
                self.slots
                    .iter()
                    .skip(1)
                    .position(Option::is_none)
                    .map(|i| i + 1)
            })
            .ok_or(JoinFail::PartyIsFull)?;
        member.slot = free as i32;
        member.state = ready_state::REJOIN;
        self.slots[free] = Some(member);
        Ok(free as i32)
    }
    /// RemoveMember + ProcessAfterRemoveMember. None when `cid` is not a member.
    /// `force_close_in_ship` is FeatureMatrix ForceClosePartyInShip.
    pub fn remove(&mut self, cid: i64, force_close_in_ship: bool) -> Option<Removal> {
        let slot = self
            .slots
            .iter()
            .position(|m| m.as_ref().is_some_and(|m| m.cid == cid))?;
        let left = self.slots[slot].take()?;
        Some(self.after_remove(&left, force_close_in_ship))
    }
    fn after_remove(&mut self, left: &Member, force_close_in_ship: bool) -> Removal {
        if self.state == party_state::IN_TOWN {
            if self.member_count() == 1 {
                return Removal::Dissolved;
            }
            return if self.master().is_some() || self.reelection() {
                Removal::Continues
            } else {
                Removal::Dissolved
            };
        }
        if self.state == party_state::IN_SHIP
            && left.state == ready_state::HOST
            && force_close_in_ship
        {
            return Removal::Dissolved;
        }
        if left.state == ready_state::HOST {
            if self.story_quest {
                return Removal::Dissolved;
            }
            if !self.reelection() {
                return Removal::Dissolved;
            }
            if self.state < party_state::STARTING {
                self.change_state(party_state::IN_TOWN);
                self.clear_ship();
            }
            return Removal::Continues;
        }
        if self.is_to_remove() && self.ship.is_none() {
            return Removal::Dissolved;
        }
        Removal::Continues
    }
    /// Reelection: the first member moves to slot 0 as Host; the old slot-0 occupant
    /// (none after a master left) becomes Ready in the new master's slot.
    pub fn reelection(&mut self) -> bool {
        let Some(i) = self.slots.iter().position(Option::is_some) else {
            return false;
        };
        if i != 0 {
            self.slots.swap(0, i);
            if let Some(m) = self.slots[0].as_mut() {
                m.state = ready_state::HOST;
                m.slot = 0;
            }
            if let Some(m) = self.slots[i].as_mut() {
                m.state = ready_state::READY;
                m.slot = i as i32;
            }
        }
        true
    }
    /// TransferMasterProcessor + Party.TransferMaster: the master swaps slots with the new
    /// master, who becomes Host; the old master takes the new master's previous state.
    pub fn transfer_master(&mut self, master: i64, new_master: i64) -> bool {
        if self.master().map(|m| m.cid) != Some(master) {
            return false;
        }
        let Some(i) = (1..self.slots.len())
            .find(|&i| self.slots[i].as_ref().is_some_and(|m| m.cid == new_master))
        else {
            return false;
        };
        self.slots.swap(0, i);
        let previous = self.slots[0]
            .as_ref()
            .map_or(ready_state::IN_TOWN, |m| m.state);
        if let Some(m) = self.slots[i].as_mut() {
            m.state = previous;
            m.slot = i as i32;
        }
        if let Some(m) = self.slots[0].as_mut() {
            m.state = ready_state::HOST;
            m.slot = 0;
        }
        true
    }
    /// KickMemberProcessor: the master names the slot and its NexonSN; nobody is kicked
    /// while the party is Starting or InGame. Returns the kicked CID.
    pub fn kick(&self, master: i64, slot: i32, nexon_sn: i32) -> Option<i64> {
        if self.master().map(|m| m.cid) != Some(master) {
            return None;
        }
        let kicked = self.by_slot(slot)?;
        if kicked.nexon_sn != nexon_sn
            || self.state == party_state::STARTING
            || self.state == party_state::IN_GAME
        {
            return None;
        }
        Some(kicked.cid)
    }
    /// QueryHostCID → Reelection(available) for a host candidate: `cid` moves to slot 0
    /// as Host (HostElectionFinished SetMemberState(Host) under CombatRejoinSystem).
    pub fn promote(&mut self, cid: i64) -> bool {
        let Some(i) = self
            .slots
            .iter()
            .position(|m| m.as_ref().is_some_and(|m| m.cid == cid))
        else {
            return false;
        };
        self.slots.swap(0, i);
        if let Some(m) = self.slots[0].as_mut() {
            m.slot = 0;
            m.state = ready_state::HOST;
        }
        if i != 0
            && let Some(m) = self.slots[i].as_mut()
        {
            m.slot = i as i32;
            m.state = ready_state::READY;
        }
        true
    }
    /// SetMemberStateProcessor: true when the state changed (BroadCastPartyInfo).
    pub fn set_member_state(&mut self, slot: i32, state: u8) -> Result<bool, StateFail> {
        let member = usize::try_from(slot)
            .ok()
            .and_then(|s| self.slots.get_mut(s))
            .and_then(Option::as_mut)
            .ok_or(StateFail::InvalidSlot)?;
        if member.state == ready_state::HOST {
            return Err(StateFail::HostCannotChangeState);
        }
        if member.state == ready_state::IN_TOWN && state == ready_state::READY {
            return Err(StateFail::CannotReadyInTown);
        }
        if member.state == state {
            return Ok(false);
        }
        member.state = state;
        Ok(true)
    }
    /// UpdatePing of a member (PartyMemberInfo LatestPing/LatestFrameRate).
    pub fn update_ping(&mut self, cid: i64, ping: i32, frame_rate: i32) {
        if let Some(m) = self.member_mut(cid) {
            m.latest_ping = ping;
            m.latest_frame_rate = frame_rate;
        }
    }
    /// Party.ChangeState: back in town every non-host member is InTown; InGame makes
    /// them InShip.
    pub fn change_state(&mut self, state: u8) {
        self.state = state;
        let reset = match state {
            party_state::IN_TOWN => ready_state::IN_TOWN,
            party_state::IN_GAME => ready_state::IN_SHIP,
            _ => return,
        };
        for m in self.slots.iter_mut().flatten() {
            if m.state != ready_state::HOST {
                m.state = reset;
            }
        }
    }
    /// SetShipInfo: the party size becomes the ship's member limit (SetPartySize).
    pub fn set_ship(&mut self, limits: ShipLimits, story_quest: bool) -> bool {
        let Ok(size) = usize::try_from(limits.max_members) else {
            return false;
        };
        if size == 0 || !self.set_party_size(size) {
            return false;
        }
        self.ship = Some(limits);
        self.story_quest = story_quest;
        true
    }
    /// ClearShipInfo.
    pub fn clear_ship(&mut self) {
        self.ship = None;
        self.story_quest = false;
        self.micro_play = false;
    }
    /// StartMicroPlay2: the MicroPlay of the party exists.
    pub fn start_micro_play(&mut self) {
        self.micro_play = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(cid: i64) -> Member {
        Member {
            cid,
            nexon_sn: cid as i32,
            character: 1,
            name: format!("c{cid}"),
            level: 10,
            slot: -1,
            state: ready_state::IN_TOWN,
            latest_ping: 0,
            latest_frame_rate: 0,
            is_return: false,
            is_event_jumping: false,
        }
    }

    #[test]
    fn master_invites_and_the_invited_joins_the_next_slot() {
        let mut p = Party::new(7);
        assert_eq!(p.join(member(1)), Ok(0));
        assert_eq!(p.master().map(|m| m.state), Some(ready_state::HOST));
        assert_eq!(p.join(member(2)), Err(JoinFail::NotInvited));
        assert_eq!(
            p.check_invite(2, 3),
            Err(RejectReason::YouAreNotPartyMaster)
        );
        assert_eq!(p.check_invite(1, 2), Ok(()));
        p.add_invitation(2);
        assert_eq!(p.check_invite(1, 2), Err(RejectReason::AlreadyInvited));
        assert!(!p.is_to_remove());
        assert_eq!(p.join(member(2)), Ok(1));
        assert!(!p.is_invited(2));
        assert_eq!(p.by_slot(1).map(|m| m.state), Some(ready_state::IN_TOWN));
    }

    #[test]
    fn a_lone_master_after_a_rejection_or_departure_dissolves_the_town_party() {
        let mut p = Party::new(7);
        p.join(member(1)).unwrap();
        p.add_invitation(2);
        p.remove_invitation(2);
        assert!(p.is_to_remove());
        p.add_invitation(2);
        p.join(member(2)).unwrap();
        assert_eq!(p.remove(2, false), Some(Removal::Dissolved));
    }

    #[test]
    fn the_master_leaving_a_town_party_elects_the_next_member() {
        let mut p = Party::new(7);
        p.join(member(1)).unwrap();
        for cid in [2, 3] {
            p.add_invitation(cid);
            p.join(member(cid)).unwrap();
        }
        assert_eq!(p.remove(1, false), Some(Removal::Continues));
        let master = p.master().unwrap();
        assert_eq!(
            (master.cid, master.slot, master.state),
            (2, 0, ready_state::HOST)
        );
        assert_eq!(p.by_slot(1), None);
        assert_eq!(p.by_slot(2).map(|m| m.cid), Some(3));
    }

    #[test]
    fn a_full_party_extends_to_six_and_eight_slots_without_a_ship() {
        let mut p = Party::new(7);
        p.join(member(1)).unwrap();
        for cid in 2..=8 {
            assert_eq!(p.check_invite(1, cid), Ok(()));
            p.add_invitation(cid);
            p.join(member(cid)).unwrap();
        }
        assert_eq!(p.party_size(), 8);
        assert_eq!(p.check_invite(1, 9), Err(RejectReason::PartyIsFull));
    }

    #[test]
    fn kick_transfer_and_ready_follow_the_processors() {
        let mut p = Party::new(7);
        p.join(member(1)).unwrap();
        p.add_invitation(2);
        p.join(member(2)).unwrap();
        assert_eq!(p.kick(2, 0, 1), None);
        assert_eq!(p.kick(1, 1, 99), None);
        assert_eq!(p.kick(1, 1, 2), Some(2));
        assert_eq!(
            p.set_member_state(1, ready_state::READY),
            Err(StateFail::CannotReadyInTown)
        );
        assert_eq!(
            p.set_member_state(0, ready_state::READY),
            Err(StateFail::HostCannotChangeState)
        );
        assert_eq!(p.set_member_state(1, ready_state::IN_SHIP), Ok(true));
        assert!(!p.is_all_ready());
        assert_eq!(p.set_member_state(1, ready_state::READY), Ok(true));
        assert!(p.is_all_ready());
        assert!(p.transfer_master(1, 2));
        assert_eq!(p.master().map(|m| m.cid), Some(2));
        assert_eq!(
            p.by_slot(1).map(|m| (m.cid, m.state)),
            Some((1, ready_state::READY))
        );
        p.change_state(party_state::STARTING);
        assert_eq!(p.kick(2, 1, 1), None);
    }

    #[test]
    fn a_disconnected_member_rejoins_its_slot_in_game() {
        let mut p = Party::new(7);
        p.join(member(1)).unwrap();
        for cid in [2, 3] {
            p.add_invitation(cid);
            p.join(member(cid)).unwrap();
        }
        p.change_state(party_state::IN_GAME);
        assert_eq!(p.remove(2, false), Some(Removal::Continues));
        assert_eq!(p.rejoin(member(2), 1), Ok(1));
        assert_eq!(p.by_slot(1).map(|m| m.state), Some(ready_state::REJOIN));
        assert_eq!(p.rejoin(member(2), 1), Err(JoinFail::InternalError));
    }

    #[test]
    fn the_ship_sets_the_size_and_a_host_leaving_before_start_returns_to_town() {
        let mut p = Party::new(7);
        p.join(member(1)).unwrap();
        p.add_invitation(2);
        p.join(member(2)).unwrap();
        let limits = ShipLimits {
            max_members: 4,
            min_level: 1,
            max_level: 20,
        };
        assert!(p.set_ship(limits, false));
        p.start_micro_play();
        p.change_state(party_state::IN_SHIP);
        let mut low = member(3);
        low.level = 30;
        assert_eq!(p.join(low), Err(JoinFail::LevelNotMatch));
        assert_eq!(p.remove(1, false), Some(Removal::Continues));
        assert_eq!(p.state(), party_state::IN_TOWN);
        assert!(!p.has_ship());
        assert_eq!(p.master().map(|m| m.cid), Some(2));
        let mut q = Party::new(8);
        q.join(member(1)).unwrap();
        q.set_ship(limits, true);
        q.change_state(party_state::IN_SHIP);
        assert_eq!(q.remove(1, false), Some(Removal::Dissolved));
    }
}
