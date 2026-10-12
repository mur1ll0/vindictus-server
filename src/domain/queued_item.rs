//! Recovered queue selection and routing. Decisions do not deliver or acknowledge items.
use crate::domain::{
    character_data::QueuedItemSnapshot,
    legacy_user_data::{LegacyBytes, LegacyUserDataError, SqlDateTime2},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueMailType {
    Gm,
    GuildLevelUp,
    Auction,
    Dungeon,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueBinder {
    Default,
    MobileApp,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueRoute {
    Expired { system_item_id: Option<i64> },
    CreateToMail,
    CreateToInventory,
    ExistingToMail { item_id: i64 },
    ExistingToInventory { item_id: i64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueDecision {
    pub mail_type: QueueMailType,
    pub binder: QueueBinder,
    pub route: QueueRoute,
}

/// DBLoader has no ORDER BY, expiry filter or character-state predicate.
pub fn selected(row: &QueuedItemSnapshot, owner_cid: i64) -> bool {
    row.cid == owner_cid && !row.processed
}
/// Empty EventCode is an event; only SQL/CLR null bypasses the refresh gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueAttemptDecision {
    pub should_attempt: bool,
    /// Source increments after the catch, even when delivery fails.
    pub count_as_event: bool,
}
pub fn attempt_decision(
    event_code: Option<&LegacyBytes<50, true>>,
    item_festival: bool,
    refresh_target_contains_owner: bool,
) -> QueueAttemptDecision {
    let should_attempt = item_festival || !refresh_target_contains_owner || event_code.is_none();
    QueueAttemptDecision {
        should_attempt,
        count_as_event: should_attempt && event_code.is_some(),
    }
}
fn equals_utf16(bytes: &[u8], text: &str) -> bool {
    bytes
        .iter()
        .copied()
        .eq(text.encode_utf16().flat_map(u16::to_le_bytes))
}
/// The clock is the source per-row UTC sample, compared to SqlClient's millisecond DateTime.
/// Title/content need only be non-null: empty and opaque UTF-16 remain valid here.
/// Missing catalog/system items and delivery failures must be handled by a future executor.
pub fn decide(
    row: &QueuedItemSnapshot,
    now: SqlDateTime2,
) -> Result<QueueDecision, LegacyUserDataError> {
    let event = row.event_code.as_ref().map(LegacyBytes::as_bytes);
    let (mail_type, binder) = match event {
        Some(value) if equals_utf16(value, "GuildLevelUp") => {
            (QueueMailType::GuildLevelUp, QueueBinder::Default)
        }
        Some(value) if equals_utf16(value, "MobileAppBuy") => {
            (QueueMailType::Auction, QueueBinder::MobileApp)
        }
        Some(value) if equals_utf16(value, "FriendEventReward_Add") => {
            (QueueMailType::Dungeon, QueueBinder::Default)
        }
        _ => (QueueMailType::Gm, QueueBinder::Default),
    };
    let route = if row
        .expire_time
        .map(|date| date.framework_date_time())
        .transpose()?
        .is_some_and(|date| date.ticks() <= now.ticks())
    {
        QueueRoute::Expired {
            system_item_id: row.item_id,
        }
    } else {
        let custom_mail = row.mail_title.is_some() && row.mail_content.is_some();
        match (row.item_id, custom_mail) {
            (None, true) => QueueRoute::CreateToMail,
            (None, false) => QueueRoute::CreateToInventory,
            (Some(item_id), true) => QueueRoute::ExistingToMail { item_id },
            (Some(item_id), false) => QueueRoute::ExistingToInventory { item_id },
        }
    };
    Ok(QueueDecision {
        mail_type,
        binder,
        route,
    })
}
/// Caller supplies the resolved base item class, not ItemClassEx with attributes.
/// Keep lazy feature evaluation before the currency override, including feature errors.
/// MobileAppBuy uses item_binding::check_character_bind; this function covers Default only.
pub fn default_character_binder<E>(
    is_binded: bool,
    item_class: &LegacyBytes<1024, true>,
    bind_feature: impl FnOnce() -> Result<bool, E>,
) -> Result<bool, E> {
    let bind = is_binded || bind_feature()?;
    if equals_utf16(item_class.as_bytes(), "gold")
        || equals_utf16(item_class.as_bytes(), "silver_coin")
    {
        Ok(false)
    } else {
        Ok(bind)
    }
}
