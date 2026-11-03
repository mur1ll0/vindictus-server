//! AllUserGoal (ServiceCore HeroesContentsClass): the collective goals of AllUserGoalInfo
//! that every player's kills and clears advance, one active goal per slot (1–8) by its
//! StartTime/EndTime window. Laboratory extension (operator decision of 03/11/2026): the
//! content's windows are from 2013–2015, so when the panel turns a goal Feature on, its rows
//! move by the same offset to start at the time the switch was published.
use super::feature_policy::FeaturePolicy;
use regex::{Regex, RegexBuilder};
use std::collections::BTreeMap;

/// AllUserGoalInfo of this server code: ID, SlotID, Feature, GoalType, Goal (regex),
/// GoalCount, RewardItemClass, RewardNum, StartTime, EndTime (Unix seconds), GoalName.
pub type GoalRow = (
    i32,
    i32,
    Option<String>,
    i32,
    String,
    i32,
    Option<String>,
    i32,
    i64,
    i64,
    Option<String>,
);

/// The FeatureMatrix keys an IsEnable expression names (split on || and &&, without !).
pub fn atoms(expression: &str) -> Vec<String> {
    expression
        .split("||")
        .flat_map(|p| p.split("&&"))
        .map(|a| a.trim().trim_start_matches('!').trim())
        .filter(|a| !a.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The slots RefreshAllUserGoalEvent and the scheduler walk (1..9 exclusive).
pub const SLOTS: std::ops::Range<i32> = 1..9;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Regex,
    Feature,
    /// AllUserGoalDicByID.Add of a repeated ID throws.
    DuplicateId,
}

/// AllUserGoalData with its compiled goal.
#[derive(Clone, Debug)]
pub struct Goal {
    pub id: i32,
    pub slot: i32,
    pub goal_type: i32,
    pub pattern: String,
    regex: Regex,
    pub goal_count: i32,
    pub reward: Option<String>,
    pub reward_num: i32,
    pub start: i64,
    pub end: i64,
    pub name: Option<String>,
    pub feature: Option<String>,
}

impl Goal {
    fn current(&self, now: i64) -> bool {
        self.start <= now && now < self.end
    }
}

#[derive(Default)]
pub struct Goals {
    rows: Vec<Goal>,
    /// The content's (StartTime, EndTime) of every row.
    original: Vec<(i64, i64)>,
    /// Indexes of the rows Initialize kept (IsEnable(Feature)), in table order.
    active: Vec<usize>,
}

impl Goals {
    pub fn load(rows: Vec<GoalRow>) -> Result<Self, Error> {
        let rows = rows
            .into_iter()
            .map(
                |(
                    id,
                    slot,
                    feature,
                    goal_type,
                    pattern,
                    goal_count,
                    reward,
                    reward_num,
                    start,
                    end,
                    name,
                )| {
                    // new Regex(Goal, IgnoreCase | Compiled).
                    let regex = RegexBuilder::new(&pattern)
                        .case_insensitive(true)
                        .size_limit(1 << 20)
                        .build()
                        .map_err(|_| Error::Regex)?;
                    Ok(Goal {
                        id,
                        slot,
                        goal_type,
                        pattern,
                        regex,
                        goal_count,
                        reward,
                        reward_num,
                        start,
                        end,
                        name,
                        feature,
                    })
                },
            )
            .collect::<Result<Vec<_>, Error>>()?;
        let original = rows.iter().map(|r| (r.start, r.end)).collect();
        Ok(Self {
            rows,
            original,
            active: Vec::new(),
        })
    }

    /// AllUserGoal.Initialize with the FeatureMatrix of the panel: the enabled rows, each
    /// Feature group moved so that its first StartTime is the time `since` gives the newest
    /// panel switch named by the Feature (rows without one keep the content's times).
    pub fn refresh(
        &mut self,
        policy: &FeaturePolicy,
        since: &BTreeMap<String, i64>,
    ) -> Result<(), Error> {
        let original = &self.original;
        let mut active = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for (i, row) in self.rows.iter().enumerate() {
            if !policy
                .is_enabled(row.feature.as_deref())
                .map_err(|_| Error::Feature)?
            {
                continue;
            }
            if !ids.insert(row.id) {
                return Err(Error::DuplicateId);
            }
            active.push(i);
        }
        // The anchor of each Feature expression and its group's first StartTime.
        let mut shift: BTreeMap<Option<String>, i64> = BTreeMap::new();
        for &i in &active {
            let feature = &self.rows[i].feature;
            if shift.contains_key(feature) {
                continue;
            }
            let anchor = feature
                .as_deref()
                .map(atoms)
                .unwrap_or_default()
                .iter()
                .filter_map(|a| since.get(a))
                .max()
                .copied();
            let offset = anchor.map_or(0, |anchor| {
                let first = active
                    .iter()
                    .filter(|&&j| self.rows[j].feature == *feature)
                    .map(|&j| original.get(j).map_or(self.rows[j].start, |t| t.0))
                    .min()
                    .unwrap_or(anchor);
                anchor - first
            });
            shift.insert(feature.clone(), offset);
        }
        for (i, row) in self.rows.iter_mut().enumerate() {
            let (start, end) = original.get(i).copied().unwrap_or((row.start, row.end));
            let offset = shift.get(&row.feature).copied().unwrap_or(0);
            row.start = start.saturating_add(offset);
            row.end = end.saturating_add(offset);
        }
        self.active = active;
        Ok(())
    }

    fn active(&self) -> impl Iterator<Item = &Goal> {
        self.active.iter().map(|&i| &self.rows[i])
    }

    /// The goals of the panel's description (every row, with its current window).
    pub fn rows(&self) -> &[Goal] {
        &self.rows
    }

    pub fn goal(&self, id: i32) -> Option<&Goal> {
        self.active().find(|g| g.id == id)
    }

    /// GetCurrentIDByGoal: the first goal (table order) whose regex matches the event, not
    /// ended, inside its window; -1 without one.
    pub fn current_id_by_goal(&self, event: &str, now: i64, ended: &dyn Fn(i32) -> bool) -> i32 {
        self.active()
            .find(|g| g.regex.is_match(event) && !ended(g.id) && g.current(now))
            .map_or(-1, |g| g.id)
    }

    /// GetCurrentGoalDataBySlotID.
    pub fn current_by_slot(&self, slot: i32, now: i64) -> Option<&Goal> {
        self.active().find(|g| g.slot == slot && g.current(now))
    }

    /// GetNextGoalDataBySlotID: the slot's earliest goal that has not started.
    pub fn next_by_slot(&self, slot: i32, now: i64) -> Option<&Goal> {
        let mut slot_goals: Vec<&Goal> = self.active().filter(|g| g.slot == slot).collect();
        slot_goals.sort_by_key(|g| g.start);
        slot_goals.into_iter().find(|g| now < g.start)
    }

    /// GetNextTimeRefreshTime: the earliest StartTime after now.
    pub fn next_refresh(&self, now: i64) -> Option<i64> {
        self.active().map(|g| g.start).filter(|s| now < *s).min()
    }

    /// GetTimeData(startTime): the goals starting then.
    pub fn starting_at(&self, start: i64) -> impl Iterator<Item = &Goal> {
        self.active().filter(move |g| g.start == start)
    }

    /// IsGroupGoal: GoalType 1 credits every party member.
    pub fn is_group(goal_type: i32) -> bool {
        goal_type == 1
    }

    /// AllUserGoalEventScheduler.scheduleAllUserGoalEvent at `now`: the AllUserGoalEventModify
    /// (GoalID, Count) pushes due at each time, the next refresh included (−1: the slot's
    /// next goal starts later; −2: the slot has no next goal; 0: the goal starts).
    pub fn schedule(&self, now: i64) -> Vec<(i64, i32, i32)> {
        let mut out = Vec::new();
        let next = self.next_refresh(now);
        for slot in SLOTS {
            let Some(goal) = self.current_by_slot(slot, now) else {
                continue;
            };
            if next.is_some_and(|n| goal.end > n) {
                continue;
            }
            match self.next_by_slot(slot, now) {
                Some(n) if next.is_some() => {
                    if n.start != goal.end {
                        out.push((goal.end, n.id, -1));
                    }
                }
                Some(_) => {}
                None => out.push((goal.end, goal.id, -2)),
            }
        }
        if let Some(n) = next {
            for goal in self.starting_at(n) {
                out.push((n, goal.id, 0));
            }
        }
        out.sort_by_key(|(t, _, _)| *t);
        out
    }
}
