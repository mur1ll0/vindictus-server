//! Complete private source snapshots. No Debug/Serialize of names or mail content.
use crate::domain::legacy_user_data::{LegacyBytes, SourceRowVersion, SqlDateTime, SqlDateTime2};
#[derive(PartialEq, Eq)]
pub struct CharacterIdentity {
    pub id: i64,
}
#[derive(PartialEq, Eq)]
pub struct CharacterSnapshot {
    pub id: i64,
    pub uid: i32,
    pub character_sn: i32,
    pub name: Option<LegacyBytes<50, true>>,
    pub class: i16,
    pub level: i32,
    pub exp: i64,
    pub str: i32,
    pub dex: i32,
    pub int: i32,
    pub will: i32,
    pub luck: i32,
    pub hp: i32,
    pub stamina: i32,
    pub dirty: bool,
    pub play_time: i64,
    pub selected_title: Option<i32>,
    pub ap: i32,
    pub total_used_ap: i32,
    pub last_ap_update: SqlDateTime,
    pub permission: i32,
    pub status: i32,
    pub quote: Option<LegacyBytes<50, true>>,
    pub delete_time: Option<SqlDateTime>,
    pub region_code: i32,
    pub is_connected: bool,
    pub last_update: Option<SourceRowVersion>,
    pub create_time: SqlDateTime2,
    pub free_match_win_count: i32,
    pub free_match_lose_count: i32,
    pub use_preset: bool,
    pub is_premium: bool,
    pub free_title_name: Option<LegacyBytes<50, true>>,
    pub today_play_time: Option<i32>,
    pub channel: Option<i16>,
    pub order_num: Option<i16>,
    pub map_state_info: i16,
}
#[derive(PartialEq, Eq)]
pub struct ConnectionSnapshot {
    pub id: i64,
    pub last_connect_date: SqlDateTime,
    pub last_cafe_bonus: Option<SqlDateTime>,
    pub last_cafe_login: Option<SqlDateTime>,
    pub last_cafe_logout: Option<SqlDateTime>,
    pub last_home_cafe_login: Option<SqlDateTime>,
    pub last_home_cafe_logout: Option<SqlDateTime>,
    pub last_logout: Option<SqlDateTime>,
}
#[derive(PartialEq, Eq)]
pub struct QueuedItemSnapshot {
    pub row_id: i32,
    pub cid: i64,
    pub item_class_ex: LegacyBytes<1024, true>,
    pub count: i32,
    pub is_character_binded: bool,
    pub mail_title: Option<LegacyBytes<256, true>>,
    pub mail_content: Option<LegacyBytes<1024, true>>,
    pub color1: i32,
    pub color2: i32,
    pub color3: i32,
    pub reduced_durability: i32,
    pub max_durability_bonus: i32,
    pub item_id: Option<i64>,
    pub processed: bool,
    pub event_code: Option<LegacyBytes<50, true>>,
    pub processed_time: Option<SqlDateTime>,
    pub expire_time: Option<SqlDateTime>,
    pub log_date: Option<SqlDateTime>,
}
pub struct CharacterBundle {
    pub characters: Vec<CharacterIdentity>,
    pub character_info: Vec<CharacterSnapshot>,
    pub connections: Vec<ConnectionSnapshot>,
    pub queued_items: Vec<QueuedItemSnapshot>,
}
impl CharacterBundle {
    pub fn validate(&self) -> Result<(), crate::error::Error> {
        use crate::error::Error;
        use std::collections::BTreeSet;
        for size in [
            self.characters.len(),
            self.character_info.len(),
            self.connections.len(),
            self.queued_items.len(),
        ] {
            if size > 1000 {
                return Err(Error::CharacterExport);
            }
        }
        fn unique<T: Ord>(values: impl Iterator<Item = T>) -> bool {
            let mut ids = BTreeSet::new();
            values.into_iter().all(|id| ids.insert(id))
        }
        if !unique(self.characters.iter().map(|x| x.id))
            || !unique(self.character_info.iter().map(|x| x.id))
            || !unique(self.connections.iter().map(|x| x.id))
            || !unique(self.queued_items.iter().map(|x| x.row_id))
        {
            return Err(Error::CharacterExport);
        }
        for row in &self.character_info {
            if !(0..=255).contains(&row.class)
                || !(0..=255).contains(&row.map_state_info)
                || row.order_num.is_some_and(|x| !(0..=255).contains(&x))
            {
                return Err(Error::CharacterExport);
            }
        }
        Ok(())
    }
}
