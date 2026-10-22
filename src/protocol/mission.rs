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
    MonsterKilled,
    PlayerKilled,
    RagdollKicked,
    DropGoldCore,
    GetItem,
    SectorPropList,
    PropBroken,
    DropErg,
    PickErg,
    PutErg,
    MonsterDamageReport,
    CombatRecord,
    UseConsumable,
    PlayerRevived,
    ApplyMicroPlayEffects,
    MicroPlayEvent,
    QuestTargetResult,
    QuestSuccessSceneStart,
    FinishGame,
    ReturnToTown,
    ReturnFromQuest,
    Kicked,
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
            Self::MonsterKilled => 501,
            Self::PlayerKilled => 522,
            Self::RagdollKicked => 552,
            Self::DropGoldCore => 350,
            Self::GetItem => 463,
            Self::SectorPropList => 563,
            Self::PropBroken => 524,
            Self::DropErg => 455,
            Self::PickErg => 520,
            Self::PutErg => 525,
            Self::MonsterDamageReport => 290,
            Self::CombatRecord => 263,
            Self::UseConsumable => 624,
            Self::PlayerRevived => 555,
            Self::ApplyMicroPlayEffects => 403,
            Self::MicroPlayEvent => 362,
            Self::QuestTargetResult => 295,
            Self::QuestSuccessSceneStart => 550,
            Self::FinishGame => 460,
            Self::ReturnToTown => 445,
            Self::ReturnFromQuest => 402,
            Self::Kicked => 471,
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
            Self::MonsterKilled => 0x8cfddc5b_72e0_3490_bb44_1ecf1a813468,
            Self::PlayerKilled => 0x71d3965f_66a2_31f1_aa15_90d54c7c7882,
            Self::RagdollKicked => 0xe7ae9d95_805f_3478_b19e_b03c14234ee7,
            Self::DropGoldCore => 0xd01fb1ee_5a73_3dee_834a_84687c126f14,
            Self::GetItem => 0xcd70d18d_dc3b_3053_b9fe_6efbc76d28bb,
            Self::SectorPropList => 0x64e27db4_48c9_3507_a366_9bbb6b1b6a79,
            Self::PropBroken => 0x15af6960_d57a_3be7_820c_d8825a163a0b,
            Self::DropErg => 0x64b4f44b_be70_3e40_a441_a61ceaf133ea,
            Self::PickErg => 0x2e9ee02b_6439_3079_9834_932286dfbb4b,
            Self::PutErg => 0xd7f2e4f9_11e7_369c_a4dd_70d46756e0ad,
            Self::MonsterDamageReport => 0x62bd766f_9899_36d4_a6fc_0e4e448471c2,
            Self::CombatRecord => 0x1c979684_101a_325c_bf59_caca716604fb,
            Self::UseConsumable => 0x8cf9f82f_0433_31a7_b88d_d61b71ee61ee,
            Self::PlayerRevived => 0x0441624d_0d61_3f4d_8194_090df4655d18,
            Self::ApplyMicroPlayEffects => 0x10c709a9_28c3_328e_8dfa_6bb6f5ecc0c8,
            Self::MicroPlayEvent => 0x52a3dbeb_8337_3288_9005_5557bfa8e7e0,
            Self::QuestTargetResult => 0xbfed3fca_9934_374f_b39d_cebb0729e57f,
            Self::QuestSuccessSceneStart => 0x62db5582_deb3_34e7_98e2_3fd6692deca6,
            Self::FinishGame => 0x4bc1c4f0_07af_3449_9246_f7a5e68f76a2,
            Self::ReturnToTown => 0x702a1018_f1b5_3c8e_ad51_e46ef0846aed,
            Self::ReturnFromQuest => 0x6764d890_113e_3d4d_9d96_24c0087489c3,
            Self::Kicked => 0x4df74867_68cb_3431_8177_ce7549890ddd,
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
impl Wire for u32 {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&self.to_be_bytes())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(u32::from_be_bytes(r.fixed::<4>()?))
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
/// MonsterKilledMessage (host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonsterKilled {
    pub attacker: i32,
    pub target: i32,
    pub action_type: Option<String>,
    pub has_evil_core: bool,
    pub damage: i32,
    pub damage_position_x: i32,
    pub damage_position_y: i32,
    pub distance: i32,
    pub actor_index: i32,
}
/// RagdollKickedMessage (host): a player kicked a core; `core_type` is EvilCoreType.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RagdollKicked {
    pub tag: i32,
    pub target_entity_name: Option<String>,
    pub core_type: i32,
    pub is_rare_core: bool,
}
/// EvilCoreInfo; `core_type` is EvilCoreType.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvilCoreInfo {
    pub entity_name: Option<String>,
    pub core_type: i32,
    pub winner: i32,
    pub rare_core_tags: Option<Vec<i32>>,
}
/// DropGoldCoreMessage (to the host): extra cores of a monster or box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropGoldCore {
    pub monster_entity_name: Option<String>,
    pub cores: Option<Vec<Option<EvilCoreInfo>>>,
    pub drop_immediately: i32,
}
/// GetItemMessage: who got which item (core type and GiveItem.ResultEnum as int).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetItem {
    pub player_name: Option<String>,
    pub item_class: Option<String>,
    pub count: i32,
    pub core_type: i32,
    pub lucky: bool,
    pub luck_bonus: i32,
    pub give_item_result: i32,
}
/// SectorPropListMessage (host): prop ID → material ID of the current sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorPropList {
    pub props: Dict<i32>,
}
/// PropBrokenMessage (host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropBroken {
    pub prop: i32,
    pub entity_name: Option<String>,
    pub attacker: i32,
}
/// ErgInfo (all private fields, winner included).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Erg {
    pub erg_id: i32,
    pub erg_class: Option<String>,
    pub erg_type: Option<String>,
    pub amount: i32,
    pub winner: i32,
}
/// DropErgMessage (to the host): ergs out of a prop or a monster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropErg {
    pub broken_prop: i32,
    pub monster_entity_name: Option<String>,
    pub ergs: Option<Vec<Option<Erg>>>,
}
/// PutErgMessage: the erg and the tag of the player who picked it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PutErg {
    pub erg_id: i32,
    pub tag: i32,
}
/// MonsterTakeDamageInfo; `attack_time` keeps the IEEE single bits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakeDamage {
    pub attacker: i32,
    pub attack_time: u32,
    pub action_name: Option<String>,
    pub damage: i32,
}
/// MonsterDamageReportMessage (host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonsterDamageReport {
    pub target: i32,
    pub damages: Option<Vec<Option<TakeDamage>>>,
}
/// CombatRecordMessage (host): a player's battle record for the result screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CombatRecord {
    pub player_number: i32,
    pub combo_max: i32,
    pub hit_max: i32,
    pub style_max: i32,
    pub death: i32,
    pub kill: i32,
    pub battle_achieve: i32,
    pub hit_take: i32,
    pub style_count: i32,
    pub rank_style: i32,
    pub rank_battle: i32,
    pub rank_total: i32,
}
/// UseConsumableMessage (host): tag, quick slot part and inner slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UseConsumable {
    pub tag: i32,
    pub part: i32,
    pub slot: i32,
}
/// PlayerRevivedMessage (host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerRevived {
    pub caster_tag: i32,
    pub reviver_tag: i32,
    pub method: Option<String>,
}
/// MicroPlayEffect; `effect` is MicroPlayEffectType (int enum).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MicroPlayEffect {
    pub player_slot_no: i32,
    pub effect: i32,
    pub amount: i32,
    pub argument: Option<String>,
}
/// ApplyMicroPlayEffectsMessage (to the host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyMicroPlayEffects {
    pub caster: i32,
    pub effect_name: Option<String>,
    pub effects: Option<Vec<Option<MicroPlayEffect>>>,
}
/// MicroPlayEventMessage (host): a client quest event string for the goal trackers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MicroPlayEvent {
    pub slot: i32,
    pub event: Option<String>,
}
/// QuestTargetResultMessage. The original constructor never sets IsQuestSuccess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestTargetResult {
    pub character_name: Option<String>,
    pub goal_id: i32,
    pub is_goal_success: bool,
    pub is_quest_success: bool,
    pub exp: i32,
    pub gold: i32,
    pub ap: i32,
}
/// SummaryGoalInfo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SummaryGoal {
    pub goal_id: i32,
    pub exp: i32,
    pub gold: i32,
    pub ap: i32,
}
/// SummaryRewardInfo: a described reward line of the result screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryReward {
    pub desc: Option<String>,
    pub arg1: i32,
    pub arg2: i32,
    pub value: i32,
}
/// QuestSummaryInfo; `result` is MicroPlayResult (byte enum).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestSummary {
    pub quest_id: Option<String>,
    pub swear_id: i32,
    pub is_today_quest: bool,
    pub result: u8,
    pub last_progress: i32,
    pub current_progress: i32,
    pub cleared_goals: Dict<Option<SummaryGoal>>,
    pub battle_exp: Option<Vec<Option<SummaryReward>>>,
    pub etc_exp: Option<Vec<Option<SummaryReward>>>,
    pub main_gold: Option<Vec<Option<SummaryReward>>>,
    pub etc_gold: Option<Vec<Option<SummaryReward>>>,
    pub quest_ap: Option<Vec<Option<SummaryReward>>>,
    pub etc_ap: Option<Vec<Option<SummaryReward>>>,
    pub exp: i32,
    pub gold: i32,
    pub ap: i32,
    pub reward_item: StrDict<i32>,
}
/// FinishGameMessage: the quest result screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinishGame {
    pub summary: Option<QuestSummary>,
    pub has_story_sector: bool,
    pub can_restart: bool,
    pub can_continue_finished_quest: bool,
}
/// ReturnFromQuestMessage; `order` is ReturnFromQuestType (ToTown 0, ToShip 1, ToStorySector 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReturnFromQuest {
    pub order: i32,
    pub story_sector_bsp: Option<String>,
    pub story_sector_entity: Option<String>,
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
    MonsterKilled(MonsterKilled),
    /// PlayerKilledMessage: the dead player's tag.
    PlayerKilled(i32),
    RagdollKicked(RagdollKicked),
    DropGoldCore(DropGoldCore),
    GetItem(GetItem),
    SectorPropList(SectorPropList),
    PropBroken(PropBroken),
    DropErg(DropErg),
    /// PickErgMessage (host): the picked erg ID.
    PickErg(i32),
    PutErg(PutErg),
    MonsterDamageReport(MonsterDamageReport),
    CombatRecord(CombatRecord),
    UseConsumable(UseConsumable),
    PlayerRevived(PlayerRevived),
    ApplyMicroPlayEffects(ApplyMicroPlayEffects),
    MicroPlayEvent(MicroPlayEvent),
    QuestTargetResult(QuestTargetResult),
    QuestSuccessSceneStart,
    FinishGame(FinishGame),
    ReturnToTown,
    ReturnFromQuest(ReturnFromQuest),
    Kicked,
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
            Self::MonsterKilled(_) => Kind::MonsterKilled,
            Self::PlayerKilled(_) => Kind::PlayerKilled,
            Self::RagdollKicked(_) => Kind::RagdollKicked,
            Self::DropGoldCore(_) => Kind::DropGoldCore,
            Self::GetItem(_) => Kind::GetItem,
            Self::SectorPropList(_) => Kind::SectorPropList,
            Self::PropBroken(_) => Kind::PropBroken,
            Self::DropErg(_) => Kind::DropErg,
            Self::PickErg(_) => Kind::PickErg,
            Self::PutErg(_) => Kind::PutErg,
            Self::MonsterDamageReport(_) => Kind::MonsterDamageReport,
            Self::CombatRecord(_) => Kind::CombatRecord,
            Self::UseConsumable(_) => Kind::UseConsumable,
            Self::PlayerRevived(_) => Kind::PlayerRevived,
            Self::ApplyMicroPlayEffects(_) => Kind::ApplyMicroPlayEffects,
            Self::MicroPlayEvent(_) => Kind::MicroPlayEvent,
            Self::QuestTargetResult(_) => Kind::QuestTargetResult,
            Self::QuestSuccessSceneStart => Kind::QuestSuccessSceneStart,
            Self::FinishGame(_) => Kind::FinishGame,
            Self::ReturnToTown => Kind::ReturnToTown,
            Self::ReturnFromQuest(_) => Kind::ReturnFromQuest,
            Self::Kicked => Kind::Kicked,
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
object!(MonsterKilled {
    attacker,
    target,
    action_type,
    has_evil_core,
    damage,
    damage_position_x,
    damage_position_y,
    distance,
    actor_index
});
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
object!(RagdollKicked {
    tag,
    target_entity_name,
    core_type,
    is_rare_core
});
object!(EvilCoreInfo {
    entity_name,
    core_type,
    winner,
    rare_core_tags
});
object!(DropGoldCore {
    monster_entity_name,
    cores,
    drop_immediately
});
object!(GetItem {
    player_name,
    item_class,
    count,
    core_type,
    lucky,
    luck_bonus,
    give_item_result
});
object!(SectorPropList { props });
object!(PropBroken {
    prop,
    entity_name,
    attacker
});
object!(Erg {
    erg_id,
    erg_class,
    erg_type,
    amount,
    winner
});
object!(DropErg {
    broken_prop,
    monster_entity_name,
    ergs
});
object!(PutErg { erg_id, tag });
object!(TakeDamage {
    attacker,
    attack_time,
    action_name,
    damage
});
object!(MonsterDamageReport { target, damages });
object!(CombatRecord {
    player_number,
    combo_max,
    hit_max,
    style_max,
    death,
    kill,
    battle_achieve,
    hit_take,
    style_count,
    rank_style,
    rank_battle,
    rank_total
});
object!(UseConsumable { tag, part, slot });
object!(PlayerRevived {
    caster_tag,
    reviver_tag,
    method
});
object!(MicroPlayEffect {
    player_slot_no,
    effect,
    amount,
    argument
});
object!(ApplyMicroPlayEffects {
    caster,
    effect_name,
    effects
});
object!(MicroPlayEvent { slot, event });
object!(QuestTargetResult {
    character_name,
    goal_id,
    is_goal_success,
    is_quest_success,
    exp,
    gold,
    ap
});
object!(SummaryGoal {
    goal_id,
    exp,
    gold,
    ap
});
object!(SummaryReward {
    desc,
    arg1,
    arg2,
    value
});
object!(QuestSummary {
    quest_id,
    swear_id,
    is_today_quest,
    result,
    last_progress,
    current_progress,
    cleared_goals,
    battle_exp,
    etc_exp,
    main_gold,
    etc_gold,
    quest_ap,
    etc_ap,
    exp,
    gold,
    ap,
    reward_item
});
object!(FinishGame {
    summary,
    has_story_sector,
    can_restart,
    can_continue_finished_quest
});
object!(ReturnFromQuest {
    order,
    story_sector_bsp,
    story_sector_entity
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
                Message::MonsterKilled(v) => v.write(&mut w)?,
                Message::PlayerKilled(v) => v.write(&mut w)?,
                Message::RagdollKicked(v) => v.write(&mut w)?,
                Message::DropGoldCore(v) => v.write(&mut w)?,
                Message::GetItem(v) => v.write(&mut w)?,
                Message::SectorPropList(v) => v.write(&mut w)?,
                Message::PropBroken(v) => v.write(&mut w)?,
                Message::DropErg(v) => v.write(&mut w)?,
                Message::PickErg(v) => v.write(&mut w)?,
                Message::PutErg(v) => v.write(&mut w)?,
                Message::MonsterDamageReport(v) => v.write(&mut w)?,
                Message::CombatRecord(v) => v.write(&mut w)?,
                Message::UseConsumable(v) => v.write(&mut w)?,
                Message::PlayerRevived(v) => v.write(&mut w)?,
                Message::ApplyMicroPlayEffects(v) => v.write(&mut w)?,
                Message::MicroPlayEvent(v) => v.write(&mut w)?,
                Message::QuestTargetResult(v) => v.write(&mut w)?,
                Message::QuestSuccessSceneStart => {}
                Message::FinishGame(v) => v.write(&mut w)?,
                Message::ReturnToTown => {}
                Message::ReturnFromQuest(v) => v.write(&mut w)?,
                Message::Kicked => {}
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
            Kind::MonsterKilled => Message::MonsterKilled(MonsterKilled::read(&mut r)?),
            Kind::PlayerKilled => Message::PlayerKilled(i32::read(&mut r)?),
            Kind::RagdollKicked => Message::RagdollKicked(RagdollKicked::read(&mut r)?),
            Kind::DropGoldCore => Message::DropGoldCore(DropGoldCore::read(&mut r)?),
            Kind::GetItem => Message::GetItem(GetItem::read(&mut r)?),
            Kind::SectorPropList => Message::SectorPropList(SectorPropList::read(&mut r)?),
            Kind::PropBroken => Message::PropBroken(PropBroken::read(&mut r)?),
            Kind::DropErg => Message::DropErg(DropErg::read(&mut r)?),
            Kind::PickErg => Message::PickErg(i32::read(&mut r)?),
            Kind::PutErg => Message::PutErg(PutErg::read(&mut r)?),
            Kind::MonsterDamageReport => {
                Message::MonsterDamageReport(MonsterDamageReport::read(&mut r)?)
            }
            Kind::CombatRecord => Message::CombatRecord(CombatRecord::read(&mut r)?),
            Kind::UseConsumable => Message::UseConsumable(UseConsumable::read(&mut r)?),
            Kind::PlayerRevived => Message::PlayerRevived(PlayerRevived::read(&mut r)?),
            Kind::ApplyMicroPlayEffects => {
                Message::ApplyMicroPlayEffects(ApplyMicroPlayEffects::read(&mut r)?)
            }
            Kind::MicroPlayEvent => Message::MicroPlayEvent(MicroPlayEvent::read(&mut r)?),
            Kind::QuestTargetResult => Message::QuestTargetResult(QuestTargetResult::read(&mut r)?),
            Kind::QuestSuccessSceneStart => Message::QuestSuccessSceneStart,
            Kind::FinishGame => Message::FinishGame(FinishGame::read(&mut r)?),
            Kind::ReturnToTown => Message::ReturnToTown,
            Kind::ReturnFromQuest => Message::ReturnFromQuest(ReturnFromQuest::read(&mut r)?),
            Kind::Kicked => Message::Kicked,
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
