//! Pet rules of PlayerService.ItemServiceCore.PetSystem.PetManager (1.69): the ItemContents
//! pet dictionaries, the name check, experience, hunger, active time, status, slots, cash
//! skills and the automatic feeding of Pet_food_V2. Pets belong to the account (OwnerUID).
//! Times are .NET ticks (UTC).
use serde::Deserialize;
use std::collections::BTreeMap;
use thiserror::Error;

/// PetStatus.
pub mod status {
    pub const EXPIRED: u8 = 1;
    pub const DISABLED: u8 = 2;
    pub const USEABLE: u8 = 3;
    pub const SELECTED: u8 = 4;
}
/// PetManager.MaxPetSlotSize.
pub const MAX_PET_SLOTS: i32 = 100;
/// PetManager.MaxCashSlot and the two cash skill slots.
pub const MAX_CASH_SKILLS: usize = 2;
pub const CASH_SLOT_FIRST: i32 = 101;
pub const CASH_SLOT_SECOND: i32 = 102;
pub const TICKS_PER_SECOND: i64 = 10_000_000;
pub const TICKS_PER_HOUR: i64 = 3600 * TICKS_PER_SECOND;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PetContentError {
    #[error("pet content row is repeated or out of range")]
    Invalid,
    #[error("pet content feature expression is not supported")]
    Feature,
}

/// PetOperationResult (its name is the dialog suffix GameUI_Heroes_Pet_Fail_<name>).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetResult {
    Success,
    DbTransactionFail,
    CreateNoPetInfo,
    CreateNotPurchaseEnable,
    CreateAllocateFail,
    CreateForbiddenName,
    CreateAgainstNamingRule,
    CreateHasSameName,
    GiveSkillNoSelectedPet,
    GiveSkillNoEmptySlot,
    GiveSkillNoPetSkillInfo,
    GiveSkillAlreadyHasSkill,
    GiveSkillAlreadyHasCashSkill,
    FeedNoSelectedPet,
    FeedAlreadyMaxHunger,
    FeedNoEmptySlot,
    RemoveSkillNotHasSkill,
    RemoveSkillExpiredTimeRemain,
    RemoveSkillNoPetInfo,
    ChangeNameHasNotPet,
}
impl PetResult {
    /// Enum.ToString of the source PetOperationResult.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::DbTransactionFail => "DBTransactionFail",
            Self::CreateNoPetInfo => "PetCreate_NoPetInfo",
            Self::CreateNotPurchaseEnable => "PetCreate_NotPurchaseEnable",
            Self::CreateAllocateFail => "PetCreate_AllocateFail",
            Self::CreateForbiddenName => "PetCreate_ForbiddenName",
            Self::CreateAgainstNamingRule => "PetCreate_AgainstNamingRule",
            Self::CreateHasSameName => "PetCreate_HasSameName",
            Self::GiveSkillNoSelectedPet => "GiveSkill_NoSelectedPet",
            Self::GiveSkillNoEmptySlot => "GiveSkill_NoEmptySlot",
            Self::GiveSkillNoPetSkillInfo => "GiveSkill_NoPetSkillInfo",
            Self::GiveSkillAlreadyHasSkill => "GiveSkill_AlreadyHasSkill",
            Self::GiveSkillAlreadyHasCashSkill => "GiveSkill_AlreadyHasCashSkill",
            Self::FeedNoSelectedPet => "Feed_NoSelectedPet",
            Self::FeedAlreadyMaxHunger => "Feed_AlreadyMaxHunger",
            Self::FeedNoEmptySlot => "Feed_NoEmptySlot",
            Self::RemoveSkillNotHasSkill => "RemoveSkill_NotHasSkill",
            Self::RemoveSkillExpiredTimeRemain => "RemoveSkill_ExpiredTimeRemain",
            Self::RemoveSkillNoPetInfo => "RemoveSkill_NoPetInfo",
            Self::ChangeNameHasNotPet => "ChangeName_HasNotPet",
        }
    }
    /// The source's LogOperation code of PetLedger.Event (the enum ordinal).
    pub const fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::DbTransactionFail => 1,
            Self::CreateNoPetInfo => 3,
            Self::CreateNotPurchaseEnable => 5,
            Self::CreateAllocateFail => 6,
            Self::CreateForbiddenName => 7,
            Self::CreateAgainstNamingRule => 8,
            Self::CreateHasSameName => 9,
            Self::GiveSkillNoSelectedPet => 10,
            Self::GiveSkillNoEmptySlot => 11,
            Self::GiveSkillNoPetSkillInfo => 12,
            Self::GiveSkillAlreadyHasSkill => 14,
            Self::GiveSkillAlreadyHasCashSkill => 15,
            Self::FeedNoSelectedPet => 16,
            Self::FeedAlreadyMaxHunger => 17,
            Self::FeedNoEmptySlot => 18,
            Self::RemoveSkillNotHasSkill => 19,
            Self::RemoveSkillExpiredTimeRemain => 20,
            Self::RemoveSkillNoPetInfo => 22,
            Self::ChangeNameHasNotPet => 23,
        }
    }
}

