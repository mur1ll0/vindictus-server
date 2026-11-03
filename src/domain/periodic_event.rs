//! PeriodicEventScheduler (PlayerService ItemServiceCore): the BoostEvent reward drawn
//! every BoostEventPeriod seconds of a logged-on character, and StoryEntity.DailyProcess:
//! the StoryLinePeriodInfo phases a storyline returns to at each period boundary.
use super::feature_policy::FeaturePolicy;
use serde::Deserialize;

/// PeriodicEventInfo: RewardType, RewardArg, Amount, Possibility, AnnounceLocalizedKey,
/// Feature.
pub type PeriodicRow = (
    Option<String>,
    Option<String>,
    i32,
    i32,
    Option<String>,
    Option<String>,
);
/// StoryLinePeriodInfo: StoryLineID, DateFrom (Unix seconds of the stored UTC time),
/// Period (hours), TargetPhaseID, Feature.
pub type StoryPeriodRow = (Option<String>, i64, i32, Option<String>, Option<String>);

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventTables {
    #[serde(default)]
    pub periodic: Vec<PeriodicRow>,
    #[serde(default)]
    pub story_periods: Vec<StoryPeriodRow>,
}

/// PeriodicEventRewardType.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewardType {
    None,
    Gold,
    Ap,
    Exp,
    Item,
}

impl RewardType {
    /// Enum.Parse(ignoreCase: true); a value it rejects (null included) is None.
    fn parse(value: Option<&str>) -> Self {
        let Some(value) = value.map(str::trim) else {
            return Self::None;
        };
        let names = [
            ("None", Self::None),
            ("Gold", Self::Gold),
            ("AP", Self::Ap),
            ("Exp", Self::Exp),
            ("Item", Self::Item),
        ];
        if let Some((_, kind)) = names.iter().find(|(n, _)| n.eq_ignore_ascii_case(value)) {
            return *kind;
        }
        match value.parse::<i32>() {
            Ok(n) => names
                .get(usize::try_from(n).unwrap_or(usize::MAX))
                .map_or(Self::None, |(_, k)| *k),
            Err(_) => Self::None,
        }
    }
}

/// PeriodicEventReward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeriodicReward {
    pub kind: RewardType,
    pub arg: Option<String>,
    pub amount: i32,
    pub rate: i32,
    pub announce: Option<String>,
    pub feature: Option<String>,
}

#[derive(Clone, Debug)]
struct StoryPeriod {
    story_line: String,
    date_from: i64,
    period: i32,
    target_phase: Option<String>,
    feature: Option<String>,
}

/// One storyline's progress: StoryLineID, PhaseID and LastUpdate (Unix seconds).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryProgress {
    pub story_line: String,
    pub phase: Option<String>,
    pub last_update: i64,
}

/// The phases DailyProcess moves to, and its next update time.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DailyStory {
    /// (StoryLineID, TargetPhaseID) in StoryLinePeriodInfo order.
    pub goto: Vec<(String, String)>,
    /// The earliest next period boundary (AddToUpdateSchedule), Unix seconds.
    pub next: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Feature,
    /// A zero Period divides by zero in the source.
    Period,
}

#[derive(Default)]
pub struct Events {
    rewards: Vec<PeriodicReward>,
    periods: Vec<StoryPeriod>,
}

impl Events {
    pub fn load(t: EventTables) -> Result<Self, Error> {
        let rewards = t
            .periodic
            .into_iter()
            .map(
                |(kind, arg, amount, rate, announce, feature)| PeriodicReward {
                    kind: RewardType::parse(kind.as_deref()),
                    arg,
                    amount,
                    rate,
                    announce,
                    feature,
                },
            )
            .collect();
        let mut periods = Vec::new();
        for (line, date_from, period, target_phase, feature) in t.story_periods {
            let Some(story_line) = line else {
                continue;
            };
            if period == 0 {
                return Err(Error::Period);
            }
            periods.push(StoryPeriod {
                story_line,
                date_from,
                period,
                target_phase,
                feature,
            });
        }
        Ok(Self { rewards, periods })
    }

