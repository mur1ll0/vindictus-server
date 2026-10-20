//! Recovered skill argument and rank-cache mutations. Not stat/AP/network callbacks.
use std::collections::BTreeMap;
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Clone)]
pub struct Argument {
    key: Option<Zeroizing<Vec<u8>>>,
    value: Option<i32>,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CacheError {
    #[error("skill cache input exceeds the admitted profile")]
    Profile,
    #[error("skill argument provider returned a null key or value")]
    Null,
    #[error("skill argument dictionary contains a duplicate key")]
    Duplicate,
}
fn bounded(key: &[u8], limit: usize) -> bool {
    key.len() <= limit && key.len().is_multiple_of(2)
}
impl Argument {
    pub fn new(key: Option<Vec<u8>>, value: Option<i32>) -> Result<Self, CacheError> {
        if key.as_ref().is_some_and(|k| !bounded(k, 1024)) {
            return Err(CacheError::Profile);
        }
        Ok(Self {
            key: key.map(Zeroizing::new),
            value,
        })
    }
}
#[derive(Default)]
pub struct Cache {
    ranks: BTreeMap<Vec<u8>, i32>,
    required_ap: BTreeMap<Vec<u8>, i32>,
    arguments: BTreeMap<Vec<u8>, BTreeMap<Vec<u8>, i32>>,
    rank_sum: i32,
}
impl Cache {
    /// Original Arg uses the supplied default only when the skill dictionary is absent.
    /// A present dictionary with a missing keyword returns zero.
    pub fn argument(&self, skill: &[u8], key: &[u8], default: i32) -> i32 {
        self.arguments
            .get(skill)
            .map_or(default, |a| a.get(key).copied().unwrap_or_default())
    }
    pub fn base_rank(&self, skill: &[u8]) -> i32 {
        self.ranks.get(skill).copied().unwrap_or(-1)
    }
    pub fn required_ap(&self, skill: &[u8]) -> Option<i32> {
        self.required_ap.get(skill).copied()
    }
    pub fn rank_sum(&self) -> i32 {
        self.rank_sum
    }
    /// None models an absent final SkillRankInfo; a present empty result creates a dictionary.
    /// Dictionary construction errors retain the previous argument dictionary.
    pub fn update_arguments(
        &mut self,
        skill: &[u8],
        rows: Option<&[Argument]>,
    ) -> Result<(), CacheError> {
        self.admit(skill)?;
        let Some(rows) = rows else {
            self.arguments.remove(skill);
            return Ok(());
        };
        if rows.len() > 4096 {
            return Err(CacheError::Profile);
        }
        let mut next = BTreeMap::new();
        for row in rows {
            let key = row.key.as_ref().ok_or(CacheError::Null)?;
            let value = row.value.ok_or(CacheError::Null)?;
            if next.insert(key.to_vec(), value).is_some() {
                return Err(CacheError::Duplicate);
            }
        }
        self.arguments.insert(skill.to_vec(), next);
        Ok(())
    }
    fn admit(&self, skill: &[u8]) -> Result<(), CacheError> {
        if !bounded(skill, 100)
            || (self.ranks.len() >= 512 && !self.ranks.contains_key(skill))
            || (self.arguments.len() >= 512 && !self.arguments.contains_key(skill))
            || (self.required_ap.len() >= 512 && !self.required_ap.contains_key(skill))
        {
            return Err(CacheError::Profile);
        }
        Ok(())
    }
    /// Normal first-Fiona GiveSkill prefix after its log. RequiredAP has already been
    /// resolved by the source-bound profile; unlock/story and SP branches are excluded there.
    pub fn after_grant(
        &mut self,
        skill: &[u8],
        rank: i32,
        required_ap: i32,
        rows: &[Argument],
    ) -> Result<(), CacheError> {
        self.admit(skill)?;
        self.required_ap.insert(
            skill.to_vec(),
            if required_ap > 0 { required_ap } else { -1 },
        );
        self.rank_changed(skill, Some(rank), 0, rank, Some(rows))
    }
    /// Original LoadBaseSkillRank updates SkillRankSum, then OnSkillRankChanged adds
    /// (to-from) again before updating Effect. Preserve both unchecked additions.
    /// Final-rank resolution/provider rows are supplied by the caller, not inferred here.
    pub fn rank_changed(
        &mut self,
        skill: &[u8],
        current: Option<i32>,
        from: i32,
        to: i32,
        rows: Option<&[Argument]>,
    ) -> Result<(), CacheError> {
        self.admit(skill)?;
        let old = self.ranks.get(skill).copied().unwrap_or_default();
        if let Some(rank) = current {
            self.ranks.insert(skill.to_vec(), rank);
        } else {
            self.ranks.remove(skill);
        }
        let new = self.ranks.get(skill).copied().unwrap_or_default();
        self.rank_sum = self.rank_sum.wrapping_add(new.wrapping_sub(old));
        self.rank_sum = self.rank_sum.wrapping_add(to.wrapping_sub(from));
        self.update_arguments(skill, rows)
    }
    /// Original LoadBaseSkillRank during CharacterEntity construction: updates baseSkillRank,
    /// adds (new - old) to SkillRankSum once, and calls Effect.UpdateSkillArgument.
    pub fn load_base(
        &mut self,
        skill: &[u8],
        rank: i32,
        rows: Option<&[Argument]>,
    ) -> Result<(), CacheError> {
        self.admit(skill)?;
        let old = self.ranks.get(skill).copied().unwrap_or_default();
        self.ranks.insert(skill.to_vec(), rank);
        self.rank_sum = self.rank_sum.wrapping_add(rank.wrapping_sub(old));
        self.update_arguments(skill, rows)
    }
}
