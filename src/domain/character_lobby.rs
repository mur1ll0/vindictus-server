//! Lossless recovered character-lobby source records; no source values in diagnostics.
use super::legacy_user_data::{LegacyBytes, SqlDateTime, SqlDateTime2};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(PartialEq, Eq)]
pub struct UserInfo {
    pub nexon_sn: i32,
    pub last_login_cid: Option<i64>,
    pub prologue_played: bool,
}
#[derive(PartialEq, Eq)]
pub struct UserProduct {
    pub id: i64,
    pub nexon_sn: i32,
    pub product_name: LegacyBytes<128, true>,
    pub argument: Option<LegacyBytes<128, true>>,
    pub expire_time: Option<SqlDateTime2>,
}
#[derive(PartialEq, Eq)]
pub struct UserCareInfo {
    pub uid: i32,
    pub kind: u8,
    pub state: i16,
    pub last_update: SqlDateTime,
}
pub struct LobbySnapshot {
    pub user_info: Vec<UserInfo>,
    pub user_products: Vec<UserProduct>,
    pub user_care_info: Vec<UserCareInfo>,
}
pub const MAX_ROWS: usize = 1000;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LobbyValueError {
    #[error("character-lobby snapshot exceeds bounds or has duplicate keys")]
    Bound,
    #[error("product name is outside the verified SQL collation subset")]
    ProductName,
}
impl LobbySnapshot {
    pub fn validate(&self) -> Result<(), LobbyValueError> {
        use std::collections::BTreeSet;
        if self.user_info.len() > MAX_ROWS
            || self.user_products.len() > MAX_ROWS
            || self.user_care_info.len() > MAX_ROWS
        {
            return Err(LobbyValueError::Bound);
        }
        let mut ids = BTreeSet::new();
        for r in &self.user_info {
            if !ids.insert(r.nexon_sn) {
                return Err(LobbyValueError::Bound);
            }
        }
        let mut ids = BTreeSet::new();
        for r in &self.user_products {
            if !ids.insert(r.id) {
                return Err(LobbyValueError::Bound);
            }
        }
        let mut ids = BTreeSet::new();
        for r in &self.user_care_info {
            if !ids.insert(r.uid) {
                return Err(LobbyValueError::Bound);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductKind {
    CharacterSlot,
    PremiumCharacterSlot,
    Other,
}
/// ASCII CI_AS comparison subset; retain arbitrary UTF16 in snapshots but refuse unproved queries.
pub fn classify_product(name: &LegacyBytes<128, true>) -> Result<ProductKind, LobbyValueError> {
    let mut key = Zeroizing::new(Vec::with_capacity(name.as_bytes().len() / 2));
    for b in name.as_bytes().as_chunks::<2>().0 {
        let u = u16::from_le_bytes([b[0], b[1]]);
        if !(32..=126).contains(&u) {
            return Err(LobbyValueError::ProductName);
        }
        key.push((u as u8).to_ascii_lowercase());
    }
    while key.last() == Some(&b' ') {
        key.pop();
    }
    Ok(match key.as_slice() {
        b"characterslot" => ProductKind::CharacterSlot,
        b"premiumcharacterslot" => ProductKind::PremiumCharacterSlot,
        _ => ProductKind::Other,
    })
}

/// This bounded result represents only a genuinely empty GetCharacters result.
/// No API fabricates summaries for source characters; nonempty data is explicitly refused.
pub struct EmptyLobby {
    pub last_cid: i64,
    pub prologue_played: bool,
    pub max_free: i32,
    pub max_purchased: i32,
    pub max_premium: i32,
    pub preset_used: i32,
}