    /// PeriodicEventWaiting: BoostEventPeriod in seconds when positive.
    pub fn period(policy: &FeaturePolicy) -> Option<u64> {
        policy
            .get_integer("BoostEventPeriod")
            .ok()
            .and_then(|p| u64::try_from(p).ok())
            .filter(|p| *p > 0)
    }

    /// A reward without a Feature (or an empty one) is always on.
    fn enabled(reward: &PeriodicReward, policy: &FeaturePolicy) -> Result<bool, Error> {
        match reward.feature.as_deref() {
            None | Some("") => Ok(true),
            feature => policy.is_enabled(feature).map_err(|_| Error::Feature),
        }
    }

    /// ItemOwnerEvent: nothing without BoostEvent; else `next(BoostPossibilitySum)`
    /// (Random.Next) walks the enabled rewards by Rate. A negative sum throws in the
    /// source, so no reward is given.
    pub fn pick(
        &self,
        policy: &FeaturePolicy,
        next: &mut dyn FnMut(i32) -> i32,
    ) -> Result<Option<&PeriodicReward>, Error> {
        if !policy
            .is_enabled(Some("BoostEvent"))
            .map_err(|_| Error::Feature)?
        {
            return Ok(None);
        }
        let mut on = Vec::new();
        for reward in &self.rewards {
            if Self::enabled(reward, policy)? {
                on.push(reward);
            }
        }
        let sum = on.iter().fold(0i32, |s, r| s.wrapping_add(r.rate));
        if sum < 0 {
            return Ok(None);
        }
        let mut roll = if sum == 0 { 0 } else { next(sum) };
        for reward in on {
            if roll < reward.rate {
                return Ok(Some(reward));
            }
            roll = roll.wrapping_sub(reward.rate);
        }
        Ok(None)
    }

    /// The rewards and their FeatureMatrix state, for the panel.
    pub fn rewards(&self) -> &[PeriodicReward] {
        &self.rewards
    }

    /// The (StoryLineID, TargetPhaseID, Feature) of the story periods, for the panel.
    pub fn story_periods(&self) -> impl Iterator<Item = (&str, Option<&str>, Option<&str>)> {
        self.periods.iter().map(|p| {
            (
                p.story_line.as_str(),
                p.target_phase.as_deref(),
                p.feature.as_deref(),
            )
        })
    }

    /// StoryEntity.DailyProcess at `now`: a storyline in progress whose period index
    /// ⌊hours since DateFrom⌋ / Period differs from its LastUpdate's goes to TargetPhaseID
    /// (`has_phase` is NpcTalkInfo.GetPhase; a missing phase is only logged). A moved
    /// storyline's LastUpdate becomes `now`.
    pub fn daily_story(
        &self,
        policy: &FeaturePolicy,
        progress: &mut [StoryProgress],
        now: i64,
        has_phase: &dyn Fn(&str, &str) -> bool,
    ) -> Result<DailyStory, Error> {
        let hours = |t: i64, from: i64| (t - from).div_euclid(3600);
        let mut out = DailyStory::default();
        for period in &self.periods {
            if period.feature.is_some()
                && !policy
                    .is_enabled(period.feature.as_deref())
                    .map_err(|_| Error::Feature)?
            {
                continue;
            }
            let Some(p) = progress
                .iter_mut()
                .find(|p| p.story_line == period.story_line && p.phase.is_some())
            else {
                continue;
            };
            let length = i64::from(period.period);
            // (int)Math.Floor(TotalHours) / Period truncates toward zero.
            let index = hours(now, period.date_from) / length;
            let next = period
                .date_from
                .saturating_add((index + 1).saturating_mul(length).saturating_mul(3600));
            out.next = Some(out.next.map_or(next, |n| n.min(next)));
            if index != hours(p.last_update, period.date_from) / length {
                let target = period.target_phase.as_deref().unwrap_or("");
                if has_phase(&period.story_line, target) {
                    p.phase = Some(target.to_owned());
                    p.last_update = now;
                    out.goto
                        .push((period.story_line.clone(), target.to_owned()));
                }
            }
        }
        Ok(out)
    }
}
