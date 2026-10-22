//! Mission entry messages recovered from the Framework serializer: AcceptQuest, party and
//! ship state, the relay ConnectionRequest, LaunchShipGranted/GameJoinMemberInfo,
//! RegisterServer, StartGame, GameStartGranted/QuestSectorInfos and MemberInfo.
use super::{
    character_list::{self, CharacterListError, Costume},
    inventory::{Dict, SkillEnhance, Wire, object},
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;
const MAX_ROWS: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    AcceptQuest,
    PartyInfo,
    UpdateShip,
    ConnectionRequest,
    LaunchShipGranted,
    RegisterServer,
    StartGame,
    GameStartGranted,
    MemberInfo,
    PayCoinCompleted,
    InsertBlessStoneComplete,
    MoveToNextSector,
    AllUserJoinComplete,
}
impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::AcceptQuest => 224,
            Self::PartyInfo => 511,
            Self::UpdateShip => 618,
            Self::ConnectionRequest => 425,
            Self::LaunchShipGranted => 473,
            Self::RegisterServer => 554,
            Self::StartGame => 591,
            Self::GameStartGranted => 462,
            Self::MemberInfo => 498,
            Self::PayCoinCompleted => 432,
            Self::InsertBlessStoneComplete => 48,
            Self::MoveToNextSector => 532,
            Self::AllUserJoinComplete => 47,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::AcceptQuest => 0xaecbcd0a_82b3_31ee_b771_fe9c53dabdd8,
            Self::PartyInfo => 0x78d2aa77_13f9_302f_b9fe_67ca096b92f3,
            Self::UpdateShip => 0xc87e7f92_c8c2_36b1_83e8_bc9fbd04e5d4,
            Self::ConnectionRequest => 0x31fc12c3_d398_355f_a15c_786e69ccc95f,
            Self::LaunchShipGranted => 0x78df701a_83a3_33f5_be94_8d27ecb6e23b,
            Self::RegisterServer => 0xb8a9e8c0_377b_35ad_b3be_bb911152a565,
            Self::StartGame => 0x7c04c215_b83c_3053_b80c_966064258660,
            Self::GameStartGranted => 0xdc3acd0b_6946_3b6c_bcaf_29057e0dfc59,
            Self::MemberInfo => 0xb89c49cb_0aa1_308d_a0c5_9fe9bb1f58ed,
            Self::PayCoinCompleted => 0x7f105751_b6a7_3c1d_a7fd_bc7a884133d2,
            Self::InsertBlessStoneComplete => 0x915f8913_1565_340c_99ca_619e37ffd1f7,
            Self::MoveToNextSector => 0x67aefac8_659d_385a_a39a_03e3d1dce102,
            Self::AllUserJoinComplete => 0x3f79e7b8_49d5_34f2_8970_b8b5b47aa1eb,
        })
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MissionWireError {
    #[error("mission message kind mismatch")]
    Kind,
    #[error("unsupported mission category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
/// IDictionary<string, T>: count, then non-null unique keys and values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StrDict<T>(pub Option<Vec<(String, T)>>);
impl<T: Wire> Wire for StrDict<T> {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        let Some(rows) = &self.0 else {
            return w.append(&[0]);
        };
        if rows.len() > MAX_ROWS {
            return Err(PayloadError::TooLarge);
        }
        w.append(&super::encode_integer((rows.len() + 1) as i32))?;
        for (i, (k, v)) in rows.iter().enumerate() {
            if rows[..i].iter().any(|(other, _)| other == k) {
                return Err(PayloadError::DuplicateKey);
            }
            Some(k.clone()).write(w)?;
            v.write(w)?;
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        let Some(n) = r.count()? else {
            return Ok(Self(None));
        };
        if n > MAX_ROWS || n > r.remaining.len() {
            return Err(PayloadError::TooLarge);
        }
        let mut rows: Vec<(String, T)> = Vec::with_capacity(n);
        for _ in 0..n {
            let k = Option::<String>::read(r)?.ok_or(PayloadError::NullKey)?;
            if rows.iter().any(|(other, _)| *other == k) {
                return Err(PayloadError::DuplicateKey);
            }
            rows.push((k, T::read(r)?));
        }
        Ok(Self(Some(rows)))
    }
}
impl Wire for u16 {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&self.to_be_bytes())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(u16::from_be_bytes(r.fixed::<2>()?))
    }
}
/// CostumeInfo, shared with the character list codec.
#[derive(Clone, PartialEq, Eq)]
pub struct CostumeField(pub Option<Costume>);
impl Wire for CostumeField {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        character_list::write_costume(w, self.0.as_ref()).map_err(payload)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        character_list::read_costume(r).map(Self).map_err(payload)
    }
}
fn payload(e: CharacterListError) -> PayloadError {
    match e {
        CharacterListError::Payload(e) => e,
        CharacterListError::DuplicateKey => PayloadError::DuplicateKey,
        CharacterListError::Pet | CharacterListError::Type => PayloadError::Unsupported,
    }
}
/// PetStatusInfo: only null is supported (no pet system is migrated).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoPet;
impl Wire for NoPet {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&[0])
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        match r.byte()? {
            0 => Ok(Self),
            _ => Err(PayloadError::Unsupported),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShipOption {
    pub max_member_count: i32,
    pub swear_member_limit: i32,
    pub until_force_start: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub is_party_only: i32,
    pub over18_only: i32,
    pub difficulty: i32,
    pub is_season2: bool,
    pub selected_boss_quest_ids: Option<Vec<i32>>,
    pub is_practice_mode: bool,
    pub user_ds_mode: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptQuest {
    pub quest_id: Option<String>,
    pub title: Option<String>,
    pub swear_id: i32,
    pub option: Option<ShipOption>,
}
/// PartyMemberInfo; `character` is BaseCharacter and `state` ReadyState (byte enums).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartyMember {
    pub nexon_sn: i32,
    pub character: u8,
    pub character_id: Option<String>,
    pub slot: i32,
    pub level: i32,
    pub state: u8,
    pub latest_ping: i32,
    pub latest_frame_rate: i32,
    pub is_return: bool,
    pub is_event_jumping: bool,
}
/// PartyInfoMessage; `state` is PartyInfoState.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartyInfo {
    pub party_id: i64,
    pub party_size: i32,
    pub state: u8,
    pub members: Option<Vec<Option<PartyMember>>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ship {
    pub party_id: i64,
    pub ship_name: Option<String>,
    pub password: Option<String>,
    pub min_level: i32,
    pub max_level: i32,
    pub member_count: i32,
    pub max_ship_member_count: i32,
    pub quest_id: Option<String>,
    pub is_hunting_quest: bool,
    pub is_giant_raid: bool,
    pub swear_id: i32,
    pub rest_time: i32,
    pub reinforce_allowed: bool,
    pub adult_rule: bool,
    pub members: Option<Vec<Option<PartyMember>>>,
    pub state: u8,
    pub party_bonus_count: i32,
    pub party_bonus_ratio: i32,
    pub is_party_only: bool,
    pub host_ping: i32,
    pub host_frame_rate: i32,
    pub difficulty: i32,
    pub is_return: bool,
    pub is_season2: bool,
    pub selected_boss_quest_ids: Option<Vec<i32>>,
    pub is_practice_mode: bool,
    pub user_ds_mode: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateShip {
    pub info: Option<Ship>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRequest {
    pub address: Option<String>,
    pub port: u16,
    pub posix_time: i32,
    pub key: i32,
    pub category: Option<String>,
    pub ping_host_cid: i64,
    pub group_id: i64,
}
/// ConsumablesInfo; `inner` holds the items of a container consumable (one level).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Consumable {
    pub item_class: Option<String>,
    pub bring: i32,
    pub used: i32,
    pub draft: i32,
    pub inner: Option<Vec<Option<InnerConsumable>>>,
}
/// A contained ConsumablesInfo; its own inner list is never filled by the original.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InnerConsumable {
    pub item_class: Option<String>,
    pub bring: i32,
    pub used: i32,
    pub draft: i32,
    pub inner: EmptyList,
}
/// A list that must be null or empty; `true` is the empty list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmptyList(pub bool);
impl Wire for EmptyList {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&[u8::from(self.0)])
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        match r.count()? {
            None => Ok(Self(false)),
            Some(0) => Ok(Self(true)),
            Some(_) => Err(PayloadError::Unsupported),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattleInventory {
    pub consumables: Dict<Option<Consumable>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusEffect {
    pub kind: Option<String>,
    pub level: i32,
    pub remain_time: i32,
    pub combat_count: i32,
}
/// GameJoinMemberInfo, built by MicroPlayer.MakeMemberInfo.
#[derive(Clone, PartialEq, Eq)]
pub struct JoinMember {
    pub order: i32,
    pub tag: i32,
    pub key: i32,
    pub name: Option<String>,
    pub base_class: u8,
    pub level: i32,
    pub exp: i32,
    pub level_up_exp: i32,
    pub title_id: i32,
    pub title_count: i32,
    pub stats: StrDict<i32>,
    pub battle_inventory: Option<BattleInventory>,
    pub costume: CostumeField,
    pub pet: NoPet,
    pub skill_list: StrDict<i32>,
    pub sp_skills: Dict<Option<String>>,
    pub vocation_skills: StrDict<i32>,
    pub transform_cool_down: i32,
    pub skill_enhances: StrDict<Option<SkillEnhance>>,
    pub def_map: Dict<i32>,
    pub armor_hp_map: Dict<i32>,
    pub equipped_items: Dict<Option<String>>,
    pub ability_list: Option<Vec<i32>>,
    pub status_effect_dict: StrDict<i32>,
    pub status_effects: Option<Vec<Option<StatusEffect>>>,
    pub destroyed_def: i32,
    pub is_teacher: bool,
    pub is_assist: bool,
    pub guild_level_bonus: StrDict<i32>,
    pub is_alive: bool,
}
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchShipGranted {
    pub quest_id: Option<String>,
    pub adult_rule: u8,
    pub is_practice_mode: u8,
    pub host_info: Option<JoinMember>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameInfo {
    pub name: Option<String>,
    pub map: Option<String>,
    pub game_dir: Option<String>,
    pub description: Option<String>,
    pub host_id: i32,
    pub ds_ip: Option<String>,
    pub ds_port: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterServer {
    pub info: Option<GameInfo>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorConnection {
    pub from: i32,
    pub from_trigger: Option<String>,
    pub to: i32,
    pub to_spawn: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrialFactor {
    pub group_number: i32,
    pub trial_mod: i32,
    pub trial_name: Option<String>,
    pub sector_group_id: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreasureBox {
    pub group_id: i32,
    pub entity_name: Option<String>,
    pub is_visible: bool,
    pub is_opened: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestSector {
    pub group_id: i32,
    pub bsp: Option<String>,
    pub entities: Option<Vec<Option<String>>>,
    pub connections: Option<Vec<Option<SectorConnection>>>,
    pub item_drop_entity: StrDict<i32>,
    pub weak_point_entity: StrDict<i32>,
    pub trial_factors: Option<Vec<Option<TrialFactor>>>,
    pub treasure_boxes: Option<Vec<Option<TreasureBox>>>,
    pub save_required: bool,
    pub last_used_spawn_point: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestSectors {
    pub current_group: i32,
    pub starting_group: i32,
    pub sectors: Option<Vec<Option<QuestSector>>>,
    pub treasure_box_max_count: i32,
    pub altar_status: Option<Vec<i32>>,
    pub boss_killed: Option<Vec<Option<String>>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameStartGranted {
    pub quest_level: i32,
    pub quest_time: i32,
    pub hard_mode: i32,
    pub solo_sector: i32,
    pub sectors: Option<QuestSectors>,
    pub is_hunting_quest: bool,
    pub init_game_time: i32,
    pub sector_move_game_time: i32,
    pub difficulty: i32,
    pub is_timer_decreasing: bool,
    pub quest_started_player_count: i32,
    pub new_slot: i32,
    pub new_key: i32,
    pub is_user_dedicated: bool,
}
#[derive(Clone, PartialEq, Eq)]
pub struct MemberInfo {
    pub uid: i32,
    pub info: Option<JoinMember>,
}
/// PaidCoinInfo; `platinum_coin_type` is CoinType (int enum).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaidCoin {
    pub slot: i32,
    pub silver_coin: Option<Vec<i32>>,
    pub platinum_coin_owner: i32,
    pub platinum_coin_type: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PayCoinCompleted {
    pub coins: Option<Vec<Option<PaidCoin>>>,
}
/// InsertBlessStoneCompleteMessage; `types` are BlessStoneType values (int enum).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlessStones {
    pub slot: i32,
    pub owners: Option<Vec<i32>>,
    pub types: Option<Vec<i32>>,
}
/// MoveToNextSectorMessage (host): the trigger used and the target sector group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveToNextSector {
    pub trigger_name: Option<String>,
    pub target_group: i32,
    pub holy_props: Option<Vec<i32>>,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Message {
    AcceptQuest(AcceptQuest),
    PartyInfo(PartyInfo),
    UpdateShip(UpdateShip),
    ConnectionRequest(ConnectionRequest),
    LaunchShipGranted(Box<LaunchShipGranted>),
    RegisterServer(RegisterServer),
    StartGame,
    GameStartGranted(GameStartGranted),
    MemberInfo(Box<MemberInfo>),
    PayCoinCompleted(PayCoinCompleted),
    InsertBlessStoneComplete(BlessStones),
    MoveToNextSector(MoveToNextSector),
    AllUserJoinComplete,
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::AcceptQuest(_) => Kind::AcceptQuest,
            Self::PartyInfo(_) => Kind::PartyInfo,
            Self::UpdateShip(_) => Kind::UpdateShip,
            Self::ConnectionRequest(_) => Kind::ConnectionRequest,
            Self::LaunchShipGranted(_) => Kind::LaunchShipGranted,
            Self::RegisterServer(_) => Kind::RegisterServer,
            Self::StartGame => Kind::StartGame,
            Self::GameStartGranted(_) => Kind::GameStartGranted,
            Self::MemberInfo(_) => Kind::MemberInfo,
            Self::PayCoinCompleted(_) => Kind::PayCoinCompleted,
            Self::InsertBlessStoneComplete(_) => Kind::InsertBlessStoneComplete,
            Self::MoveToNextSector(_) => Kind::MoveToNextSector,
            Self::AllUserJoinComplete => Kind::AllUserJoinComplete,
        }
    }
}
object!(ShipOption {
    max_member_count,
    swear_member_limit,
    until_force_start,
    min_level,
    max_level,
    is_party_only,
    over18_only,
    difficulty,
    is_season2,
    selected_boss_quest_ids,
    is_practice_mode,
    user_ds_mode
});
object!(AcceptQuest {
    quest_id,
    title,
    swear_id,
    option
});
object!(PartyMember {
    nexon_sn,
    character,
    character_id,
    slot,
    level,
    state,
    latest_ping,
    latest_frame_rate,
    is_return,
    is_event_jumping
});
object!(PartyInfo {
    party_id,
    party_size,
    state,
    members
});
object!(Ship {
    party_id,
    ship_name,
    password,
    min_level,
    max_level,
    member_count,
    max_ship_member_count,
    quest_id,
    is_hunting_quest,
    is_giant_raid,
    swear_id,
    rest_time,
    reinforce_allowed,
    adult_rule,
    members,
    state,
    party_bonus_count,
    party_bonus_ratio,
    is_party_only,
    host_ping,
    host_frame_rate,
    difficulty,
    is_return,
    is_season2,
    selected_boss_quest_ids,
    is_practice_mode,
    user_ds_mode
});
object!(UpdateShip { info });
object!(ConnectionRequest {
    address,
    port,
    posix_time,
    key,
    category,
    ping_host_cid,
    group_id
});
object!(Consumable {
    item_class,
    bring,
    used,
    draft,
    inner
});
object!(InnerConsumable {
    item_class,
    bring,
    used,
    draft,
    inner
});
object!(BattleInventory { consumables });
object!(StatusEffect {
    kind,
    level,
    remain_time,
    combat_count
});
object!(JoinMember {
    order,
    tag,
    key,
    name,
    base_class,
    level,
    exp,
    level_up_exp,
    title_id,
    title_count,
    stats,
    battle_inventory,
    costume,
    pet,
    skill_list,
    sp_skills,
    vocation_skills,
    transform_cool_down,
    skill_enhances,
    def_map,
    armor_hp_map,
    equipped_items,
    ability_list,
    status_effect_dict,
    status_effects,
    destroyed_def,
    is_teacher,
    is_assist,
    guild_level_bonus,
    is_alive
});
object!(LaunchShipGranted {
    quest_id,
    adult_rule,
    is_practice_mode,
    host_info
});
object!(GameInfo {
    name,
    map,
    game_dir,
    description,
    host_id,
    ds_ip,
    ds_port
});
object!(RegisterServer { info });
object!(SectorConnection {
    from,
    from_trigger,
    to,
    to_spawn
});
object!(TrialFactor {
    group_number,
    trial_mod,
    trial_name,
    sector_group_id
});
object!(TreasureBox {
    group_id,
    entity_name,
    is_visible,
    is_opened
});
object!(QuestSector {
    group_id,
    bsp,
    entities,
    connections,
    item_drop_entity,
    weak_point_entity,
    trial_factors,
    treasure_boxes,
    save_required,
    last_used_spawn_point
});
object!(QuestSectors {
    current_group,
    starting_group,
    sectors,
    treasure_box_max_count,
    altar_status,
    boss_killed
});
object!(GameStartGranted {
    quest_level,
    quest_time,
    hard_mode,
    solo_sector,
    sectors,
    is_hunting_quest,
    init_game_time,
    sector_move_game_time,
    difficulty,
    is_timer_decreasing,
    quest_started_player_count,
    new_slot,
    new_key,
    is_user_dedicated
});
object!(MemberInfo { uid, info });
object!(PaidCoin {
    slot,
    silver_coin,
    platinum_coin_owner,
    platinum_coin_type
});
object!(PayCoinCompleted { coins });
object!(MoveToNextSector {
    trigger_name,
    target_group,
    holy_props
});
object!(BlessStones {
    slot,
    owners,
    types
});
pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, MissionWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    match message {
        None => w.append(&[0])?,
        Some(m) => {
            if kind != m.kind() {
                return Err(MissionWireError::Kind);
            }
            w.append(&[1])?;
            match m {
                Message::AcceptQuest(v) => v.write(&mut w)?,
                Message::PartyInfo(v) => v.write(&mut w)?,
                Message::UpdateShip(v) => v.write(&mut w)?,
                Message::ConnectionRequest(v) => v.write(&mut w)?,
                Message::LaunchShipGranted(v) => v.write(&mut w)?,
                Message::RegisterServer(v) => v.write(&mut w)?,
                Message::StartGame => {}
                Message::GameStartGranted(v) => v.write(&mut w)?,
                Message::MemberInfo(v) => v.write(&mut w)?,
                Message::PayCoinCompleted(v) => v.write(&mut w)?,
                Message::InsertBlessStoneComplete(v) => v.write(&mut w)?,
                Message::MoveToNextSector(v) => v.write(&mut w)?,
                Message::AllUserJoinComplete => {}
            }
        }
    }
    Ok(Zeroizing::new(w.bytes))
}
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, MissionWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::AcceptQuest => Message::AcceptQuest(AcceptQuest::read(&mut r)?),
            Kind::PartyInfo => Message::PartyInfo(PartyInfo::read(&mut r)?),
            Kind::UpdateShip => Message::UpdateShip(UpdateShip::read(&mut r)?),
            Kind::ConnectionRequest => Message::ConnectionRequest(ConnectionRequest::read(&mut r)?),
            Kind::LaunchShipGranted => {
                Message::LaunchShipGranted(Box::new(LaunchShipGranted::read(&mut r)?))
            }
            Kind::RegisterServer => Message::RegisterServer(RegisterServer::read(&mut r)?),
            Kind::StartGame => Message::StartGame,
            Kind::GameStartGranted => Message::GameStartGranted(GameStartGranted::read(&mut r)?),
            Kind::MemberInfo => Message::MemberInfo(Box::new(MemberInfo::read(&mut r)?)),
            Kind::PayCoinCompleted => Message::PayCoinCompleted(PayCoinCompleted::read(&mut r)?),
            Kind::InsertBlessStoneComplete => {
                Message::InsertBlessStoneComplete(BlessStones::read(&mut r)?)
            }
            Kind::MoveToNextSector => Message::MoveToNextSector(MoveToNextSector::read(&mut r)?),
            Kind::AllUserJoinComplete => Message::AllUserJoinComplete,
        })
    };
    if !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}
/// Registered category, or category 0 with the type GUID prefix (unregistered serializer).
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, MissionWireError> {
    if category == kind.category() {
        return decode(kind, body, max_body);
    }
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode(kind, &body[16..], max_body.saturating_sub(16));
    }
    Err(MissionWireError::Type)
}
