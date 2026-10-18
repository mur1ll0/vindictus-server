//! Recovered constructor cache only;no training,transformation,content lookups or Save.
use crate::domain::legacy_user_data::LegacyBytes;
use thiserror::Error;
pub struct Details {
    pub class: i32,
    pub level: i32,
    pub exp: i32,
    pub last_transform_ticks: Option<i64>,
}
pub struct Skill {
    pub row_id: i64,
    pub id: LegacyBytes<50, true>,
    pub rank: i32,
}
#[derive(Debug, Error)]
#[error("vocation constructor input exceeds its admitted profile")]
pub struct Profile;
pub struct Loaded {
    details: Option<Details>,
    skills: Vec<Skill>,
    changed: Vec<(LegacyBytes<50, true>, i32)>,
    used: i32,
    duplicates: usize,
    changed_vocation: bool,
    transform_level: i32,
}
impl Loaded {
    pub fn load(details: Option<Details>, input: Vec<Skill>) -> Result<Self, Profile> {
        if input.len() > 1024 || (details.is_none() && !input.is_empty()) {
            return Err(Profile);
        }
        let (mut skills, mut duplicates, mut used) = (Vec::<Skill>::new(), 0, 0i32);
        for row in input {
            if skills.iter().any(|s| s.id == row.id) {
                duplicates += 1;
                continue;
            }
            used = used.wrapping_add(row.rank);
            skills.push(row);
        }
        Ok(Self {
            details,
            skills,
            changed: vec![],
            used,
            duplicates,
            changed_vocation: false,
            transform_level: 0,
        })
    }
    pub fn details(&self) -> Option<&Details> {
        self.details.as_ref()
    }
    pub fn skills(&self) -> impl Iterator<Item = &Skill> {
        self.skills.iter()
    }
    pub fn changed_skills(&self) -> &[(LegacyBytes<50, true>, i32)] {
        &self.changed
    }
    pub fn used_points(&self) -> i32 {
        self.used
    }
    pub fn duplicates(&self) -> usize {
        self.duplicates
    }
    pub fn vocation_changed(&self) -> bool {
        self.changed_vocation
    }
    pub fn transform_level(&self) -> i32 {
        self.transform_level
    }
}
