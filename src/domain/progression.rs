//! CharacterContents level tables and CharacterEntity.GiveExp for the Fiona class:
//! CharacterClassInfo base stats per level and LevelUpExpInfo, both filtered by Feature.
use super::feature_policy::FeaturePolicy;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProgressionError {
    #[error("progression content is outside the admitted profile")]
    Content,
    #[error("progression feature expression is outside the admitted profile")]
    Feature,
}

/// Exported tables: CharacterClassInfo rows of class 1 (Level, STR, DEX, INT, WILL, HP,
/// STAMINA, Feature) and LevelUpExpInfo rows (Level, RequiredExp, Feature).
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressionTables {
    pub class_stats: Vec<ClassStatRow>,
    pub level_exp: Vec<(i32, i32, Option<String>)>,
    /// TitleInfo: ID, AutoGiveLevel, ClassRestriction, HideUntil, Feature.
    #[serde(default)]
    pub titles: Vec<TitleRow>,
    /// TitleGoalInfo: ID, TitleID.
    #[serde(default)]
    pub title_goals: Vec<(i32, i32)>,
    /// TitleStatInfo: TitleID, Stat, Amount.
    #[serde(default)]
    pub title_stats: Vec<(i32, Option<String>, i32)>,
    /// TitleInfo: ID, RequiredLevel, AllowShare (account titles of GetTitleStats).
    #[serde(default)]
    pub title_sharing: Vec<(i32, i32, bool)>,
    /// TitleGoalInfo: ID, Target, TargetCount, IsParty (TitleGoalTracker and Proceed).
    #[serde(default)]
    pub title_goal_targets: Vec<(i32, Option<String>, i32, bool)>,
}
/// Base stat columns in CharacterInfo order (the only TitleStatInfo stats the exported
/// content uses).
const STAT_COLUMNS: [&str; 7] = ["STR", "DEX", "INT", "WILL", "LUCK", "HP", "STAMINA"];
/// TitleInfo: ID, AutoGiveLevel, ClassRestriction, HideUntil, Feature.
pub type TitleRow = (i32, Option<i32>, i32, i32, Option<String>);
/// CharacterClassInfo: Level, BaseSTR, BaseDEX, BaseINT, BaseWILL, BaseHP, BaseSTAMINA, Feature.
pub type ClassStatRow = (i32, i32, i32, i32, i32, i32, i32, Option<String>);

/// TitleInfo columns read by GiveTitleByLevelUp.
#[derive(Clone, Debug)]
struct Title {
    id: i32,
    auto_give_level: Option<i32>,
    class_restriction: i32,
    hide_until: i32,
    /// FeatureMatrix.IsEnable(Feature) at load.
    enabled: bool,
    goals: Vec<i32>,
    /// titleStats[ID]: STR, DEX, INT, WILL, LUCK, HP, STAMINA.
    stats: [i32; 7],
    required_level: i32,
    allow_share: bool,
}

/// A title GiveTitleByLevelUp inserts (Acquired false): its goal IDs (progress 0) and
/// whether its slot is pushed (HideUntil 0, so the title is Known).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GivenTitle {
    pub id: i32,
    pub goals: Vec<i32>,
    pub listed: bool,
}

/// A title NoticeTitle opened or made visible: `new` when GiveTitle inserted it, and
/// its goals with the progress to store (the first one is HideUntil). Its slot is Known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoticedTitle {
    pub id: i32,
    pub new: bool,
    pub goals: Vec<(i32, i32)>,
}

/// A title AcquireTitle marked acquired: `new` when GiveTitle inserted it first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcquiredTitle {
    pub id: i32,
    pub new: bool,
    pub goals: Vec<i32>,
}

/// TitleGoalInfo read by TitleGoalTracker and TitleManager.Proceed.
#[derive(Clone, Debug)]
struct GoalTarget {
    title: i32,
    /// new Regex(Target, IgnoreCase); None when the pattern does not compile here.
    regex: Option<regex::Regex>,
    count: i32,
    party: bool,
}

/// TitleManager.Proceed of one goal: the goal's new progress (-1 when completed), whether
/// the title became acquired, and the TitleSlotInfo to push (None: nothing to push).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proceeded {
    pub title: i32,
    pub goal: i32,
    pub progress: i32,
    pub completed: bool,
    pub acquired: bool,
    /// TitleState (1 Known, 2 Acquired) and the goal progress list of the slot.
    pub slot: Option<(i32, Vec<(i32, i32)>)>,
}

