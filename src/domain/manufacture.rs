//! ManufactureDataContainer (recipes, grade intervals, dependencies) and the
//! ManufactureManager rules that change a character's manufacture state: acquiring a
//! manufacture skill, levelling it up, acquiring and removing recipes.
use std::collections::BTreeMap;

/// ManufactureCraftRecipe: the recipe row plus its Target and Material/KeyMaterial rows
/// (AddItem sums repeated classes; the first-seen order is the dictionary order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub id: String,
    pub manufacture_id: String,
    pub experience_required: i32,
    pub grade_required: i32,
    pub base_reward_exp: i32,
    pub always_listed: u8,
    pub feature: Option<String>,
    pub materials: Vec<(String, i32)>,
    pub targets: Vec<(String, i32)>,
}

/// One ManufactureInfo row: the experience interval of a grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    pub grade: i32,
    pub min: i32,
    pub max: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentError {
    /// Dictionary.Add of a repeated (ManufactureID, Grade) throws in the static ctor.
    DuplicateGrade,
}

/// The static dictionaries of ManufactureDataContainer.
#[derive(Clone, Debug, Default)]
pub struct Contents {
    recipes: BTreeMap<String, Recipe>,
    levels: BTreeMap<String, Vec<Interval>>,
    required: BTreeMap<String, Vec<(String, i32)>>,
    by_mid: BTreeMap<String, Vec<String>>,
}

/// ManufactureRecipeInfo columns the container keeps.
pub struct RecipeRow {
    pub manufacture_id: String,
    pub recipe_id: String,
    pub experience_required: i32,
    pub grade_required: i32,
    pub base_reward_exp: i32,
    pub always_listed: u8,
    pub feature: Option<String>,
}

/// ManufactureCraftRecipe.ItemType.
const TARGET: u8 = 0;
const MATERIAL: u8 = 1;
const KEY_MATERIAL: u8 = 2;

fn add(rows: &mut Vec<(String, i32)>, class: &str, num: i32) {
    match rows.iter_mut().find(|(c, _)| c == class) {
        Some((_, n)) => *n = n.wrapping_add(num),
        None => rows.push((class.to_owned(), num)),
    }
}

impl Contents {
    /// Builds the container in provider order. `known` is ItemContents.GetItemClassInfo:
    /// material rows of unknown classes are skipped, as are rows of an unknown recipe or
    /// of another Type (logged by the source).
    pub fn build(
        recipes: impl IntoIterator<Item = RecipeRow>,
        materials: impl IntoIterator<Item = (String, String, i32, u8)>,
        infos: impl IntoIterator<Item = (String, Interval)>,
        dependencies: impl IntoIterator<Item = (String, String, i32)>,
        known: impl Fn(&str) -> bool,
    ) -> Result<Self, ContentError> {
        let mut out = Self::default();
        for r in recipes {
            // A duplicated RecipeID keeps the first recipe but is listed again by MID.
            out.recipes.entry(r.recipe_id.clone()).or_insert(Recipe {
                id: r.recipe_id.clone(),
                manufacture_id: r.manufacture_id.clone(),
                experience_required: r.experience_required,
                grade_required: r.grade_required,
                base_reward_exp: r.base_reward_exp,
                always_listed: r.always_listed,
                feature: r.feature,
                materials: Vec::new(),
                targets: Vec::new(),
            });
            out.by_mid
                .entry(r.manufacture_id)
                .or_default()
                .push(r.recipe_id);
        }
        for (recipe, class, num, kind) in materials {
            let Some(r) = out.recipes.get_mut(&recipe) else {
                continue;
            };
            if !known(&class) {
                continue;
            }
            match kind {
                TARGET => add(&mut r.targets, &class, num),
                MATERIAL | KEY_MATERIAL => add(&mut r.materials, &class, num),
                _ => {}
            }
        }
        for (mid, interval) in infos {
            let grades = out.levels.entry(mid).or_default();
            if grades.iter().any(|g| g.grade == interval.grade) {
                return Err(ContentError::DuplicateGrade);
            }
            grades.push(interval);
        }
        for (mid, required, experience) in dependencies {
            out.required
                .entry(mid)
                .or_default()
                .push((required, experience));
        }
        Ok(out)
    }
    pub fn recipe(&self, id: &str) -> Option<&Recipe> {
        self.recipes.get(id)
    }
    /// GetManufactureMaxExpByLevel (-1 for an unknown MID or grade).
    pub fn max_exp(&self, mid: &str, grade: i32) -> i32 {
        self.interval(mid, grade).map_or(-1, |i| i.max)
    }
    /// GetManufactureMinExpByLevel.
    pub fn min_exp(&self, mid: &str, grade: i32) -> i32 {
        self.interval(mid, grade).map_or(-1, |i| i.min)
    }
    fn interval(&self, mid: &str, grade: i32) -> Option<&Interval> {
        self.levels.get(mid)?.iter().find(|i| i.grade == grade)
    }
    /// GetMaxManufactureEXP: the largest ExperienceMax of the MID (-1 when unknown).
    pub fn max_manufacture_exp(&self, mid: &str) -> i32 {
        self.levels
            .get(mid)
            .map_or(-1, |g| g.iter().fold(-1, |n, i| n.max(i.max)))
    }
    pub fn required(&self, mid: &str) -> Option<&[(String, i32)]> {
        self.required.get(mid).map(Vec::as_slice)
    }
    pub fn recipes_by_mid(&self, mid: &str) -> Option<&[String]> {
        self.by_mid.get(mid).map(Vec::as_slice)
    }
}

