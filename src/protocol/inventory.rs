//! Bounded inventory snapshot codec recovered from the Framework serializer.
//! No storage reads or item mutations.
use super::{
    encode_integer,
    wire::{PayloadError, Reader, Writer},
};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};
const MAX_ROWS: usize = 4096;
const MOVE_TYPE_ID: u128 = 0x7c957f24_e8d7_338f_a93c_97bc6903f320;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    QueryInventory,
    InventoryInfo,
    UpdateInventoryInfo,
    ItemFail,
    MoveInventoryItem,
    EquipBundle,
    QuickSlotInfo,
    QueryAp,
    Ap,
    UseInventoryItem,
    UseInventoryItemOk,
    QuerySkillList,
    SkillList,
    LearnSkill,
    SkillCompletion,
    QueryCharacterCommonInfo,
    LearnNewSkill,
    LearnNewSkillResult,
    QueryNpcList,
    NpcList,
    QueryNpcTalk,
    NpcTalk,
    SelectButton,
    ExecuteNpcServerCommand,
    SetStoryLineStatus,
    StoryLines,
    BuyItem,
    BuyItemResult,
    QueryShopItemInfo,
    SellItem,
    QueryBuybackList,
    BuybackListResult,
    RepairItem,
    UpdateStoryGuide,
    StoryVariables,
    QueryQuestProgress,
    QuestProgress,
    UpdateTitle,
    HideCostume,
    TitleList,
    EnhanceItem,
    EnhanceItemResult,
    QueryRestoreItemList,
    RestoreItemList,
    EnchantItem,
    EnchantItemResult,
    ManufactureInfo,
    ManufactureCraft,
    ManufactureLevelUp,
}
impl Kind {
    pub const fn category(self) -> i32 {
        match self {
            Self::QueryInventory => 529,
            Self::InventoryInfo => 468,
            Self::UpdateInventoryInfo => 617,
            Self::ItemFail => 358,
            Self::MoveInventoryItem => 502,
            Self::EquipBundle => 456,
            Self::QuickSlotInfo => 551,
            Self::QueryAp => 526,
            Self::Ap => 229,
            Self::UseInventoryItem => 629,
            Self::UseInventoryItemOk => 627,
            Self::QuerySkillList => 540,
            Self::SkillList => 583,
            Self::LearnSkill => 475,
            Self::SkillCompletion => 582,
            Self::QueryCharacterCommonInfo => 527,
            Self::LearnNewSkill => 340,
            Self::LearnNewSkillResult => 341,
            Self::QueryNpcList => 533,
            Self::NpcList => 507,
            Self::QueryNpcTalk => 534,
            Self::NpcTalk => 508,
            Self::SelectButton => 564,
            Self::ExecuteNpcServerCommand => 459,
            Self::SetStoryLineStatus => 574,
            Self::StoryLines => 598,
            Self::BuyItem => 234,
            Self::BuyItemResult => 235,
            Self::QueryShopItemInfo => 167,
            Self::SellItem => 566,
            Self::QueryBuybackList => 293,
            Self::BuybackListResult => 294,
            Self::RepairItem => 424,
            Self::UpdateStoryGuide => 622,
            Self::StoryVariables => 599,
            Self::QueryQuestProgress => 535,
            Self::QuestProgress => 546,
            Self::UpdateTitle => 191,
            Self::HideCostume => 414,
            Self::TitleList => 610,
            Self::EnhanceItem => 354,
            Self::EnhanceItemResult => 355,
            Self::QueryRestoreItemList => 152,
            Self::RestoreItemList => 154,
            Self::EnchantItem => 352,
            Self::EnchantItemResult => 353,
            Self::ManufactureInfo => 359,
            Self::ManufactureCraft => 360,
            Self::ManufactureLevelUp => 361,
        }
    }
    pub const fn type_id(self) -> Uuid {
        Uuid::from_u128(match self {
            Self::QueryInventory => 0xefe95078_ef7e_34eb_82e1_2daf3280a576,
            Self::InventoryInfo => 0xbdc965b5_aca1_3b62_b222_279e96683d93,
            Self::UpdateInventoryInfo => 0xabacd3bc_1a84_3570_a62c_f1b0007675fb,
            Self::ItemFail => 0x2f756b90_8fba_38b8_bcb2_a51f0f132502,
            Self::MoveInventoryItem => MOVE_TYPE_ID,
            Self::EquipBundle => 0x6652c42f_bf01_3da0_9415_f390b085e456,
            Self::QuickSlotInfo => 0xedc4290e_196c_3f17_903f_d0f3317e47d9,
            Self::QueryAp => 0xf061ae98_f5c8_333d_82ca_3b217e268120,
            Self::Ap => 0x3d59ac68_6b3c_3f92_968e_b569b973f065,
            Self::UseInventoryItem => 0xc182cb18_7469_3390_a0e7_8eba31d6e45d,
            Self::UseInventoryItemOk => 0x451d459f_bb17_325c_bb10_d608f03f4c3f,
            Self::QuerySkillList => 0x860e6484_d1b5_34a9_9412_70807bbbf79c,
            Self::SkillList => 0xdd3b94a9_64da_381a_b2e7_0fad58835746,
            Self::LearnSkill => 0x3ec00564_95e3_3507_b9dc_a5a185215b19,
            Self::SkillCompletion => 0x2b5614fc_8830_3aec_a03f_c33c270de9f1,
            Self::QueryCharacterCommonInfo => 0xc9cda3f6_8b67_3c69_ba31_ba2fe827fd7b,
            Self::LearnNewSkill => 0x7fbd47e1_96a3_3472_9e71_f36d90f7294f,
            Self::LearnNewSkillResult => 0x0430910f_e20e_3736_850d_b619e836f14f,
            Self::QueryNpcList => 0x6d029dfe_89e5_32d0_aa79_27d298e3b87c,
            Self::NpcList => 0x35be098a_89fe_3185_a8f3_1cf91f5c07c1,
            Self::QueryNpcTalk => 0x986c6c40_c9b6_3aed_9303_0b5679aa8f53,
            Self::NpcTalk => 0x859ce7b4_cfd1_3695_9a73_b5c094d7085e,
            Self::SelectButton => 0xd5dda62d_44c1_3c6a_8906_72164e564921,
            Self::ExecuteNpcServerCommand => 0x048fdbb2_e452_3c5f_b0b2_cfc80994e365,
            Self::SetStoryLineStatus => 0x92e441fa_adce_304e_aeb2_a77fb76d2900,
            Self::StoryLines => 0x5498b052_d119_30e2_8224_1e2b990d4ef8,
            Self::BuyItem => 0x2f52e151_33c1_34dc_bd6e_9535c35ab918,
            Self::BuyItemResult => 0x64a08bdf_6762_3be2_9f6f_c7ff64a2b4a3,
            Self::QueryShopItemInfo => 0x4088bd54_bc81_3398_8386_61bdaf5917c1,
            Self::SellItem => 0x62669042_85a8_354c_9cab_ccae3c2becae,
            Self::QueryBuybackList => 0x2271100a_effc_3693_b75d_165507d49b79,
            Self::BuybackListResult => 0x88ce410b_fcb7_3794_ab5f_9cb25771ce3d,
            Self::RepairItem => 0xb83c8562_c97c_343c_b421_d3cdada26774,
            Self::UpdateStoryGuide => 0x4f661234_1161_3db9_8127_877ed7807435,
            Self::StoryVariables => 0xde76b127_f09d_369a_8008_26af3e5c8383,
            Self::QueryQuestProgress => 0x5b634b00_c047_339e_a4fe_b0ef8e3b9a2e,
            Self::QuestProgress => 0xc229a0f5_4eda_3bbc_a4be_61f3aad3f82b,
            Self::UpdateTitle => 0xbb01bf3c_8194_3f94_9480_4bf20654b7b3,
            Self::HideCostume => 0x80e685d2_1ace_3cf5_90aa_7fda984d169d,
            Self::TitleList => 0xe537b12b_c3c8_3ae2_816e_df0465b4883b,
            Self::EnhanceItem => 0x04804f38_5cec_31f6_885a_cc0fc33628eb,
            Self::EnhanceItemResult => 0x1726cb88_3409_3a2b_8923_56e0250aab6a,
            Self::QueryRestoreItemList => 0xa91b7337_2cad_3eae_8175_e1763f2413ee,
            Self::RestoreItemList => 0xb80b34de_ba17_3cef_9ae3_577cfd045708,
            Self::EnchantItem => 0xddc9fc48_e5dc_304e_bfd2_8c5fed04f83e,
            Self::EnchantItemResult => 0x0a5b1d79_7e63_32d7_9520_ab7d8bfaa7ed,
            Self::ManufactureInfo => 0x9848342d_cc2f_326e_9b4e_d30c6decd5b5,
            Self::ManufactureCraft => 0x42f64ed3_c84b_3cfb_853b_0dec85cfac07,
            Self::ManufactureLevelUp => 0x4b8c26ec_6412_3016_b31e_157b18638117,
        })
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryWireError {
    #[error("inventory message kind mismatch")]
    Kind,
    #[error("unsupported inventory category/type")]
    Type,
    #[error(transparent)]
    Payload(#[from] PayloadError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dict<T>(pub Option<Vec<(i32, T)>>);
#[derive(Clone, PartialEq, Eq)]
pub struct Storage {
    pub id: u8,
    pub available: u8,
    pub last_seconds: i32,
    pub name: Option<String>,
    pub tag: i32,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: Option<String>,
    pub value: Option<String>,
    pub arg: i32,
    pub arg2: i32,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Slot {
    pub id: i64,
    pub class: Option<String>,
    pub count: i32,
    pub tab: i32,
    pub slot: i32,
    pub expireable: bool,
    pub expire_diff: i64,
    pub expire_ticks: i64,
    pub colors: Option<Vec<i32>>,
    pub durability: i32,
    pub maximum: i32,
    pub bonus: i32,
    pub tradable: bool,
    pub expander: Option<String>,
    pub attributes: Option<Vec<Option<Attribute>>>,
    pub prefix: Dict<bool>,
    pub suffix: Dict<bool>,
    pub min_price: i32,
    pub max_price: i32,
    pub avg_price: i32,
    pub min_up_diff: i64,
}
#[derive(Clone, PartialEq, Eq)]
pub struct QuickSlots {
    pub classes: Dict<Option<String>>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Inventory {
    pub storages: Option<Vec<Option<Storage>>>,
    pub slots: Option<Vec<Option<Slot>>>,
    pub equipment: Dict<i64>,
    pub quick_slots: Option<QuickSlots>,
    pub unequippable: Option<Vec<i32>>,
}
/// ServiceCore HeroesString: Format and Parameters (ICollection<string>).
#[derive(Clone, PartialEq, Eq)]
pub struct HeroesString {
    pub format: Option<String>,
    pub parameters: Option<Vec<Option<String>>>,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Message {
    QueryInventory,
    InventoryInfo(Inventory),
    /// UpdateInventoryInfoMessage.slotInfos.
    UpdateInventoryInfo(Option<Vec<Option<Slot>>>),
    /// ItemFailMessage.Message.
    ItemFail(Option<HeroesString>),
    /// MoveInventoryItemMessage: targetItemID, targetStorage, targetSlot.
    MoveInventoryItem(Move),
    /// EquipBundleMessage: ItemClass ("" clears), QuickSlotID.
    EquipBundle(Bundle),
    /// QuickSlotInfoMessage.info.
    QuickSlotInfo(Option<QuickSlots>),
    /// QueryAPMessage (fieldless).
    QueryAp,
    /// APMessage: field declaration order nextBonusTimeTicks, ap, maxap, apBonusInterval.
    Ap(Ap),
    UseInventoryItem(UseItem),
    /// UseInventoryItemOkMessage.UseItemClass.
    UseInventoryItemOk(Option<String>),
    /// QuerySkillListMessage (fieldless).
    QuerySkillList,
    SkillList(SkillList),
    /// LearnSkillMessage: skillID, ap.
    LearnSkill(SkillAp),
    /// SkillCompletionMessage: SkillID, SkillRank.
    SkillCompletion(SkillAp),
    /// QueryCharacterCommonInfoMessage: QueryID, CID.
    QueryCharacterCommonInfo(CommonQuery),
    /// LearnNewSkillMessage.SkillID.
    LearnNewSkill(Option<String>),
    /// LearnNewSkillResultMessage.result (LearnNewSkillResult as Int32).
    LearnNewSkillResult(i32),
    /// QueryNpcListMessage (fieldless).
    QueryNpcList,
    /// NpcListMessage.Buildings.
    NpcList(Option<Vec<Option<Building>>>),
    QueryNpcTalk(NpcQuery),
    /// NpcTalkMessage.Content.
    NpcTalk(Option<Vec<Option<TalkEntity>>>),
    /// SelectButtonMessage.ButtonIndex.
    SelectButton(i32),
    /// ExecuteNpcServerCommandMessage (fieldless).
    ExecuteNpcServerCommand,
    SetStoryLineStatus(StoryLineStatus),
    StoryLines(StoryLines),
    BuyItem(BuyItem),
    BuyItemResult(BuyItemResult),
    /// QueryShopItemInfoMessage.ShopID.
    QueryShopItemInfo(Option<String>),
    SellItem(SellItem),
    /// QueryBuybackListMessage (fieldless).
    QueryBuybackList,
    /// BuybackListResultMessage.BuybackList.
    BuybackListResult(Option<Vec<Option<BuybackInfo>>>),
    RepairItem(RepairItem),
    /// UpdateStoryGuideMessage: TargetLandMark, GuideMessage.
    UpdateStoryGuide(StoryGuide),
    /// StoryVariablesMessage.StoryVariables.
    StoryVariables(Option<Vec<Option<StoryVariable>>>),
    QueryQuestProgress,
    /// QuestProgressMessage: QuestProgress, AchievedGoals.
    QuestProgress(QuestProgress),
    /// UpdateTitleMessage.Titles.
    UpdateTitle(Option<Vec<Option<TitleSlot>>>),
    HideCostume(HideCostume),
    /// TitleListMessage: AccountTitles, Titles.
    TitleList(TitleList),
    EnhanceItem(EnhanceItem),
    EnhanceItemResult(EnhanceItemResult),
    QueryRestoreItemList,
    /// RestoreItemListMessage.RestoreItemList.
    RestoreItemList(Option<Vec<Option<RestoreItemInfo>>>),
    EnchantItem(EnchantItem),
    EnchantItemResult(EnchantItemResult),
    ManufactureInfo(ManufactureInfo),
    ManufactureCraft(ManufactureCraft),
    ManufactureLevelUp(ManufactureLevelUp),
}
/// TitleSlotInfo: TitleID, Status (TitleState), TimeStamp, QuestID, GoalProgress,
/// IsExpireable, ExpireDateTime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleSlot {
    pub title_id: i32,
    pub status: i32,
    pub time_stamp: i64,
    pub quest_id: Option<String>,
    pub goal_progress: Option<Vec<(i32, i32)>>,
    pub is_expireable: bool,
    pub expire_date_time: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestProgress {
    pub progress: Option<Vec<Option<QuestProgressInfo>>>,
    pub achieved_goals: Option<Vec<Option<AchieveGoal>>>,
}
/// QuestProgressInfo fields in declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestProgressInfo {
    pub id: Option<String>,
    pub progress: i32,
    pub clear_count: i32,
    pub today_quest: bool,
    pub appear_named_monster: bool,
    pub available: bool,
    pub revealed: bool,
    pub max_play_count: i32,
    pub play_count: i32,
    pub clear_times: Option<Vec<Option<ClearTimeElement>>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClearTimeElement {
    pub difficulty: i32,
    pub member_count: i32,
    pub clear_time: i32,
    pub play_rank: i32,
    pub clear_info: i32,
    pub is_old: bool,
}
/// AchieveGoalInfo: ID, Status, ClearCount, RegenedAP, LastClearTimeDiff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AchieveGoal {
    pub id: i32,
    pub status: i32,
    pub clear_count: u8,
    pub regened_ap: u8,
    pub last_clear_time_diff: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryGuide {
    pub target: Option<String>,
    pub message: Option<String>,
}
/// StoryVariableInfo: StoryLine, Key, Value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryVariable {
    pub story_line: Option<String>,
    pub key: Option<String>,
    pub value: i32,
}
/// BuyItemMessage: ShopID, Order, Count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuyItem {
    pub shop_id: Option<String>,
    pub order: i32,
    pub count: i32,
}
/// BuyItemResultMessage in declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuyItemResult {
    pub result: i32,
    pub item_class: Option<String>,
    pub item_count: i32,
    pub price_item_class: Option<String>,
    pub price_item_count: i32,
    pub restriction_order: i32,
    pub restriction_count: i32,
}
/// SellItemMessage: ItemID, Count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SellItem {
    pub item_id: i64,
    pub count: i32,
}
/// BuybackInfo fields in declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuybackInfo {
    pub shop_id: Option<String>,
    pub order: i32,
    pub item_class: Option<String>,
    pub item_count: i32,
    pub color1: i32,
    pub color2: i32,
    pub color3: i32,
    pub price_item_class: Option<String>,
    pub price_count: i32,
}
/// TitleListMessage: the account's shared titles and the character's titles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleList {
    pub account: Option<Vec<Option<TitleSlot>>>,
    pub titles: Option<Vec<Option<TitleSlot>>>,
}
/// EnhanceItemMessage: ItemID, Material1, Material2, AdditionalMaterial,
/// IsEventEnhanceAShot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnhanceItem {
    pub item_id: i64,
    pub material1: Option<String>,
    pub material2: Option<String>,
    pub additional_material: Option<String>,
    pub is_event_enhance_a_shot: bool,
}
/// EnhanceItemResultMessage: Result, ItemID, PrevItemClass, EnhancedItemClass and the
/// FailReward dictionary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnhanceItemResult {
    pub result: Option<String>,
    pub item_id: i64,
    pub prev_item_class: Option<String>,
    pub enhanced_item_class: Option<String>,
    pub fail_reward: super::mission::StrDict<i32>,
}
/// EnchantItemMessage: OpenNewSession, TargetItemID, EnchantScrollItemID,
/// IndestructibleScrollItemID, DiceItemClass ("DoEnchant", "CancelEnchant" or a dice).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnchantItem {
    pub open_new_session: bool,
    pub target_item_id: i64,
    pub enchant_scroll_item_id: i64,
    pub indestructible_scroll_item_id: i64,
    pub dice_item_class: Option<String>,
}
/// EnchantItemResultMessage: CurrentValue, GoalValue, Result (EnchantItemResult),
/// RolledDice, CurrentSuccessRatio ((int)(ratio × 100f)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnchantItemResult {
    pub current_value: i32,
    pub goal_value: i32,
    pub result: i32,
    pub rolled_dice: Option<String>,
    pub current_success_ratio: i32,
}
/// ManufactureInfoMessage: ExpDictionary, GradeDictionary (ManufactureManager.EXP order)
/// and Recipes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManufactureInfo {
    pub exp: super::mission::StrDict<i32>,
    pub grade: super::mission::StrDict<i32>,
    pub recipes: Option<Vec<Option<String>>>,
}
/// ManufactureCraftMessage: RecipeID, PartsIDList (Combine Craft parts).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManufactureCraft {
    pub recipe_id: Option<String>,
    pub parts_id_list: Option<Vec<i64>>,
}
/// ManufactureLevelUpMessage: MID, Grade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManufactureLevelUp {
    pub mid: Option<String>,
    pub grade: i32,
}
/// RestoreItemInfo: the restore-storage SlotInfo, PriceType ("AP") and PriceValue.
#[derive(Clone, PartialEq, Eq)]
pub struct RestoreItemInfo {
    pub slot: Option<Slot>,
    pub price_type: Option<String>,
    pub price_value: i32,
}
/// HideCostumeMessage: HideHead, AvatarPart, AvatarState (the constructor sets -1/-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HideCostume {
    pub hide_head: bool,
    pub avatar_part: i32,
    pub avatar_state: i32,
}
/// RepairItemMessage: ItemIDs and the three Add* flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepairItem {
    pub item_ids: Option<Vec<i64>>,
    pub all_equipped: bool,
    pub all_broken: bool,
    pub all_repairable: bool,
}
/// BriefStoryLineInfo: StoryLine, Phase, Status, PhaseText.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefStoryLine {
    pub story_line: Option<String>,
    pub phase: Option<String>,
    pub status: u8,
    pub phase_text: Option<String>,
}
/// StoryLinesMessage: StoryStatus, HintCategory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryLines {
    pub status: Option<Vec<Option<BriefStoryLine>>>,
    pub hint_category: i32,
}
/// StoryInfo: StoryLineID, IsRevealing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryInfo {
    pub story_line: Option<String>,
    pub is_revealing: bool,
}
/// NpcInfo: NpcID, Feature, StoryLines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcInfo {
    pub npc: Option<String>,
    pub feature: Option<String>,
    pub story_lines: Option<Vec<Option<StoryInfo>>>,
}
/// BuildingInfo: BuildingID, Npcs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Building {
    pub building: Option<String>,
    pub npcs: Option<Vec<Option<NpcInfo>>>,
}
/// SetStoryLineStatusMessage: StoryLineID, Status (StoryLineStatus byte).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryLineStatus {
    pub story_line: Option<String>,
    pub status: u8,
}
/// QueryNpcTalkMessage: Location, NpcID, StoryLine, Command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcQuery {
    pub location: Option<String>,
    pub npc: Option<String>,
    pub story_line: Option<String>,
    pub command: Option<String>,
}
/// NpcTalkEntity: Command, Arg1, Arg2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalkEntity {
    pub command: Option<String>,
    pub arg1: Option<String>,
    pub arg2: Option<String>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct SkillAp {
    pub skill: Option<String>,
    pub value: i32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CommonQuery {
    pub query_id: i64,
    pub cid: i64,
}
/// BriefSkillEnhance in declaration order.
#[derive(Clone, PartialEq, Eq)]
pub struct SkillEnhance {
    pub group_key: i32,
    pub index_key: i32,
    pub kind: u8,
    pub reduce_durability: i32,
    pub max_durability_bonus: i32,
}
/// BriefSkillInfo in declaration order.
#[derive(Clone, PartialEq, Eq)]
pub struct BriefSkill {
    pub id: Option<String>,
    pub base_rank: i32,
    pub final_rank: i32,
    pub required_ap: i32,
    pub is_locked: u8,
    pub can_start_training: u8,
    pub current_ap: i32,
    pub reset_ap: i32,
    pub untrain_ap: i32,
    pub enhances: Option<Vec<Option<SkillEnhance>>>,
}
/// SkillListMessage: LearningSkillID, CurrentAp, SkillList, IsResetVocation.
#[derive(Clone, PartialEq, Eq)]
pub struct SkillList {
    pub learning: Option<String>,
    pub current_ap: i32,
    pub skills: Option<Vec<Option<BriefSkill>>>,
    pub reset_vocation: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ap {
    pub next_bonus_ms: i64,
    pub ap: i32,
    pub max_ap: i32,
    pub bonus_interval: i32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UseItem {
    pub item_id: i64,
    pub target_item_id: i64,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Bundle {
    pub class: Option<String>,
    pub quick_slot: i32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub item_id: i64,
    pub storage: u8,
    pub slot: i32,
}
impl Message {
    pub const fn kind(&self) -> Kind {
        match self {
            Self::QueryInventory => Kind::QueryInventory,
            Self::InventoryInfo(_) => Kind::InventoryInfo,
            Self::UpdateInventoryInfo(_) => Kind::UpdateInventoryInfo,
            Self::ItemFail(_) => Kind::ItemFail,
            Self::MoveInventoryItem(_) => Kind::MoveInventoryItem,
            Self::EquipBundle(_) => Kind::EquipBundle,
            Self::QuickSlotInfo(_) => Kind::QuickSlotInfo,
            Self::QueryAp => Kind::QueryAp,
            Self::Ap(_) => Kind::Ap,
            Self::UseInventoryItem(_) => Kind::UseInventoryItem,
            Self::UseInventoryItemOk(_) => Kind::UseInventoryItemOk,
            Self::QuerySkillList => Kind::QuerySkillList,
            Self::SkillList(_) => Kind::SkillList,
            Self::LearnSkill(_) => Kind::LearnSkill,
            Self::SkillCompletion(_) => Kind::SkillCompletion,
            Self::QueryCharacterCommonInfo(_) => Kind::QueryCharacterCommonInfo,
            Self::LearnNewSkill(_) => Kind::LearnNewSkill,
            Self::LearnNewSkillResult(_) => Kind::LearnNewSkillResult,
            Self::QueryNpcList => Kind::QueryNpcList,
            Self::NpcList(_) => Kind::NpcList,
            Self::QueryNpcTalk(_) => Kind::QueryNpcTalk,
            Self::NpcTalk(_) => Kind::NpcTalk,
            Self::SelectButton(_) => Kind::SelectButton,
            Self::ExecuteNpcServerCommand => Kind::ExecuteNpcServerCommand,
            Self::SetStoryLineStatus(_) => Kind::SetStoryLineStatus,
            Self::StoryLines(_) => Kind::StoryLines,
            Self::BuyItem(_) => Kind::BuyItem,
            Self::BuyItemResult(_) => Kind::BuyItemResult,
            Self::QueryShopItemInfo(_) => Kind::QueryShopItemInfo,
            Self::SellItem(_) => Kind::SellItem,
            Self::QueryBuybackList => Kind::QueryBuybackList,
            Self::BuybackListResult(_) => Kind::BuybackListResult,
            Self::RepairItem(_) => Kind::RepairItem,
            Self::UpdateStoryGuide(_) => Kind::UpdateStoryGuide,
            Self::StoryVariables(_) => Kind::StoryVariables,
            Self::QueryQuestProgress => Kind::QueryQuestProgress,
            Self::QuestProgress(_) => Kind::QuestProgress,
            Self::UpdateTitle(_) => Kind::UpdateTitle,
            Self::HideCostume(_) => Kind::HideCostume,
            Self::TitleList(_) => Kind::TitleList,
            Self::EnhanceItem(_) => Kind::EnhanceItem,
            Self::EnhanceItemResult(_) => Kind::EnhanceItemResult,
            Self::QueryRestoreItemList => Kind::QueryRestoreItemList,
            Self::RestoreItemList(_) => Kind::RestoreItemList,
            Self::EnchantItem(_) => Kind::EnchantItem,
            Self::EnchantItemResult(_) => Kind::EnchantItemResult,
            Self::ManufactureInfo(_) => Kind::ManufactureInfo,
            Self::ManufactureCraft(_) => Kind::ManufactureCraft,
            Self::ManufactureLevelUp(_) => Kind::ManufactureLevelUp,
        }
    }
}
pub(super) trait Wire: Sized {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError>;
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError>;
}
macro_rules! number {
    ($t:ty,$n:expr) => {
        impl Wire for $t {
            fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
                w.append(&self.to_be_bytes())
            }
            fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
                Ok(<$t>::from_be_bytes(r.fixed::<$n>()?))
            }
        }
    };
}
number!(i32, 4);
number!(i64, 8);
impl Wire for u8 {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.append(&[*self])
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        r.byte()
    }
}
impl Wire for bool {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        u8::from(*self).write(w)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(r.byte()? != 0)
    }
}
impl Wire for Option<String> {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        w.nullable(self.as_ref().map(|v| v.as_bytes()))
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok(r.string()?.map(str::to_owned))
    }
}
impl<T: Wire> Wire for Option<Vec<T>> {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        let Some(v) = self else { return w.append(&[0]) };
        if v.len() > MAX_ROWS {
            return Err(PayloadError::TooLarge);
        }
        w.append(&encode_integer((v.len() + 1) as i32))?;
        for x in v {
            x.write(w)?
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        let Some(n) = r.count()? else { return Ok(None) };
        if n > MAX_ROWS || n > r.remaining.len() {
            return Err(PayloadError::TooLarge);
        }
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(T::read(r)?)
        }
        Ok(Some(v))
    }
}
impl<T: Wire> Wire for (i32, T) {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        self.0.write(w)?;
        self.1.write(w)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        Ok((i32::read(r)?, T::read(r)?))
    }
}
impl<T: Wire> Wire for Dict<T> {
    fn write(&self, w: &mut Writer) -> Result<(), PayloadError> {
        check_keys(&self.0)?;
        self.0.write(w)
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, PayloadError> {
        let v = Option::<Vec<(i32, T)>>::read(r)?;
        check_keys(&v)?;
        Ok(Self(v))
    }
}
fn check_keys<T>(v: &Option<Vec<(i32, T)>>) -> Result<(), PayloadError> {
    let mut keys = std::collections::BTreeSet::new();
    if let Some(v) = v {
        for (k, _) in v {
            if !keys.insert(*k) {
                return Err(PayloadError::DuplicateKey);
            }
        }
    }
    Ok(())
}
macro_rules! object {($t:ident {$($f:ident),*$(,)?})=>{
 impl Wire for $t{fn write(&self,w:&mut Writer)->Result<(),PayloadError>{$ (self.$f.write(w)?;)*Ok(())}fn read(r:&mut Reader<'_>)->Result<Self,PayloadError>{Ok(Self{$($f:Wire::read(r)?),*})}}
 impl Wire for Option<$t>{fn write(&self,w:&mut Writer)->Result<(),PayloadError>{match self{None=>w.append(&[0]),Some(v)=>{w.append(&[1])?;v.write(w)}}}fn read(r:&mut Reader<'_>)->Result<Self,PayloadError>{if r.byte()?==0{Ok(None)}else{Ok(Some($t::read(r)?))}}}
};}
pub(super) use object;
object!(Storage {
    id,
    available,
    last_seconds,
    name,
    tag
});
object!(Attribute {
    name,
    value,
    arg,
    arg2
});
object!(Slot {
    id,
    class,
    count,
    tab,
    slot,
    expireable,
    expire_diff,
    expire_ticks,
    colors,
    durability,
    maximum,
    bonus,
    tradable,
    expander,
    attributes,
    prefix,
    suffix,
    min_price,
    max_price,
    avg_price,
    min_up_diff
});
object!(QuickSlots { classes });
object!(HeroesString { format, parameters });
object!(Bundle { class, quick_slot });
object!(Ap {
    next_bonus_ms,
    ap,
    max_ap,
    bonus_interval
});
object!(SkillEnhance {
    group_key,
    index_key,
    kind,
    reduce_durability,
    max_durability_bonus
});
object!(BriefSkill {
    id,
    base_rank,
    final_rank,
    required_ap,
    is_locked,
    can_start_training,
    current_ap,
    reset_ap,
    untrain_ap,
    enhances
});
object!(SkillList {
    learning,
    current_ap,
    skills,
    reset_vocation
});
object!(SkillAp { skill, value });
object!(StoryInfo {
    story_line,
    is_revealing
});
object!(NpcInfo {
    npc,
    feature,
    story_lines
});
object!(Building { building, npcs });
object!(NpcQuery {
    location,
    npc,
    story_line,
    command
});
object!(BriefStoryLine {
    story_line,
    phase,
    status,
    phase_text
});
object!(StoryLines {
    status,
    hint_category
});
object!(BuyItem {
    shop_id,
    order,
    count
});
object!(BuyItemResult {
    result,
    item_class,
    item_count,
    price_item_class,
    price_item_count,
    restriction_order,
    restriction_count
});
object!(SellItem { item_id, count });
object!(BuybackInfo {
    shop_id,
    order,
    item_class,
    item_count,
    color1,
    color2,
    color3,
    price_item_class,
    price_count
});
object!(RepairItem {
    item_ids,
    all_equipped,
    all_broken,
    all_repairable
});
object!(StoryGuide { target, message });
object!(TitleList { account, titles });
object!(EnhanceItem {
    item_id,
    material1,
    material2,
    additional_material,
    is_event_enhance_a_shot
});
object!(ManufactureInfo {
    exp,
    grade,
    recipes
});
object!(ManufactureCraft {
    recipe_id,
    parts_id_list
});
object!(ManufactureLevelUp { mid, grade });
object!(EnchantItem {
    open_new_session,
    target_item_id,
    enchant_scroll_item_id,
    indestructible_scroll_item_id,
    dice_item_class
});
object!(EnchantItemResult {
    current_value,
    goal_value,
    result,
    rolled_dice,
    current_success_ratio
});
object!(RestoreItemInfo {
    slot,
    price_type,
    price_value
});
object!(EnhanceItemResult {
    result,
    item_id,
    prev_item_class,
    enhanced_item_class,
    fail_reward
});
object!(HideCostume {
    hide_head,
    avatar_part,
    avatar_state
});
object!(StoryLineStatus { story_line, status });
object!(TitleSlot {
    title_id,
    status,
    time_stamp,
    quest_id,
    goal_progress,
    is_expireable,
    expire_date_time
});
object!(QuestProgress {
    progress,
    achieved_goals
});
object!(QuestProgressInfo {
    id,
    progress,
    clear_count,
    today_quest,
    appear_named_monster,
    available,
    revealed,
    max_play_count,
    play_count,
    clear_times
});
object!(ClearTimeElement {
    difficulty,
    member_count,
    clear_time,
    play_rank,
    clear_info,
    is_old
});
object!(AchieveGoal {
    id,
    status,
    clear_count,
    regened_ap,
    last_clear_time_diff
});
object!(StoryVariable {
    story_line,
    key,
    value
});
object!(TalkEntity {
    command,
    arg1,
    arg2
});
object!(CommonQuery { query_id, cid });
object!(UseItem {
    item_id,
    target_item_id
});
object!(Move {
    item_id,
    storage,
    slot
});
object!(Inventory {
    storages,
    slots,
    equipment,
    quick_slots,
    unequippable
});
pub fn encode(
    kind: Kind,
    message: Option<&Message>,
    max_body: usize,
) -> Result<Zeroizing<Vec<u8>>, InventoryWireError> {
    let mut w = Writer {
        bytes: Vec::new(),
        limit: max_body,
    };
    let result = (|| -> Result<(), InventoryWireError> {
        let Some(m) = message else {
            w.append(&[0])?;
            return Ok(());
        };
        if kind != m.kind() {
            return Err(InventoryWireError::Kind);
        }
        w.append(&[1])?;
        match m {
            Message::QueryInventory => {}
            Message::InventoryInfo(v) => v.write(&mut w)?,
            Message::UpdateInventoryInfo(v) => v.write(&mut w)?,
            Message::ItemFail(v) => v.write(&mut w)?,
            Message::MoveInventoryItem(v) => v.write(&mut w)?,
            Message::EquipBundle(v) => v.write(&mut w)?,
            Message::QuickSlotInfo(v) => v.write(&mut w)?,
            Message::QueryAp => {}
            Message::Ap(v) => v.write(&mut w)?,
            Message::UseInventoryItem(v) => v.write(&mut w)?,
            Message::UseInventoryItemOk(v) => v.write(&mut w)?,
            Message::QuerySkillList => {}
            Message::SkillList(v) => v.write(&mut w)?,
            Message::LearnSkill(v) | Message::SkillCompletion(v) => v.write(&mut w)?,
            Message::QueryCharacterCommonInfo(v) => v.write(&mut w)?,
            Message::LearnNewSkill(v) => v.write(&mut w)?,
            Message::LearnNewSkillResult(v) => v.write(&mut w)?,
            Message::QueryNpcList | Message::ExecuteNpcServerCommand => {}
            Message::SetStoryLineStatus(v) => v.write(&mut w)?,
            Message::NpcList(v) => v.write(&mut w)?,
            Message::QueryNpcTalk(v) => v.write(&mut w)?,
            Message::NpcTalk(v) => v.write(&mut w)?,
            Message::SelectButton(v) => v.write(&mut w)?,
            Message::StoryLines(v) => v.write(&mut w)?,
            Message::BuyItem(v) => v.write(&mut w)?,
            Message::BuyItemResult(v) => v.write(&mut w)?,
            Message::QueryShopItemInfo(v) => v.write(&mut w)?,
            Message::SellItem(v) => v.write(&mut w)?,
            Message::QueryBuybackList => {}
            Message::BuybackListResult(v) => v.write(&mut w)?,
            Message::RepairItem(v) => v.write(&mut w)?,
            Message::UpdateStoryGuide(v) => v.write(&mut w)?,
            Message::StoryVariables(v) => v.write(&mut w)?,
            Message::QueryQuestProgress => {}
            Message::QuestProgress(v) => v.write(&mut w)?,
            Message::UpdateTitle(v) => v.write(&mut w)?,
            Message::HideCostume(v) => v.write(&mut w)?,
            Message::TitleList(v) => v.write(&mut w)?,
            Message::EnhanceItem(v) => v.write(&mut w)?,
            Message::EnhanceItemResult(v) => v.write(&mut w)?,
            Message::QueryRestoreItemList => {}
            Message::RestoreItemList(v) => v.write(&mut w)?,
            Message::EnchantItem(v) => v.write(&mut w)?,
            Message::EnchantItemResult(v) => v.write(&mut w)?,
            Message::ManufactureInfo(v) => v.write(&mut w)?,
            Message::ManufactureCraft(v) => v.write(&mut w)?,
            Message::ManufactureLevelUp(v) => v.write(&mut w)?,
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(Zeroizing::new(w.bytes)),
        Err(e) => {
            w.bytes.zeroize();
            Err(e)
        }
    }
}
/// Strict decode: the whole body must be consumed (encoder round trips and fixtures).
pub fn decode(
    kind: Kind,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, InventoryWireError> {
    decode_body(kind, body, max_body, true)
}
/// SerializeReader.FromBinary reads the type's fields and ignores the rest of the packet
/// body, so client ingress tolerates trailing bytes (the 1.69 EN client sends some).
fn decode_body(
    kind: Kind,
    body: &[u8],
    max_body: usize,
    strict: bool,
) -> Result<Option<Message>, InventoryWireError> {
    if body.len() > max_body {
        return Err(PayloadError::TooLarge.into());
    }
    let mut r = Reader { remaining: body };
    let result = if r.byte()? == 0 {
        None
    } else {
        Some(match kind {
            Kind::QueryInventory => Message::QueryInventory,
            Kind::InventoryInfo => Message::InventoryInfo(Inventory::read(&mut r)?),
            Kind::UpdateInventoryInfo => Message::UpdateInventoryInfo(Wire::read(&mut r)?),
            Kind::ItemFail => Message::ItemFail(Wire::read(&mut r)?),
            Kind::MoveInventoryItem => Message::MoveInventoryItem(Move::read(&mut r)?),
            Kind::EquipBundle => Message::EquipBundle(Bundle::read(&mut r)?),
            Kind::QuickSlotInfo => Message::QuickSlotInfo(Wire::read(&mut r)?),
            Kind::QueryAp => Message::QueryAp,
            Kind::Ap => Message::Ap(Ap::read(&mut r)?),
            Kind::UseInventoryItem => Message::UseInventoryItem(UseItem::read(&mut r)?),
            Kind::UseInventoryItemOk => Message::UseInventoryItemOk(Wire::read(&mut r)?),
            Kind::QuerySkillList => Message::QuerySkillList,
            Kind::SkillList => Message::SkillList(SkillList::read(&mut r)?),
            Kind::LearnSkill => Message::LearnSkill(SkillAp::read(&mut r)?),
            Kind::SkillCompletion => Message::SkillCompletion(SkillAp::read(&mut r)?),
            Kind::QueryCharacterCommonInfo => {
                Message::QueryCharacterCommonInfo(CommonQuery::read(&mut r)?)
            }
            Kind::LearnNewSkill => Message::LearnNewSkill(Wire::read(&mut r)?),
            Kind::LearnNewSkillResult => Message::LearnNewSkillResult(Wire::read(&mut r)?),
            Kind::QueryNpcList => Message::QueryNpcList,
            Kind::NpcList => Message::NpcList(Wire::read(&mut r)?),
            Kind::QueryNpcTalk => Message::QueryNpcTalk(NpcQuery::read(&mut r)?),
            Kind::NpcTalk => Message::NpcTalk(Wire::read(&mut r)?),
            Kind::SelectButton => Message::SelectButton(Wire::read(&mut r)?),
            Kind::ExecuteNpcServerCommand => Message::ExecuteNpcServerCommand,
            Kind::SetStoryLineStatus => Message::SetStoryLineStatus(StoryLineStatus::read(&mut r)?),
            Kind::StoryLines => Message::StoryLines(StoryLines::read(&mut r)?),
            Kind::BuyItem => Message::BuyItem(BuyItem::read(&mut r)?),
            Kind::BuyItemResult => Message::BuyItemResult(BuyItemResult::read(&mut r)?),
            Kind::QueryShopItemInfo => Message::QueryShopItemInfo(Wire::read(&mut r)?),
            Kind::SellItem => Message::SellItem(SellItem::read(&mut r)?),
            Kind::QueryBuybackList => Message::QueryBuybackList,
            Kind::BuybackListResult => Message::BuybackListResult(Wire::read(&mut r)?),
            Kind::RepairItem => Message::RepairItem(RepairItem::read(&mut r)?),
            Kind::UpdateStoryGuide => Message::UpdateStoryGuide(StoryGuide::read(&mut r)?),
            Kind::StoryVariables => Message::StoryVariables(Wire::read(&mut r)?),
            Kind::QueryQuestProgress => Message::QueryQuestProgress,
            Kind::QuestProgress => Message::QuestProgress(QuestProgress::read(&mut r)?),
            Kind::UpdateTitle => Message::UpdateTitle(Wire::read(&mut r)?),
            Kind::HideCostume => Message::HideCostume(HideCostume::read(&mut r)?),
            Kind::TitleList => Message::TitleList(TitleList::read(&mut r)?),
            Kind::EnhanceItem => Message::EnhanceItem(EnhanceItem::read(&mut r)?),
            Kind::EnhanceItemResult => Message::EnhanceItemResult(EnhanceItemResult::read(&mut r)?),
            Kind::QueryRestoreItemList => Message::QueryRestoreItemList,
            Kind::RestoreItemList => Message::RestoreItemList(Wire::read(&mut r)?),
            Kind::EnchantItem => Message::EnchantItem(EnchantItem::read(&mut r)?),
            Kind::EnchantItemResult => Message::EnchantItemResult(EnchantItemResult::read(&mut r)?),
            Kind::ManufactureInfo => Message::ManufactureInfo(ManufactureInfo::read(&mut r)?),
            Kind::ManufactureCraft => Message::ManufactureCraft(ManufactureCraft::read(&mut r)?),
            Kind::ManufactureLevelUp => {
                Message::ManufactureLevelUp(ManufactureLevelUp::read(&mut r)?)
            }
        })
    };
    if strict && !r.remaining.is_empty() {
        return Err(PayloadError::TrailingBytes.into());
    }
    Ok(result)
}
pub fn decode_ingress(
    kind: Kind,
    category: i32,
    body: &[u8],
    max_body: usize,
) -> Result<Option<Message>, InventoryWireError> {
    if category == kind.category() {
        return decode_body(kind, body, max_body, false);
    }
    if category == 0 && body.len() <= max_body && body.get(..16) == Some(kind.type_id().as_bytes())
    {
        return decode_body(kind, &body[16..], max_body.saturating_sub(16), false);
    }
    if category == 0
        && body.len() <= max_body
        && body.get(..16)
            == Some(Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21).as_bytes())
        && body.get(16..) == Some(&[0])
    {
        return Ok(None);
    }
    Err(InventoryWireError::Type)
}