#[derive(Clone, Debug)]
pub struct Progression {
    /// characterClassInfo (Identifier 1): STR, DEX, INT, WILL, HP, STAMINA per level.
    class_stats: BTreeMap<i32, [i32; 6]>,
    /// levelUpExpDic: Level → RequiredExp.
    level_exp: BTreeMap<i32, i32>,
    /// FeatureMatrix.GetInteger("MaxLevel").
    max_level: i32,
    /// titleInfoDict in provider order.
    titles: Vec<Title>,
    /// FeatureMatrix.GetInteger("AccountTitleStatRatio"); None when the profile has no
    /// title stats (older exports), so title stats are not computed from it.
    account_title_ratio: Option<i32>,
    /// titleGoalInfo by goal ID (empty for exports without targets).
    goal_targets: BTreeMap<i32, GoalTarget>,
}

/// GiveExp result: the stored Exp/Level and CharacterEntity.LevelUpExp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Growth {
    pub previous_exp: i32,
    pub exp: i32,
    pub previous_level: i32,
    pub level: i32,
    pub level_up_exp: i32,
}
impl Growth {
    pub fn leveled(&self) -> bool {
        self.level != self.previous_level
    }
}

/// CharacterStats key order (Stats enum without INVALID/STATS_NUM).
pub const STAT_NAMES: [&str; 25] = [
    "ATK",
    "MATK",
    "DEF",
    "STR",
    "DEX",
    "INT",
    "WILL",
    "LUCK",
    "HP",
    "STAMINA",
    "HEAVY_LOAD",
    "LOAD_PENALTY",
    "MEDIUM_LOAD",
    "TOTAL_LOAD",
    "ATK_Speed",
    "ATK_Absolute",
    "Critical",
    "CritFactor",
    "Res_Critical",
    "Balance",
    "PVP_ATK",
    "PVP_MATK",
    "PVP_DEF",
    "TOWN_SPEED",
    "SKILL_RANK_SUM",
];