/// PetInfo (DeactiveTime and Feature are not read by the server).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PetInfo {
    pub pet_id: i32,
    pub default_level: i32,
    pub stat_type: i32,
    pub max_active_time: i32,
    pub item_class: Option<String>,
    pub max_hunger: i32,
    pub hunger_time: i32,
    pub hunger_decrease: i32,
}
/// PetStatusBalanceInfo of one StatType and level.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Balance {
    pub stat_type: i32,
    pub level: i32,
    pub required_exp: i32,
    pub max_exp: i32,
    pub hp: i32,
    pub res_damage: i32,
    pub hp_recovery: i32,
    pub def_break: i32,
    pub atk_balance: i32,
    pub atk: i32,
    pub defense: i32,
    pub critical: i32,
    pub res_critical: i32,
}
/// PetSkillInfo columns the server reads.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillInfo {
    pub skill_id: i32,
    pub cool_time_type: Option<i32>,
}
/// DefaultPetSkillInfo: content skills, never stored.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DefaultSkill {
    pub pet_id: i32,
    pub slot_order: i32,
    pub skill_id: i32,
    pub open_level: i32,
}
/// PetActiveTimePeriodInfo row with its Feature expression.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Period {
    pub pet_id: i32,
    pub date_from_ticks: i64,
    pub period: i32,
    pub feature: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Feed {
    pub feed_type: i32,
    pub item_class: Option<String>,
}
/// ItemClassInfo.ExtraData of a pet_ item.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PetItem {
    pub item_class: String,
    pub extra_data: Option<String>,
}
/// One FeatureMatrix value of the pet system.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FeatureValue {
    pub key: String,
    pub value: Option<String>,
}
/// The exported pet tables.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Export {
    /// PetPurchaseLimit, PetMaxLevel, Pet_ExpBattle_Point, NamingRuleMaxBytes (and the raw
    /// Pet/Pet_food_V2 versions, whose IsEnable needs the measured version pairs).
    #[serde(default)]
    pub features: Vec<FeatureValue>,
    pub infos: Vec<PetInfo>,
    pub balance: Vec<Balance>,
    pub skills: Vec<SkillInfo>,
    pub default_skills: Vec<DefaultSkill>,
    pub periods: Vec<Period>,
    pub feeds: Vec<Feed>,
    pub items: Vec<PetItem>,
}

