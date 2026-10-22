//! MicroPlay quest goals and rewards: TargetTracker/TrackingElement, MicroPlayer
//! AddQuestGoalReward with QuestGoalInfo.GetAP/AddClearCount, the reward lists of
//! Reward_SetPenaltyMultiplyer and the migrated sources of Reward_SetBonusMultiplyer, and
//! Reward_CalculateReward with its QuestSummaryInfo lines.
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GoalError {
    /// TargetCount.Value of a tracked goal is null (the original constructor throws).
    #[error("quest goal content is invalid")]
    Content,
    /// A Dictionary.Add of the reward calculation meets a duplicate key.
    #[error("quest reward has a duplicate key")]
    Duplicate,
}

const MAX_PATTERN: usize = 512;

/// QuestGoalInfo columns used after mission entry (goal IDs are unique).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct GoalInfo {
    pub id: i32,
    pub target_count: Option<i32>,
    pub is_positive: i32,
    pub is_party: i32,
    pub gold: i32,
    pub xp: i32,
    pub max_ap: i32,
    pub min_ap: i32,
    pub step_ap: i32,
    pub ap_regen_time: i32,
    pub ap_regen_max: i32,
    pub item: Option<String>,
    pub item_num: i32,
    pub item_multiplier_feature: Option<String>,
    pub item_multiplier: i32,
    pub weight: i32,
}

impl GoalInfo {
    /// QuestGoalInfo.GetAP(now, …); `minutes` is (now − lastClearTime).TotalMinutes.
    pub fn ap(&self, clear_count: u8, regened_ap: u8, minutes: f64) -> i32 {
        let mut base = self
            .max_ap
            .wrapping_sub(self.step_ap.wrapping_mul(i32::from(clear_count)));
        if base < self.min_ap {
            base = self.min_ap;
        }
        let mut regen = 0_i32;
        if self.ap_regen_time > 0 {
            regen = i32::from(regened_ap)
                .wrapping_add((minutes / f64::from(self.ap_regen_time)) as i32)
                .max(0);
            if base.wrapping_add(regen) > self.ap_regen_max {
                regen = self.ap_regen_max.wrapping_sub(base).max(0);
            }
        }
        base.wrapping_add(regen)
    }
    /// QuestGoalInfo.AddClearCount with times in SQL datetime units (1/300 s).
    pub fn add_clear_count(
        &self,
        cleared: i64,
        clear_count: &mut u8,
        regened_ap: &mut u8,
        last_clear: &mut i64,
    ) {
        let mut span = 0_i64;
        if self.ap_regen_time > 0 && cleared > *last_clear {
            let regen_units = i64::from(self.ap_regen_time) * 60 * 300;
            let elapsed = cleared - *last_clear;
            let regens = ((elapsed as f64 / 18_000.0 / f64::from(self.ap_regen_time)) as i32).max(0);
            span = (elapsed - i64::from(regens) * regen_units).max(0);
            if regens.wrapping_add(i32::from(*regened_ap)) > 255 {
                *regened_ap = u8::MAX;
            } else {
                *regened_ap = regened_ap.wrapping_add(regens as u8);
            }
        }
        *last_clear = cleared - span;
        let mut ap = self
            .max_ap
            .wrapping_sub(self.step_ap.wrapping_mul(i32::from(*clear_count)));
        if ap <= self.min_ap {
            ap = self.min_ap;
        }
        if self.ap_regen_max < i32::from(*regened_ap).wrapping_add(ap) {
            *regened_ap = if ap > self.ap_regen_max {
                0
            } else {
                self.ap_regen_max.wrapping_sub(ap) as u8
            };
        }
        if ap <= self.min_ap {
            *regened_ap = (i32::from(*regened_ap).max(self.step_ap) - self.step_ap) as u8;
        } else {
            *clear_count = clear_count.wrapping_add(1);
        }
    }
}

/// A QuestGoalInfo of the MicroPlay quest (GetQuestGoalInfos order).
#[derive(Clone, Copy, Debug)]
pub struct QuestGoal<'a> {
    pub id: i32,
    pub relation: i32,
    pub target: Option<&'a str>,
    pub game_setting: Option<&'a str>,
    pub info: &'a GoalInfo,
}