/// Manufacture row: ManufacturelID, Grade, ExperiencePoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub id: String,
    pub grade: i32,
    pub experience: i32,
}

/// ManufactureManager.EXP (load order) and ManufactureRecipes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub skills: Vec<Skill>,
    pub recipes: Vec<String>,
}

impl State {
    pub fn skill(&self, mid: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.id == mid)
    }
    fn skill_mut(&mut self, mid: &str) -> Option<&mut Skill> {
        self.skills.iter_mut().find(|s| s.id == mid)
    }
    /// AcquireManufactureSkill: an unknown, unowned MID whose dependency (any one) is met
    /// is added at grade 0 with 0 experience.
    pub fn acquire(&mut self, contents: &Contents, mid: &str) -> bool {
        if self.skill(mid).is_some() || contents.max_manufacture_exp(mid) == -1 {
            return false;
        }
        if let Some(required) = contents.required(mid)
            && !required
                .iter()
                .any(|(id, experience)| self.skill(id).is_some_and(|s| s.experience >= *experience))
        {
            return false;
        }
        self.skills.push(Skill {
            id: mid.to_owned(),
            grade: 0,
            experience: 0,
        });
        true
    }
    /// LevelUpManufacture: only at exactly the grade's maximum experience; the new grade
    /// starts at its minimum. Returns the new grade.
    pub fn level_up(&mut self, contents: &Contents, mid: &str) -> Option<i32> {
        let skill = self.skill_mut(mid)?;
        if skill.experience != contents.max_exp(mid, skill.grade) {
            return None;
        }
        skill.grade = skill.grade.wrapping_add(1);
        skill.experience = contents.min_exp(mid, skill.grade);
        Some(skill.grade)
    }
    /// AcquireManufactureRecipe: Some(true) when added, Some(false) when already known,
    /// None when refused.
    pub fn acquire_recipe(&mut self, contents: &Contents, rid: &str) -> Option<bool> {
        let recipe = contents.recipe(rid)?;
        if self.recipes.iter().any(|r| r == rid) {
            return Some(false);
        }
        let skill = self.skill(&recipe.manufacture_id)?;
        if contents.max_exp(&recipe.manufacture_id, skill.grade) < recipe.experience_required {
            return None;
        }
        self.recipes.push(rid.to_owned());
        Some(true)
    }
    /// RemoveManufactureSkill's list: the MID (every skill for "") and, recursively, the
    /// owned skills that depend on it (FillRemoveManufactureSkillList).
    pub fn removal(&self, contents: &Contents, mid: &str) -> Vec<String> {
        let mut out = Vec::new();
        if mid.is_empty() {
            for s in &self.skills {
                if !out.contains(&s.id) {
                    out.push(s.id.clone());
                    self.fill_removal(contents, &s.id, &mut out);
                }
            }
        } else {
            out.push(mid.to_owned());
            self.fill_removal(contents, mid, &mut out);
        }
        out
    }
    fn fill_removal(&self, contents: &Contents, removed: &str, out: &mut Vec<String>) {
        for s in &self.skills {
            if out.contains(&s.id) || s.id == removed {
                continue;
            }
            let Some(required) = contents.required(&s.id) else {
                continue;
            };
            if required.iter().any(|(id, _)| id == removed) {
                out.push(s.id.clone());
                self.fill_removal(contents, &s.id, out);
            }
        }
    }
}

/// ProcessProduct refusals that answer CraftItemFail_NoRecipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftRefusal {
    NoRecipe,
}

/// ProcessProduct steps the port cannot reproduce (the source throws or the branch is
/// not migrated); nothing is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftUnsupported {
    /// EXP[recipe.ManufactureID] throws KeyNotFoundException for an unlearnt skill.
    UnknownSkill,
    /// Combine Craft (ItemCombinationSession) belongs to B3 item 5.
    CombineCraft,
    /// Integer division by zero or a negative Random.Next bound.
    Arithmetic,
}

