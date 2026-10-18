//! Restricted, unpublished skill-phase context. It has no loaded equipment,
//! frontend, observer proxies or event subscribers and cannot acquire them.
//! This is not a general CharacterEntity or a loader for an existing/selected actor.
use super::{
    character_ap::{self, Snapshot},
    legacy_user_data::SqlDateTime2,
    skill_multicast::{Call, Runtime, is_meditation},
    skill_runtime::Cache,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("skill phase exceeds the detached, unloaded creation context")]
pub struct ContextError;

pub struct Detached {
    ap: character_ap::State,
}
impl Detached {
    /// Constructs a separate local context; no existing entity is attached or reset.
    /// GuildScore must be disabled by the bound profile. Its general runtime is excluded.
    pub fn new(
        ap: i32,
        last_update: SqlDateTime2,
        guild_score: bool,
    ) -> Result<Self, ContextError> {
        if guild_score {
            return Err(ContextError);
        }
        Ok(Self {
            ap: character_ap::State::fresh(ap, last_update),
        })
    }
    /// Admission happens before any independent grant log. Meditation would require
    /// AP calculation and is outside this context, even if its metadata flag is wrong.
    pub fn admits(key: &[u8]) -> bool {
        !is_meditation(key)
    }
    pub fn ap(&self) -> Snapshot {
        self.ap.snapshot()
    }
}
impl Runtime for Detached {
    type Error = ContextError;
    fn call(&mut self, call: Call<'_>, _cache: &Cache) -> Result<(), ContextError> {
        match call {
            // Not reachable through admitted grants. Do not simulate a trained interval.
            Call::RecalculateAp => Err(ContextError),
            // Original methods return before reading stats/APMax/clock or creating messages
            // because this context has no FrontendConn and exposes no way to bind one.
            Call::PushAp | Call::PushSkillList | Call::PushUpdateStat => Ok(()),
            // Original guard returns when GuildScore is disabled.
            Call::GuildSkillUp { .. } => Ok(()),
            // CalculateItemStat(null,null) retains null EquippedItemInfo; CalculateFinalStat
            // returns with null ItemStat. No base-load provider, INT/AP or stat event is read.
            Call::CalculateItemStat | Call::CalculateFinalStat => Ok(()),
            // Updated iterates zero proxies and sees a null CacheUpdated delegate. This
            // type cannot subscribe/attach observers; do not invent dirty revisions.
            Call::SkillUpdated | Call::MicroPlayUpdated => Ok(()),
        }
    }
}
