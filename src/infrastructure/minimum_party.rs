//! BattleServiceProxy party messages outside the MicroPlay (Initial, PartyMaster,
//! PartyMember and the ship host states): PartyInvite, the invitation answers, LeaveParty,
//! Kick and JoinShip's party side, through the party hub.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
    party_hub::{Invite, PartyHub, Role},
};
use crate::domain::character_stats::Arithmetic;
use crate::domain::map_partition::OutgoingPacket;
use crate::domain::party::{Member, ready_state};
use crate::protocol::mission::Message;
use sqlx::PgPool;

/// SystemMessageCategory.System / Dialog.
const SYSTEM: u8 = 0;
const DIALOG: u8 = 4;

/// MicroPlayErrorCode names of the party failures.
pub mod error {
    pub const NO_SUCH_CHARACTER: &str = "NoSuchCharacter";
    pub const ALREADY_IN_PARTY: &str = "AlreadyInParty";
    pub const NOT_MASTER: &str = "NotMaster";
    pub const JOIN_PARTY_FAIL: &str = "JoinPartyFail";
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}
/// BattleServiceProxy.SendErrorMessage(category, key, error).
pub fn error_message(
    category_dialog: bool,
    key: &str,
    error: &str,
) -> Result<OutgoingPacket, MinimumError> {
    super::minimum_shop::system_message(
        if category_dialog { DIALOG } else { SYSTEM },
        &format!("GameUI_Heroes_PartyInMicroPlay_{key}_{error}"),
        &[],
    )
}
fn kicked() -> Result<OutgoingPacket, MinimumError> {
    super::minimum_mission::packet(&Message::Kicked)
}

/// WhoIs: the CharacterSummary fields of the party entry.
pub async fn member(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<Member, MinimumError> {
    let base =
        super::minimum_selection::view_member(pool, owner, assets, cid, config, arithmetic).await?;
    Ok(Member {
        cid,
        nexon_sn: owner.uid,
        character: base.base_class,
        name: base.name.unwrap_or_default(),
        level: base.level,
        slot: -1,
        state: ready_state::IN_TOWN,
        latest_ping: 0,
        latest_frame_rate: 0,
        is_return: base.is_return,
        is_event_jumping: base.is_event_jumping,
    })
}

/// The character name of `cid` (CharacterSummary.CharacterID).
pub async fn name_of(pool: &PgPool, cid: i64) -> Result<String, MinimumError> {
    let raw: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT name_utf16 FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(pool)
            .await?;
    decode(&raw.ok_or_else(fail)?)
}
pub(super) fn decode(raw: &[u8]) -> Result<String, MinimumError> {
    let (pairs, rest) = raw.as_chunks::<2>();
    if !rest.is_empty() {
        return Err(fail());
    }
    String::from_utf16(
        &pairs
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}

/// QueryCIDByName: a living character whose name equals `name` (case-insensitive, as the
/// SQL Server collation compares names).
pub async fn character_by_name(pool: &PgPool, name: &str) -> Result<Option<i64>, MinimumError> {
    Ok(character_info_by_name(pool, name).await?.map(|(id, _)| id))
}

/// QueryCIDByName with the account of the character (QueryCIDByName.UID): (CID, UID).
pub async fn character_info_by_name(
    pool: &PgPool,
    name: &str,
) -> Result<Option<(i64, i32)>, MinimumError> {
    let rows: Vec<(i64, i32, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT id,uid,name_utf16 FROM game_character_info WHERE status<>1 AND name_utf16 IS NOT NULL LIMIT 1001",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().find_map(|(id, uid, raw)| {
        let found = decode(&raw?).ok()?;
        found
            .to_lowercase()
            .eq(&name.to_lowercase())
            .then_some((id, uid))
    }))
}

/// What a party message did to the connection's own party membership.
pub struct Outcome {
    pub packets: Vec<OutgoingPacket>,
    /// The connection left its party (LeaveParty): its ship/MicroPlay state goes with it.
    pub left: bool,
}
impl Outcome {
    fn packets(packets: Vec<OutgoingPacket>) -> Self {
        Self {
            packets,
            left: false,
        }
    }
}

