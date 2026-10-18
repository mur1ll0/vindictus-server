//! Restricted empty StatusEffect constructor and following TownCampfire.
//! This detached context cannot attach a frontend or a loaded Player.Character.
use super::{
    character_stats::Stats,
    initial_social::{self, CharacterContext},
    legacy_user_data::SqlDateTime2,
};
use std::{sync::Arc, time::Instant};

/// Dedicated constructor observations; not the global PostSharp profile registry.
#[derive(Default)]
pub struct Profile {
    calls: i64,
    total_ms: f64,
    min_ms: Option<f64>,
    max_ms: Option<f64>,
}
impl Profile {
    fn update(&mut self, ms: f64) {
        if ms >= 0.0 {
            self.calls = self.calls.wrapping_add(1);
            self.total_ms += ms;
            self.min_ms = Some(self.min_ms.map_or(ms, |v| v.min(ms)));
            self.max_ms = Some(self.max_ms.map_or(ms, |v| v.max(ms)));
        }
    }
    pub fn calls(&self) -> i64 {
        self.calls
    }
    pub fn total_ms(&self) -> f64 {
        self.total_ms
    }
    pub fn min_ms(&self) -> f64 {
        self.min_ms.unwrap_or(0.0)
    }
    pub fn max_ms(&self) -> f64 {
        self.max_ms.unwrap_or(0.0)
    }
    pub fn average_ms(&self) -> f64 {
        if self.calls <= 0 {
            0.0
        } else {
            self.total_ms / self.calls as f64
        }
    }
}
pub struct Status {
    character: Arc<CharacterContext>,
    by_type: std::collections::HashMap<String, std::convert::Infallible>,
    by_tag: std::collections::HashMap<String, std::convert::Infallible>,
    stats: Stats,
    save_succeeded: bool,
}
pub struct TownCampfire {
    character: Arc<CharacterContext>,
    status: String,
}
pub struct Loaded {
    status: Status,
    campfire: TownCampfire,
}
impl Loaded {
    /// Empty association must be proven by persistence under the owner lock.
    /// Direct use constructs only local state and provides no database admission.
    /// Clock is invoked inside profiling, once even when there are no effects.
    pub fn load<E>(
        social: &initial_social::Loaded,
        clock: impl FnOnce() -> Result<SqlDateTime2, E>,
        profile: &mut Profile,
    ) -> Result<Self, E> {
        let start = Instant::now();
        let status = (|| {
            let character = social.character_context();
            let by_type = std::collections::HashMap::new();
            let by_tag = std::collections::HashMap::new();
            let _utc_now = clock()?;
            let stats = Stats::default();
            let mut status = Status {
                character,
                by_type,
                by_tag,
                stats,
                save_succeeded: false,
            };
            status.save_succeeded = status.save();
            Ok(status)
        })();
        // ProfileThisMethod.OnSuccess/OnException are inherited no-ops.
        // OnExit records elapsed time on both normal return and propagated error.
        profile.update(start.elapsed().as_secs_f64() * 1000.0);
        let status = status?;
        let campfire = TownCampfire {
            character: social.character_context(),
            status: String::new(),
        };
        Ok(Self { status, campfire })
    }
    pub fn status(&self) -> &Status {
        &self.status
    }
    pub fn campfire(&self) -> &TownCampfire {
        &self.campfire
    }
    pub fn shares_character_context(&self) -> bool {
        Arc::ptr_eq(&self.status.character, &self.campfire.character)
    }
    pub fn matches_social(&self, social: &initial_social::Loaded) -> bool {
        Arc::ptr_eq(&self.status.character, &social.character_context())
    }
}
impl Status {
    fn save(&self) -> bool {
        // Empty association leaves both Need* flags false, so no Submit or stat
        // calculation. The shared detached context has no FrontendConn nor
        // Player.Character. Both notification guards therefore return without
        // messages, another clock sample, MicroPlay update or ReloadCharacter.
        true
    }
    pub fn uid(&self) -> i32 {
        self.character.uid()
    }
    pub fn cid(&self) -> i64 {
        self.character.cid()
    }
    pub fn type_count(&self) -> usize {
        self.by_type.len()
    }
    pub fn tag_count(&self) -> usize {
        self.by_tag.len()
    }
    pub fn stats(&self) -> &Stats {
        &self.stats
    }
    pub fn updated_count(&self) -> i32 {
        0
    }
    pub fn need_db_submit(&self) -> bool {
        false
    }
    pub fn need_update_stat(&self) -> bool {
        false
    }
    pub fn save_succeeded(&self) -> bool {
        self.save_succeeded
    }
}
impl TownCampfire {
    pub fn uid(&self) -> i32 {
        self.character.uid()
    }
    pub fn cid(&self) -> i64 {
        self.character.cid()
    }
    pub fn status(&self) -> &str {
        &self.status
    }
}