/// TrackingElement; a pattern the regex engine rejects never matches.
#[derive(Clone, Debug)]
pub struct TrackingElement {
    pub goal: i32,
    pub relation: i32,
    pub is_party: bool,
    pub positive: bool,
    pub current: i32,
    pub target: i32,
    regex: Option<Regex>,
}
impl TrackingElement {
    fn new(goal: &QuestGoal<'_>, uncompiled: &mut usize) -> Result<Self, GoalError> {
        let pattern = goal.target.unwrap_or("");
        // new Regex(Target, RegexOptions.Compiled): case-sensitive.
        let regex = if pattern.len() > MAX_PATTERN {
            None
        } else {
            RegexBuilder::new(pattern).size_limit(1 << 20).build().ok()
        };
        if regex.is_none() {
            *uncompiled += 1;
        }
        Ok(Self {
            goal: goal.id,
            relation: goal.relation,
            is_party: goal.info.is_party == 1,
            positive: goal.info.is_positive == 1,
            current: 0,
            target: goal.info.target_count.ok_or(GoalError::Content)?,
            regex,
        })
    }
    fn matches(&self, event: &str) -> bool {
        self.regex.as_ref().is_some_and(|r| r.is_match(event))
    }
    /// TrackingElement.IsSuccess.
    pub fn is_success(&self) -> bool {
        if self.positive {
            self.current == self.target
        } else {
            self.current < self.target
        }
    }
}

/// One MicroPlay.Trigger_QuestGoal call; `cid` −1 is a party goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoalTrigger {
    pub cid: i64,
    pub goal: i32,
    pub relation: i32,
    pub is_party: bool,
    pub success: bool,
    /// Tracker.MainGoalAchieved when the trigger runs.
    pub main_goal_achieved: bool,
}

/// TargetTracker of one MicroPlay.
#[derive(Clone, Debug)]
pub struct TargetTracker {
    pub common: Vec<TrackingElement>,
    pub individual: Vec<(i64, Vec<TrackingElement>)>,
    /// SettingSwear: goal ID, relation and IsParty.
    pub setting_swear: Option<(i32, i32, bool)>,
    pub essential_left: Vec<i32>,
    pub main_goal_achieved: bool,
    pub is_single_boss: bool,
    pub uncompiled: usize,
}

