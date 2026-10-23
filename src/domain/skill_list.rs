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

/// SkillArgumentInfo KeyWord (UTF16 bytes) and nullable Value.
pub type SkillArgument = (Vec<u8>, Option<i32>);

#[derive(Clone, Debug, Default)]
pub struct SkillContent {
    /// SkillInfo.ClassRestriction; None when the SkillInfo row is absent.
    pub class_restriction: Option<i32>,
    /// SkillInfo.UseType; IsActiveSkill is UseType != 0.
    pub use_type: Option<i32>,
    /// SkillInfo.Price: LearnNewSkill cost in gold; negative cannot be bought.
    pub price: Option<i32>,
    pub ranks: BTreeMap<i32, RankInfo>,
    /// SkillConstraintInfo by rank, in source list order.
    pub constraints: BTreeMap<i32, Vec<(String, i32)>>,
    /// SkillArgumentInfo by rank: (KeyWord UTF16, Value) in source list order.
    pub arguments: BTreeMap<i32, Vec<SkillArgument>>,
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

/// Result of SkillManager.Learn followed by CompleteSkill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Learned {
    /// Character AP after UseAP and any refund.
    pub ap: i32,
    /// TotalUsedAP increment (ap - refund).
    pub used: i32,
    pub rank: i32,
    pub current_ap: i32,
    pub locked: u8,
    /// CompleteSkill raised the rank (SkillCompletionMessage is sent).
    pub completed: bool,
}

/// Learn(skill, ap) then CompleteSkill(skill), for an AP manager with BonusAPTime 0
/// (UpdateAP is a no-op). None is the source `false` path: nothing changes.
pub fn learn(
    skills: &[Skill],
    contents: &BTreeMap<String, SkillContent>,
    character: &Character<'_>,
    skill_id: &str,
    character_ap: i32,
    ap: i32,
) -> Result<Option<Learned>, SkillListError> {
    let list = to_skill_list(skills, contents, character)?;
    let (Some(skill), Some(brief)) = (
        skills.iter().find(|s| s.id == skill_id),
        list.iter().find(|b| b.id == skill_id),
    ) else {
        return Ok(None);
    };
    // Learn: ap > 0, ConstraintPassedForNextRank == null, RequiredAP > 0 and UseAP.
    if ap <= 0 || brief.can_start_training == 0 || brief.required_ap <= 0 || ap > character_ap {
        return Ok(None);
    }
    let required = brief.required_ap;
    let mut remaining = character_ap - ap;
    let mut current = skill.current_ap.wrapping_add(ap);
    let mut refund = 0;
    if current > required {
        refund = current - required;
        current = required;
        // GiveAP(refund, allowExceed: true): unchecked Int32 addition.
        remaining = remaining.wrapping_add(refund);
    }
    let used = ap - refund;
    // CompleteSkill: only when CurrentAP reaches the next rank's RequiredAP.
    if required - current > 0 {
        return Ok(Some(Learned {
            ap: remaining,
            used,
            rank: skill.rank,
            current_ap: current,
            locked: skill.locked,
            completed: false,
        }));
    }
    let rank = skill.rank.wrapping_add(1);
    // RecalculateRequiredAP: the new next rank's RequireUnlock locks the skill.
    let content = contents.get(skill_id).ok_or(SkillListError)?;
    let locked = u8::from(
        content
            .ranks
            .get(&rank.wrapping_add(1))
            .is_some_and(|r| r.require_unlock),
    );
    Ok(Some(Learned {
        ap: remaining,
        used,
        rank,
        current_ap: 0,
        locked,
        completed: true,
    }))
}

/// ToSkillList's vocation entry: VocationEnum.Paladin = 0, DarkKnight = 1 (None = -1).
pub fn vocation_entry(vocation_class: Option<i32>) -> Option<Brief> {
    let id = match vocation_class {
        Some(0) => "skill_voc_pl_transform00",
        Some(1) => "skill_voc_dk_transform00",
        _ => return None,
    };
    Some(Brief {
        id: id.to_owned(),
        base_rank: 1,
        final_rank: 1,
        required_ap: -1,
        is_locked: 0,
        can_start_training: 0,
        current_ap: 0,
        reset_ap: 0,
        untrain_ap: 0,
    })
}

/// LearnNewSkillProcessor fail reasons, as LearnNewSkillResultMessage values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnNewRefusal {
    NotEnoughGold = 2,
    NoSuchSkill = 3,
    Class = 4,
    Level = 5,
    Constraint = 6,
    AlreadyLearnt = 7,
    System = 8,
    CannotBuySkill = 9,
}
/// LearnNewSkillResult.Succeeced.
pub const LEARN_NEW_SUCCEEDED: i32 = 1;