/// PartyInviteMessage in every state. Initial opens a party first (OpenNewParty) and leaves
/// it again when the invitation fails; a party member is refused with NotMaster.
#[allow(clippy::too_many_arguments)]
pub async fn invite(
    hub: &PartyHub,
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    invited_name: &str,
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<Outcome, MinimumError> {
    let party_id = match hub.role(cid) {
        Role::Member { .. } => {
            return Ok(Outcome::packets(vec![error_message(
                false,
                "Invite",
                error::NOT_MASTER,
            )?]));
        }
        Role::Master { party_id, .. } => Some(party_id),
        Role::None => None,
    };
    let me = member(pool, owner, assets, cid, config, arithmetic).await?;
    if invited_name == me.name {
        return Ok(Outcome::packets(vec![error_message(
            true,
            "Invite",
            error::ALREADY_IN_PARTY,
        )?]));
    }
    let Some(invited) = character_by_name(pool, invited_name).await? else {
        return Ok(Outcome::packets(vec![error_message(
            true,
            "Invite",
            error::NO_SUCH_CHARACTER,
        )?]));
    };
    let mut packets = Vec::new();
    let opened = party_id.is_none();
    let party_id = match party_id {
        Some(id) => id,
        None => {
            let id = super::minimum_mission::entity_id()?;
            packets.extend(hub.open(id, me)?);
            id
        }
    };
    match hub.invite(party_id, cid, invited, invited_name)? {
        Invite::Sent(p) => {
            packets.extend(p);
            Ok(Outcome::packets(packets))
        }
        Invite::Failed(p) => {
            packets.extend(p);
            // Initial: InvitePartyMember.OnFail → LeaveParty (KickedMessage from PartyMaster).
            let removed = matches!(hub.role(cid), Role::None);
            if opened || removed {
                hub.leave(cid)?;
                packets.push(kicked()?);
                return Ok(Outcome {
                    packets,
                    left: true,
                });
            }
            Ok(Outcome::packets(packets))
        }
    }
}

/// PartyInvitationAcceptMessage (Initial with InvitedPartyID == PartyID): JoinParty.
#[allow(clippy::too_many_arguments)]
pub async fn accept(
    hub: &PartyHub,
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    party_id: i64,
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<Outcome, MinimumError> {
    if hub.role(cid) != Role::None {
        return Ok(Outcome::packets(Vec::new()));
    }
    let me = member(pool, owner, assets, cid, config, arithmetic).await?;
    match hub.accept(party_id, me)? {
        None => {
            tracing::warn!("invitation accept for another party ignored; values omitted");
            Ok(Outcome::packets(Vec::new()))
        }
        Some(Ok(packets)) => Ok(Outcome::packets(packets)),
        // JoinParty failed: LeaveParty (PartyMember → KickedMessage), then the dialog.
        Some(Err(_)) => Ok(Outcome {
            packets: vec![
                kicked()?,
                error_message(true, "InviteAccept", error::JOIN_PARTY_FAIL)?,
            ],
            left: true,
        }),
    }
}

/// PartyInvitationRejectMessage (Initial).
pub fn reject(hub: &PartyHub, cid: i64, party_id: i64) -> Result<Outcome, MinimumError> {
    if hub.role(cid) == Role::None {
        hub.reject(cid, party_id)?;
    }
    Ok(Outcome::packets(Vec::new()))
}

/// LeavePartyMessage outside the MicroPlay: BattleServiceProxy.LeaveParty.
pub fn leave(hub: &PartyHub, cid: i64) -> Result<Outcome, MinimumError> {
    if !hub.leave(cid)? {
        return Ok(Outcome::packets(Vec::new()));
    }
    Ok(Outcome {
        packets: vec![kicked()?],
        left: true,
    })
}

/// KickMessage of a master state.
pub fn kick(hub: &PartyHub, cid: i64, slot: i32, nexon_sn: i32) -> Result<Outcome, MinimumError> {
    if !matches!(hub.role(cid), Role::Master { .. }) {
        return Ok(Outcome::packets(Vec::new()));
    }
    Ok(Outcome::packets(hub.kick(cid, slot, nexon_sn)?))
}

/// The Kick operation pushed by the party: KickProcessor → LeaveParty (KickedMessage, the
/// proxy was in a party state) and its own KickedMessage.
pub fn kicked_by_party() -> Result<Vec<OutgoingPacket>, MinimumError> {
    Ok(vec![kicked()?, kicked()?])
}

/// MicroPlayService.RecommendShip for the asking character: listed ships of its
/// difficulty and level range with a free place, carrying the selected bosses, whose quest
/// is in the restriction's quest set (or is its target quest) and passes the character's
/// quest constraints (QueryAvailableQuestsByQuestSet). At most MaxRecommendShipCount (6).
#[allow(clippy::too_many_arguments)]
pub async fn recommended(
    hub: &PartyHub,
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    restriction: &crate::protocol::mission::RecommendShipRestriction,
) -> Result<Vec<crate::protocol::mission::Ship>, MinimumError> {
    const MAX_RECOMMEND_SHIP_COUNT: usize = 6;
    if restriction.quest_set == 2000 {
        return Ok(Vec::new());
    }
    let Some(quests) = assets.npc.as_ref().and_then(|n| n.quests.as_ref()) else {
        return Ok(Vec::new());
    };
    let contents = assets.mission().ok_or_else(fail)?;
    let level: i32 = sqlx::query_scalar("SELECT level FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(pool)
        .await?;
    let target = restriction.target_quest_id.as_deref().unwrap_or("");
    let bosses = restriction
        .selected_boss_quest_ids
        .clone()
        .unwrap_or_default();
    let mut out = Vec::new();
    for ship in hub.open_ships() {
        let quest = ship.quest_id.clone().unwrap_or_default();
        let in_set = quests
            .info(&quest)
            .is_some_and(|q| q.quest_set == restriction.quest_set);
        let carries = |b: &i32| {
            ship.selected_boss_quest_ids
                .as_ref()
                .is_some_and(|ids| ids.contains(b))
        };
        if ship.difficulty != restriction.difficulty
            || ship.max_level < level
            || ship.min_level > level
            || ship.max_ship_member_count <= ship.member_count
            || !bosses.iter().all(carries)
            || !in_set
            || (!target.is_empty() && quest != target)
        {
            continue;
        }
        let story = contents
            .digest(&quest, -1)
            .ok()
            .and_then(|d| d.required_story_line.zip(d.required_phase));
        let available = super::minimum_quest::digest_constraint(
            pool,
            owner,
            assets,
            cid,
            &quest,
            ship.difficulty,
            ship.is_practice_mode,
            story.as_ref().map(|(l, p)| (l.as_str(), p.as_str())),
        )
        .await?
        .is_none();
        if available {
            out.push(ship);
            if out.len() == MAX_RECOMMEND_SHIP_COUNT {
                break;
            }
        }
    }
    Ok(out)
}