impl TargetTracker {
    /// TargetTracker(play) over GetQuestGoalInfos(QuestID, SwearID) for the players `cids`.
    pub fn new(goals: &[QuestGoal<'_>], swear_id: i32, cids: &[i64]) -> Result<Self, GoalError> {
        let mut tracker = Self {
            common: Vec::new(),
            individual: cids.iter().map(|&c| (c, Vec::new())).collect(),
            setting_swear: None,
            essential_left: Vec::new(),
            main_goal_achieved: false,
            is_single_boss: false,
            uncompiled: 0,
        };
        let mut bosses = 0;
        for g in goals {
            if g.relation == 2 && g.id != swear_id {
                continue;
            }
            if g.relation == 0 && g.info.is_positive == 1 && !tracker.essential_left.contains(&g.id) {
                tracker.essential_left.push(g.id);
            }
            if g.relation == 3 {
                bosses += 1;
            }
            if g.game_setting.is_some() {
                if g.id == swear_id {
                    tracker.setting_swear = Some((g.id, g.relation, g.info.is_party == 1));
                }
                continue;
            }
            if g.target.is_none() {
                continue;
            }
            if g.info.is_party == 1 {
                let element = TrackingElement::new(g, &mut tracker.uncompiled)?;
                tracker.common.push(element);
                continue;
            }
            for i in 0..tracker.individual.len() {
                let element = TrackingElement::new(g, &mut tracker.uncompiled)?;
                tracker.individual[i].1.push(element);
            }
        }
        tracker.is_single_boss = bosses == 1;
        Ok(tracker)
    }
    /// TargetTracker.Process(actor, eventstr).
    pub fn process(&mut self, actor: i64, event: &str) -> Vec<GoalTrigger> {
        let mut triggers = Vec::new();
        for i in 0..self.common.len() {
            let e = &mut self.common[i];
            if e.current >= e.target || !e.matches(event) {
                continue;
            }
            e.current = e.current.wrapping_add(1);
            if e.current != e.target {
                continue;
            }
            let (goal, relation, is_party, success) = (e.goal, e.relation, e.is_party, e.is_success());
            if relation == 0 {
                self.essential_left.retain(|g| *g != goal);
                if self.essential_left.is_empty() {
                    self.main_goal_achieved = true;
                }
            } else if relation == 3 || relation == 4 {
                self.main_goal_achieved = true;
            }
            triggers.push(GoalTrigger {
                cid: -1,
                goal,
                relation,
                is_party,
                success,
                main_goal_achieved: self.main_goal_achieved,
            });
        }
        let main = self.main_goal_achieved;
        if let Some((_, list)) = self.individual.iter_mut().find(|(c, _)| *c == actor) {
            for e in list {
                if e.current < e.target && e.matches(event) {
                    e.current = e.current.wrapping_add(1);
                    if e.current == e.target {
                        triggers.push(GoalTrigger {
                            cid: actor,
                            goal: e.goal,
                            relation: e.relation,
                            is_party: e.is_party,
                            success: e.is_success(),
                            main_goal_achieved: main,
                        });
                    }
                }
            }
        }
        triggers
    }
    /// TargetTracker.OnQuestFinished: unmet negative goals, then the swear for the party
    /// or each player, then the unmet negative individual goals.
    pub fn on_quest_finished(&self) -> Vec<GoalTrigger> {
        let main = self.main_goal_achieved;
        let trigger = |cid: i64, e: &TrackingElement| GoalTrigger {
            cid,
            goal: e.goal,
            relation: e.relation,
            is_party: e.is_party,
            success: e.is_success(),
            main_goal_achieved: main,
        };
        let mut triggers: Vec<GoalTrigger> = self
            .common
            .iter()
            .filter(|e| e.current < e.target && !e.positive)
            .map(|e| trigger(-1, e))
            .collect();
        if let Some((goal, relation, is_party)) = self.setting_swear {
            let swear = |cid| GoalTrigger {
                cid,
                goal,
                relation,
                is_party,
                success: true,
                main_goal_achieved: main,
            };
            if is_party {
                triggers.push(swear(-1));
            } else {
                triggers.extend(self.individual.iter().map(|(c, _)| swear(*c)));
            }
        }
        for (cid, list) in &self.individual {
            triggers.extend(
                list.iter()
                    .filter(|e| e.current < e.target && !e.positive)
                    .map(|e| trigger(*cid, e)),
            );
        }
        triggers
    }
}

/// QuestGoalRewardInfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalReward {
    pub goal: i32,
    pub exp: i32,
    pub gold: i32,
    pub ap: i32,
    pub item: Option<String>,
    pub item_num: i32,
    pub item_multiplier_feature: Option<String>,
    pub item_multiplier: i32,
    /// GoalClearTime (DateTime.Now) in SQL datetime units.
    pub clear_time: i64,
    pub quest_service_updated: bool,
}

/// The achieved goal of the character when the mission started (AchieveGoalInfo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Achieved {
    pub clear_count: u8,
    pub regened_ap: u8,
    pub last_clear: i64,
}

/// MicroPlayer.AddQuestGoalReward for a known QuestDifficultyInfo: `ratios` are
/// GoalExpRatio/GoalGoldRatio, `rates` ExpRate_Combat/GoldRate_Combat/APRate_Combat.
pub fn goal_reward(
    info: &GoalInfo,
    ratios: (i32, i32),
    rates: (i32, i32, i32),
    achieved: Option<Achieved>,
    today_quest: bool,
    now: i64,
) -> GoalReward {
    let scale = |value: i32, ratio: i32, rate: i32| {
        (value.wrapping_mul(ratio) / 10_000)
            .wrapping_mul(100)
            .wrapping_mul(rate)
            / 100
    };
    let (item, item_num) = match &info.item {
        Some(i) if !i.is_empty() && info.item_num != 0 => (Some(i.clone()), info.item_num),
        _ => (None, 0),
    };
    let mut ap = match achieved {
        Some(a) if !today_quest => {
            info.ap(a.clear_count, a.regened_ap, (now - a.last_clear) as f64 / 18_000.0)
        }
        _ => info.max_ap,
    };
    if !today_quest {
        ap = ap.wrapping_mul(rates.2) / 100;
    }
    GoalReward {
        goal: info.id,
        exp: scale(info.xp, ratios.0, rates.0),
        gold: scale(info.gold, ratios.1, rates.1),
        ap,
        item,
        item_num,
        item_multiplier_feature: info.item_multiplier_feature.clone(),
        item_multiplier: info.item_multiplier,
        clear_time: now,
        quest_service_updated: false,
    }
}

