//! Recovered login effect decisions. Plans are not database execution or game login.
use crate::domain::legacy_user_data::{SqlDateTime, SqlDateTime2};
use thiserror::Error;

#[derive(Clone, Copy)]
pub struct EffectFeatures {
    pub friend_loyalty: bool,
    pub friend_return_days: i32,
    pub dormant_account: bool,
    pub dormant_new_account_days: i32,
    pub dormant_account_days: i32,
    pub dormant_account_v2: bool,
}
pub struct EffectInputs {
    pub new_user_required: bool,
    pub friend_recommend_flag: Option<i32>,
    /// The two GetLatestConnectedDate calls can observe different values.
    pub latest_for_loyalty: Option<SqlDateTime2>,
    pub dormant_since: Option<SqlDateTime2>,
    pub latest_for_dormancy: Option<SqlDateTime2>,
}
#[derive(Clone, Copy)]
pub struct EffectClockSamples {
    pub loyalty_now: SqlDateTime2,
    pub dormant_elapsed_now: SqlDateTime2,
    pub latest_elapsed_now: SqlDateTime2,
    pub set_dormant_now: SqlDateTime2,
    pub set_event_now: SqlDateTime2,
}
/// Ordered calls recovered from UserLoginProcessor, including the post-write event read.
#[derive(Debug, PartialEq, Eq)]
pub enum LoginEffect {
    SetFriendRecommendFlag(i32),
    InsertNewUser,
    ReplaceAchievementOwner,
    ReleaseDormant,
    SetDormantUser(SqlDateTime2),
    SendDormantRewards,
    UpdateDormantEvent(Option<SqlDateTime2>),
    ReadDormantEvent,
}
pub struct LoginEffectPlan {
    actions: Vec<LoginEffect>,
    is_dormant: bool,
    return_from_write: bool,
    reads_event: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub struct EffectFlags {
    pub is_dormant: bool,
    pub is_return: bool,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EffectCompletionError {
    #[error("post-write dormant event read is required")]
    MissingEventRead,
}
impl LoginEffectPlan {
    pub fn actions(&self) -> &[LoginEffect] {
        &self.actions
    }
    /// Call only after executing every action; reads are observations, not cached input.
    /// A null/empty legacy result is represented by Some(&[]), not a missing read.
    pub fn complete(
        &self,
        post_write_events: Option<&[Option<SqlDateTime>]>,
    ) -> Result<EffectFlags, EffectCompletionError> {
        let event_return = if self.reads_event {
            post_write_events
                .ok_or(EffectCompletionError::MissingEventRead)?
                .iter()
                .any(Option::is_some)
        } else {
            false
        };
        Ok(EffectFlags {
            is_dormant: self.is_dormant,
            is_return: self.return_from_write || event_return,
        })
    }
}
fn elapsed_days(now: SqlDateTime2, then: SqlDateTime2) -> i64 {
    // Framework TimeSpan.Days truncates toward zero, including future timestamps.
    (now.ticks() - then.ticks()) / 864_000_000_000
}
pub fn plan_login_effects(
    input: &EffectInputs,
    feature: EffectFeatures,
    clock: EffectClockSamples,
) -> LoginEffectPlan {
    let mut actions = Vec::with_capacity(8);
    let new_friend =
        input.new_user_required && feature.friend_loyalty && input.friend_recommend_flag.is_none();
    if new_friend {
        actions.push(LoginEffect::SetFriendRecommendFlag(1));
    }
    if input.new_user_required {
        actions.push(LoginEffect::InsertNewUser);
        // The INSERT/SubmitChanges barrier must complete before ownership replacement.
        actions.push(LoginEffect::ReplaceAchievementOwner);
    }
    if feature.friend_loyalty && !new_friend && input.friend_recommend_flag.is_none() {
        match input.latest_for_loyalty {
            Some(latest)
                if i64::from(feature.friend_return_days)
                    <= elapsed_days(clock.loyalty_now, latest) =>
            {
                actions.push(LoginEffect::SetFriendRecommendFlag(2));
            }
            None => actions.push(LoginEffect::SetFriendRecommendFlag(1)),
            Some(_) => {}
        }
    }
    let mut is_dormant = false;
    let mut return_from_write = false;
    let reads_event = feature.dormant_account && feature.dormant_account_v2;
    if feature.dormant_account {
        let dormant_days = input
            .dormant_since
            .map(|since| elapsed_days(clock.dormant_elapsed_now, since));
        if dormant_days.is_some_and(|days| i64::from(feature.dormant_new_account_days) < days) {
            actions.push(LoginEffect::ReleaseDormant);
        }
        // Release does not erase the elapsed time captured before that call.
        if input.latest_for_dormancy.is_some_and(|latest| {
            dormant_days.is_none_or(|days| i64::from(feature.dormant_account_days) <= days)
                && i64::from(feature.dormant_account_days)
                    <= elapsed_days(clock.latest_elapsed_now, latest)
        }) {
            actions.push(LoginEffect::SetDormantUser(clock.set_dormant_now));
            actions.push(LoginEffect::SendDormantRewards);
            is_dormant = true;
            if feature.dormant_account_v2 {
                actions.push(LoginEffect::UpdateDormantEvent(Some(clock.set_event_now)));
                return_from_write = true;
            }
        }
        if reads_event {
            actions.push(LoginEffect::ReadDormantEvent);
        } else {
            actions.push(LoginEffect::UpdateDormantEvent(None));
        }
    }
    LoginEffectPlan {
        actions,
        is_dormant,
        return_from_write,
        reads_event,
    }
}
/// Source predicate: zero means an unbounded endpoint; signed values are not normalized.
pub fn dormant_reward_matches(level: i32, min: i32, max: i32, feature_enabled: bool) -> bool {
    feature_enabled && (level >= min || min == 0) && (level <= max || max == 0)
}
pub struct LatestConnection {
    pub last_connect: Option<SqlDateTime>,
    pub last_cafe_logout: Option<SqlDateTime>,
}
/// GetLatestConnectedDate CASE/MAX semantics; NULL does not compare as a low timestamp.
pub fn latest_connected_date(rows: &[LatestConnection]) -> Option<SqlDateTime> {
    rows.iter()
        .filter_map(|row| match (row.last_connect, row.last_cafe_logout) {
            (Some(connect), Some(cafe)) if connect.units300() < cafe.units300() => Some(cafe),
            (connect, _) => connect,
        })
        .max_by_key(|date| date.units300())
}
