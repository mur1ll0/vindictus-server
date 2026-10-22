//! QuestContents and QuestOwner of the quest ownership service: content tables with the
//! load-time feature changes, owned quests, constraints, QuestProgressInfo and the
//! TodayQuest draw (String.GetHashCode of the configured CLR profile + System.Random).
use super::{character_stats::Arithmetic, feature_policy::FeaturePolicy, item_colors::ColorRandom};
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuestError {
    #[error("quest content is outside the admitted profile")]
    Content,
    #[error("quest feature expression is outside the admitted profile")]
    Feature,
    #[error("quest state is not migrated")]
    Unsupported,
}

/// SQL datetime units (1/300 s since 1900-01-01) per day.
pub const UNITS_PER_DAY: i64 = 300 * 86_400;
/// Quest.LastPlayTime of a new quest: DateTime.Parse("1990-01-01 00:00:00").
pub const NEVER_PLAYED: i64 = 32_872 * UNITS_PER_DAY;

/// String.GetHashCode of the .NET Framework 4.8 runtime without randomized hashing.
pub fn framework_string_hash(text: &str, arithmetic: Arithmetic) -> i32 {
    let chars: Vec<u16> = text.encode_utf16().collect();
    match arithmetic {
        Arithmetic::FrameworkX64 => {
            let (mut hash1, mut hash2) = (5381_i32, 5381_i32);
            // The loop stops at the first NUL, as the runtime reads the terminated buffer.
            let mut s = chars.iter().copied().chain(std::iter::once(0));
            loop {
                let c = s.next().unwrap_or(0);
                if c == 0 {
                    break;
                }
                hash1 = (hash1 << 5).wrapping_add(hash1) ^ i32::from(c);
                let c = s.next().unwrap_or(0);
                if c == 0 {
                    break;
                }
                hash2 = (hash2 << 5).wrapping_add(hash2) ^ i32::from(c);
            }
            hash1.wrapping_add(hash2.wrapping_mul(1_566_083_941))
        }
        Arithmetic::FrameworkX86 => {
            let start = (5381_i32 << 16).wrapping_add(5381);
            let (mut hash1, mut hash2) = (start, start);
            // Int32 reads over the NUL-terminated UTF-16 buffer (the terminator and one
            // padding char read as 0).
            let word = |i: usize| {
                let low = u32::from(chars.get(i).copied().unwrap_or(0));
                let high = u32::from(chars.get(i + 1).copied().unwrap_or(0));
                (low | (high << 16)) as i32
            };
            let step = |h: i32| (h << 5).wrapping_add(h).wrapping_add(h >> 27);
            let mut len = chars.len() as i64;
            let mut i = 0;
            while len > 2 {
                hash1 = step(hash1) ^ word(i);
                hash2 = step(hash2) ^ word(i + 2);
                i += 4;
                len -= 4;
            }
            if len > 0 {
                hash1 = step(hash1) ^ word(i);
            }
            hash1.wrapping_add(hash2.wrapping_mul(1_566_083_941))
        }
    }
}

/// Year and DayOfYear of a day number counted from 1900-01-01.
pub fn year_and_day(days: i64) -> (i64, i64) {
    // Howard Hinnant's civil_from_days (March-based year), shifted from 1970-01-01.
    let z = days - 25_567 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let year = yoe + era * 400 + i64::from(mp >= 10);
    (year, days - days_from_civil(year, 1, 1) + 1)
}

/// SQL datetime text ("yyyy-MM-dd HH:mm:ss[.fff]") in 1/300 s units from 1900-01-01.
fn parse_datetime(text: &str) -> Result<i64, QuestError> {
    let number = |s: &str| -> Result<i64, QuestError> {
        if s.is_empty() || s.len() > 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(QuestError::Content);
        }
        s.parse().map_err(|_| QuestError::Content)
    };
    let (date, time) = text
        .trim()
        .split_once(' ')
        .unwrap_or((text.trim(), "00:00:00"));
    let mut d = date.split('-');
    let (Some(y), Some(m), Some(day), None) = (d.next(), d.next(), d.next(), d.next()) else {
        return Err(QuestError::Content);
    };
    let (clock, millis) = time.split_once('.').unwrap_or((time, "0"));
    let mut t = clock.split(':');
    let (Some(h), Some(mi), Some(s), None) = (t.next(), t.next(), t.next(), t.next()) else {
        return Err(QuestError::Content);
    };
    let (m, day) = (number(m)?, number(day)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return Err(QuestError::Content);
    }
    let seconds = number(h)? * 3600 + number(mi)? * 60 + number(s)?;
    Ok(days_from_civil(number(y)?, m, day) * UNITS_PER_DAY
        + seconds * 300
        + number(millis)? * 3 / 10)
}