/// MicroPlayerRewardInfo: Desc, DescArg and Value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewardLine {
    pub desc: String,
    pub arg: i32,
    pub value: i32,
}
/// List.AddMultiplier: only positive multipliers are kept.
fn add_multiplier(list: &mut Vec<RewardLine>, desc: &str, arg: i32, multiplier: i32) {
    if multiplier > 0 {
        list.push(RewardLine {
            desc: desc.to_owned(),
            arg,
            value: multiplier,
        });
    }
}
/// List.AddReward: an existing Desc takes the argument and adds the value.
fn add_reward(list: &mut Vec<RewardLine>, desc: &str, arg: i32, value: i32) {
    match list.iter_mut().find(|l| l.desc == desc) {
        Some(l) => {
            l.arg = arg;
            l.value = l.value.wrapping_add(value);
        }
        None => list.push(RewardLine {
            desc: desc.to_owned(),
            arg,
            value,
        }),
    }
}

/// Character, quest and feature facts the reward multipliers read.
#[derive(Clone, Debug, Default)]
pub struct RewardFacts {
    pub quest_id: String,
    pub level: i32,
    /// FeatureMatrix MaxLevel.
    pub max_level: i32,
    pub quest_level: i32,
    pub is_season2: bool,
    pub difficulty: i32,
    pub game_mode: i32,
    /// QuestDifficultyInfo.MaxLevel.
    pub difficulty_max_level: i32,
    pub today_quest: bool,
    pub today_play_count: i32,
    pub free_quest: bool,
    pub free_repeat_penalty: bool,
    pub daily_free_play_count: i32,
    pub main_goal_achieved: bool,
    pub extreme_mode: bool,
    pub xe_play_rank: i32,
    pub combat_gold_renewal: bool,
    /// ChristmasCounterEvent EXP, Gold and AP rates when enabled.
    pub christmas: Option<(i32, i32, i32)>,
    /// TargetQuestEvent_Exp/Gold (0 read as 100), _Quest and _Text.
    pub target_event: (i32, i32, String, String),
}

/// The reward lists of one MicroPlayer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RewardLists {
    pub additional_battle_exp: Vec<RewardLine>,
    pub battle_exp_multiplier: Vec<RewardLine>,
    pub exp_multiplier: Vec<RewardLine>,
    pub main_gold_multiplier: Vec<RewardLine>,
    pub additional_gold: Vec<RewardLine>,
    pub gold_multiplier: Vec<RewardLine>,
    pub quest_ap_multiplier: Vec<RewardLine>,
    pub additional_ap: Vec<RewardLine>,
    pub ap_multiplier: Vec<RewardLine>,
    pub level_gold_penalty: Option<RewardLine>,
    pub camping_gold_penalty: Option<RewardLine>,
}

