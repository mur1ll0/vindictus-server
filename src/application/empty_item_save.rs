//! Source Save ordering for the admitted empty initial context.
//! ReloadRequired is deliberately not a completed Undo implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    QueueSync,
    Sync,
    ProcessAfterDelete,
    WriteLogs,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Saved,
    ReloadRequired,
}
pub fn after_submit(accepted: bool, queued: bool, mut apply: impl FnMut(Effect)) -> Flow {
    if !accepted {
        return Flow::ReloadRequired;
    }
    apply(if queued {
        Effect::QueueSync
    } else {
        Effect::Sync
    });
    apply(Effect::ProcessAfterDelete);
    apply(Effect::WriteLogs);
    Flow::Saved
}

/// Original ItemOwner.IsCharacter uses the signed entity ID category bits.
pub fn is_character_entity(cid: i64) -> bool {
    cid >> 54 == 35
}