/// The rank-1 skill GiveSkill adds once the gold is paid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewSkill {
    /// SkillInfo.Price; 0 destroys no gold.
    pub price: i32,
    /// RecalculateRequiredAP: rank 2 RequireUnlock locks the new skill (IsLocked 1).
    pub locked: u8,
    /// SkillInfo.IsActiveSkill (UseType != 0): SetSpSkillToEmptySlot.
    pub active: bool,
}

/// SkillManager.GiveSkill(skillID, 1) for story grants: ConstraintPassed (rank 1
/// feature, class, level, constraints) or, for a preset character,
/// ForceConstraintPassed (feature and class only), then Already_Learnt. No price.
pub fn give(
    base_rank: &dyn Fn(&str) -> i32,
    contents: &BTreeMap<String, SkillContent>,
    class: i32,
    level: i32,
    use_preset: bool,
    skill_id: &str,
) -> Result<NewSkill, LearnNewRefusal> {
    let content = contents.get(skill_id).ok_or(LearnNewRefusal::NoSuchSkill)?;
    let rank = content.ranks.get(&1).ok_or(LearnNewRefusal::NoSuchSkill)?;
    let restriction = content
        .class_restriction
        .ok_or(LearnNewRefusal::NoSuchSkill)?;
    if !rank.feature_enabled {
        return Err(LearnNewRefusal::NoSuchSkill);
    }
    let bit = 1_i32.wrapping_shl(class as u32 & 31);
    if restriction & bit != bit {
        return Err(LearnNewRefusal::Class);
    }
    if !use_preset {
        if level < rank.level {
            return Err(LearnNewRefusal::Level);
        }
        if content
            .constraints
            .get(&1)
            .is_some_and(|rows| rows.iter().any(|(id, r)| base_rank(id) < *r))
        {
            return Err(LearnNewRefusal::Constraint);
        }
    }
    if base_rank(skill_id) >= 0 {
        return Err(LearnNewRefusal::AlreadyLearnt);
    }
    Ok(NewSkill {
        price: 0,
        locked: u8::from(content.ranks.get(&2).is_some_and(|r| r.require_unlock)),
        active: content.use_type.is_some_and(|u| u != 0),
    })
}

/// LearnNewSkillProcessor checks for a non-preset character, in source order
/// (ConstraintPassed of rank 1, SkillInfo, Price), then GiveSkill's Already_Learnt.
/// The source destroys the gold before GiveSkill can answer Already_Learnt; this
/// port answers it before any payment so a refused learn never loses gold.
pub fn learn_new(
    skills: &[Skill],
    contents: &BTreeMap<String, SkillContent>,
    character: &Character<'_>,
    skill_id: &str,
) -> Result<Result<NewSkill, LearnNewRefusal>, SkillListError> {
    let base_rank = |id: &str| skills.iter().find(|s| s.id == id).map_or(-1, |s| s.rank);
    let Some(content) = contents.get(skill_id) else {
        return Ok(Err(LearnNewRefusal::NoSuchSkill));
    };
    // ConstraintPassed(GetSkillRankInfo(skillID, 1)).
    let Some(rank) = content.ranks.get(&1) else {
        return Ok(Err(LearnNewRefusal::NoSuchSkill));
    };
    let (Some(restriction), Some(price)) = (content.class_restriction, content.price) else {
        return Ok(Err(LearnNewRefusal::NoSuchSkill));
    };
    if !rank.feature_enabled {
        return Ok(Err(LearnNewRefusal::NoSuchSkill));
    }
    let bit = 1_i32.wrapping_shl(character.class as u32 & 31);
    if restriction & bit != bit {
        return Ok(Err(LearnNewRefusal::Class));
    }
    if character.level < rank.level {
        return Ok(Err(LearnNewRefusal::Level));
    }
    if content
        .constraints
        .get(&1)
        .is_some_and(|rows| rows.iter().any(|(id, r)| base_rank(id) < *r))
    {
        return Ok(Err(LearnNewRefusal::Constraint));
    }
    if price < 0 {
        return Ok(Err(LearnNewRefusal::CannotBuySkill));
    }
    if skills.iter().any(|s| s.id == skill_id) {
        return Ok(Err(LearnNewRefusal::AlreadyLearnt));
    }
    Ok(Ok(NewSkill {
        price,
        locked: u8::from(content.ranks.get(&2).is_some_and(|r| r.require_unlock)),
        active: content.use_type.is_some_and(|u| u != 0),
    }))
}