impl RewardLists {
    /// InitializeRewardInfo with Reward_SetPenaltyMultiplyer (no platinum coin, no giant raid).
    pub fn new(f: &RewardFacts) -> Self {
        let mut lists = Self::default();
        if f.free_repeat_penalty {
            if f.free_quest && f.level > 10 && f.daily_free_play_count > 10 {
                lists.camping_gold_penalty = Some(RewardLine {
                    desc: "CampingPenalty".into(),
                    arg: f.daily_free_play_count,
                    value: 0,
                });
            }
        } else if f.level > 10 && f.today_play_count > 10 && f.free_quest {
            lists.camping_gold_penalty = Some(RewardLine {
                desc: "CampingPenalty".into(),
                arg: f.today_play_count,
                value: 100 - f.today_play_count.saturating_sub(10).saturating_mul(10).min(100),
            });
        }
        let reference = if f.is_season2 {
            f.difficulty_max_level
        } else {
            f.quest_level
        };
        let level = f64::from(f.level);
        if (f.is_season2 || !f.today_quest)
            && reference < 60
            && f.level > 10
            && f.game_mode != 3
            && f64::from(reference) < level * 0.87
        {
            let value = if reference >= (level * 0.84 - 1.5) as i32 {
                75
            } else if reference >= (level * 0.81 - 3.5) as i32 {
                50
            } else {
                0
            };
            lists.level_gold_penalty = Some(RewardLine {
                desc: "LevelPenalty".into(),
                arg: 0,
                value,
            });
        }
        lists
    }
    /// Reward_SetBonusMultiplyer for the migrated sources. Status effects, cafes, coins,
    /// bless stones, guilds, friends, party and boost events add nothing in the laboratory.
    pub fn add_bonuses(&mut self, f: &RewardFacts) {
        if let Some((exp, _, _)) = f.christmas {
            add_multiplier(&mut self.battle_exp_multiplier, "ChristmasCounterBonus", 0, exp);
        }
        if f.max_level == f.level {
            if f.level < f.quest_level.wrapping_add(20) {
                add_multiplier(&mut self.gold_multiplier, "MaxLevel", 0, 20);
            } else if f.level < f.quest_level.wrapping_add(40) {
                add_multiplier(&mut self.gold_multiplier, "MaxLevel", 0, 10);
            }
        }
        if let Some((_, gold, ap)) = f.christmas {
            add_multiplier(&mut self.gold_multiplier, "ChristmasCounterBonus", 0, gold);
            add_reward(&mut self.additional_ap, "ChristmasCounterBonus", 0, ap);
        }
        if f.extreme_mode {
            let (exp, gold, ap) = match f.xe_play_rank {
                1 => (10, 0, 0),
                2 => (50, 1, 0),
                3 => (60, 2, 0),
                4 => (70, 5, 0),
                5 => (85, 7, 0),
                6 => (100, 10, 5),
                _ => (0, 0, 0),
            };
            add_multiplier(&mut self.exp_multiplier, "XERankBonus", f.xe_play_rank, exp);
            add_multiplier(&mut self.gold_multiplier, "XERankBonus", f.xe_play_rank, gold);
            add_reward(&mut self.additional_ap, "XERankBonus", f.xe_play_rank, ap);
        }
        if f.game_mode == 3 {
            add_reward(&mut self.exp_multiplier, "DifficultyBonus", f.difficulty, 50);
            add_reward(&mut self.gold_multiplier, "DifficultyBonus", f.difficulty, 100);
            if f.combat_gold_renewal {
                add_reward(&mut self.gold_multiplier, "CombatGold_Renewal", f.difficulty, 100);
            }
            if f.main_goal_achieved {
                add_reward(&mut self.additional_ap, "DifficultyBonus", f.difficulty, 5);
            }
        } else if f.difficulty == 2 && !f.is_season2 {
            add_reward(&mut self.exp_multiplier, "DifficultyBonus", f.difficulty, 30);
            add_reward(&mut self.gold_multiplier, "DifficultyBonus", f.difficulty, 30);
            if f.combat_gold_renewal {
                add_reward(&mut self.gold_multiplier, "CombatGold_Renewal", f.difficulty, 30);
            }
        }
        let (mut exp, mut gold, quests, text) = f.target_event.clone();
        if exp == 0 {
            exp = 100;
        }
        if gold == 0 {
            gold = 100;
        }
        if exp != 100 || gold != 100 {
            let listed = quests
                .split([';', ',', ' '])
                .any(|q| q == f.quest_id || (f.is_season2 && q == "season2"));
            if listed {
                add_reward(&mut self.exp_multiplier, &text, 0, exp.wrapping_sub(100));
                add_reward(&mut self.gold_multiplier, &text, 0, gold.wrapping_sub(100));
            }
        }
    }
}

