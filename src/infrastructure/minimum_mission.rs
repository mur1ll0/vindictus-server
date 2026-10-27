//! Mission entry composition: the QueryMicroPlayInfo(InitItemDropEntities) answer as a
//! GameStartGranted message, from the digest and the generated sector sequence.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::domain::map_partition::OutgoingPacket;
use crate::domain::{
    character_stats::Arithmetic,
    mission::{
        self as domain, ItemSlotInfo, MissionContents, MissionError, MissionRandom, QuestDigest,
        SectorInfos, SlotSource,
    },
};
use crate::protocol::mission::{
    self as wire, AcceptQuest, BlessStones, ConnectionRequest, DropErg, DropGoldCore, Erg,
    EvilCoreInfo, GameInfo, GetItem, LaunchShipGranted, Message, PaidCoin, PartyInfo, PartyMember,
    PayCoinCompleted, PutErg, Ship, ShipOption, UpdateShip,
};
use crate::protocol::{
    inventory::Dict,
    mission::{
        BattleInventory, Consumable, CostumeField, GameStartGranted, JoinMember, NoPet,
        QuestSector, QuestSectors, SectorConnection, StrDict, TreasureBox, TrialFactor,
    },
};
use rand_core::{OsRng, RngCore};
use regex::{Regex, RegexBuilder};
use sqlx::PgPool;
use std::sync::atomic::{AtomicI64, Ordering};

/// Uniform operating-system randomness for the unseeded System.Random of the builder.
pub struct OsMissionRandom;
impl MissionRandom for OsMissionRandom {
    fn next(&mut self) -> i32 {
        loop {
            let v = (OsRng.next_u32() >> 1) as i32;
            if v != i32::MAX {
                return v;
            }
        }
    }
    fn next_range(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = (i64::from(max) - i64::from(min)) as u64;
        // Rejection sampling keeps the draw uniform.
        let zone = u64::MAX - (u64::MAX % span);
        loop {
            let v = OsRng.next_u64();
            if v < zone {
                return (i64::from(min) + (v % span) as i64) as i32;
            }
        }
    }
    fn next_double(&mut self) -> f64 {
        (OsRng.next_u64() >> 11) as f64 * (1.0 / (1_u64 << 53) as f64)
    }
}

/// QuestSectorInfos on the wire (empty drop dictionaries, no altar or boss list entries).
pub fn sectors_wire(infos: &SectorInfos) -> QuestSectors {
    QuestSectors {
        current_group: infos.current_group,
        starting_group: infos.starting_group,
        sectors: Some(
            infos
                .sectors
                .iter()
                .map(|s| {
                    Some(QuestSector {
                        group_id: s.group_id,
                        bsp: s.bsp.clone(),
                        entities: Some(s.entities.iter().cloned().map(Some).collect()),
                        connections: Some(
                            s.connections
                                .iter()
                                .map(|c| {
                                    Some(SectorConnection {
                                        from: c.from,
                                        from_trigger: Some(c.from_trigger.clone()),
                                        to: c.to,
                                        to_spawn: Some(c.to_spawn.clone()),
                                    })
                                })
                                .collect(),
                        ),
                        item_drop_entity: StrDict(Some(s.item_drop_entity.clone())),
                        weak_point_entity: StrDict(Some(Vec::new())),
                        trial_factors: Some(
                            s.trial_factors
                                .iter()
                                .map(|(m, g, name, sector)| {
                                    Some(TrialFactor {
                                        group_number: *g,
                                        trial_mod: *m,
                                        trial_name: name.clone(),
                                        sector_group_id: *sector,
                                    })
                                })
                                .collect(),
                        ),
                        treasure_boxes: Some(
                            s.treasure_boxes
                                .iter()
                                .map(|(g, name)| {
                                    Some(TreasureBox {
                                        group_id: *g,
                                        entity_name: name.clone(),
                                        is_visible: false,
                                        is_opened: false,
                                    })
                                })
                                .collect(),
                        ),
                        save_required: s.save_required,
                        last_used_spawn_point: None,
                    })
                })
                .collect(),
        ),
        treasure_box_max_count: 0,
        altar_status: Some(Vec::new()),
        boss_killed: Some(Vec::new()),
    }
}

/// BattleServiceProxy.StartGame (non-DS): GameStartGranted from QueryMicroPlayInfo.
pub fn game_start_granted(
    digest: &QuestDigest,
    infos: &SectorInfos,
    difficulty: i32,
    timer_decreasing: bool,
    player_count: i32,
) -> GameStartGranted {
    GameStartGranted {
        quest_level: digest.quest_level,
        quest_time: digest.time_limit,
        hard_mode: i32::from(digest.hard_mode),
        solo_sector: digest.solo_sector,
        sectors: Some(sectors_wire(infos)),
        is_hunting_quest: digest.is_hunting_quest,
        init_game_time: digest.init_game_time,
        sector_move_game_time: digest.sector_move_game_time,
        difficulty,
        is_timer_decreasing: timer_decreasing,
        quest_started_player_count: player_count,
        new_slot: -1,
        new_key: -1,
        is_user_dedicated: false,
    }
}

