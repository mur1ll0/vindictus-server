//! Detached constructor links and initial guild projection, not whisper/guild services.
use crate::{domain::feature_policy::FeaturePolicy, error::Error};
use std::sync::Arc;

struct PlayerContext {
    uid: i32,
    cid: i64,
}
struct CharacterContext {
    player: Arc<PlayerContext>,
}
/// The two source links share the same admitted player context.
pub struct Whisper {
    player: Arc<PlayerContext>,
    character: Arc<CharacterContext>,
}
pub struct Guild {
    parent: Arc<PlayerContext>,
    info_loaded: bool,
    pending_procedures: usize,
}
pub struct Loaded {
    whisper: Whisper,
    guild: Guild,
}
impl Loaded {
    /// Only the recovered InGameGuild=true branch is admitted. The false branch
    /// requires UpdateGuildInfo, notifications, rank titles and daily item processing.
    pub fn load(uid: i32, cid: i64, policy: &FeaturePolicy) -> Result<Self, Error> {
        if !policy.is_enabled(Some("InGameGuild"))? {
            return Err(Error::FionaInitializerProfile);
        }
        let player = Arc::new(PlayerContext { uid, cid });
        let character = Arc::new(CharacterContext {
            player: Arc::clone(&player),
        });
        let whisper = Whisper {
            player: Arc::clone(&player),
            character,
        };
        let guild = Guild {
            parent: player,
            info_loaded: false,
            pending_procedures: 0,
        };
        Ok(Self { whisper, guild })
    }
    pub fn whisper(&self) -> &Whisper {
        &self.whisper
    }
    pub fn guild(&self) -> &Guild {
        &self.guild
    }
    pub fn shares_player_context(&self) -> bool {
        Arc::ptr_eq(&self.whisper.player, &self.guild.parent)
    }
}
impl Whisper {
    pub fn shares_character_player_context(&self) -> bool {
        Arc::ptr_eq(&self.player, &self.character.player)
    }
    pub fn uid(&self) -> i32 {
        self.player.uid
    }
    pub fn cid(&self) -> i64 {
        self.player.cid
    }
}
impl Guild {
    pub fn uid(&self) -> i32 {
        self.parent.uid
    }
    pub fn cid(&self) -> i64 {
        self.parent.cid
    }
    pub fn info_loaded(&self) -> bool {
        self.info_loaded
    }
    /// Constructor projection only. No DailyItemProcedure queue mutation is exposed.
    pub fn pending_procedure_count(&self) -> usize {
        self.pending_procedures
    }
}