/// SummaryRewardInfo: Desc, Arg1, Arg2 and Value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryLine {
    pub desc: String,
    pub arg1: i32,
    pub arg2: i32,
    pub value: i32,
}
fn summary_multiplier(list: &mut Vec<SummaryLine>, info: &RewardLine, value: i32) {
    if value != 0 {
        list.push(SummaryLine {
            desc: info.desc.clone(),
            arg1: info.arg,
            arg2: info.value,
            value,
        });
    }
}
fn summary_reward(list: &mut Vec<SummaryLine>, info: &RewardLine) {
    if info.value != 0 {
        list.push(SummaryLine {
            desc: info.desc.clone(),
            arg1: info.arg,
            arg2: 0,
            value: info.value,
        });
    }
}

/// The QuestSummaryInfo parts filled by Reward_CalculateReward.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// ClearedGoals: GoalID → (Exp, Gold, Ap).
    pub cleared_goals: Vec<(i32, (i32, i32, i32))>,
    pub battle_exp: Vec<SummaryLine>,
    pub etc_exp: Vec<SummaryLine>,
    pub main_gold: Vec<SummaryLine>,
    pub etc_gold: Vec<SummaryLine>,
    pub quest_ap: Vec<SummaryLine>,
    pub etc_ap: Vec<SummaryLine>,
}

/// The RewardCharacter values of Reward_CalculateReward.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Calculation {
    pub exp: i32,
    pub gold: i32,
    pub ap: i32,
    /// QuestGoal: goal ID → GoalClearTime of the goals not yet sent.
    pub quest_goals: Vec<(i32, i64)>,
    /// ItemRewards: item key → count.
    pub items: Vec<(String, i32)>,
    pub summary: Summary,
}

fn add_unique<K: PartialEq, V>(list: &mut Vec<(K, V)>, key: K, value: V) -> Result<(), GoalError> {
    if list.iter().any(|(k, _)| *k == key) {
        return Err(GoalError::Duplicate);
    }
    list.push((key, value));
    Ok(())
}