/// Digest, sequence and GameStartGranted of a solo start (no story drops, no today quest).
pub fn preview(
    contents: &MissionContents,
    quest: &str,
    swear_id: i32,
    difficulty: i32,
    practice: bool,
) -> Result<GameStartGranted, MissionError> {
    contents.check_supported(quest, difficulty)?;
    let mut digest = contents.digest(quest, swear_id)?;
    contents.item_limit(&mut digest, swear_id, difficulty);
    let sequence =
        contents.sector_sequence(&digest, difficulty, practice, false, &mut OsMissionRandom)?;
    let infos = contents.info(quest, &sequence, practice)?;
    Ok(game_start_granted(&digest, &infos, difficulty, true, 1))
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
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

/// MicroPlayer.MakeMemberInfo for a player admitted by AddPlayer: WhoIs (summary,
/// stats, skills, equipment maps), QueryPlayerInfo (equipped items, status-effect
/// and ability lists) and the battle inventory. Vocations, SP-skill enhancements,
/// guilds, pets and stored status effects are outside the migrated profile and refused.
#[allow(clippy::too_many_arguments)]
pub async fn member_info(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    slot: i32,
    key: i32,
    item_limit: Option<&Regex>,
    settings: &[(String, String)],
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<HostMember, MinimumError> {
    let mut tx = pool.begin().await?;
    let view =
        super::minimum_selection::view(&mut tx, owner, assets, cid, config, arithmetic, false, 0)
            .await?;
    let vocation: Option<i32> =
        sqlx::query_scalar("SELECT vocation_class FROM game_vocations WHERE cid=$1")
            .bind(cid)
            .fetch_optional(&mut *tx)
            .await?;
    // WhoIs.StatusEffects: the exported effects; other stored effects are not migrated.
    let (effects, other) = match assets.cash_shop.as_ref() {
        Some(shop) => {
            super::minimum_status_effect::load(&mut tx, &shop.customize().status_effects, cid)
                .await?
        }
        None => {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM game_status_effects WHERE cid=$1")
                    .bind(cid)
                    .fetch_one(&mut *tx)
                    .await?;
            (Vec::new(), count != 0)
        }
    };
    if vocation.is_some() || other {
        return Err(fail());
    }
    let status_effects = super::minimum_status_effect::elements(
        &effects,
        super::minimum_status_effect::now_ticks()?,
    );
    // MakeMemberInfo: SkillManager.GetBriefSkillEnhanceData.
    let skill_enhances = super::minimum_skill_enhance::brief_data(&mut tx, cid).await?;
    // SkillManager.ToDictionary: base ranks without item skill ranks (refused by view).
    let skills: Vec<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT skill_utf16,rank FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    let sp: Vec<(i32, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT slot_id,skill_utf16 FROM game_sp_skills WHERE cid=$1 ORDER BY row_id LIMIT 257",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    if skills.len() > 4096 || sp.len() > 256 {
        return Err(fail());
    }
    let skills = skills
        .into_iter()
        .map(|(id, rank)| Ok((text(&id)?, rank)))
        .collect::<Result<Vec<_>, MinimumError>>()?;
    let mut sp_skills: Vec<(i32, Option<String>)> = Vec::new();
    for (slot_id, id) in sp {
        if sp_skills.iter().any(|(s, _)| *s == slot_id) {
            return Err(fail());
        }
        sp_skills.push((slot_id, id.map(|i| text(&i)).transpose()?));
    }
    // Gear.ToEquippedItemInfos: part → ItemClass of non-empty classes.
    let mut equipped: Vec<(i32, Option<String>)> = Vec::new();
    for (part, id) in &view.equipment {
        let class: Vec<u8> = sqlx::query_scalar(
            "SELECT item_class_utf16 FROM game_items WHERE id=$1 AND owner_id=$2",
        )
        .bind(id)
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
        let class = text(&class)?;
        if !class.is_empty() && !equipped.iter().any(|(p, _)| p == part) {
            equipped.push((*part, Some(class)));
        }
    }
    let quick = super::minimum_quickslot::load(&mut tx, assets, cid).await?;
    let held: Vec<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT item_class_utf16,count FROM game_items WHERE owner_id=$1 ORDER BY id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let contents = assets.skills.as_ref().ok_or_else(fail)?;
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let sub_weapon = utf16("SUBWEAPON");
    let mut slots = Vec::new();
    for (part, class) in super::minimum_quickslot::info(&quick)
        .classes
        .0
        .unwrap_or_default()
    {
        let Some(class) = class else { continue };
        let source = if class.starts_with("skill_") {
            SlotSource::Skill {
                info: contents
                    .get(&class)
                    .map(|c| c.use_type.is_some_and(|u| u != 0)),
                rank: skills.iter().find(|(id, _)| *id == class).map(|(_, r)| *r),
            }
        } else {
            let bytes = utf16(&class);
            let info = match assets.stats_index.class(Some(&bytes)) {
                None => None,
                Some(indexed) => {
                    let e = economy.get(indexed.source_index).ok_or_else(fail)?;
                    Some(ItemSlotInfo {
                        class: class.clone(),
                        sub_weapon: e.category.as_ref().map(|c| c.as_bytes())
                            == Some(&sub_weapon[..]),
                        quick_slot_max_stack: e.quick_slot_max_stack,
                        draft: e.draft,
                        alchemist_box: economy.is_alchemist_box(&bytes),
                    })
                }
            };
            let usable = held
                .iter()
                .filter(|(c, _)| *c == bytes)
                .try_fold(0_i32, |n, (_, count)| n.checked_add(*count))
                .ok_or_else(fail)?;
            SlotSource::Item { info, usable }
        };
        slots.push((part, class, source));
    }
    let consumables = domain::battle_inventory(
        slots,
        domain::multiplier(settings, "QuickSlotMultiplier", "SetQuickSlotLimit")
            .map_err(|_| fail())?,
        domain::multiplier(settings, "SubWeaponMultiplier", "SetSubWeaponLimit")
            .map_err(|_| fail())?,
        item_limit,
    )
    .map_err(|_| fail())?;
    let m = view.member;
    let (is_return, is_event_jumping) = (m.is_return, m.is_event_jumping);
    let join = JoinMember {
        order: slot,
        tag: slot,
        key,
        name: m.name,
        base_class: m.base_class,
        level: m.level,
        exp: m.exp,
        level_up_exp: m.level_up_exp,
        title_id: m.title,
        title_count: m.title_count,
        stats: StrDict(Some(m.stats)),
        battle_inventory: Some(BattleInventory {
            consumables: Dict(Some(
                consumables
                    .into_iter()
                    .map(|(part, c)| {
                        (
                            part,
                            Some(Consumable {
                                item_class: Some(c.class),
                                bring: c.bring,
                                used: 0,
                                draft: c.draft,
                                inner: Some(Vec::new()),
                            }),
                        )
                    })
                    .collect(),
            )),
        }),
        costume: CostumeField(view.costume.clone()),
        pet: NoPet,
        skill_list: StrDict(Some(skills)),
        sp_skills: Dict(Some(sp_skills)),
        vocation_skills: StrDict(Some(Vec::new())),
        // MakeMemberInfo: TransformCoolDown is null without a vocation.
        transform_cool_down: -1,
        skill_enhances: StrDict(Some(skill_enhances)),
        def_map: Dict(Some(m.def_map.into_iter().collect())),
        armor_hp_map: Dict(Some(m.armor_hp.into_iter().collect())),
        equipped_items: Dict(Some(equipped)),
        ability_list: Some(Vec::new()),
        status_effect_dict: StrDict(Some(Vec::new())),
        status_effects: Some(status_effects),
        destroyed_def: m.destroyed_def,
        is_teacher: false,
        is_assist: false,
        guild_level_bonus: StrDict(Some(Vec::new())),
        is_alive: true,
    };
    Ok(HostMember {
        join,
        is_return,
        is_event_jumping,
    })
}

/// A host's member info plus the CharacterSummary flags of its party entry.
pub struct HostMember {
    pub join: JoinMember,
    pub is_return: bool,
    pub is_event_jumping: bool,
}

/// RemoteCategory of the MicroPlay connection (ConnectionRequest.Category, P2P parent).
pub const MICRO_PLAY_CATEGORY: &str = "MicroPlayServiceCore.MicroPlayService";
/// GameStartGranted bodies reach ~240 KB; 102,400 is only MessageAnalyzer's log threshold.
pub const MAX_MISSION_BODY: usize = 4 * 1024 * 1024;
/// SystemMessageCategory.Dialog.
const DIALOG: u8 = 4;
/// PartyInfoState.
pub mod party_state {
    pub const IN_TOWN: u8 = 0;
    pub const IN_TOWN_WITH_SHIP_INFO: u8 = 1;
    pub const ENTERING_SHIP: u8 = 2;
    pub const IN_SHIP: u8 = 3;
    pub const STARTING: u8 = 4;
    pub const IN_GAME: u8 = 5;
}
/// ReadyState.Host.
const HOST: u8 = 3;

pub(super) fn packet(message: &Message) -> Result<OutgoingPacket, MinimumError> {
    let kind = message.kind();
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(message), MAX_MISSION_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
/// InGame QueryRecommendShipMessage (the boat's "Loading Battle List"). A target quest asked
/// again within 2 s of the last answered query is Ignored. Otherwise the original lists the
/// open ships of other parties from the MicroPlay services; the solo laboratory has none, so
/// the list is empty and the client offers Create Party. Quest set 2000 (DS raids) needs the
/// DS boss service, whose failure path is also an empty list.
pub fn recommend_ships(
    query: &wire::QueryRecommendShip,
    last_answered: &mut Option<std::time::Instant>,
    now: std::time::Instant,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    const COOLDOWN: std::time::Duration = std::time::Duration::from_millis(2000);
    let restriction = query.restriction.as_ref().ok_or_else(fail)?;
    let targeted = restriction.target_quest_id.as_deref() != Some("");
    let cooling = last_answered.is_some_and(|t| now.saturating_duration_since(t) <= COOLDOWN);
    let ignored = targeted && cooling;
    if !ignored && restriction.quest_set != 2000 {
        *last_answered = Some(now);
    }
    Ok(vec![packet(&Message::ShipList(wire::ShipList {
        ignored,
        ships: Some(Vec::new()),
    }))?])
}
/// BattleServiceProxy.SendErrorMessage(Dialog, key, error).
fn general_fail(key: &str, error: &str) -> Result<Vec<OutgoingPacket>, MinimumError> {
    Ok(vec![super::minimum_shop::system_message(
        DIALOG,
        &format!("GameUI_Heroes_PartyInMicroPlay_{key}_{error}"),
        &[],
    )?])
}

/// MicroPlayService.MakeEntityID: DateTime ticks, strictly increasing in this process.
fn entity_id() -> Result<i64, MinimumError> {
    static LAST: AtomicI64 = AtomicI64::new(0);
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| fail())?;
    let ticks = 621_355_968_000_000_000_i64
        .checked_add(i64::try_from(since.as_nanos() / 100).map_err(|_| fail())?)
        .ok_or_else(fail)?;
    let mut last = LAST.load(Ordering::Relaxed);
    loop {
        let next = ticks.max(last + 1);
        match LAST.compare_exchange(last, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Ok(next),
            Err(seen) => last = seen,
        }
    }
}

/// The party and MicroPlay of one solo host, as the MicroPlay service keeps them.
pub struct Mission {
    pub party_id: i64,
    pub group: i32,
    pub quest: String,
    pub title: Option<String>,
    pub option: ShipOption,
    pub digest: QuestDigest,
    /// QuestDigest.SwearID: the requested swear when the quest has it, else -1.
    pub swear_id: i32,
    pub settings: Vec<(String, String)>,
    pub member: PartyMember,
    /// PartyInfoState; ShipInfo exists from RegisterShipInfo on.
    pub state: u8,
    pub host_info: Option<GameInfo>,
    pub sectors: Option<SectorInfos>,
    /// SectorSequence of the started MicroPlay.
    pub sequence: Option<domain::SectorSequence>,
    /// MicroPlay.CurGameTime (hour of day in the dungeon).
    pub game_time: i32,
    /// MicroPlay.PlayState.InGame after the first sector move.
    pub in_game: bool,
    /// MicroPlayer.MonsterKillExp (given with the quest result).
    pub monster_kill_exp: i32,
    /// MicroPlayer.Alive.
    pub alive: bool,
    /// Informer events for the goal trackers, in order: actor CID (−1 none, 0 the
    /// play) and event ("kill <type> <action>", ...).
    pub events: Vec<(i64, String)>,
    /// MicroPlay.TitleTracker goals of the player (QueryPlayerInfo.TitleGoalIDs), taken
    /// before the first informer event; None until then.
    pub title_goals: Option<Vec<i32>>,
    /// Informer events of a non-practice game waiting for the title tracker.
    pub title_events: Vec<(i64, String)>,
    /// SectorSequence.BossKilledList.
    pub boss_killed: Vec<String>,
    /// MicroPlayer.Luck (Stats LUCK; no platinum, bless stone or dormant bonus).
    pub luck: i32,
    /// MicroPlayer.BaseCharacter: required-class drops need its class bit.
    pub base_character: u8,
    /// MicroPlayer.StoryDropData (QueryPlayerInfo → StoryEntity.QueryStoryDropData).
    pub story_drops: Vec<domain::StoryDropData>,
    /// BattleInventory: part → (ItemClass, BringNum, UsedNum, DraftNum).
    pub consumables: Vec<(i32, String, i32, i32, i32)>,
    /// MicroPlayer.actionRecord of the latest CombatRecord.
    pub combat_record: Option<wire::CombatRecord>,
    /// ReduceMinClearTime amounts applied by server-only effects.
    pub min_clear_time_reduced: i32,
    /// MicroPlayer.ReservedRequiredItem (AddPlayer), paid by CheckCoinPaid.
    pub reserved_item: Option<String>,
    /// Goal tracking, rewards and the EndGame of the quest.
    pub completion: super::minimum_mission_finish::Completion,
}
impl Mission {
    /// ShipOptionInfo.MemberCountLimit.
    fn member_count_limit(&self) -> i32 {
        if self.option.swear_member_limit != -1 {
            self.option
                .swear_member_limit
                .min(self.option.max_member_count)
        } else {
            self.option.max_member_count
        }
    }
    fn max_ship_member_count(&self) -> i32 {
        self.digest.party_limit.min(self.member_count_limit())
    }
    fn has_ship_info(&self) -> bool {
        self.state != party_state::IN_TOWN
    }
    /// Party.SetShipInfo (refreshed on every broadcast once the MicroPlay exists).
    fn ship(&self) -> Ship {
        Ship {
            party_id: self.party_id,
            ship_name: self.title.clone(),
            password: Some(String::new()),
            min_level: self.option.min_level,
            max_level: self.option.max_level,
            member_count: 1,
            max_ship_member_count: self.max_ship_member_count(),
            quest_id: Some(self.quest.clone()),
            is_hunting_quest: self.digest.is_hunting_quest,
            is_giant_raid: false,
            swear_id: self.swear_id,
            rest_time: -1,
            reinforce_allowed: false,
            adult_rule: self.option.over18_only != 0,
            members: Some(vec![Some(self.member.clone())]),
            state: self.state,
            party_bonus_count: 0,
            party_bonus_ratio: 0,
            is_party_only: self.option.is_party_only != 0,
            host_ping: self.member.latest_ping,
            host_frame_rate: self.member.latest_frame_rate,
            difficulty: self.option.difficulty,
            is_return: self.member.is_return,
            is_season2: self.digest.is_season2,
            selected_boss_quest_ids: self.option.selected_boss_quest_ids.clone(),
            is_practice_mode: self.option.is_practice_mode,
            user_ds_mode: self.option.user_ds_mode,
        }
    }
    /// Party.BroadCastPartyInfo → UpdatePartyProcessor: PartyInfo, then UpdateShip when the
    /// ship exists. The single authority point for party-visible state; skipped while
    /// EnteringShip (IsLoadingState).
    pub fn broadcast(&self) -> Result<Vec<OutgoingPacket>, MinimumError> {
        if self.state == party_state::ENTERING_SHIP {
            return Ok(Vec::new());
        }
        let size = if self.has_ship_info() {
            self.max_ship_member_count()
        } else {
            4
        };
        let mut out = vec![packet(&Message::PartyInfo(PartyInfo {
            party_id: self.party_id,
            party_size: size,
            state: self.state,
            members: Some(vec![Some(self.member.clone())]),
        }))?];
        if self.has_ship_info() {
            out.push(packet(&Message::UpdateShip(UpdateShip {
                info: Some(self.ship()),
            }))?);
        }
        Ok(out)
    }
}

/// AcceptQuest of a player in the Initial state.
pub enum Accept {
    /// The party, the relay registration to send and the packets in order.
    Started {
        mission: Box<Mission>,
        packets: Vec<OutgoingPacket>,
    },
    /// GeneralFail("AcceptQuest") after LeaveParty, with any packets already sent.
    Failed(Vec<OutgoingPacket>),
}

/// Relay registration of AddPlayer: RelayServer endpoint and AddClientWaiting.
pub struct RelayTicket {
    pub address: std::net::SocketAddr,
    pub group: i32,
    pub sn: i32,
    pub key: i32,
}

/// BattleServiceProxy Initial/AcceptQuest: OpenNewParty → JoinParty → RegisterShipInfo
/// (QueryQuestDigest) → StartMicroPlay (StartMicroPlay2) → JoinMicroPlay (AddPlayer,
/// QueryMicroPlayInfo) → ConnectionRequest + LaunchShipGranted. `relay` registers the
/// waiting client and opens the P2P group of the party; it is called once the digest
/// and member info are admitted.
#[allow(clippy::too_many_arguments)]
pub async fn accept_quest(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    message: &AcceptQuest,
    relay: impl FnOnce(i64) -> RelayTicket,
    config: QueryConfig,
    clock: super::minimum_quest::QuestClock,
) -> Result<Accept, MinimumError> {
    let arithmetic = clock.arithmetic;
    let contents = assets.mission().ok_or_else(fail)?;
    let fail_with = |sent: Vec<OutgoingPacket>, error: &str| -> Result<Accept, MinimumError> {
        let mut sent = sent;
        sent.extend(general_fail("AcceptQuest", error)?);
        Ok(Accept::Failed(sent))
    };
    let (Some(quest), Some(option)) = (message.quest_id.clone(), message.option.clone()) else {
        return fail_with(Vec::new(), "InvalidQuest");
    };
    // OpenParty + JoinParty: the host enters slot 0 of a 4-slot party in town.
    let base =
        super::minimum_selection::view_member(pool, owner, assets, cid, config, arithmetic).await?;
    let party_id = entity_id()?;
    let member = PartyMember {
        nexon_sn: owner.uid,
        character: base.base_class,
        character_id: base.name.clone(),
        slot: 0,
        level: base.level,
        state: HOST,
        latest_ping: 0,
        latest_frame_rate: 0,
        is_return: base.is_return,
        is_event_jumping: base.is_event_jumping,
    };
    let mut packets = vec![packet(&Message::PartyInfo(PartyInfo {
        party_id,
        party_size: 4,
        state: party_state::IN_TOWN,
        members: Some(vec![Some(member.clone())]),
    }))?];
    // RegisterShipInfo → QueryQuestDigest(CheckDifficulty, CheckDisableUserShip, practice).
    let difficulty = option.difficulty;
    let practice = option.is_practice_mode;
    let story = contents
        .digest(&quest, -1)
        .ok()
        .and_then(|d| d.required_story_line.zip(d.required_phase));
    if let Some(error) = super::minimum_quest::digest_constraint(
        pool,
        owner,
        assets,
        cid,
        &quest,
        difficulty,
        practice,
        story.as_ref().map(|(l, p)| (l.as_str(), p.as_str())),
    )
    .await?
    {
        return fail_with(packets, error);
    }
    let Ok(mut digest) = contents.digest(&quest, message.swear_id) else {
        return fail_with(packets, "InvalidQuest");
    };
    contents.item_limit(&mut digest, message.swear_id, difficulty);
    if digest.disable_user_ship {
        return fail_with(packets, "DisableUserShip");
    }
    if digest.party_limit < 1 {
        return fail_with(packets, "TooManyPartyMembers");
    }
    // Forms this server does not migrate: fishing/hunting/tower modules and gear limits
    // (UnequipClassifiedParts).
    if contents.check_supported(&quest, difficulty).is_err() || !digest.gear_limit.is_empty() {
        return fail_with(packets, "InvalidQuest");
    }
    // AddPlayer of the host outside practice mode reserves QuestDigest.RequiredItem
    // (practice mode's ticket_training is not migrated).
    let reserved_item = digest
        .required_item
        .clone()
        .filter(|i| !i.is_empty() && !practice);
    let mut mission = Mission {
        party_id,
        group: 0,
        quest: quest.clone(),
        title: message.title.clone(),
        option,
        digest,
        swear_id: if message.swear_id >= 0 && contents.has_swear(&quest, message.swear_id) {
            message.swear_id
        } else {
            -1
        },
        settings: contents.settings(&quest, difficulty).map_err(|_| fail())?,
        member,
        state: party_state::IN_TOWN_WITH_SHIP_INFO,
        host_info: None,
        sectors: None,
        sequence: None,
        game_time: 0,
        in_game: false,
        monster_kill_exp: 0,
        alive: true,
        events: Vec::new(),
        title_goals: None,
        title_events: Vec::new(),
        boss_killed: Vec::new(),
        luck: 0,
        base_character: 1,
        story_drops: Vec::new(),
        consumables: Vec::new(),
        combat_record: None,
        min_clear_time_reduced: 0,
        reserved_item,
        completion: super::minimum_mission_finish::Completion::new(cid),
    };
    // AddPlayer: QueryQuestState and the achieved goals of QueryPlayerInfo.
    let (achieved, today_play_count, today) =
        super::minimum_mission_finish::load_player(pool, owner, assets, cid, &quest, clock).await?;
    mission.completion.achieved_info = achieved;
    mission.completion.today_play_count = today_play_count;
    mission.completion.today_quest = today;
    packets.extend(mission.broadcast()?);
    if let Some(class) = &mission.reserved_item
        && held(pool, cid, class).await? <= 0
    {
        // AddPlayer.FailReasonEnum.ItemRequired.
        return fail_with(packets, "ItemRequired");
    }
    // StartMicroPlay2: P2P group and EnteringShip (no broadcast while loading).
    mission.state = party_state::ENTERING_SHIP;
    // AddPlayer: the item limit regex, member info, relay registration.
    let item_limit = if mission.digest.item_limit.is_empty() {
        None
    } else {
        Some(
            RegexBuilder::new(&mission.digest.item_limit)
                .case_insensitive(true)
                .size_limit(1 << 20)
                .build()
                .map_err(|_| fail())?,
        )
    };
    let key = OsMissionRandom.next();
    let member = member_info(
        pool,
        owner,
        assets,
        cid,
        0,
        key,
        item_limit.as_ref(),
        &mission.settings,
        config,
        arithmetic,
    )
    .await?;
    let luck = member
        .join
        .stats
        .0
        .as_ref()
        .and_then(|s| s.iter().find(|(k, _)| k == "LUCK"))
        .map_or(0, |(_, v)| *v);
    mission.luck = luck;
    let class: i16 = sqlx::query_scalar("SELECT class FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(pool)
        .await?;
    mission.base_character = u8::try_from(class).map_err(|_| MinimumError::Conflict)?;
    mission.story_drops = story_drop_data(pool, contents, cid, &mission.quest).await?;
    mission.consumables = member
        .join
        .battle_inventory
        .as_ref()
        .and_then(|b| b.consumables.0.as_ref())
        .map(|rows| {
            rows.iter()
                .filter_map(|(part, c)| {
                    let c = c.as_ref()?;
                    Some((
                        *part,
                        c.item_class.clone().unwrap_or_default(),
                        c.bring,
                        c.used,
                        c.draft,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let ticket = relay(party_id);
    mission.group = ticket.group;
    // BroadCastCoinInfo (no coin system: empty silver coins) and BlessStone.SendStoneInfo.
    packets.push(packet(&Message::PayCoinCompleted(PayCoinCompleted {
        coins: Some(vec![Some(PaidCoin {
            slot: 0,
            silver_coin: Some(Vec::new()),
            platinum_coin_owner: -1,
            platinum_coin_type: 0,
        })]),
    }))?);
    packets.push(packet(&Message::InsertBlessStoneComplete(BlessStones {
        slot: 0,
        owners: Some(vec![-1; 4]),
        // SlotDic order: ALL, AP, EXP, LUCK.
        types: Some(vec![0, 2, 1, 3]),
    }))?);
    packets.push(packet(&Message::ConnectionRequest(ConnectionRequest {
        address: Some(ticket.address.ip().to_string()),
        port: ticket.address.port(),
        posix_time: ticket.sn,
        key: ticket.key,
        category: Some(MICRO_PLAY_CATEGORY.to_owned()),
        ping_host_cid: -1,
        group_id: 0,
    }))?);
    packets.push(packet(&Message::LaunchShipGranted(Box::new(
        LaunchShipGranted {
            quest_id: Some(quest),
            adult_rule: u8::try_from(mission.option.over18_only).unwrap_or(0),
            is_practice_mode: u8::from(practice),
            host_info: Some(member.join),
        },
    )))?);
    Ok(Accept::Started {
        mission: Box::new(mission),
        packets,
    })
}

/// ShipHost RegisterServerMessage: HostStarted (host info) and ShipLaunched (InShip).
pub fn register_server(
    mission: &mut Mission,
    info: Option<GameInfo>,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if mission.state != party_state::ENTERING_SHIP {
        // ShipLaunched refuses a party that is not launching; nothing reaches the client.
        return Ok(Vec::new());
    }
    mission.host_info = info;
    mission.state = party_state::IN_SHIP;
    mission.broadcast()
}

/// ShipHost StartGameMessage: GameStarting (Starting) → CheckCoinPaid →
/// QueryMicroPlayInfo(InitItemDropEntities) → GameStartGranted → GameStarted (InGame).
pub fn start_game(
    mission: &mut Mission,
    contents: &MissionContents,
    random: &mut dyn MissionRandom,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    if mission.state != party_state::IN_SHIP {
        return general_fail("StartGame", "GameStartingFail");
    }
    mission.state = party_state::STARTING;
    let mut packets = mission.broadcast()?;
    let practice = mission.option.is_practice_mode;
    let built = contents
        .sector_sequence(
            &mission.digest,
            mission.option.difficulty,
            practice,
            false,
            random,
        )
        .and_then(|sequence| {
            Ok((
                contents.info(&mission.quest, &sequence, practice)?,
                sequence,
            ))
        });
    let Ok((infos, sequence)) = built else {
        // QueryMicroPlayInfo failure: LeaveParty and GeneralFail.
        packets.extend(general_fail("StartGame", "QueryMicroPlayInfoFail")?);
        return Ok(packets);
    };
    let granted = game_start_granted(&mission.digest, &infos, mission.option.difficulty, true, 1);
    mission.sectors = Some(infos);
    mission.sequence = Some(sequence);
    packets.push(packet(&Message::GameStartGranted(granted))?);
    mission.state = party_state::IN_GAME;
    packets.extend(mission.broadcast()?);
    Ok(packets)
}

/// MicroPlayHost MoveToNextSectorMessage → MoveSectorProcessor (AntiSectorSkip off):
/// the first move starts the play (InGame, CurGameTime = InitGameTime); later moves
/// advance the game time by SectorMoveGameTime modulo 24. Nothing reaches the client;
/// a refused move keeps the state.
pub fn move_sector(
    mission: &mut Mission,
    contents: &MissionContents,
    trigger: &str,
    target_group: i32,
) -> Result<bool, MinimumError> {
    let Some(sequence) = mission.sequence.as_mut() else {
        return Ok(false);
    };
    let first = sequence.current.is_none();
    let passed = sequence.passed.len();
    let game_time = if first {
        mission.digest.init_game_time
    } else {
        let t = mission.game_time + mission.digest.sector_move_game_time;
        if t >= 24 { t - 24 } else { t }
    };
    let moved = contents
        .move_next(&mission.quest, sequence, trigger, target_group)
        .map_err(|_| fail())?;
    let explored = sequence.passed.len() > passed;
    let sector = sequence.passed.last().copied();
    if !moved {
        return Ok(false);
    }
    if mission.in_game {
        // Not the Wait state: SectorCleared, Alive and PlayState.InGame again.
        mission.alive = true;
        mission.completion.finished = false;
    } else {
        super::minimum_mission_finish::start(mission, contents, std::time::Instant::now())?;
    }
    mission.in_game = true;
    mission.game_time = game_time;
    // OnMoveSector: a newly passed sector of a season 2 quest is explored.
    if explored
        && mission.digest.is_season2
        && let Some(id) = sector
    {
        mission.events.push((-1, format!("explore_sector {id}")));
    }
    Ok(true)
}

/// MonsterKilledProcessor (MDRSystem anti-cheat deferred to the damage reports) and
/// Monster.MonsterKilled: dead count, MonsterKillExpCalculator, the "kill <type> <action>"
/// informer event; unless the action is "no_item", the boss list or sector kill count and,
/// for an item drop entity outside practice, MakeEvilCore with a DropGoldCore announcement
/// of the lucky cores to the host.
#[allow(clippy::too_many_arguments)]
pub fn monster_killed(
    mission: &mut Mission,
    contents: &MissionContents,
    attacker: i32,
    target: i32,
    action: &str,
    has_evil_core: bool,
    exp_rate_monster: i32,
    random: &mut dyn MissionRandom,
) -> Result<(domain::Kill, Vec<OutgoingPacket>), MinimumError> {
    let killer = if attacker == mission.member.slot {
        mission.completion.cid
    } else {
        -1
    };
    if target < 0 {
        return Ok((domain::Kill::Unknown, Vec::new()));
    }
    let difficulty = mission.option.difficulty;
    let practice = mission.option.is_practice_mode;
    let Some(sequence) = mission.sequence.as_mut() else {
        return Ok((domain::Kill::NotInSector, Vec::new()));
    };
    let kill = contents.kill(sequence, target);
    let mut packets = Vec::new();
    if let domain::Kill::Killed {
        monster_type,
        is_boss,
        spawn_count,
        all_dead,
        key,
        drop_item,
        info,
        ..
    } = &kill
    {
        if let Some(info) = info {
            mission.monster_kill_exp =
                mission
                    .monster_kill_exp
                    .saturating_add(domain::monster_kill_exp(
                        *info,
                        *spawn_count,
                        exp_rate_monster,
                    ));
        }
        mission
            .events
            .push((killer, format!("kill {monster_type} {action}")));
        if action.to_lowercase() == "no_item" {
            return Ok((kill, packets));
        }
        if *is_boss {
            // UpdateQuestPlayCount (no DirectCollectCoin) outside practice, then the
            // boss lists.
            if !practice {
                mission.completion.play_count_due = true;
            }
            mission.boss_killed.push(monster_type.clone());
            match mission
                .completion
                .boss_kills
                .iter_mut()
                .find(|(m, _)| m == monster_type)
            {
                Some((_, n)) => *n = n.saturating_add(1),
                None => mission
                    .completion
                    .boss_kills
                    .push((monster_type.clone(), 1)),
            }
        }
        if has_evil_core && !practice {
            let additional = contents
                .make_evil_core(
                    sequence,
                    key,
                    *drop_item,
                    *info,
                    *is_boss,
                    &mission.quest,
                    difficulty,
                    mission.luck,
                    mission.base_character,
                    (&mut mission.story_drops, monster_type),
                    random,
                )
                .map_err(|_| fail())?;
            if !additional.is_empty() {
                packets.push(packet(&Message::DropGoldCore(DropGoldCore {
                    monster_entity_name: Some(key.clone()),
                    cores: Some(
                        additional
                            .iter()
                            .map(|(kind, rare)| {
                                Some(EvilCoreInfo {
                                    entity_name: Some(key.clone()),
                                    core_type: *kind,
                                    winner: mission.member.slot,
                                    rare_core_tags: Some(if *rare {
                                        vec![mission.member.slot]
                                    } else {
                                        Vec::new()
                                    }),
                                })
                            })
                            .collect(),
                    ),
                    drop_immediately: 0,
                }))?);
            }
            if !all_dead {
                contents.clear_drop(sequence, target);
            }
        }
    }
    Ok((kill, packets))
}

/// RagdollKickedProcessor → EvilCore.Loot for the solo host: the looted ItemClassEX, or
/// None when there is nothing to loot (KickFail, logged only).
pub fn ragdoll_kicked(
    mission: &mut Mission,
    contents: &MissionContents,
    tag: i32,
    key: &str,
    core_type: i32,
    rare: bool,
) -> Option<String> {
    if tag != mission.member.slot {
        return None;
    }
    let sequence = mission.sequence.as_mut()?;
    contents.loot(sequence, key, core_type, rare).ok().flatten()
}

/// MicroPlayer.GiveItemReward: GiveItem (count 1) then the GetItemMessage broadcast and
/// the "get item <class> <count>" informer event. Attributes the give planner does not
/// carry and full inventories (ForceMail) are refused.
#[allow(clippy::too_many_arguments)]
pub async fn give_reward(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    mission: &mut Mission,
    cid: i64,
    class_ex: &str,
    count: i32,
    core_type: i32,
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    mission
        .events
        .push((cid, format!("get item {class_ex} {count}")));
    // GiveItem(Dictionary<ItemClassEx, int>): ItemClassExBuilder.Parse per key.
    let (class, attributes) =
        crate::domain::item_attribute::parse_class_ex(class_ex).map_err(|_| fail())?;
    let key = utf16(&class);
    let request = [super::minimum_give::Request {
        class: &key,
        count,
        bound: false,
        equipment: None,
        attributes: &attributes,
    }];
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let applied = super::minimum_give::apply(&mut tx, owner, assets, cid, &[], &request, ids)
        .await?
        .map_err(|_| fail())?;
    tx.commit().await?;
    Ok(vec![
        inventory_packet(applied.slots)?,
        packet(&Message::GetItem(GetItem {
            player_name: mission.member.character_id.clone(),
            item_class: Some(class_ex.to_owned()),
            count,
            core_type,
            lucky: false,
            luck_bonus: 0,
            give_item_result: 0,
        }))?,
    ])
}

pub(super) fn inventory_packet(
    slots: Vec<Option<crate::protocol::inventory::Slot>>,
) -> Result<OutgoingPacket, MinimumError> {
    use crate::protocol::inventory::{self as inv, Kind as InvKind, Message as InvMessage};
    Ok(OutgoingPacket {
        category: InvKind::UpdateInventoryInfo.category(),
        body: inv::encode(
            InvKind::UpdateInventoryInfo,
            Some(&InvMessage::UpdateInventoryInfo(Some(slots))),
            crate::protocol::DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    })
}

/// PlayerKilledProcessor for the solo host (slot 0): Alive false and "player_dead".
pub fn player_killed(mission: &mut Mission, tag: i32) -> bool {
    if tag != mission.member.slot || !mission.alive {
        return false;
    }
    mission.alive = false;
    mission
        .events
        .push((mission.completion.cid, "player_dead".to_owned()));
    true
}

/// ReportSectorPropsProcessor → SectorInstance.RegisterPropList.
pub fn sector_props(
    mission: &mut Mission,
    contents: &MissionContents,
    props: &[(i32, i32)],
    random: &mut dyn MissionRandom,
) -> Result<(), MinimumError> {
    let practice = mission.option.is_practice_mode;
    let Some(sequence) = mission.sequence.as_mut() else {
        return Ok(());
    };
    contents
        .register_props(&mission.quest, sequence, props, practice, random)
        .map_err(|_| fail())
}

/// PropBrokenProcessor: Prop.BreakProp ("prop<BrokenProp>") then NamedProp.BreakNamedProp
/// (EntityName), each with its "destroyprop"/"destroynamedprop" event and its DropErg
/// announcement to the host.
pub fn prop_broken(
    mission: &mut Mission,
    contents: &MissionContents,
    prop: i32,
    entity_name: Option<&str>,
    attacker: i32,
    random: &mut dyn MissionRandom,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let practice = mission.option.is_practice_mode;
    let difficulty = mission.option.difficulty;
    let slot = mission.member.slot;
    let Some(sequence) = mission.sequence.as_mut() else {
        return Ok(Vec::new());
    };
    // The broken prop's material group, before it is removed.
    let group = sequence.current.and_then(|g| {
        sequence
            .sectors
            .iter()
            .find(|s| s.group_id == g)
            .and_then(|s| s.props.iter().find(|p| p.id == prop))
            .and_then(|p| p.group.clone())
    });
    let mut packets = Vec::new();
    if let Some(ergs) = contents.break_prop(&mission.quest, sequence, prop, slot, practice, random)
    {
        if let Some(group) = group
            && attacker == slot
        {
            mission
                .events
                .push((mission.completion.cid, format!("destroyprop {group}")));
        }
        packets.push(drop_erg(prop, ergs)?);
    }
    let Some(name) = entity_name else {
        return Ok(packets);
    };
    let Some(sequence) = mission.sequence.as_mut() else {
        return Ok(packets);
    };
    // The DropTable of the named prop (its map name before the uniqueness suffix).
    let drop_table = sequence.current.and_then(|g| {
        sequence
            .sectors
            .iter()
            .find(|s| s.group_id == g)
            .and_then(|s| s.objects.iter().find(|(t, _)| *t == "NamedProp"))
            .and_then(|(_, list)| list.iter().find(|(k, _)| k.as_deref() == Some(name)))
            .and_then(|(_, o)| match o {
                domain::SectorObject::NamedProp { drop_table, .. } => Some(drop_table.clone()),
                _ => None,
            })
    });
    if let Some(ergs) = contents.break_named_prop(sequence, name, slot, difficulty, random) {
        if let Some(table) = drop_table
            && attacker == slot
        {
            mission
                .events
                .push((mission.completion.cid, format!("destroynamedprop {table}")));
        }
        packets.push(drop_erg(prop, ergs)?);
    }
    Ok(packets)
}

/// DropErgMessage(BrokenProp, ergs) for the host.
fn drop_erg(prop: i32, ergs: Vec<domain::ErgDrop>) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::DropErg(DropErg {
        broken_prop: prop,
        monster_entity_name: None,
        ergs: Some(
            ergs.into_iter()
                .map(|e| {
                    Some(Erg {
                        erg_id: e.id,
                        erg_class: Some(e.class),
                        erg_type: Some(e.kind),
                        amount: e.amount,
                        winner: e.winner,
                    })
                })
                .collect(),
        ),
    }))
}

/// PickErgProcessor → Erg.PickErg: the picked erg, to give (GiveItemReward with
/// QuestErg) and answer with PutErg.
pub fn pick_erg(
    mission: &mut Mission,
    contents: &MissionContents,
    erg_id: i32,
) -> Option<domain::ErgDrop> {
    let slot = mission.member.slot;
    let sequence = mission.sequence.as_mut()?;
    contents.pick_erg(sequence, erg_id, slot)
}

/// PutErgMessage of a picked erg.
pub fn put_erg(erg_id: i32, tag: i32) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::PutErg(PutErg { erg_id, tag }))
}

/// An effect for a player slot: (slot, (MicroPlayEffectType, Amount, Argument)).
type TargetedEffect = (i32, (i32, i32, Option<String>));
/// The class to destroy (unless free) and the effect packets of a consumable use.
pub type ConsumableUse = (Option<String>, Vec<OutgoingPacket>);

/// MicroPlay.ApplyMicroPlayEffect for the solo player: Self effects go to the caster,
/// TargetDead/TargetAlive to the target by its Alive state (party targets select other
/// players, none here), None effects run on the server (ReduceMinClearTime). Revives set
/// Alive and inform "reviving"/"revived"; the list reaches the host as
/// ApplyMicroPlayEffects.
pub fn apply_effect(
    mission: &mut Mission,
    contents: &MissionContents,
    caster: i32,
    target: i32,
    source: &str,
    operation: i32,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let slot = mission.member.slot;
    let mut list: Vec<TargetedEffect> = Vec::new();
    for element in contents.effects(source) {
        if element.operation != operation {
            continue;
        }
        let add = |list: &mut Vec<TargetedEffect>, who: i32| {
            for e in &element.effects {
                list.push((who, e.clone()));
            }
        };
        match element.target {
            0 => {
                for (effect, amount, _) in &element.effects {
                    if *effect == 16 {
                        mission.min_clear_time_reduced += amount;
                    }
                }
            }
            1 if caster == slot => add(&mut list, caster),
            2 if target == slot && !mission.alive => add(&mut list, target),
            3 if target == slot && mission.alive => add(&mut list, target),
            _ => {}
        }
    }
    if list.is_empty() {
        return Ok(Vec::new());
    }
    for (who, (effect, _, _)) in &list {
        if matches!(effect, 1 | 2) && *who == slot {
            let cid = mission.completion.cid;
            mission.events.push((cid, "reviving".to_owned()));
            mission.events.push((cid, "revived".to_owned()));
            mission.alive = true;
        }
    }
    let mut packets = vec![packet(&Message::ApplyMicroPlayEffects(
        wire::ApplyMicroPlayEffects {
            caster,
            effect_name: Some(source.to_owned()),
            effects: Some(
                list.into_iter()
                    .map(|(who, (effect, amount, argument))| {
                        Some(wire::MicroPlayEffect {
                            player_slot_no: who,
                            effect,
                            amount,
                            argument,
                        })
                    })
                    .collect(),
            ),
        },
    ))?];
    if matches!(source, "cash_revive_party" | "cash_revive_party_exchange") {
        let name = mission.member.character_id.clone().unwrap_or_default();
        packets.push(super::minimum_shop::system_message(
            2,
            "GameUI_Heroes_PartyProtection_Done",
            &[name],
        )?);
    }
    Ok(packets)
}

/// UseConsumableProcessor for the solo host: BattleInventory.UseConsumable (no inner
/// slots), the UseConsumable effects, then the item to destroy unless the quest is
/// ConsumableFree or the consumable is of infinite use (DraftNum 0). Returns the class to
/// destroy and the effect packets; `None` when the use is refused.
pub fn use_consumable(
    mission: &mut Mission,
    contents: &MissionContents,
    tag: i32,
    part: i32,
    slot: i32,
) -> Result<Option<ConsumableUse>, MinimumError> {
    if tag != mission.member.slot || slot != 0 {
        return Ok(None);
    }
    let Some(row) = mission.consumables.iter_mut().find(|r| r.0 == part) else {
        return Ok(None);
    };
    // ConsumablesInfo.Used(1).
    let (bring, used) = (row.2, row.3);
    if bring < 1 || bring - 1 < used {
        row.3 = bring;
        return Ok(None);
    }
    row.3 += 1;
    let (class, draft) = (row.1.clone(), row.4);
    let packets = apply_effect(mission, contents, tag, 0, &class, 1)?;
    let free = mission.settings.iter().any(|(k, _)| k == "ConsumableFree") || draft == 0;
    mission.events.push((
        mission.completion.cid,
        format!(
            "use consumable {} {class} 1",
            if part >= 100 {
                "QuickSlot"
            } else {
                "SubWeapon"
            }
        ),
    ));
    Ok(Some(((!free).then_some(class), packets)))
}

/// MicroPlayer.HoldingItems[class]: the count the character holds.
async fn held(pool: &PgPool, cid: i64, class: &str) -> Result<i64, MinimumError> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(SUM(count),0)::BIGINT FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2",
    )
    .bind(cid)
    .bind(utf16(class))
    .fetch_one(pool)
    .await?)
}

/// CheckCoinPaid for the reserved required item: DestroyItem(1, QuestToken) and its
/// inventory update; `Ok(Err(packets))` is the StartGame failure (ITEM_REQUIRED or
/// CANNOT_REMOVE_COIN → GameStartingFailed, LeaveParty, CheckCoinPaidFail).
pub async fn pay_required_item(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    mission: &mut Mission,
) -> Result<Result<Vec<OutgoingPacket>, Vec<OutgoingPacket>>, MinimumError> {
    if mission.state != party_state::IN_SHIP {
        return Ok(Ok(Vec::new()));
    }
    let Some(class) = mission.reserved_item.take() else {
        return Ok(Ok(Vec::new()));
    };
    if held(pool, cid, &class).await? <= 0 {
        return Ok(Err(general_fail("StartGame", "CheckCoinPaidFail")?));
    }
    match destroy_consumable(pool, owner, assets, cid, &class).await {
        Ok(packets) => Ok(Ok(packets)),
        Err(_) => Ok(Err(general_fail("StartGame", "CheckCoinPaidFail")?)),
    }
}

/// DestroyItem of a used consumable (one item), with its inventory update.
pub async fn destroy_consumable(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    class: &str,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let key = utf16(class);
    let request = [super::minimum_give::Request {
        class: &key,
        count: 1,
        bound: false,
        equipment: None,
        attributes: &crate::domain::item_attribute::EMPTY,
    }];
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let applied =
        super::minimum_give::apply(&mut tx, owner, assets, cid, &request, &[], &mut || {
            Err(fail())
        })
        .await?
        .map_err(|_| fail())?;
    tx.commit().await?;
    Ok(vec![inventory_packet(applied.slots)?])
}

/// StoryEntity.QueryStoryDropData for the mission: the character's story progress phases
/// and the inventory count of each candidate class. StoryDrop.TryCount is not persisted
/// yet, so every mission starts at zero tries (rows with MaxTry 1 are unaffected).
async fn story_drop_data(
    pool: &PgPool,
    contents: &domain::MissionContents,
    cid: i64,
    quest: &str,
) -> Result<Vec<domain::StoryDropData>, MinimumError> {
    let rows: Vec<(Vec<u8>, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT story_line_utf16,phase_utf16 FROM game_story_progress WHERE cid=$1 ORDER BY row_id LIMIT 4096",
    )
    .bind(cid)
    .fetch_all(pool)
    .await?;
    let text = |b: &[u8]| -> Result<String, MinimumError> {
        let units: Vec<u16> = b
            .as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect();
        String::from_utf16(&units).map_err(|_| fail())
    };
    let mut progress = Vec::with_capacity(rows.len());
    for (line, phase) in rows {
        progress.push((text(&line)?, phase.as_deref().map(text).transpose()?));
    }
    let tries = std::collections::BTreeMap::new();
    let candidates = contents.story_drop_data(&progress, quest, &|_| 0, &tries);
    let mut held = std::collections::BTreeMap::new();
    for d in &candidates {
        if held.contains_key(&d.item_class) {
            continue;
        }
        let key: Vec<u8> = d
            .item_class
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let count: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(count),0)::BIGINT FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2",
        )
        .bind(cid)
        .bind(key)
        .fetch_one(pool)
        .await?;
        held.insert(
            d.item_class.clone(),
            i32::try_from(count).unwrap_or(i32::MAX),
        );
    }
    Ok(contents.story_drop_data(
        &progress,
        quest,
        &|class| held.get(class).copied().unwrap_or(0),
        &tries,
    ))
}
