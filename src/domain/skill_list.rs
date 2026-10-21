//! SkillManager.ToSkillList for a loaded character: BaseSkillRank order, next-rank
//! AP, ConstraintPassedForNextRank, untrain/reset AP. Skill enhances are not
//! migrated and must be established absent by the caller.
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("skill rows or contents are ambiguous or outside the bounded profile")]
pub struct SkillListError;

/// SkillRankInfo after GetTable/ToDoubleDictionary (the last duplicate wins).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RankInfo {
    pub level: i32,
    pub required_ap: i32,
    pub require_unlock: bool,
    /// FeatureMatrix.IsEnable(rankInfo.Feature), evaluated by the content binding.
    pub feature_enabled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SkillContent {
    /// SkillInfo.ClassRestriction; None when the SkillInfo row is absent.
    pub class_restriction: Option<i32>,
    /// SkillInfo.UseType; IsActiveSkill is UseType != 0.
    pub use_type: Option<i32>,
    pub ranks: BTreeMap<i32, RankInfo>,
    /// SkillConstraintInfo by rank, in source list order.
    pub constraints: BTreeMap<i32, Vec<(String, i32)>>,
}

/// Character.Skill row.
#[derive(Clone, Debug)]
pub struct Skill {
    pub id: String,
    pub rank: i32,
    pub locked: u8,
    pub current_ap: i32,
}

/// BriefSkillInfo without enhances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brief {
    pub id: String,
    pub base_rank: i32,
    pub final_rank: i32,
    pub required_ap: i32,
    pub is_locked: u8,
    pub can_start_training: u8,
    pub current_ap: i32,
    pub reset_ap: i32,
    pub untrain_ap: i32,
}

pub struct Character<'a> {
    pub class: i32,
    pub level: i32,
    /// SetBonusInfo skill ranks (ItemSkillRank).
    pub item_ranks: &'a BTreeMap<String, i32>,
}

/// `skills` in Character.Skill enumeration order; a duplicate SkillID would make
/// BaseSkillRank.Add throw in the source and is refused.
pub fn to_skill_list(
    skills: &[Skill],
    contents: &BTreeMap<String, SkillContent>,
    character: &Character<'_>,
) -> Result<Vec<Brief>, SkillListError> {
    if skills.len() > 4096 {
        return Err(SkillListError);
    }
    let mut base: Vec<(&str, i32)> = Vec::new();
    for s in skills {
        if base.iter().any(|(id, _)| *id == s.id) {
            return Err(SkillListError);
        }
        base.push((&s.id, s.rank));
    }
    let base_rank = |id: &str| {
        base.iter()
            .find(|(k, _)| *k == id)
            .map_or(-1, |(_, rank)| *rank)
    };
    let empty = SkillContent::default();
    let mut list = Vec::with_capacity(skills.len());
    for s in skills {
        let content = contents.get(&s.id).unwrap_or(&empty);
        let rank_info = |rank: i32| content.ranks.get(&rank).copied();
        // CalculateRequiredAP: the next rank's positive RequiredAP, otherwise -1.
        let required_ap = rank_info(s.rank.wrapping_add(1))
            .filter(|r| r.required_ap > 0)
            .map_or(-1, |r| r.required_ap);
        let item_rank = character.item_ranks.get(&s.id).copied().unwrap_or(0);
        let next = rank_info(base_rank(&s.id).wrapping_add(1));
        let can_start = s.locked != 1
            && s.locked != 2
            && next.is_some_and(|info| {
                required_ap >= 0
                    && content.class_restriction.is_some_and(|restriction| {
                        info.feature_enabled
                            && {
                                // IsAvailableClass: (ClassRestriction & bit) == bit.
                                let bit = 1_i32.wrapping_shl(character.class as u32 & 31);
                                restriction & bit == bit
                            }
                            && character.level >= info.level
                            && content
                                .constraints
                                .get(&base_rank(&s.id).wrapping_add(1))
                                .is_none_or(|rows| rows.iter().all(|(id, r)| base_rank(id) >= *r))
                    })
            });
        // GetUntrainAP: CanUntrain requires a rank info above rank 1.
        let untrain_ap = match rank_info(s.rank) {
            Some(info) if s.rank > 1 => info.required_ap.wrapping_add(s.current_ap),
            _ => -1,
        };
        // GetResetAP: current AP plus every RequiredAP down to rank 2.
        let mut reset_ap = s.current_ap;
        let mut rank = s.rank;
        while rank > 1 {
            match rank_info(rank) {
                Some(info) => reset_ap = reset_ap.wrapping_add(info.required_ap),
                None => {
                    reset_ap = -1;
                    break;
                }
            }
            rank -= 1;
        }
        list.push(Brief {
            id: s.id.clone(),
            base_rank: s.rank,
            final_rank: s.rank.wrapping_add(item_rank),
            required_ap,
            is_locked: s.locked,
            can_start_training: u8::from(can_start),
            current_ap: s.current_ap,
            reset_ap,
            untrain_ap,
        });
    }
    Ok(list)
}