/// Day number from 1900-01-01 of a proleptic Gregorian date.
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468 + 25_567
}

/// QuestInfo columns the ownership service reads.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct QuestInfo {
    #[serde(rename = "ID")]
    pub id: String,
    pub auto_give_level: i32,
    pub quest_set: i32,
    pub service_coin: i32,
    pub max_member_count: i32,
    pub max_play_count: Option<i32>,
    pub enable_today_quest: Option<String>,
    pub season3_today_group: Option<String>,
    pub auto_reveal: bool,
    pub feature: Option<String>,
    pub is_season2: bool,
}

/// QuestPeriodInfo: QuestID, DateFrom, Period, MaxPlayCount, Feature.
pub type PeriodRow = (
    Option<String>,
    Option<String>,
    i32,
    Option<i32>,
    Option<String>,
);

/// QuestContents tables in provider order.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestTables {
    pub quests: Vec<QuestInfo>,
    /// QuestGoalInfo: ID, QuestID, Relation, Weight.
    pub goals: Vec<(i32, Option<String>, i32, i32)>,
    /// QuestConstraintInfo: QuestID, RequiredQuestID, RequiredStatus.
    pub constraints: Vec<(Option<String>, Option<String>, i32)>,
    /// QuestDifficultyInfo: QuestID, Difficulty, MinLevel, Feature.
    pub difficulties: Vec<(Option<String>, i32, i32, Option<String>)>,
    /// QuestPeriodInfo: QuestID, DateFrom, Period, MaxPlayCount, Feature.
    pub periods: Vec<PeriodRow>,
}

#[derive(Clone, Debug)]
struct Goal {
    id: i32,
    relation: i32,
    weight: i32,
}

/// QuestPeriodInfo: DateFrom (SQL datetime units), Period, Feature.
type QuestPeriod = (Option<i64>, i32, Option<String>);

/// QuestContents after Load (ChangeBasicBattleGoal, ChangeMaxPlayCount).
#[derive(Clone, Debug)]
pub struct QuestContents {
    quests: Vec<QuestInfo>,
    index: HashMap<String, usize>,
    /// QuestGoalInfoByQuests in first-seen order.
    goals: HashMap<String, Vec<Goal>>,
    constraints: HashMap<String, Vec<(String, i32)>>,
    /// Feature-enabled QuestDifficultyInfo: Difficulty, MinLevel.
    difficulties: HashMap<String, Vec<(i32, i32)>>,
    /// autoGiveLevelInfo (AutoGiveLevel > 0) in provider order.
    auto_give: BTreeMap<i32, Vec<usize>>,
    /// season3TodayQuestGroupInfos in first-seen order.
    season3: Vec<(String, Vec<String>)>,
    /// QuestPeriodInfo per quest: DateFrom (SQL datetime units), Period, Feature.
    periods: HashMap<String, Vec<QuestPeriod>>,
}

fn enabled(policy: &FeaturePolicy, feature: Option<&str>) -> Result<bool, QuestError> {
    policy.is_enabled(feature).map_err(|_| QuestError::Feature)
}

/// Convert.ToInt32(string): Int32.Parse with the current culture (FormatException fails).
fn convert_int(text: &str) -> Result<i32, QuestError> {
    let t = text.trim_matches(|c| matches!(c, ' ' | '\u{0009}'..='\u{000d}'));
    let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(QuestError::Content);
    }
    t.parse().map_err(|_| QuestError::Content)
}