/// ItemContents pet dictionaries.
#[derive(Clone, Debug, Default)]
pub struct Contents {
    infos: BTreeMap<i32, PetInfo>,
    balance: BTreeMap<(i32, i32), Balance>,
    skills: BTreeMap<i32, SkillInfo>,
    defaults: BTreeMap<i32, Vec<DefaultSkill>>,
    periods: BTreeMap<i32, Period>,
    feeds: BTreeMap<u8, String>,
    items: BTreeMap<String, Option<String>>,
    features: BTreeMap<String, String>,
}
impl Contents {
    /// ToDictionary semantics: a repeated key is refused. `enabled` evaluates the period
    /// Feature (FeatureMatrix.IsEnable); only enabled periods are kept.
    pub fn new(
        export: Export,
        enabled: impl Fn(Option<&str>) -> Option<bool>,
    ) -> Result<Self, PetContentError> {
        let mut c = Self::default();
        for i in export.infos {
            if c.infos.insert(i.pet_id, i).is_some() {
                return Err(PetContentError::Invalid);
            }
        }
        for b in export.balance {
            if c.balance.insert((b.stat_type, b.level), b).is_some() {
                return Err(PetContentError::Invalid);
            }
        }
        for s in export.skills {
            if c.skills.insert(s.skill_id, s).is_some() {
                return Err(PetContentError::Invalid);
            }
        }
        for d in export.default_skills {
            c.defaults.entry(d.pet_id).or_default().push(d);
        }
        for p in export.periods {
            if !enabled(p.feature.as_deref()).ok_or(PetContentError::Feature)? {
                continue;
            }
            if p.period <= 0 || c.periods.insert(p.pet_id, p).is_some() {
                return Err(PetContentError::Invalid);
            }
        }
        for f in export.feeds {
            let key = u8::try_from(f.feed_type).map_err(|_| PetContentError::Invalid)?;
            let class = f.item_class.ok_or(PetContentError::Invalid)?;
            if c.feeds.insert(key, class).is_some() {
                return Err(PetContentError::Invalid);
            }
        }
        for i in export.items {
            if c.items.insert(i.item_class, i.extra_data).is_some() {
                return Err(PetContentError::Invalid);
            }
        }
        for f in export.features {
            if c.features
                .insert(f.key, f.value.unwrap_or_default())
                .is_some()
            {
                return Err(PetContentError::Invalid);
            }
        }
        Ok(c)
    }
    /// FeatureMatrix.GetInteger of an exported pet feature.
    pub fn feature_int(&self, key: &str) -> Option<i32> {
        parse_int(self.features.get(key)?)
    }
    /// FeatureMatrix.GetFloat of an exported pet feature.
    pub fn feature_float(&self, key: &str) -> Option<f32> {
        self.features.get(key)?.trim().parse().ok()
    }
    pub fn info(&self, pet_type: i32) -> Option<&PetInfo> {
        self.infos.get(&pet_type)
    }
    pub fn infos(&self) -> impl Iterator<Item = &PetInfo> {
        self.infos.values()
    }
    pub fn balance(&self, stat_type: i32, level: i32) -> Option<&Balance> {
        self.balance.get(&(stat_type, level))
    }
    pub fn skill(&self, skill_id: i32) -> Option<&SkillInfo> {
        self.skills.get(&skill_id)
    }
    /// GetDefaultPetSkillInfo (empty when the type has none).
    pub fn default_skills(&self, pet_type: i32) -> &[DefaultSkill] {
        self.defaults.get(&pet_type).map_or(&[], Vec::as_slice)
    }
    pub fn period(&self, pet_type: i32) -> Option<&Period> {
        self.periods.get(&pet_type)
    }
    /// GetPetFeedItemClass.
    pub fn feed_class(&self, feed_type: u8) -> Option<&str> {
        self.feeds.get(&feed_type).map(String::as_str)
    }
    /// GetPetFeedType (0 when the class is not a pet food).
    pub fn feed_type(&self, item_class: &str) -> u8 {
        self.feeds
            .iter()
            .find(|(_, c)| c.as_str() == item_class)
            .map_or(0, |(t, _)| *t)
    }
    /// int.TryParse(ItemClassInfo.ExtraData) of a pet_ item.
    pub fn extra_int(&self, item_class: &str) -> Option<i32> {
        parse_int(self.items.get(item_class)?.as_deref()?)
    }
}
/// int.TryParse with NumberStyles.Integer (surrounding white space, a leading sign).
fn parse_int(text: &str) -> Option<i32> {
    let t = text.trim_matches(|c: char| matches!(c, ' ' | '\t'..='\r'));
    let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// FeatureMatrix values of the pet system.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// IsEnable("Pet").
    pub enabled: bool,
    /// IsEnable("Pet_food_V2").
    pub food_v2: bool,
    pub purchase_limit: i32,
    pub max_level: i32,
    /// GetFloat("Pet_ExpBattle_Point") / 100.
    pub exp_battle_rate: f64,
    /// NamingRuleMaxBytes.
    pub name_max_bytes: i32,
}