impl Progression {
    pub fn load(
        tables: ProgressionTables,
        policy: &FeaturePolicy,
    ) -> Result<Self, ProgressionError> {
        let enabled = |f: &Option<String>| {
            policy
                .is_enabled(f.as_deref())
                .map_err(|_| ProgressionError::Feature)
        };
        let mut class_stats = BTreeMap::new();
        for (level, s, d, i, w, hp, st, feature) in &tables.class_stats {
            // ToDictionary throws on a duplicate (class, level) key.
            if enabled(feature)?
                && class_stats
                    .insert(*level, [*s, *d, *i, *w, *hp, *st])
                    .is_some()
            {
                return Err(ProgressionError::Content);
            }
        }
        let mut level_exp = BTreeMap::new();
        for (level, required, feature) in &tables.level_exp {
            if enabled(feature)? && level_exp.insert(*level, *required).is_some() {
                return Err(ProgressionError::Content);
            }
        }
        let max_level = policy
            .get_integer("MaxLevel")
            .map_err(|_| ProgressionError::Feature)?;
        let mut titles: Vec<Title> = Vec::new();
        for (id, auto_give_level, class_restriction, hide_until, feature) in &tables.titles {
            // ToDictionary throws on a duplicate ID.
            if titles.iter().any(|t| t.id == *id) {
                return Err(ProgressionError::Content);
            }
            titles.push(Title {
                id: *id,
                auto_give_level: *auto_give_level,
                class_restriction: *class_restriction,
                hide_until: *hide_until,
                enabled: enabled(feature)?,
                goals: Vec::new(),
                stats: [0; 7],
                required_level: 0,
                allow_share: false,
            });
        }
        let mut goal_ids = std::collections::BTreeSet::new();
        for (goal, title) in &tables.title_goals {
            if !goal_ids.insert(*goal) {
                return Err(ProgressionError::Content);
            }
            // ToListDictionary by TitleID; a goal of an unknown title is never read.
            if let Some(t) = titles.iter_mut().find(|t| t.id == *title) {
                t.goals.push(*goal);
            }
        }
        // CharacterStats.StatNames (OrdinalIgnoreCase); a row of an unknown title is skipped
        // (titleStats.TryGetValue), an unknown stat name throws in the source.
        for (title, stat, amount) in &tables.title_stats {
            let index = STAT_COLUMNS
                .iter()
                .position(|c| stat.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(c)))
                .ok_or(ProgressionError::Content)?;
            if let Some(t) = titles.iter_mut().find(|t| t.id == *title) {
                t.stats[index] = t.stats[index].wrapping_add(*amount);
            }
        }
        for (id, required_level, allow_share) in &tables.title_sharing {
            let t = titles
                .iter_mut()
                .find(|t| t.id == *id)
                .ok_or(ProgressionError::Content)?;
            t.required_level = *required_level;
            t.allow_share = *allow_share;
        }
        let account_title_ratio = if tables.title_sharing.is_empty() {
            None
        } else {
            Some(
                policy
                    .get_integer("AccountTitleStatRatio")
                    .map_err(|_| ProgressionError::Feature)?,
            )
        };
        let mut goal_targets = BTreeMap::new();
        for (goal, target, count, party) in &tables.title_goal_targets {
            let title = tables
                .title_goals
                .iter()
                .find(|(g, _)| g == goal)
                .map(|(_, t)| *t)
                .ok_or(ProgressionError::Content)?;
            let regex = regex::RegexBuilder::new(target.as_deref().unwrap_or(""))
                .case_insensitive(true)
                .size_limit(1 << 20)
                .build()
                .ok();
            goal_targets.insert(
                *goal,
                GoalTarget {
                    title,
                    regex,
                    count: *count,
                    party: *party,
                },
            );
        }
        Ok(Self {
            class_stats,
            level_exp,
            max_level,
            titles,
            account_title_ratio,
            goal_targets,
        })
    }
    /// TitleManager.GetTitleStats for the base stat columns: the TitleStatInfo stats of
    /// every acquired title (`own`), plus the account titles (`account`: titles acquired
    /// by any character of the account) that allow sharing, fit the class, are not
    /// acquired here and whose RequiredLevel the character reached, times
    /// AccountTitleStatRatio / 100 truncated per stat. `None` for a profile without
    /// title stats when a title is acquired.
    pub fn title_stats(
        &self,
        own: &[i32],
        account: &[i32],
        class: i32,
        level: i32,
    ) -> Option<[i32; 7]> {
        let mut total = [0_i32; 7];
        let mut counted = std::collections::BTreeSet::new();
        for id in own {
            if let Some(t) = self.titles.iter().find(|t| t.id == *id) {
                for (v, s) in total.iter_mut().zip(t.stats) {
                    *v = v.wrapping_add(s);
                }
                counted.insert(*id);
            }
        }
        if !own.is_empty() && self.account_title_ratio.is_none() {
            return None;
        }
        let mut shared = [0_i32; 7];
        let mut seen = std::collections::BTreeSet::new();
        for id in account {
            let Some(t) = self.titles.iter().find(|t| t.id == *id) else {
                continue;
            };
            let available = 1_i32.wrapping_shl(class as u32 & 31) & t.class_restriction != 0;
            if !t.allow_share || !available || !seen.insert(*id) || counted.contains(id) {
                continue;
            }
            if level >= t.required_level {
                for (v, s) in shared.iter_mut().zip(t.stats) {
                    *v = v.wrapping_add(s);
                }
            }
        }
        if seen.is_empty() {
            return Some(total);
        }
        let ratio = f64::from(self.account_title_ratio?) / 100.0;
        for (v, s) in total.iter_mut().zip(shared) {
            // (int)((double)stat * ratio).
            *v = v.wrapping_add((f64::from(s) * ratio) as i32);
        }
        Some(total)
    }
    pub fn max_level(&self) -> i32 {
        self.max_level
    }
    /// levelUpExpDic.TryGetValue(level).
    pub fn level_exp(&self, level: i32) -> Option<i32> {
        self.level_exp.get(&level).copied()
    }
    /// GetCharacterStats(class, level): the level row, else level 1.
    pub fn class_stats(&self, level: i32) -> Option<[i32; 6]> {
        self.class_stats
            .get(&level)
            .or_else(|| self.class_stats.get(&1))
            .copied()
    }
    /// CharacterEntity.GiveExp(exp) (no E3/PAX zeroing: both events are disabled in the
    /// admitted region). None is the early MaxLevel return without changes.
    pub fn give_exp(&self, stored_exp: i64, level: i32, exp: i64) -> Option<Growth> {
        if self.max_level <= 0 || self.max_level <= level {
            return None;
        }
        // (int)Exp and (int)Exp + (int)exp, unchecked.
        let previous_exp = stored_exp as i32;
        let mut current = previous_exp.wrapping_add(exp as i32);
        let mut new_level = level;
        let mut value: i32;
        loop {
            let Some(required) = self.level_exp(new_level) else {
                value = 0;
                current = 0;
                break;
            };
            value = required;
            if self.max_level <= new_level {
                current = 0;
                break;
            }
            if value >= current {
                break;
            }
            new_level += 1;
            current = current.wrapping_sub(value);
        }
        if new_level > self.max_level {
            new_level = self.max_level;
        }
        Some(Growth {
            previous_exp,
            exp: current,
            previous_level: level,
            level: new_level,
            level_up_exp: value,
        })
    }
    /// TitleManager.GiveTitleByLevelUp: titles with AutoGiveLevel ≤ level, enabled,
    /// not owned and available to the class (GiveTitle), in TitleInfo order.
    pub fn titles_by_level(&self, level: i32, class: i32, owned: &[i32]) -> Vec<GivenTitle> {
        self.titles
            .iter()
            .filter(|t| {
                t.auto_give_level.is_some_and(|a| level >= a)
                    && t.enabled
                    && !owned.contains(&t.id)
                    && 1_i32.wrapping_shl(class as u32 & 31) & t.class_restriction != 0
            })
            .map(|t| GivenTitle {
                id: t.id,
                goals: t.goals.clone(),
                listed: t.hide_until == 0,
            })
            .collect()
    }
    /// TitleManager.NoticeTitle(titleID) for a title the character owns (`Some`: Acquired
    /// and the stored progress of its goals) or not. `None` when nothing changes; an owned,
    /// hidden title whose visibility needs a completed goal (progress -1, which counts the
    /// goal's TargetCount) is refused because TargetCount is not in the profile.
    pub fn notice_title(
        &self,
        id: i32,
        class: i32,
        owned: Option<(bool, &[(i32, i32)])>,
    ) -> Result<Option<NoticedTitle>, ProgressionError> {
        // GetTitleInfo null, or FeatureMatrix.IsEnable(Feature) false.
        let Some(t) = self.titles.iter().find(|t| t.id == id && t.enabled) else {
            return Ok(None);
        };
        // The first TitleGoalProgress gets Progress = HideUntil; the others keep theirs.
        let revealed = |stored: &dyn Fn(i32) -> i32| -> Vec<(i32, i32)> {
            t.goals
                .iter()
                .enumerate()
                .map(|(i, g)| (*g, if i == 0 { t.hide_until } else { stored(*g) }))
                .collect()
        };
        let Some((acquired, progress)) = owned else {
            // GiveTitle: IsAvailableFor(BaseCharacter); AddNewTitle (Acquired false).
            if 1_i32.wrapping_shl(class as u32 & 31) & t.class_restriction == 0 {
                return Ok(None);
            }
            return Ok(Some(NoticedTitle {
                id,
                new: true,
                goals: revealed(&|_| 0),
            }));
        };
        let stored = |g: i32| progress.iter().find(|(id, _)| *id == g).map_or(0, |p| p.1);
        // Title.IsVisible.
        let visible = acquired || t.goals.is_empty() || {
            let mut num = 0_i32;
            let mut seen = false;
            for g in &t.goals {
                if t.hide_until <= num {
                    seen = true;
                    break;
                }
                let p = stored(*g);
                if p == -1 {
                    return Err(ProgressionError::Content);
                }
                num = num.wrapping_add(p);
            }
            seen || t.hide_until <= num
        };
        if visible {
            return Ok(None);
        }
        Ok(Some(NoticedTitle {
            id,
            new: false,
            goals: revealed(&stored),
        }))
    }
    /// TitleManager.GetTrackingGoalID: the goals of the owned, unacquired titles whose
    /// feature is enabled and whose progress is not -1 (a goal without a row is 0), in
    /// owned-title then TitleInfo goal order.
    pub fn tracking_goals(&self, owned: &[(i32, bool)], progress: &[(i32, i32)]) -> Vec<i32> {
        let stored = |g: i32| progress.iter().find(|(id, _)| *id == g).map_or(0, |p| p.1);
        let mut goals = Vec::new();
        for (id, acquired) in owned {
            if *acquired {
                continue;
            }
            if let Some(t) = self.titles.iter().find(|t| t.id == *id && t.enabled) {
                goals.extend(t.goals.iter().copied().filter(|g| stored(*g) != -1));
            }
        }
        goals
    }
    /// TitleGoalTracker.Match then the IsParty filter of InformToOtherService: the
    /// tracked goals whose Target matches the event, for the event's actor (`own`) or,
    /// for party goals, any actor.
    pub fn matching_goals(&self, tracked: &[i32], event: &str, own: bool) -> Vec<i32> {
        tracked
            .iter()
            .copied()
            .filter(|g| {
                self.goal_targets.get(g).is_some_and(|t| {
                    (t.party || own) && t.regex.as_ref().is_some_and(|r| r.is_match(event))
                })
            })
            .collect()
    }
    /// TitleManager.Proceed(goalID, where, count) against the character's titles and
    /// goal progress (rows; a missing row is 0). `update_count` is
    /// FeatureMatrix.IsEnable("UpdateTitleCount"). None when the source returns before
    /// changing anything.
    pub fn proceed(
        &self,
        goal: i32,
        owned: &[(i32, bool)],
        progress: &[(i32, i32)],
        count: i32,
        update_count: bool,
    ) -> Option<Proceeded> {
        let target = self.goal_targets.get(&goal)?;
        // GetTitle(TitleID): a title the character owns; its TitleInfo feature enabled.
        let (_, acquired) = owned.iter().find(|(id, _)| *id == target.title)?;
        let t = self.titles.iter().find(|t| t.id == target.title)?;
        if !t.enabled || *acquired {
            return None;
        }
        let stored = |g: i32| progress.iter().find(|(id, _)| *id == g).map_or(0, |p| p.1);
        let before = stored(goal);
        if before == -1 {
            return None;
        }
        let target_count = |g: i32| self.goal_targets.get(&g).map_or(0, |t| t.count);
        // Title.IsVisible over the title's goal progress list.
        let visible = |value: &dyn Fn(i32) -> i32| {
            if t.goals.is_empty() {
                return true;
            }
            let mut num = 0_i32;
            for g in &t.goals {
                if t.hide_until <= num {
                    return true;
                }
                let p = value(*g);
                num = num.wrapping_add(if p == -1 { target_count(*g) } else { p });
            }
            t.hide_until <= num
        };
        let was_visible = visible(&stored);
        let mut progress_now = before.wrapping_add(count);
        let completed = progress_now >= target.count;
        if completed {
            progress_now = -1;
        }
        let after = |g: i32| if g == goal { progress_now } else { stored(g) };
        let list: Vec<(i32, i32)> = t.goals.iter().map(|g| (*g, after(*g))).collect();
        let mut out = Proceeded {
            title: t.id,
            goal,
            progress: progress_now,
            completed,
            acquired: false,
            slot: None,
        };
        // Title.IsAcquirable: every goal of the title completed.
        if t.goals.iter().all(|g| after(*g) == -1) {
            out.acquired = true;
            out.slot = Some((2, t.goals.iter().map(|g| (*g, -1)).collect()));
            return Some(out);
        }
        let is_visible = visible(&after);
        if (!was_visible && is_visible) || (is_visible && update_count) {
            out.slot = Some((1, list));
        }
        Some(out)
    }
    /// TitleManager.AcquireTitle(titleID) for an owned title (`Some(acquired)`) or not:
    /// `None` when nothing changes (unknown, feature-disabled, class-restricted or
    /// already acquired). The goals are listed for the slot (-1 each, acquired).
    pub fn acquire_title(&self, id: i32, class: i32, owned: Option<bool>) -> Option<AcquiredTitle> {
        let t = self.titles.iter().find(|t| t.id == id && t.enabled)?;
        let new = match owned {
            Some(true) => return None,
            Some(false) => false,
            // GiveTitle: IsAvailableFor(BaseCharacter).
            None if 1_i32.wrapping_shl(class as u32 & 31) & t.class_restriction == 0 => {
                return None;
            }
            None => true,
        };
        Some(AcquiredTitle {
            id,
            new,
            goals: t.goals.clone(),
        })
    }
    /// LevelUpMessage.StatIncreased: GetCharacterStats(new) − GetCharacterStats(old) over
    /// every CharacterStats key (LUCK is 100 on both sides).
    pub fn stat_increase(&self, from: i32, to: i32) -> Option<Vec<(String, i32)>> {
        let (a, b) = (self.class_stats(to)?, self.class_stats(from)?);
        let value = |name: &str| match name {
            "STR" => a[0].wrapping_sub(b[0]),
            "DEX" => a[1].wrapping_sub(b[1]),
            "INT" => a[2].wrapping_sub(b[2]),
            "WILL" => a[3].wrapping_sub(b[3]),
            "HP" => a[4].wrapping_sub(b[4]),
            "STAMINA" => a[5].wrapping_sub(b[5]),
            _ => 0,
        };
        Some(
            STAT_NAMES
                .iter()
                .map(|n| ((*n).to_owned(), value(n)))
                .collect(),
        )
    }
}