impl QuestContents {
    pub fn load(tables: QuestTables, policy: &FeaturePolicy) -> Result<Self, QuestError> {
        if tables.quests.len() > 65_536
            || tables.goals.len() > 262_144
            || tables.constraints.len() > 65_536
            || tables.difficulties.len() > 262_144
            || tables.periods.len() > 65_536
        {
            return Err(QuestError::Content);
        }
        let mut quests = tables.quests;
        let mut index = HashMap::new();
        for (i, q) in quests.iter().enumerate() {
            // ToDictionary throws on a duplicate key.
            if index.insert(q.id.clone(), i).is_some() {
                return Err(QuestError::Content);
            }
        }
        let mut auto_give: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
        for (i, q) in quests.iter().enumerate() {
            if q.auto_give_level > 0 {
                auto_give.entry(q.auto_give_level).or_default().push(i);
            }
        }
        let mut constraints: HashMap<String, Vec<(String, i32)>> = HashMap::new();
        for (quest, required, status) in tables.constraints {
            let (Some(quest), Some(required)) = (quest, required) else {
                return Err(QuestError::Content);
            };
            constraints
                .entry(quest)
                .or_default()
                .push((required, status));
        }
        let mut goal_ids = HashSet::new();
        let mut goals: HashMap<String, Vec<Goal>> = HashMap::new();
        for (id, quest, relation, weight) in tables.goals {
            let quest = quest.ok_or(QuestError::Content)?;
            let list = goals.entry(quest).or_default();
            // ToDictionary/ToDoubleDictionary throw on duplicates.
            if !goal_ids.insert(id) || list.iter().any(|g| g.id == id) {
                return Err(QuestError::Content);
            }
            list.push(Goal {
                id,
                relation,
                weight,
            });
        }
        let mut difficulties: HashMap<String, Vec<(i32, i32)>> = HashMap::new();
        for (quest, difficulty, min_level, feature) in tables.difficulties {
            if !enabled(policy, feature.as_deref())? {
                continue;
            }
            let list = difficulties
                .entry(quest.ok_or(QuestError::Content)?)
                .or_default();
            if list.iter().any(|d| d.0 == difficulty) {
                return Err(QuestError::Content);
            }
            list.push((difficulty, min_level));
        }
        let mut contents = Self {
            quests: Vec::new(),
            index,
            goals,
            constraints,
            difficulties,
            auto_give,
            season3: Vec::new(),
            periods: HashMap::new(),
        };
        for (quest, from, period, _, feature) in &tables.periods {
            let quest = quest.as_deref().ok_or(QuestError::Content)?;
            let from = from.as_deref().map(parse_datetime).transpose()?;
            contents.periods.entry(quest.to_owned()).or_default().push((
                from,
                *period,
                feature.clone(),
            ));
        }
        contents.change_basic_battle_goal(policy, &quests)?;
        // ChangeMaxPlayCount: the last enabled period row with MaxPlayCount wins.
        for (quest, _, _, max, feature) in &tables.periods {
            let quest = quest.as_deref().ok_or(QuestError::Content)?;
            if feature.is_some()
                && enabled(policy, feature.as_deref())?
                && let Some(max) = max
                && let Some(&i) = contents.index.get(quest)
            {
                quests[i].max_play_count = Some(*max);
            }
        }
        for q in &quests {
            if let Some(group) = &q.season3_today_group
                && enabled(policy, q.enable_today_quest.as_deref())?
                && enabled(policy, q.feature.as_deref())?
            {
                match contents.season3.iter_mut().find(|(g, _)| g == group) {
                    Some(entry) => entry.1.push(q.id.clone()),
                    None => contents.season3.push((group.clone(), vec![q.id.clone()])),
                }
            }
        }
        contents.quests = quests;
        Ok(contents)
    }
    /// ChangeBasicBattleGoal on QuestGoalInfoByQuests: the first Relation 0 goal with a
    /// weight of each quest in the listed quest sets takes the configured weight.
    fn change_basic_battle_goal(
        &mut self,
        policy: &FeaturePolicy,
        quests: &[QuestInfo],
    ) -> Result<(), QuestError> {
        let parts: Vec<&str> = policy
            .get_string("ChangeBasicBattleGoal")
            .split(';')
            .collect();
        if parts.len() < 2 {
            return Ok(());
        }
        let weight = convert_int(parts[0])?;
        let _min_level = convert_int(parts[1])?;
        let sets = parts[2..]
            .iter()
            .map(|p| convert_int(p))
            .collect::<Result<Vec<_>, _>>()?;
        if weight == 0 || sets.is_empty() {
            return Ok(());
        }
        for (quest, goals) in &mut self.goals {
            for goal in goals.iter_mut() {
                if goal.relation != 0 || goal.weight == 0 {
                    continue;
                }
                let info = self
                    .index
                    .get(quest)
                    .and_then(|&i| quests.get(i))
                    .ok_or(QuestError::Content)?;
                if !sets.contains(&info.quest_set) {
                    continue;
                }
                goal.weight = weight;
                break;
            }
        }
        Ok(())
    }
    pub fn info(&self, id: &str) -> Option<&QuestInfo> {
        self.index.get(id).and_then(|&i| self.quests.get(i))
    }
    fn goals_of(&self, id: &str) -> &[Goal] {
        self.goals.get(id).map_or(&[], Vec::as_slice)
    }
    /// QuestInfo.EventAppliedMaxPlayCount.
    fn event_max_play_count(
        &self,
        info: &QuestInfo,
        policy: &FeaturePolicy,
    ) -> Result<Option<i32>, QuestError> {
        if enabled(policy, Some("Event_PlayCount_Unlimited"))? {
            return Ok(None);
        }
        let Some(max) = info.max_play_count else {
            return Ok(None);
        };
        let mut extra = 0;
        let departures = policy.get_string("AddDeparture");
        if !departures.is_empty() && departures.split(',').any(|v| v == info.id) {
            extra = policy
                .get_integer("AddDepartureCount")
                .map_err(|_| QuestError::Feature)?;
        }
        let plus = policy
            .get_integer("Event_PlayCount_Plus")
            .map_err(|_| QuestError::Feature)?;
        Ok(Some(max.wrapping_add(plus).wrapping_add(extra)))
    }
    /// QuestContentsExtension.GetMaxPlayCount(isCafe).
    pub fn max_play_count(
        &self,
        info: &QuestInfo,
        policy: &FeaturePolicy,
        cafe: bool,
    ) -> Result<Option<i32>, QuestError> {
        let max = self.event_max_play_count(info, policy)?;
        Ok(if cafe {
            max.map(|m| m.wrapping_add(1))
        } else {
            max
        })
    }
    /// TodayQuest.UpdateTodayQuest for the day of `now` (SQL datetime units, UTC).
    pub fn today_quests(
        &self,
        policy: &FeaturePolicy,
        now: i64,
        arithmetic: Arithmetic,
    ) -> Result<HashSet<String>, QuestError> {
        let mut today = HashSet::new();
        if !enabled(policy, Some("TodayQuest"))? {
            return Ok(today);
        }
        let adder = policy
            .get_integer("TodayQuest_InitTimeAdder")
            .map_err(|_| QuestError::Feature)?;
        let day = (now - i64::from(adder) * 300 * 3600).div_euclid(UNITS_PER_DAY);
        let mut sets: Vec<(i32, Vec<&str>)> = Vec::new();
        fn add<'q>(sets: &mut Vec<(i32, Vec<&'q str>)>, set: i32, id: &'q str) {
            match sets.iter_mut().find(|(s, _)| *s == set) {
                Some(entry) => entry.1.push(id),
                None => sets.push((set, vec![id])),
            }
        }
        for q in &self.quests {
            if enabled(policy, q.feature.as_deref())?
                && enabled(policy, q.enable_today_quest.as_deref())?
                && !q.is_season2
                && q.season3_today_group.is_none()
            {
                add(&mut sets, q.quest_set, q.id.as_str());
            }
        }
        let (year, day_of_year) = year_and_day(day);
        let text = format!("{year}{day_of_year}");
        let seed = framework_string_hash(&text, arithmetic);
        let mut random = ColorRandom::from_seed(seed);
        let next = |random: &mut ColorRandom, n: usize| {
            random
                .next(i32::try_from(n).map_err(|_| QuestError::Content)?)
                .map_err(|_| QuestError::Content)
                .and_then(|v| usize::try_from(v).map_err(|_| QuestError::Content))
        };
        if enabled(policy, Some("TodayQuest_S2"))? {
            let mut season2 = Vec::new();
            for q in &self.quests {
                if q.is_season2
                    && enabled(policy, q.enable_today_quest.as_deref())?
                    && enabled(policy, q.feature.as_deref())?
                {
                    season2.push(q);
                }
            }
            let pick = next(&mut random, season2.len())?;
            // ElementAt on an empty list throws.
            let q = season2.get(pick).ok_or(QuestError::Content)?;
            add(&mut sets, q.quest_set, q.id.as_str());
        }
        for (_, ids) in &sets {
            let pick = next(&mut random, ids.len())?;
            today.insert((*ids.get(pick).ok_or(QuestError::Content)?).to_owned());
        }
        if enabled(policy, Some("TodayQuest_S3"))? && !self.season3.is_empty() {
            let mut random = ColorRandom::from_seed(seed);
            for (_, ids) in &self.season3 {
                let pick = next(&mut random, ids.len())?;
                today.insert(ids.get(pick).ok_or(QuestError::Content)?.clone());
            }
        }
        Ok(today)
    }
}