/// MicroPlayer.Reward_CalculateReward. `game_mode` is the sector sequence's
/// QuestDifficultyInfo.GameMode and `enabled` the FeatureMatrix of item multipliers.
pub fn calculate(
    main_goal_achieved: bool,
    main: &mut [GoalReward],
    sub: &mut [GoalReward],
    game_mode: i32,
    monster_kill_exp: i32,
    lists: &RewardLists,
    enabled: &dyn Fn(&str) -> bool,
) -> Result<Calculation, GoalError> {
    let mut c = Calculation::default();
    let (mut goal_exp, mut sub_gold, mut main_gold, mut goal_ap) = (0_i32, 0_i32, 0_i32, 0_i32);
    if main_goal_achieved {
        for g in main.iter_mut() {
            goal_exp = goal_exp.wrapping_add(g.exp);
            main_gold = main_gold.wrapping_add(g.gold);
            goal_ap = goal_ap.wrapping_add(g.ap);
            if !g.quest_service_updated {
                add_unique(&mut c.quest_goals, g.goal, g.clear_time)?;
                g.quest_service_updated = true;
            }
            if let Some(item) = &g.item
                && game_mode != 4
            {
                for key in item.split(';') {
                    let mut count = g.item_num;
                    if let Some(feature) = g.item_multiplier_feature.as_deref()
                        && !feature.is_empty()
                        && enabled(feature)
                        && g.item_multiplier > 0
                    {
                        count = count.wrapping_mul(g.item_multiplier);
                    }
                    add_unique(&mut c.items, key.to_owned(), count)?;
                }
            }
            add_unique(&mut c.summary.cleared_goals, g.goal, (g.exp, g.gold, g.ap))?;
        }
        for g in sub.iter_mut() {
            goal_exp = goal_exp.wrapping_add(g.exp);
            sub_gold = sub_gold.wrapping_add(g.gold);
            goal_ap = goal_ap.wrapping_add(g.ap);
            if !g.quest_service_updated {
                add_unique(&mut c.quest_goals, g.goal, g.clear_time)?;
                g.quest_service_updated = true;
            }
            if let Some(item) = &g.item
                && game_mode != 4
            {
                add_unique(&mut c.items, item.clone(), g.item_num)?;
            }
            add_unique(&mut c.summary.cleared_goals, g.goal, (g.exp, g.gold, g.ap))?;
        }
    }
    let s = &mut c.summary;
    let monster = RewardLine {
        desc: "MonsterKill".into(),
        arg: 0,
        value: monster_kill_exp,
    };
    let mut battle = monster_kill_exp;
    summary_reward(&mut s.battle_exp, &monster);
    for l in &lists.additional_battle_exp {
        battle = battle.wrapping_add(l.value);
        summary_reward(&mut s.battle_exp, l);
    }
    let mut battle_bonus = 0_i32;
    for l in &lists.battle_exp_multiplier {
        let v = battle.wrapping_mul(l.value) / 100;
        battle_bonus = battle_bonus.wrapping_add(v);
        summary_multiplier(&mut s.battle_exp, l, v);
    }
    let exp_base = battle.wrapping_add(battle_bonus).wrapping_add(goal_exp);
    let mut exp = exp_base;
    for l in &lists.exp_multiplier {
        let v = exp_base.wrapping_mul(l.value) / 100;
        exp = exp.wrapping_add(v);
        summary_multiplier(&mut s.etc_exp, l, v);
    }
    let mut main_bonus = 0_i32;
    for l in &lists.main_gold_multiplier {
        let v = main_gold.wrapping_mul(l.value) / 100;
        main_bonus = main_bonus.wrapping_add(v);
        summary_multiplier(&mut s.main_gold, l, v);
    }
    let mut gold_base = main_gold.wrapping_add(main_bonus).wrapping_add(sub_gold);
    for l in &lists.additional_gold {
        gold_base = gold_base.wrapping_add(l.value);
        summary_reward(&mut s.etc_gold, l);
    }
    let mut gold = gold_base;
    for l in &lists.gold_multiplier {
        let v = gold_base.wrapping_mul(l.value) / 100;
        gold = gold.wrapping_add(v);
        summary_multiplier(&mut s.etc_gold, l, v);
    }
    // Bless stones add no fatigue bonus, so the level penalty always applies.
    for penalty in [&lists.level_gold_penalty, &lists.camping_gold_penalty]
        .into_iter()
        .flatten()
    {
        let v = (gold.wrapping_mul(penalty.value) / 100).wrapping_sub(gold);
        gold = gold.wrapping_add(v);
        summary_multiplier(&mut s.etc_gold, penalty, v);
    }
    let mut quest_ap_bonus = 0_i32;
    for l in &lists.quest_ap_multiplier {
        let v = goal_ap.wrapping_mul(l.value) / 100;
        quest_ap_bonus = quest_ap_bonus.wrapping_add(v);
        summary_multiplier(&mut s.quest_ap, l, v);
    }
    let mut additional = 0_i32;
    for l in &lists.additional_ap {
        additional = additional.wrapping_add(l.value);
        summary_reward(&mut s.etc_ap, l);
    }
    let ap_base = goal_ap.wrapping_add(quest_ap_bonus).wrapping_add(additional);
    let mut ap = ap_base;
    for l in &lists.ap_multiplier {
        let v = ap_base.wrapping_mul(l.value) / 100;
        ap = ap.wrapping_add(v);
        summary_multiplier(&mut s.etc_ap, l, v);
    }
    c.exp = exp;
    c.gold = gold;
    c.ap = ap;
    Ok(c)
}

/// CalculateLastQuestProgress (`achieved` None) or CalculateCurrentQuestProgress over
/// GetAllQuestGoalInfos: (goal ID, Weight), the stored statuses and the goals achieved now.
pub fn quest_progress(
    goals: &[(i32, i32)],
    stored: &[(i32, i32)],
    achieved_now: Option<&[i32]>,
    is_season2: bool,
    difficulty: i32,
) -> i32 {
    let bit = 1_i32.wrapping_shl(difficulty as u32 & 31);
    let mut sum = 0_i32;
    for &(id, weight) in goals {
        if achieved_now.is_some_and(|a| a.contains(&id)) {
            sum = sum.wrapping_add(weight);
            continue;
        }
        let Some(&(_, status)) = stored.iter().find(|(g, _)| *g == id) else {
            continue;
        };
        let counted = if is_season2 {
            status | bit == status
        } else {
            status > 0
        };
        if counted {
            sum = sum.wrapping_add(weight);
        }
    }
    sum
}
