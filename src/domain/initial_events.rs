//! Instance-only Roulette -> Marble constructor prefix. Static beforefieldinit
//! state, event loading/scheduling, RNG draws and frontend messages are excluded.
use super::initial_social::{self, CharacterContext};
use std::{collections::BTreeMap, sync::Arc};
struct ItemContext {
    character: Arc<CharacterContext>,
}
pub struct Roulette {
    parent: Arc<CharacterContext>,
    context: Arc<ItemContext>,
    slots: Vec<i32>,
    appearances: BTreeMap<i32, BTreeMap<i32, i32>>,
}
pub struct Marble {
    parent: Arc<CharacterContext>,
    context: Arc<ItemContext>,
    journal: i64,
    chance_face: i32,
    chance_indices: Vec<i32>,
}
/// Journal attempt occurs after both distinct item-context allocations and before
/// the final Marble assignments. No successful actor or DB capability is implied.
pub struct Pending {
    roulette: Roulette,
    marble: Marble,
}
pub struct Loaded {
    roulette: Roulette,
    marble: Marble,
}
impl Pending {
    pub fn before_journal(social: &initial_social::Loaded) -> Self {
        let parent = social.character_context();
        let roulette = Roulette {
            parent: Arc::clone(&parent),
            context: Arc::new(ItemContext {
                character: Arc::clone(&parent),
            }),
            slots: Vec::new(),
            appearances: BTreeMap::new(),
        };
        let marble = Marble {
            parent: Arc::clone(&parent),
            context: Arc::new(ItemContext { character: parent }),
            journal: 0,
            chance_face: 0,
            chance_indices: Vec::new(),
        };
        Self { roulette, marble }
    }
    pub fn roulette(&self) -> &Roulette {
        &self.roulette
    }
    pub fn marble_before_journal(&self) -> &Marble {
        &self.marble
    }
    pub fn finish(mut self, journal: i64) -> Loaded {
        self.marble.journal = journal;
        self.marble.chance_face = -1;
        Loaded {
            roulette: self.roulette,
            marble: self.marble,
        }
    }
}
impl Loaded {
    pub fn roulette(&self) -> &Roulette {
        &self.roulette
    }
    pub fn marble(&self) -> &Marble {
        &self.marble
    }
    pub fn contexts_distinct(&self) -> bool {
        !Arc::ptr_eq(&self.roulette.context, &self.marble.context)
    }
    pub fn shares_parent(&self) -> bool {
        Arc::ptr_eq(&self.roulette.parent, &self.marble.parent)
    }
    pub fn matches_social(&self, social: &initial_social::Loaded) -> bool {
        Arc::ptr_eq(&self.roulette.parent, &social.character_context())
    }
}
impl Roulette {
    pub fn uid(&self) -> i32 {
        self.parent.uid()
    }
    pub fn cid(&self) -> i64 {
        self.parent.cid()
    }
    pub fn version(&self) -> i32 {
        -1
    }
    pub fn id(&self) -> i32 {
        -1
    }
    pub fn expiry_ticks(&self) -> i64 {
        0
    }
    /// Initial ID=-1 short-circuits the original getter before DateTime.UtcNow.
    pub fn initially_expired(&self) -> bool {
        true
    }
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
    pub fn appearance_count(&self) -> usize {
        self.appearances.len()
    }
    pub fn context_matches_parent(&self) -> bool {
        Arc::ptr_eq(&self.parent, &self.context.character)
    }
}
impl Marble {
    pub fn uid(&self) -> i32 {
        self.parent.uid()
    }
    pub fn cid(&self) -> i64 {
        self.parent.cid()
    }
    pub fn id(&self) -> i32 {
        -1
    }
    pub fn current_index(&self) -> i32 {
        -1
    }
    pub fn schedule_id(&self) -> i64 {
        0
    }
    pub fn journal_id(&self) -> i64 {
        self.journal
    }
    pub fn state(&self) -> i32 {
        0
    }
    pub fn chance_face(&self) -> i32 {
        self.chance_face
    }
    pub fn chance_index_count(&self) -> usize {
        self.chance_indices.len()
    }
    pub fn board_present(&self) -> bool {
        false
    }
    pub fn node_list_present(&self) -> bool {
        false
    }
    pub fn initialized(&self) -> bool {
        false
    }
    pub fn context_matches_parent(&self) -> bool {
        Arc::ptr_eq(&self.parent, &self.context.character)
    }
}