/// dbo.Quest row (ROWID None until the entity ID is assigned on save).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedQuest {
    pub row_id: Option<i64>,
    pub quest: String,
    pub revealed: i32,
    pub status: i32,
    pub clear_count: i32,
    pub play_count: i32,
    /// LastPlayTime in SQL datetime units.
    pub last_play: i64,
    pub today_play_count: i32,
    pub named_kill: Option<i64>,
}

/// dbo.GoalAchievement row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalAchievement {
    pub goal: i32,
    pub status: i32,
    pub clear_count: u8,
    pub regened_ap: u8,
    pub last_clear: i64,
}

/// dbo.QuestClearTime row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestClearTime {
    pub quest: String,
    pub difficulty: i32,
    pub player_count: i32,
    pub clear_time: i32,
    pub play_rank: i32,
    pub clear_info: i32,
    pub is_old: bool,
}

/// QuestOwner.CheckConstraint results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint {
    Success,
    InvalidQuest,
    NoSuchDifficulty,
    LevelConstraintFail,
    RequiredStatusFail,
    MaxPlayCountFail,
}

/// QuestImpl.GiveQuest outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiveQuest {
    Success,
    AlreadyOwned,
    NoSuchQuest,
}

/// QuestProgressInfo values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub id: String,
    pub progress: i32,
    pub clear_count: i32,
    pub today_quest: bool,
    pub appear_named_monster: bool,
    pub available: bool,
    pub revealed: bool,
    pub max_play_count: i32,
    pub play_count: i32,
    pub clear_times: Vec<QuestClearTime>,
}