/// One cash skill row (PetSkill).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PetSkill {
    pub row_id: i64,
    pub slot: i32,
    pub skill_id: i32,
    pub open_level: i32,
    pub cool_time_type: u8,
    pub expired_ticks: Option<i64>,
}
/// One Pet row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pet {
    pub id: i64,
    pub owner_uid: i32,
    pub name: String,
    pub pet_type: i32,
    pub stat_type: i32,
    pub slot: i32,
    pub status: u8,
    pub level: i32,
    pub exp: i32,
    pub total_exp: i32,
    pub hunger: i32,
    pub remain_active_time: Option<i32>,
    pub deactive_start_ticks: Option<i64>,
    pub expired_ticks: Option<i64>,
    pub skills: Vec<PetSkill>,
}
impl Pet {
    /// PushAddPet with the PetInfo initial values (status Useabled, full hunger).
    pub fn new(id: i64, owner_uid: i32, name: String, info: &PetInfo, slot: i32, now: i64) -> Self {
        Self {
            id,
            owner_uid,
            name,
            pet_type: info.pet_id,
            stat_type: info.stat_type,
            slot,
            status: status::USEABLE,
            level: info.default_level,
            exp: 0,
            total_exp: 0,
            hunger: info.max_hunger,
            remain_active_time: Some(info.max_active_time),
            deactive_start_ticks: Some(now),
            expired_ticks: None,
            skills: Vec::new(),
        }
    }
}

/// CheckPetName against the account's pets. Forbidden names are not checked: free names
/// (operator decision 26/10/2026, as character names). The rule is NamingRule (only the
/// recovered EN-EU pattern), with Length ≤ NamingRuleMaxBytes for an ASCII name.
pub fn check_name(name: &str, settings: &Settings, pets: &[Pet]) -> PetResult {
    let units: Vec<u16> = name.encode_utf16().collect();
    let rule = super::character_name::NameRule::recovered(
        super::character_name::EN_EU_NAMING_RULE,
        settings.name_max_bytes,
        0,
    );
    match rule.and_then(|r| r.check_valid_name(Some(&units))) {
        Ok(true) => {}
        _ => return PetResult::CreateAgainstNamingRule,
    }
    let lower = name.to_lowercase();
    if pets.iter().any(|p| p.name.to_lowercase() == lower) {
        return PetResult::CreateHasSameName;
    }
    PetResult::Success
}
/// GetEmptyPetSlot: the lowest free slot below MaxPetSlotSize.
pub fn empty_slot(pets: &[Pet]) -> Option<i32> {
    (0..MAX_PET_SLOTS).find(|s| !pets.iter().any(|p| p.slot == *s))
}
/// IsPetPurchaseEnable.
pub fn purchase_enabled(settings: &Settings, count: usize) -> bool {
    settings.enabled && usize::try_from(settings.purchase_limit).is_ok_and(|l| l > count)
}
/// SwapSlot: Some(pets whose slot changed) when `dest` is within PetPurchaseLimit.
pub fn swap_slot(pets: &mut [Pet], id: i64, dest: i32, settings: &Settings) -> Option<Vec<i64>> {
    if dest < 0 || settings.purchase_limit <= dest {
        return None;
    }
    let from = pets.iter().find(|p| p.id == id)?.slot;
    let mut changed = Vec::new();
    for p in pets.iter_mut() {
        if p.id == id {
            p.slot = dest;
            changed.push(p.id);
        } else if p.slot == dest {
            p.slot = from;
            changed.push(p.id);
        }
    }
    Some(changed)
}

