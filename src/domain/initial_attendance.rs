//! Initial Attendance state only. No reward scheduler, registration or existing
//! Newbie loader is silently simulated. External paths fail admission.
use super::{
    feature_policy::FeaturePolicy,
    initial_social::{self, CharacterContext},
    legacy_user_data::SqlDateTime2,
};
use crate::error::Error;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Disabled,
    NoNewbie,
    CaughtIntegerOverflow,
    CaughtClockError,
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AdmissionError {
    #[error("existing newbie attendance requires its recovered loader")]
    Existing,
    #[error("active newbie attendance requires registration and journal writers")]
    Active,
}
struct ManagerContext {
    character: Arc<CharacterContext>,
}
/// A separate deferred item-context link, not an open connection or reward provider.
struct ItemContext {
    character: Arc<CharacterContext>,
}
pub struct AllUser {
    manager: Arc<ManagerContext>,
    context: ItemContext,
    attendance_info: Vec<std::convert::Infallible>,
    bonus_info: BTreeMap<i32, std::convert::Infallible>,
}
pub struct Loaded {
    all_user: AllUser,
    outcome: Outcome,
}
impl Loaded {
    /// Called before the feature query, matching Initialize's Add(AllUser,...).
    pub fn all_user(social: &initial_social::Loaded) -> Self {
        let character = social.character_context();
        let manager = Arc::new(ManagerContext {
            character: Arc::clone(&character),
        });
        Self {
            all_user: AllUser {
                manager,
                context: ItemContext { character },
                attendance_info: Vec::new(),
                bonus_info: BTreeMap::new(),
            },
            outcome: Outcome::Disabled,
        }
    }
    pub fn enabled(policy: &FeaturePolicy) -> Result<bool, Error> {
        policy.is_enabled(Some("Attendance_Newbie"))
    }
    /// Invoke only after the active feature and original Count/First query.
    /// Source GetNullableInt precedes the first-character conditional, including
    /// overflow, which CreateNewbieInstance catches. UTC is sampled separately
    /// for each side of the window and short-circuits on the first comparison.
    pub fn initialize_newbie<E>(
        &mut self,
        first: bool,
        policy: &FeaturePolicy,
        period: impl FnOnce(i32) -> Option<(i64, i64)>,
        mut clock: impl FnMut() -> Result<SqlDateTime2, E>,
    ) -> Result<(), AdmissionError> {
        self.outcome = Outcome::NoNewbie;
        let version = match policy.get_nullable_integer("AttendanceEvent_Newbie") {
            Ok(v) => v,
            Err(_) => {
                self.outcome = Outcome::CaughtIntegerOverflow;
                return Ok(());
            }
        };
        if !first {
            return Err(AdmissionError::Existing);
        }
        if let Some(version) = version.filter(|v| *v > 0)
            && let Some((from, until)) = period(version)
        {
            let now = match clock() {
                Ok(v) => v.ticks(),
                Err(_) => {
                    self.outcome = Outcome::CaughtClockError;
                    return Ok(());
                }
            };
            if from > now {
                return Ok(());
            }
            let now = match clock() {
                Ok(v) => v.ticks(),
                Err(_) => {
                    self.outcome = Outcome::CaughtClockError;
                    return Ok(());
                }
            };
            if until < now {
                return Ok(());
            }
            return Err(AdmissionError::Active);
        }
        Ok(())
    }
    pub fn all_user_state(&self) -> &AllUser {
        &self.all_user
    }
    pub fn outcome(&self) -> Outcome {
        self.outcome
    }
    pub fn instance_count(&self) -> usize {
        1
    }
    pub fn matches_social(&self, social: &initial_social::Loaded) -> bool {
        Arc::ptr_eq(
            &self.all_user.manager.character,
            &social.character_context(),
        )
    }
}
impl AllUser {
    pub fn uid(&self) -> i32 {
        self.manager.character.uid()
    }
    pub fn cid(&self) -> i64 {
        self.manager.character.cid()
    }
    pub fn event_version(&self) -> i32 {
        -1
    }
    pub fn date_from_ticks(&self) -> i64 {
        0
    }
    pub fn date_until_ticks(&self) -> i64 {
        0
    }
    pub fn db_destroyed_callback_present(&self) -> bool {
        false
    }
    pub fn last_feature_error(&self) -> i32 {
        0
    }
    pub fn attendance_count(&self) -> usize {
        self.attendance_info.len()
    }
    pub fn bonus_count(&self) -> usize {
        self.bonus_info.len()
    }
    pub fn context_matches_parent(&self) -> bool {
        Arc::ptr_eq(&self.manager.character, &self.context.character)
    }
}