/// Values outside the quest service that its checks read.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    pub contents: &'a QuestContents,
    pub policy: &'a FeaturePolicy,
    /// IsMaxQuestConditionSatisfied (cafe bonus or VIP effect).
    pub cafe: bool,
    /// ItemOwner.GetItems("coupon_cash_raid_1").Count().
    pub raid_coupons: i64,
}

/// QuestOwner state of one character.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestOwner {
    /// Quests in load order, given quests appended.
    pub quests: Vec<OwnedQuest>,
    pub goals: Vec<GoalAchievement>,
    pub clear_times: Vec<QuestClearTime>,
    /// CharacterLevel (0 until NotifyLevel).
    pub level: i32,
}

impl QuestOwner {
    /// Load: one ClearTimeDict entry per (quest, difficulty); merged duplicates
    /// (QuestClearTime.Apply) are not migrated.
    pub fn load(
        quests: Vec<OwnedQuest>,
        goals: Vec<GoalAchievement>,
        clear_times: Vec<QuestClearTime>,
        contents: &QuestContents,
    ) -> Result<Self, QuestError> {
        for (i, q) in quests.iter().enumerate() {
            if quests[..i].iter().any(|o| o.quest == q.quest) {
                return Err(QuestError::Content);
            }
        }
        for (i, g) in goals.iter().enumerate() {
            if goals[..i].iter().any(|o| o.goal == g.goal) {
                return Err(QuestError::Content);
            }
        }
        for (i, c) in clear_times.iter().enumerate() {
            if clear_times[..i]
                .iter()
                .any(|o| o.quest == c.quest && o.difficulty == c.difficulty)
            {
                return Err(QuestError::Unsupported);
            }
        }
        let _ = contents;
        Ok(Self {
            quests,
            goals,
            clear_times,
            level: 0,
        })
    }
    /// InitQuestStatus: Status is the weight of the achieved goals of each quest.
    pub fn init_status(&mut self, contents: &QuestContents) {
        for i in 0..self.quests.len() {
            self.quests[i].status = self.weight_of(contents, &self.quests[i].quest, None);
        }
    }
    fn weight_of(&self, contents: &QuestContents, quest: &str, difficulty: Option<i32>) -> i32 {
        let mut sum = 0_i32;
        for goal in contents.goals_of(quest) {
            let Some(a) = self.goals.iter().find(|a| a.goal == goal.id) else {
                continue;
            };
            let accepted = match difficulty {
                None => true,
                // (Status | difficulty.ToBitField()) == Status.
                Some(d) => {
                    let bit = 1_i32.wrapping_shl(d as u32 & 31);
                    a.status | bit == a.status
                }
            };
            if accepted {
                sum = sum.wrapping_add(goal.weight);
            }
        }
        sum
    }
    pub fn owned(&self, quest: &str) -> Option<&OwnedQuest> {
        self.quests.iter().find(|q| q.quest == quest)
    }
    /// GetQuestProgress(questID, -1).
    pub fn progress_of(&self, quest: &str) -> i32 {
        self.owned(quest).map_or(-1, |q| q.status)
    }
    /// GetAchievedGoalIDs.
    pub fn achieved_goal_ids(&self, contents: &QuestContents, quest: &str) -> Vec<i32> {
        if contents.info(quest).is_none() {
            return Vec::new();
        }
        contents
            .goals_of(quest)
            .iter()
            .filter(|g| self.goals.iter().any(|a| a.goal == g.id))
            .map(|g| g.id)
            .collect()
    }
    /// QuestOwner.GiveQuest without Save.
    fn give(&mut self, info: &QuestInfo) -> GiveQuest {
        if self.owned(&info.id).is_some() {
            return GiveQuest::AlreadyOwned;
        }
        self.quests.push(OwnedQuest {
            row_id: None,
            quest: info.id.clone(),
            revealed: i32::from(info.auto_reveal),
            status: 0,
            clear_count: 0,
            play_count: 0,
            last_play: NEVER_PLAYED,
            today_play_count: 0,
            named_kill: None,
        });
        GiveQuest::Success
    }
    /// QuestImpl.GiveQuest(questID, reveal): the ServiceCoin of a newly given quest.
    pub fn give_quest(
        &mut self,
        contents: &QuestContents,
        quest: &str,
        reveal: bool,
    ) -> (GiveQuest, i32) {
        let Some(info) = contents.info(quest) else {
            return (GiveQuest::NoSuchQuest, 0);
        };
        let given = self.give(info);
        if reveal && let Some(q) = self.quests.iter_mut().find(|q| q.quest == quest) {
            q.revealed = 1;
        }
        match given {
            // AlreadyOwned is an error the source undoes unless reveal succeeded.
            GiveQuest::AlreadyOwned if !reveal => (GiveQuest::AlreadyOwned, 0),
            GiveQuest::Success => (GiveQuest::Success, info.service_coin),
            // AlreadyOwned with reveal: RevealQuest succeeds.
            _ => (GiveQuest::Success, 0),
        }
    }
    /// Quests a login gives: the first quest when missing, then NotifyLevel auto-gives.
    pub fn login_openings(&self, contents: &QuestContents, level: i32) -> usize {
        let mut owned: HashSet<&str> = self.quests.iter().map(|q| q.quest.as_str()).collect();
        let mut count = 0;
        if !owned.contains(FIRST_QUEST) && contents.info(FIRST_QUEST).is_some() {
            owned.insert(FIRST_QUEST);
            count += 1;
        }
        let levels = if self.level < level {
            contents.auto_give.range(self.level..=level)
        } else {
            contents.auto_give.range(1..1)
        };
        for (_, list) in levels {
            for &i in list {
                if let Some(q) = contents.quests.get(i)
                    && owned.insert(q.id.as_str())
                {
                    count += 1;
                }
            }
        }
        count
    }
    /// QuestOwner constructor (first quest given and revealed) then NotifyLevel.
    pub fn login(&mut self, contents: &QuestContents, level: i32) {
        if self.owned(FIRST_QUEST).is_none()
            && let Some(info) = contents.info(FIRST_QUEST)
        {
            self.give(info);
            if let Some(q) = self.quests.iter_mut().find(|q| q.quest == FIRST_QUEST) {
                q.revealed = 1;
            }
        }
        if self.level < level {
            for i in self.level..=level {
                for &q in contents.auto_give.get(&i).map_or(&[][..], Vec::as_slice) {
                    if let Some(info) = contents.quests.get(q) {
                        self.give(info);
                    }
                }
            }
        }
        self.level = level;
    }
    /// QuestOwner.CheckConstraint (the E3/PAX cheat bypass needs Cheat permission).
    pub fn check_constraint(
        &self,
        ctx: Context<'_>,
        quest: &str,
        difficulty: i32,
        ignore_play_count: bool,
    ) -> Result<Constraint, QuestError> {
        let Some(info) = ctx.contents.info(quest) else {
            return Ok(Constraint::InvalidQuest);
        };
        let difficulty_info = ctx
            .contents
            .difficulties
            .get(quest)
            .and_then(|l| l.iter().find(|d| d.0 == difficulty));
        if difficulty > 0 {
            match difficulty_info {
                None => return Ok(Constraint::NoSuchDifficulty),
                Some(d) if self.level < d.1 => return Ok(Constraint::LevelConstraintFail),
                Some(_) => {}
            }
        }
        if info.feature.as_deref().is_some_and(|f| !f.is_empty())
            && !enabled(ctx.policy, info.feature.as_deref())?
        {
            return Ok(Constraint::InvalidQuest);
        }
        let Some(owned) = self.owned(quest) else {
            return Ok(Constraint::InvalidQuest);
        };
        for (required, status) in ctx
            .contents
            .constraints
            .get(quest)
            .map_or(&[][..], Vec::as_slice)
        {
            if self.progress_of(required) < *status {
                return Ok(Constraint::RequiredStatusFail);
            }
        }
        if !ignore_play_count
            && ctx.raid_coupons <= 0
            && let Some(max) = ctx.contents.max_play_count(info, ctx.policy, ctx.cafe)?
            && owned.play_count >= max
        {
            return Ok(Constraint::MaxPlayCountFail);
        }
        Ok(Constraint::Success)
    }
    /// QuestOwner.GetQuestProgress() at `now` (SQL datetime units, UTC).
    pub fn progress(
        &self,
        ctx: Context<'_>,
        now: i64,
        arithmetic: Arithmetic,
    ) -> Result<Vec<Progress>, QuestError> {
        let today = ctx.contents.today_quests(ctx.policy, now, arithmetic)?;
        let adder = ctx
            .policy
            .get_integer("TodayQuest_InitTimeAdder")
            .map_err(|_| QuestError::Feature)?;
        let named_day = (now - i64::from(adder) * 300 * 3600).div_euclid(UNITS_PER_DAY);
        let mut out = Vec::new();
        for q in &self.quests {
            let Some(info) = ctx.contents.info(&q.quest) else {
                continue;
            };
            if info.feature.as_deref().is_some_and(|f| !f.is_empty())
                && !enabled(ctx.policy, info.feature.as_deref())?
            {
                continue;
            }
            let difficulty = if info.is_season2 { 0 } else { 2 };
            let today_quest = q.last_play.div_euclid(UNITS_PER_DAY)
                != now.div_euclid(UNITS_PER_DAY)
                && today.contains(&q.quest);
            let named_kill = q
                .named_kill
                .is_some_and(|k| k.div_euclid(UNITS_PER_DAY) == named_day);
            out.push(Progress {
                id: q.quest.clone(),
                progress: q.status,
                clear_count: q.clear_count,
                today_quest,
                appear_named_monster: !named_kill,
                available: self.check_constraint(ctx, &q.quest, difficulty, false)?
                    == Constraint::Success,
                revealed: q.revealed != 0,
                max_play_count: ctx
                    .contents
                    .max_play_count(info, ctx.policy, ctx.cafe)?
                    .unwrap_or(-1),
                play_count: q.play_count,
                clear_times: self
                    .clear_times
                    .iter()
                    .filter(|c| c.quest == q.quest)
                    .cloned()
                    .collect(),
            });
        }
        Ok(out)
    }
    /// QuestOwner.IsTodayQuest(questID) at `now` (SQL datetime units, UTC).
    pub fn is_today_quest(
        &self,
        ctx: Context<'_>,
        quest: &str,
        now: i64,
        arithmetic: Arithmetic,
    ) -> Result<bool, QuestError> {
        let Some(q) = self.owned(quest) else {
            return Ok(false);
        };
        Ok(
            q.last_play.div_euclid(UNITS_PER_DAY) != now.div_euclid(UNITS_PER_DAY)
                && ctx
                    .contents
                    .today_quests(ctx.policy, now, arithmetic)?
                    .contains(quest),
        )
    }
    /// QuestOwner.UpdateQuestPlayCount(questID, startTime, IgnoreTodayCount): the period
    /// and day resets, then PlayCount/TodayPlayCount (or one raid coupon past the maximum,
    /// returned as `Ok(Some(true))`). `Ok(None)` is the original false (no quest or a
    /// failed constraint).
    pub fn update_play_count(
        &mut self,
        ctx: Context<'_>,
        quest: &str,
        start: i64,
        ignore_today_count: bool,
    ) -> Result<Option<bool>, QuestError> {
        let Some(index) = self.quests.iter().position(|q| q.quest == quest) else {
            return Ok(None);
        };
        let info = ctx.contents.info(quest).ok_or(QuestError::Content)?;
        if self.check_constraint(ctx, quest, 0, true)? != Constraint::Success {
            return Ok(None);
        }
        let q = &mut self.quests[index];
        let mut reset = false;
        for (from, period, feature) in ctx
            .contents
            .periods
            .get(quest)
            .map_or(&[][..], Vec::as_slice)
        {
            if feature.is_some() && !enabled(ctx.policy, feature.as_deref())? {
                continue;
            }
            // DateFrom is NOT NULL in the original table.
            let from = from.ok_or(QuestError::Content)?;
            // TimeSpan.Days truncates toward zero; a zero Period divides by zero.
            let days = |t: i64| (t - from) / UNITS_PER_DAY;
            let (last, now) = (days(q.last_play), days(start));
            let period = i64::from(*period);
            if period == 0 {
                return Err(QuestError::Content);
            }
            if last / period != now / period {
                reset = true;
            }
        }
        if reset {
            q.play_count = 0;
        }
        if q.last_play.div_euclid(UNITS_PER_DAY) != start.div_euclid(UNITS_PER_DAY) {
            q.today_play_count = 0;
        }
        let mut coupon = false;
        if !ignore_today_count {
            match ctx.contents.max_play_count(info, ctx.policy, ctx.cafe)? {
                Some(max) if q.play_count >= max && ctx.raid_coupons > 0 => coupon = true,
                _ => {
                    q.play_count = q.play_count.wrapping_add(1);
                    q.today_play_count = q.today_play_count.wrapping_add(1);
                }
            }
        }
        q.last_play = start;
        Ok(Some(coupon))
    }
    /// QuestOwner.UpdateQuest: each goal of `quest` gets the difficulty bit and
    /// QuestGoalInfo.AddClearCount (`add_clear`), or a new achievement cleared once at
    /// `now`; then the (quest, difficulty) clear time unless `clear_time` is Int32.MaxValue.
    #[allow(clippy::too_many_arguments)]
    pub fn update_quest(
        &mut self,
        contents: &QuestContents,
        quest: &str,
        goals: &[(i32, i64)],
        (difficulty, members, clear_time, play_rank, clear_info): (i32, i32, i32, i32, i32),
        now: i64,
        add_clear: &dyn Fn(i32, i64, &mut u8, &mut u8, &mut i64) -> bool,
    ) -> bool {
        if self.owned(quest).is_none() {
            return false;
        }
        let bit = 1_i32.wrapping_shl(difficulty as u32 & 31);
        for &(goal, cleared) in goals {
            if !contents.goals_of(quest).iter().any(|g| g.id == goal) {
                continue;
            }
            match self.goals.iter_mut().find(|a| a.goal == goal) {
                Some(a) => {
                    a.status |= bit;
                    let (mut count, mut regen, mut last) =
                        (a.clear_count, a.regened_ap, a.last_clear);
                    if add_clear(goal, cleared, &mut count, &mut regen, &mut last) {
                        (a.clear_count, a.regened_ap, a.last_clear) = (count, regen, last);
                    }
                }
                None => self.goals.push(GoalAchievement {
                    goal,
                    status: bit,
                    clear_count: 1,
                    regened_ap: 0,
                    last_clear: now,
                }),
            }
        }
        if clear_time != i32::MAX {
            match self
                .clear_times
                .iter_mut()
                .find(|c| c.quest == quest && c.difficulty == difficulty)
            {
                // QuestClearTime.Apply.
                Some(c) => {
                    if c.clear_info < clear_info {
                        c.clear_info = clear_info;
                        c.clear_time = clear_time;
                    } else if c.clear_info == clear_info && c.clear_time > clear_time {
                        c.clear_time = clear_time;
                    }
                    if c.play_rank < play_rank {
                        c.play_rank = play_rank;
                    }
                }
                None => self.clear_times.push(QuestClearTime {
                    quest: quest.to_owned(),
                    difficulty,
                    player_count: members,
                    clear_time,
                    play_rank,
                    clear_info,
                    is_old: false,
                }),
            }
        }
        true
    }
    /// QuestOwner.UpdateQuestStatus: ClearCount++ and CalculateQuestStatus.
    pub fn update_quest_status(&mut self, contents: &QuestContents, quest: &str) -> bool {
        let Some(index) = self.quests.iter().position(|q| q.quest == quest) else {
            return false;
        };
        let status = self.weight_of(contents, quest, None);
        let q = &mut self.quests[index];
        q.clear_count = q.clear_count.wrapping_add(1);
        q.status = status;
        true
    }
}

/// The quest every QuestOwner gives and reveals when missing.
pub const FIRST_QUEST: &str = "quest_gnoll_watchman";