/// PushGivePetExp: levels up through PetStatusBalanceInfo while both levels exist; the
/// level stops at PetMaxLevel with no experience. Returns the TotalExp gained.
pub fn give_exp(pet: &mut Pet, exp: i32, settings: &Settings, contents: &Contents) -> Option<i32> {
    let max = settings.max_level;
    if max < 0 {
        return None;
    }
    let before = pet.total_exp;
    if max <= pet.level {
        pet.level = max;
        pet.exp = 0;
        return Some(0);
    }
    pet.exp = pet.exp.wrapping_add(exp);
    pet.total_exp = pet.total_exp.wrapping_add(exp);
    while let (Some(now), Some(_)) = (
        contents.balance(pet.stat_type, pet.level),
        contents.balance(pet.stat_type, pet.level + 1),
    ) {
        if pet.exp < now.required_exp {
            break;
        }
        pet.exp -= now.required_exp;
        pet.level += 1;
        if max <= pet.level {
            pet.level = max;
            pet.total_exp -= pet.exp;
            pet.exp = 0;
        }
    }
    Some(pet.total_exp.wrapping_sub(before))
}
/// PushSetPetLevelExp (GM): the highest existing level at or below `level`.
pub fn set_level_exp(
    pet: &mut Pet,
    level: i32,
    exp: i32,
    settings: &Settings,
    contents: &Contents,
) -> bool {
    if settings.max_level < 0 || level > settings.max_level {
        return false;
    }
    let mut n = level;
    loop {
        if contents.balance(pet.stat_type, n).is_some() {
            break;
        }
        n -= 1;
        if n == pet.level || n < 0 {
            break;
        }
    }
    let Some(b) = contents.balance(pet.stat_type, n) else {
        return false;
    };
    pet.level = n;
    pet.exp = if b.required_exp > exp {
        exp
    } else {
        b.required_exp - 1
    };
    true
}
/// PushFeedPet: false at full hunger; the hunger stops at MaxHunger.
pub fn feed(pet: &mut Pet, amount: i32, info: &PetInfo) -> bool {
    if pet.hunger == info.max_hunger {
        return false;
    }
    pet.hunger = pet.hunger.saturating_add(amount).min(info.max_hunger);
    true
}
/// PushExhaustPet: hunger falls HungerDecrease × playTime / HungerTime. Returns the decrease.
pub fn exhaust(pet: &mut Pet, play_time: i32, info: &PetInfo) -> i32 {
    let decrease = if info.hunger_time > 0 {
        info.hunger_decrease.wrapping_mul(play_time) / info.hunger_time
    } else {
        info.hunger_decrease.wrapping_mul(play_time)
    };
    pet.hunger = if pet.hunger - decrease < 0 {
        0
    } else {
        pet.hunger - decrease
    };
    decrease
}
/// UpdatePetStatus. The source marks the pet expired while ExpiredTime is still ahead,
/// which no 1.69 path reaches (ExpiredTime is always null); the expiry is applied once it
/// has passed.
pub fn update_status(pet: &mut Pet, now: i64) {
    if pet.expired_ticks.is_some_and(|e| e <= now) {
        pet.remain_active_time = Some(0);
        pet.status = status::EXPIRED;
    } else if pet.status == status::DISABLED {
        if pet.remain_active_time.is_some_and(|t| t > 0) {
            pet.status = status::USEABLE;
        }
    } else if (pet.status == status::USEABLE || pet.status == status::SELECTED)
        && pet.remain_active_time.unwrap_or(0) <= 0
    {
        pet.remain_active_time = Some(0);
        pet.status = status::DISABLED;
        pet.deactive_start_ticks = Some(now);
    }
}
/// PushDecreaseActiveTime then UpdatePetStatus.
pub fn decrease_active_time(pet: &mut Pet, play_time: i32, now: i64) {
    pet.remain_active_time = Some(match pet.remain_active_time {
        Some(t) if t - play_time > 0 => t - play_time,
        _ => 0,
    });
    update_status(pet, now);
}
/// PushResetActiveTime.
pub fn reset_active_time(pet: &mut Pet, info: &PetInfo, now: i64) {
    pet.deactive_start_ticks = Some(now);
    pet.remain_active_time = Some(info.max_active_time);
    if pet.status == status::DISABLED {
        pet.status = status::USEABLE;
    }
}
fn period_index(period: &Period, ticks: i64) -> i64 {
    // (int)Math.Floor(TimeSpan.TotalHours) / Period.
    (ticks - period.date_from_ticks).div_euclid(TICKS_PER_HOUR) / i64::from(period.period)
}
/// DailyActiveTimeResetProcess for one pet: whether its DeactiveStartTime lies in an earlier
/// period than now, and the start of the next period.
pub fn period_reset(period: &Period, deactive_start: i64, now: i64) -> (bool, i64) {
    let index = period_index(period, now);
    let next = period.date_from_ticks + (index + 1) * i64::from(period.period) * TICKS_PER_HOUR;
    (index != period_index(period, deactive_start), next)
}
/// CalculateAutoFeedTypeAndNum: (feed type, count, hunger) in FeedType order, filling the
/// goal with whole foods; `amount` is the food's ExtraData.
pub fn auto_feed_plan(
    shared: &BTreeMap<u8, i32>,
    mut goal: i32,
    amount: impl Fn(u8) -> Option<i32>,
) -> Vec<(u8, i32, i32)> {
    let mut plan = Vec::new();
    for (&feed, &have) in shared {
        let Some(each) = amount(feed) else { continue };
        if each <= 0 || goal < each {
            break;
        }
        let wanted = goal / each;
        if wanted < have {
            plan.push((feed, wanted, each * wanted));
            break;
        }
        plan.push((feed, have, each * have));
        goal -= each * have;
    }
    plan
}
/// One food of AutoPetFeed: FeedType, count used and the hunger it gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fed {
    pub feed_type: u8,
    pub count: i32,
    pub hunger: i32,
}
/// AutoPetFeed over the account's shared food (FeedType → count), changed in place; the
/// food amount is the ExtraData of the PetFeedInfo item.
pub fn auto_feed(
    pet: &mut Pet,
    info: &PetInfo,
    shared: &mut BTreeMap<u8, i32>,
    contents: &Contents,
) -> Vec<Fed> {
    if pet.hunger == info.max_hunger {
        return Vec::new();
    }
    let plan = auto_feed_plan(shared, info.max_hunger - pet.hunger, |t| {
        contents.extra_int(contents.feed_class(t)?)
    });
    let mut fed = Vec::new();
    for (feed_type, count, hunger) in plan {
        let Some(have) = shared.get(&feed_type).copied() else {
            continue;
        };
        if have - count == 0 {
            shared.remove(&feed_type);
        } else {
            shared.insert(feed_type, have - count);
        }
        pet.hunger = pet.hunger.saturating_add(hunger);
        fed.push(Fed {
            feed_type,
            count,
            hunger,
        });
    }
    fed
}
/// What UpdatePetAfterBattle changed, for PetLedger.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Battle {
    pub exp_gained: Option<i32>,
    pub hunger_decrease: i32,
    pub fed: Vec<Fed>,
}
/// UpdatePetAfterBattle: experience (capped at MaxExp of the level), hunger (with
/// AutoPetFeed when `shared` is the Pet_food_V2 food) and active time. None for a disabled
/// pet or missing content, as the source's false.
pub fn after_battle(
    pet: &mut Pet,
    play_time: i32,
    exp: i32,
    settings: &Settings,
    contents: &Contents,
    shared: Option<&mut BTreeMap<u8, i32>>,
    now: i64,
) -> Option<Battle> {
    if pet.status == status::DISABLED {
        return None;
    }
    let mut battle = Battle::default();
    if exp > 0 {
        let b = contents.balance(pet.stat_type, pet.level)?;
        battle.exp_gained = Some(give_exp(pet, exp.min(b.max_exp), settings, contents)?);
    }
    let info = contents.info(pet.pet_type)?;
    battle.hunger_decrease = exhaust(pet, play_time, info);
    if let Some(shared) = shared {
        battle.fed = auto_feed(pet, info, shared, contents);
    }
    decrease_active_time(pet, play_time, now);
    Some(battle)
}
/// EmptyCashSkillSlot: 101, or 102 when the first cash skill is in 101; None when full.
pub fn cash_skill_slot(pet: &Pet) -> Option<i32> {
    if pet.skills.len() >= MAX_CASH_SKILLS {
        return None;
    }
    Some(match pet.skills.first() {
        Some(s) if s.slot == CASH_SLOT_FIRST => CASH_SLOT_SECOND,
        _ => CASH_SLOT_FIRST,
    })
}
/// GiveCashSkill checks: the slot for a skill the pet has neither by default nor by cash.
pub fn check_cash_skill(pet: &Pet, skill_id: i32, contents: &Contents) -> Result<i32, PetResult> {
    let slot = cash_skill_slot(pet).ok_or(PetResult::GiveSkillNoEmptySlot)?;
    if contents.skill(skill_id).is_none() {
        return Err(PetResult::GiveSkillNoPetSkillInfo);
    }
    if contents
        .default_skills(pet.pet_type)
        .iter()
        .any(|d| d.skill_id == skill_id)
    {
        return Err(PetResult::GiveSkillAlreadyHasSkill);
    }
    if pet.skills.iter().any(|s| s.skill_id == skill_id) {
        return Err(PetResult::GiveSkillAlreadyHasCashSkill);
    }
    Ok(slot)
}
/// UseInventoryItemProcessor.GetTimeSpanInfoByItemClass: the number after the first "day"
/// (else after the first "min") of the item name, 0 when it does not parse; None for a
/// name with neither.
pub fn item_validity_ticks(item_class: &str) -> Option<i64> {
    if let Some(rest) = item_class.split("day").nth(1) {
        return Some(i64::from(parse_int(rest).unwrap_or(0)) * 24 * TICKS_PER_HOUR);
    }
    let rest = item_class.split("min").nth(1)?;
    Some(i64::from(parse_int(rest).unwrap_or(0)) * 60 * TICKS_PER_SECOND)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            enabled: true,
            food_v2: true,
            purchase_limit: 100,
            max_level: 3,
            exp_battle_rate: 0.2,
            name_max_bytes: 20,
        }
    }
    fn contents() -> Contents {
        let row = |level, required_exp| Balance {
            stat_type: 1,
            level,
            required_exp,
            max_exp: 50,
            hp: 1,
            res_damage: 0,
            hp_recovery: 0,
            def_break: 0,
            atk_balance: 0,
            atk: 0,
            defense: 0,
            critical: 0,
            res_critical: 0,
        };
        Contents::new(
            Export {
                features: vec![FeatureValue {
                    key: "PetMaxLevel".into(),
                    value: Some(" 30".into()),
                }],
                infos: vec![PetInfo {
                    pet_id: 518,
                    default_level: 1,
                    stat_type: 1,
                    max_active_time: 86400,
                    item_class: Some("pet_create_x".into()),
                    max_hunger: 200,
                    hunger_time: 36,
                    hunger_decrease: 1,
                }],
                balance: vec![row(1, 88), row(2, 287), row(3, 464)],
                skills: vec![
                    SkillInfo {
                        skill_id: 6,
                        cool_time_type: Some(0),
                    },
                    SkillInfo {
                        skill_id: 9,
                        cool_time_type: Some(0),
                    },
                ],
                default_skills: vec![DefaultSkill {
                    pet_id: 518,
                    slot_order: 0,
                    skill_id: 6,
                    open_level: 1,
                }],
                periods: vec![
                    Period {
                        pet_id: 518,
                        date_from_ticks: 634_647_492_000_000_000,
                        period: 24,
                        feature: Some("enUS".into()),
                    },
                    Period {
                        pet_id: 518,
                        date_from_ticks: 0,
                        period: 24,
                        feature: Some("koKR".into()),
                    },
                ],
                feeds: vec![Feed {
                    feed_type: 1,
                    item_class: Some("pet_feed_make".into()),
                }],
                items: vec![PetItem {
                    item_class: "pet_feed_make".into(),
                    extra_data: Some(" 25 ".into()),
                }],
            },
            |f| Some(f == Some("enUS")),
        )
        .unwrap()
    }
    fn pet() -> Pet {
        let c = contents();
        Pet::new(1, 7, "Rex".into(), c.info(518).unwrap(), 0, 100)
    }

    #[test]
    fn content_keeps_the_enabled_period_and_parses_extra_data() {
        let c = contents();
        assert_eq!(
            c.period(518).unwrap().date_from_ticks,
            634_647_492_000_000_000
        );
        assert_eq!(c.extra_int("pet_feed_make"), Some(25));
        assert_eq!(c.feed_type("pet_feed_make"), 1);
        assert_eq!(c.feed_type("pet_feed_large"), 0);
        assert_eq!(c.default_skills(519), &[]);
        assert_eq!(c.feature_int("PetMaxLevel"), Some(30));
        assert_eq!(c.feature_int("PetPurchaseLimit"), None);
    }
    #[test]
    fn names_follow_the_rule_and_are_unique_per_account() {
        let s = settings();
        assert_eq!(check_name("Bolt", &s, &[]), PetResult::Success);
        assert_eq!(
            check_name("Rex", &s, &[]),
            PetResult::CreateAgainstNamingRule
        );
        assert_eq!(check_name("rexx", &s, &[]), PetResult::Success);
        let mut p = pet();
        p.name = "Bolt".into();
        assert_eq!(check_name("BOLT", &s, &[p]), PetResult::CreateHasSameName);
        assert_eq!(
            check_name("Abcdefghijklmnopqrstu", &s, &[]),
            PetResult::CreateAgainstNamingRule
        );
    }
    #[test]
    fn experience_levels_up_to_the_maximum() {
        let (s, c) = (settings(), contents());
        let mut p = pet();
        assert_eq!(give_exp(&mut p, 100, &s, &c), Some(100));
        assert_eq!((p.level, p.exp), (2, 12));
        assert_eq!(give_exp(&mut p, 300, &s, &c), Some(275));
        assert_eq!((p.level, p.exp, p.total_exp), (3, 0, 375));
        assert_eq!(give_exp(&mut p, 10, &s, &c), Some(0));
    }
    #[test]
    fn battle_caps_experience_and_spends_hunger_and_time() {
        let (s, c) = (settings(), contents());
        let mut p = pet();
        let battle = after_battle(&mut p, 3600, 1000, &s, &c, None, 200).unwrap();
        assert_eq!(battle.exp_gained, Some(50));
        assert_eq!((p.level, p.exp), (1, 50));
        assert_eq!((p.hunger, p.remain_active_time), (100, Some(82800)));
        let mut shared = BTreeMap::from([(1u8, 3)]);
        let battle = after_battle(&mut p, 3600, 0, &s, &c, Some(&mut shared), 250).unwrap();
        assert_eq!(
            battle.fed,
            vec![Fed {
                feed_type: 1,
                count: 3,
                hunger: 75
            }]
        );
        assert_eq!((p.hunger, shared.len()), (75, 0));
        assert!(after_battle(&mut p, 90000, 0, &s, &c, None, 300).is_some());
        assert_eq!((p.hunger, p.status), (0, status::DISABLED));
        assert_eq!(p.deactive_start_ticks, Some(300));
        assert!(after_battle(&mut p, 10, 10, &s, &c, None, 400).is_none());
    }
    #[test]
    fn periods_reset_once_per_period() {
        let c = contents();
        let period = c.period(518).unwrap();
        let start = period.date_from_ticks;
        let (reset, next) =
            period_reset(period, start + TICKS_PER_HOUR, start + 23 * TICKS_PER_HOUR);
        assert!(!reset);
        assert_eq!(next, start + 24 * TICKS_PER_HOUR);
        assert!(period_reset(period, start, start + 25 * TICKS_PER_HOUR).0);
        let mut p = pet();
        p.status = status::DISABLED;
        p.remain_active_time = Some(0);
        reset_active_time(&mut p, c.info(518).unwrap(), 9);
        assert_eq!(
            (p.status, p.remain_active_time),
            (status::USEABLE, Some(86400))
        );
    }
    #[test]
    fn automatic_feeding_fills_whole_foods_in_type_order() {
        let shared = BTreeMap::from([(1u8, 3), (2u8, 10)]);
        let amount = |t: u8| Some(if t == 1 { 25 } else { 100 });
        assert_eq!(auto_feed_plan(&shared, 60, amount), vec![(1, 2, 50)]);
        assert_eq!(
            auto_feed_plan(&shared, 275, amount),
            vec![(1, 3, 75), (2, 2, 200)]
        );
        assert_eq!(auto_feed_plan(&shared, 10, amount), vec![]);
    }
    #[test]
    fn slots_cash_skills_and_validity() {
        let (s, c) = (settings(), contents());
        let mut pets = vec![pet(), pet()];
        pets[1].id = 2;
        pets[1].slot = 1;
        assert_eq!(empty_slot(&pets), Some(2));
        assert_eq!(swap_slot(&mut pets, 1, 1, &s), Some(vec![1, 2]));
        assert_eq!((pets[0].slot, pets[1].slot), (1, 0));
        assert_eq!(swap_slot(&mut pets, 1, 100, &s), None);
        let mut p = pet();
        assert_eq!(check_cash_skill(&p, 9, &c), Ok(101));
        assert_eq!(
            check_cash_skill(&p, 6, &c),
            Err(PetResult::GiveSkillAlreadyHasSkill)
        );
        assert_eq!(
            check_cash_skill(&p, 99, &c),
            Err(PetResult::GiveSkillNoPetSkillInfo)
        );
        p.skills.push(PetSkill {
            row_id: 1,
            slot: 101,
            skill_id: 9,
            open_level: 1,
            cool_time_type: 0,
            expired_ticks: None,
        });
        assert_eq!(cash_skill_slot(&p), Some(102));
        assert_eq!(
            item_validity_ticks("pet_skill_atk_buff_day30"),
            Some(30 * 24 * TICKS_PER_HOUR)
        );
        assert_eq!(
            item_validity_ticks("pet_skill_atk_buff_min30"),
            Some(30 * 60 * TICKS_PER_SECOND)
        );
        assert_eq!(item_validity_ticks("pet_feed_make"), None);
    }
}
