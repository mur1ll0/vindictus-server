//! Lossless HomeCafeBonus record and recovered account-level preference.
use super::legacy_user_data::SqlDateTime;
use thiserror::Error;
#[derive(PartialEq, Eq)]
pub struct HomeCafeBonus {
    pub uid: i32,
    pub cid: i64,
    pub bonus_type: u8,
    pub expired_time: SqlDateTime,
}
pub const MAX_ROWS: usize = 1000;
#[derive(Debug, Error, PartialEq, Eq)]
#[error("home-cafe snapshot exceeds bounds or contains duplicate source keys")]
pub struct HomeCafeValueError;
pub fn validate(rows: &[HomeCafeBonus]) -> Result<(), HomeCafeValueError> {
    if rows.len() > MAX_ROWS {
        return Err(HomeCafeValueError);
    }
    let mut keys = std::collections::BTreeSet::new();
    for r in rows {
        if !keys.insert((r.uid, r.cid, r.bonus_type)) {
            return Err(HomeCafeValueError);
        }
    }
    Ok(())
}
/// GetHomeCafeBonusInfo's CID=NULL branch does not inspect expiration or modify that output.
/// Its absent-row result retains the caller's original bonus_type, including NULL.
pub fn account_bonus(rows: &[HomeCafeBonus], uid: i32, initial: Option<u8>) -> Option<u8> {
    [3, 4, 2]
        .into_iter()
        .find(|b| {
            rows.iter()
                .any(|r| r.uid == uid && r.cid == 0 && r.bonus_type == *b)
        })
        .or(initial)
}
