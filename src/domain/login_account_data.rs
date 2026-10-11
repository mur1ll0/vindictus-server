//! Complete source columns for loyalty and achievement preservation; no credentials or implicit clocks.
use crate::domain::legacy_user_data::{LegacyBytes, SqlDateTime};
#[derive(PartialEq, Eq)]
pub struct LoyaltySnapshot {
    pub recommender_uid: i32,
    pub recommender_cid: Option<i64>,
    pub friend_cid: Option<i64>,
    pub recommend_flag: i32,
    pub update_time: SqlDateTime,
}
#[derive(PartialEq, Eq)]
pub struct AchievementSnapshot {
    pub id: i32,
    pub uid: i32,
    pub name: LegacyBytes<24, true>,
    pub achievement: LegacyBytes<50, true>,
}
