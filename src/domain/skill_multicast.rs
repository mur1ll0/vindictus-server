//! Ordered GiveSkill multicast suffix. Hooks are mandatory runtime operations, not
//! acknowledgements: a completed dispatch alone does not certify an initialized actor.
use super::skill_runtime::{Argument, Cache, CacheError};
use thiserror::Error;

pub struct Grant<'a> {
    pub key: &'a [u8],
    pub rank: i32,
    pub required_ap: i32,
    pub arguments: &'a [Argument],
    /// Classification must come from the bound original SkillEffect/profile.
    pub item_skill: bool,
    pub stat_skill: bool,
}
/// Internal calls recovered from the multicast subscribers; these are not wire messages.
/// Pushes and Calculate* operations retain their original guards in the runtime adapter.
#[derive(Clone, Copy)]
pub enum Call<'a> {
    RecalculateAp,
    PushAp,
    GuildSkillUp { key: &'a [u8], rank: i32 },
    PushSkillList,
    CalculateItemStat,
    CalculateFinalStat,
    PushUpdateStat,
    SkillUpdated,
    MicroPlayUpdated,
}
pub trait Runtime {
    type Error;
    /// Cache changes precede every call. Implementations must execute the operation,
    /// including source null/connection guards, or return an error; never silently defer it.
    fn call(&mut self, call: Call<'_>, cache: &Cache) -> Result<(), Self::Error>;
}
#[derive(Debug, Error)]
pub enum DispatchError<E> {
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error("skill runtime operation failed; earlier cache/effects are retained")]
    Runtime(E),
}
/// Starts after the independent GiveSkill log and before the next grant/Submit.
/// Cache failures stop before the first hook. Any later failure stops subsequent
/// subscribers without rewinding cache or completed runtime effects.
pub fn after_grant<R: Runtime>(
    cache: &mut Cache,
    grant: Grant<'_>,
    runtime: &mut R,
) -> Result<(), DispatchError<R::Error>> {
    cache.after_grant(grant.key, grant.rank, grant.required_ap, grant.arguments)?;
    let mut call = |operation| {
        runtime
            .call(operation, cache)
            .map_err(DispatchError::Runtime)
    };
    // Original IsMeditation uses case-sensitive string equality.
    if is_meditation(grant.key) {
        call(Call::RecalculateAp)?;
        call(Call::PushAp)?;
    }
    call(Call::GuildSkillUp {
        key: grant.key,
        rank: grant.rank,
    })?;
    call(Call::PushSkillList)?;
    // Original StatManager subscriber uses if/else-if, even if both sets contain the ID.
    if grant.item_skill {
        call(Call::CalculateItemStat)?;
        call(Call::CalculateFinalStat)?;
        call(Call::PushUpdateStat)?;
    } else if grant.stat_skill {
        call(Call::CalculateFinalStat)?;
        call(Call::PushUpdateStat)?;
    }
    call(Call::SkillUpdated)?;
    call(Call::MicroPlayUpdated)
}

pub(crate) fn is_meditation(key: &[u8]) -> bool {
    key == b"s\0k\0i\0l\0l\0_\0m\0e\0d\0i\0t\0a\0t\0i\0o\0n\0"
}
