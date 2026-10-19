//! Initial empty-pool Captcha constructor and logout Save parameters;not challenges.
use super::{
    initial_social::{self, CharacterContext},
    legacy_user_data::{SqlDateTime, SqlDateTime2},
};
use std::sync::Arc;
#[derive(Debug, thiserror::Error)]
pub enum ConstructorError {
    #[error("initial captcha clock failed")]
    Clock,
    #[error(transparent)]
    Date(#[from] super::legacy_user_data::LegacyUserDataError),
}
#[derive(Clone, Copy, Default)]
pub struct Source {
    pub punished: Option<u8>,
    pub failure: Option<u8>,
    pub last_failure: Option<SqlDateTime>,
    pub last_succeed: Option<SqlDateTime>,
    pub condition: Option<u8>,
}
pub struct Loaded {
    parent: Arc<CharacterContext>,
    punished: u8,
    failure: u8,
    last_failure: Option<SqlDateTime2>,
    last_succeed: Option<SqlDateTime2>,
    force: bool,
}
impl Loaded {
    /// Provider lookup precedes this call. Clock is called once only when LastFailureTime exists.
    pub fn construct(
        social: &initial_social::Loaded,
        row: Option<Source>,
        mut utc: impl FnMut() -> Result<SqlDateTime2, ConstructorError>,
    ) -> Result<Self, ConstructorError> {
        let r = row.unwrap_or_default();
        // SqlDateTime's validated range always maps into DateTime's range.
        let last_failure = r
            .last_failure
            .map(|d| d.framework_date_time())
            .transpose()?;
        let last_succeed = r
            .last_succeed
            .map(|d| d.framework_date_time())
            .transpose()?;
        let mut failure = r.failure.unwrap_or(0);
        if let Some(last) = last_failure
            && last.ticks() / 864_000_000_000 != utc()?.ticks() / 864_000_000_000
        {
            failure = 0;
        }
        Ok(Self {
            parent: social.character_context(),
            punished: r.punished.unwrap_or(0),
            failure,
            last_failure,
            last_succeed,
            force: r.condition.is_some_and(|v| v != 0),
        })
    }
    pub fn uid(&self) -> i32 {
        self.parent.uid()
    }
    pub fn cid(&self) -> i64 {
        self.parent.cid()
    }
    pub fn punished(&self) -> u8 {
        self.punished
    }
    pub fn failure(&self) -> u8 {
        self.failure
    }
    pub fn last_failure(&self) -> Option<SqlDateTime2> {
        self.last_failure
    }
    pub fn last_succeed(&self) -> Option<SqlDateTime2> {
        self.last_succeed
    }
    pub fn force(&self) -> bool {
        self.force
    }
    pub fn pool_count(&self) -> usize {
        0
    }
    pub fn timeout_schedule(&self) -> i64 {
        0
    }
    pub fn refresh_count(&self) -> i32 {
        0
    }
    pub fn last_check(&self) -> Option<SqlDateTime2> {
        None
    }
    pub fn last_refresh(&self) -> Option<SqlDateTime2> {
        None
    }
    pub fn matches_social(&self, social: &initial_social::Loaded) -> bool {
        Arc::ptr_eq(&self.parent, &social.character_context())
    }
}
