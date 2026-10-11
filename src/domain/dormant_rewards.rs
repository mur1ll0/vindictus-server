//! Recovered character/reward loop; planning never mutates dormancy or delivers inventory.
use crate::{
    domain::{
        character_data::CharacterSnapshot,
        dormant_item::DormantItemRequest,
        feature_policy::FeaturePolicy,
        legacy_user_data::{LegacyBytes, SqlDateTime},
        login_effects::dormant_reward_matches,
    },
    error::Error,
};
pub struct DormantReward {
    pub item_class: LegacyBytes<50, true>,
    pub num: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub feature: Option<String>,
}
pub struct RewardCharacter<'a> {
    pub id: i64,
    pub level: i32,
    pub name: Option<&'a LegacyBytes<50, true>>,
}
impl<'a> From<&'a CharacterSnapshot> for RewardCharacter<'a> {
    fn from(row: &'a CharacterSnapshot) -> Self {
        Self {
            id: row.id,
            level: row.level,
            name: row.name.as_ref(),
        }
    }
}
const TITLE: &str = "#GameUI_Heroes_DormantAccount_Mail_Title";
const CONTENT: &str = "#GameUI_Heroes_DormantAccount_Mail_Text,";
fn utf16(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
/// Keep character input order (GetCharacters) and catalog iteration order, including duplicates.
/// Capture one explicit quantized UTC clock per eligible source SendDormantItem call.
pub fn plan_rewards(
    characters: &[RewardCharacter<'_>],
    rewards: &[DormantReward],
    policy: &FeaturePolicy,
    mut clock: impl FnMut() -> Result<SqlDateTime, Error>,
) -> Result<Vec<DormantItemRequest>, Error> {
    if characters.len() > 1000 || rewards.len() > 1000 {
        return Err(Error::RewardPlanLimit);
    }
    let mut result = Vec::new();
    for character in characters {
        for reward in rewards {
            // Preserve source short-circuit order: evaluate Feature before both level bounds.
            if dormant_reward_matches(
                character.level,
                reward.min_level,
                reward.max_level,
                policy.is_enabled(reward.feature.as_deref())?,
            ) {
                if result.len() >= 8192 {
                    return Err(Error::RewardPlanLimit);
                }
                let mut content = utf16(CONTENT);
                if let Some(name) = character.name {
                    content.extend_from_slice(name.as_bytes());
                }
                result.push(DormantItemRequest {
                    cid: character.id,
                    item_class_ex: LegacyBytes::from_bytes(reward.item_class.as_bytes().to_vec())?,
                    count: reward.num,
                    is_binded: true,
                    mail_title: Some(LegacyBytes::from_bytes(utf16(TITLE))?),
                    mail_content: Some(LegacyBytes::from_bytes(content)?),
                    log_date: clock()?,
                });
            }
        }
    }
    Ok(result)
}