/// The checked recipe with its materials and targets times Count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftPlan {
    pub recipe: Recipe,
    pub materials: Vec<(String, i32)>,
    pub targets: Vec<(String, i32)>,
}

/// ProcessProduct up to the item requests. `enabled` is FeatureMatrix.IsEnable.
pub fn plan_craft(
    contents: &Contents,
    state: &State,
    recipe_id: &str,
    count: i32,
    enabled: impl Fn(&str) -> bool,
) -> Result<Result<CraftPlan, CraftRefusal>, CraftUnsupported> {
    if count <= 0 {
        return Ok(Err(CraftRefusal::NoRecipe));
    }
    let Some(recipe) = contents.recipe(recipe_id) else {
        return Ok(Err(CraftRefusal::NoRecipe));
    };
    if !state.recipes.iter().any(|r| r == recipe_id) && recipe.always_listed == 0 {
        return Ok(Err(CraftRefusal::NoRecipe));
    }
    let skill = state
        .skill(&recipe.manufacture_id)
        .ok_or(CraftUnsupported::UnknownSkill)?;
    if skill.experience.wrapping_add(20) < recipe.experience_required
        || skill.grade < recipe.grade_required
        || recipe.feature.as_deref().is_some_and(|f| !enabled(f))
    {
        return Ok(Err(CraftRefusal::NoRecipe));
    }
    let times = |rows: &[(String, i32)]| -> Vec<(String, i32)> {
        rows.iter()
            .map(|(c, n)| (c.clone(), n.wrapping_mul(count)))
            .collect()
    };
    if recipe.targets.is_empty() {
        return Ok(Err(CraftRefusal::NoRecipe));
    }
    Ok(Ok(CraftPlan {
        materials: times(&recipe.materials),
        targets: times(&recipe.targets),
        recipe: recipe.clone(),
    }))
}

/// The quality roll of an equippable target with a QualityType: 2 without dice rows;
/// a dice table holding quality 4 adds ((exp + 1) / (required + 1) - 1) / 30 chances of 4.
pub fn roll_quality(
    dice: &[(i32, i32)],
    experience: i32,
    required: i32,
    random: &mut dyn crate::domain::enchant::Random,
) -> Result<i32, CraftUnsupported> {
    let mut quality = 2;
    if dice.is_empty() {
        return Ok(quality);
    }
    let mut total = dice.iter().fold(0i32, |n, (_, d)| n.wrapping_add(*d));
    let mut bonus = 0;
    if dice.iter().any(|(q, _)| *q == 4) {
        let ratio = experience
            .wrapping_add(1)
            .checked_div(required.wrapping_add(1))
            .ok_or(CraftUnsupported::Arithmetic)?;
        bonus = ratio.wrapping_sub(1) / 30;
        total = total.wrapping_add(bonus);
    }
    let mut roll = random.next(total).ok_or(CraftUnsupported::Arithmetic)?;
    for (q, d) in dice {
        if roll < *d {
            quality = *q;
            break;
        }
        roll = roll.wrapping_sub(*d);
    }
    if bonus > 0 && roll < bonus {
        quality = 4;
    }
    Ok(quality)
}

/// The experience step after a successful craft: the gained amount shown by
/// GameUI_Heroes_Manufacture_GetExperience (None: no message) and the new experience.
pub fn craft_experience(
    max: i32,
    experience: i32,
    required: i32,
    base_reward: i32,
    event_exp: i32,
    random: &mut dyn crate::domain::enchant::Random,
) -> Result<(Option<i32>, i32), CraftUnsupported> {
    let mut next = |n| random.next(n).ok_or(CraftUnsupported::Arithmetic);
    let mut experience = experience;
    let mut gained = None;
    let near_max = max.wrapping_sub(10);
    let bonus = if experience >= near_max && required < near_max {
        // BreakEXP: no experience at the grade's cap.
        None
    } else if required.wrapping_sub(experience) > 10 {
        Some(if next(1000)? >= 25 {
            if next(100)? >= 70 { next(2)? + 1 } else { 0 }
        } else {
            0
        })
    } else if experience.wrapping_sub(required) <= 10 {
        Some(i32::from(next(1000)? >= 50 && next(100)? >= 80))
    } else {
        None
    };
    if let Some(bonus) = bonus {
        let mut value = base_reward.wrapping_mul(event_exp).wrapping_add(bonus);
        if experience < max {
            value = value.min(max.wrapping_sub(experience));
            experience = experience.wrapping_add(value);
            gained = Some(value);
        }
    }
    if experience > max {
        experience = max;
    }
    Ok((gained, experience))
}
