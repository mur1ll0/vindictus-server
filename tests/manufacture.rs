use vindictus_server::domain::manufacture::{Contents, Interval, RecipeRow, Skill, State};

fn contents() -> Contents {
    let interval = |mid: &str, grade, min, max| (mid.to_owned(), Interval { grade, min, max });
    let recipe = |mid: &str, id: &str, experience| RecipeRow {
        manufacture_id: mid.into(),
        recipe_id: id.into(),
        experience_required: experience,
        grade_required: 0,
        base_reward_exp: 1,
        always_listed: 0,
        feature: None,
    };
    Contents::build(
        [recipe("metal", "sword", 50), recipe("metal", "axe", 150)],
        [],
        [
            interval("metal", 0, 0, 99),
            interval("metal", 1, 100, 199),
            interval("metal_dc", 0, 0, 99),
            interval("forge", 0, 0, 9),
        ],
        [
            ("metal_dc".to_owned(), "metal".to_owned(), 150),
            ("forge".to_owned(), "metal_dc".to_owned(), 0),
        ],
        |_| true,
    )
    .unwrap()
}

#[test]
fn skills_are_acquired_levelled_and_removed_like_the_manager() {
    let c = contents();
    let mut s = State::default();
    assert!(!s.acquire(&c, "unknown"));
    // metal_dc needs metal with 150 experience.
    assert!(!s.acquire(&c, "metal_dc"));
    assert!(s.acquire(&c, "metal"));
    assert!(!s.acquire(&c, "metal"));
    assert_eq!(s.acquire_recipe(&c, "sword"), Some(true));
    assert_eq!(s.acquire_recipe(&c, "sword"), Some(false));
    // Grade 0 tops at 99 < 150.
    assert_eq!(s.acquire_recipe(&c, "axe"), None);
    assert_eq!(s.acquire_recipe(&c, "missing"), None);
    // Level up only at exactly the maximum; the next grade starts at its minimum.
    assert_eq!(s.level_up(&c, "metal"), None);
    s.skills[0].experience = 99;
    assert_eq!(s.level_up(&c, "metal"), Some(1));
    assert_eq!(
        s.skill("metal"),
        Some(&Skill {
            id: "metal".into(),
            grade: 1,
            experience: 100
        })
    );
    assert_eq!(s.acquire_recipe(&c, "axe"), Some(true));
    s.skills[0].experience = 150;
    assert!(s.acquire(&c, "metal_dc"));
    assert!(s.acquire(&c, "forge"));
    // Removing metal removes what depends on it, transitively.
    assert_eq!(s.removal(&c, "metal"), ["metal", "metal_dc", "forge"]);
    assert_eq!(s.removal(&c, "forge"), ["forge"]);
    assert_eq!(s.removal(&c, ""), ["metal", "metal_dc", "forge"]);
}

struct Script(Vec<i32>);
impl vindictus_server::domain::enchant::Random for Script {
    fn next_double(&mut self) -> f64 {
        0.0
    }
    fn next(&mut self, maximum: i32) -> Option<i32> {
        let v = self.0.remove(0);
        assert!(v < maximum.max(1), "roll {v} >= {maximum}");
        Some(v)
    }
}

#[test]
fn craft_plans_follow_process_product_checks() {
    use vindictus_server::domain::manufacture::{
        CraftRefusal, CraftUnsupported, Interval, RecipeRow, plan_craft,
    };
    let row = |id: &str, experience, grade, always, feature: Option<&str>| RecipeRow {
        manufacture_id: "metal".into(),
        recipe_id: id.into(),
        experience_required: experience,
        grade_required: grade,
        base_reward_exp: 1,
        always_listed: always,
        feature: feature.map(str::to_owned),
    };
    let c = Contents::build(
        [
            row("sword", 30, 0, 0, None),
            row("listed", 0, 0, 1, None),
            row("deep", 50, 0, 1, None),
            row("graded", 0, 1, 1, None),
            row("off", 0, 0, 1, Some("off")),
            row("empty", 0, 0, 1, None),
        ],
        [
            ("sword".to_owned(), "blade".to_owned(), 1, 0),
            ("sword".to_owned(), "iron".to_owned(), 2, 1),
            ("sword".to_owned(), "iron".to_owned(), 3, 2),
            ("listed".to_owned(), "blade".to_owned(), 1, 0),
            ("deep".to_owned(), "blade".to_owned(), 1, 0),
            ("graded".to_owned(), "blade".to_owned(), 1, 0),
            ("off".to_owned(), "blade".to_owned(), 1, 0),
        ],
        [(
            "metal".to_owned(),
            Interval {
                grade: 0,
                min: 0,
                max: 99,
            },
        )],
        [],
        |_| true,
    )
    .unwrap();
    let mut s = State::default();
    let enabled = |f: &str| f != "off";
    // An unknown recipe is refused before the skill lookup; a known one without the
    // skill throws (EXP[mid]).
    assert_eq!(
        plan_craft(&c, &s, "missing", 1, enabled),
        Ok(Err(CraftRefusal::NoRecipe))
    );
    assert_eq!(
        plan_craft(&c, &s, "listed", 1, enabled),
        Err(CraftUnsupported::UnknownSkill)
    );
    s.skills.push(Skill {
        id: "metal".into(),
        grade: 0,
        experience: 10,
    });
    // Not acquired and not AlwaysListed; experience + 20 below the requirement; grade;
    // disabled feature; no target rows.
    assert_eq!(
        plan_craft(&c, &s, "sword", 1, enabled),
        Ok(Err(CraftRefusal::NoRecipe))
    );
    s.recipes.push("sword".into());
    for id in ["deep", "graded", "off", "empty"] {
        assert_eq!(
            plan_craft(&c, &s, id, 1, enabled),
            Ok(Err(CraftRefusal::NoRecipe)),
            "{id}"
        );
    }
    let plan = plan_craft(&c, &s, "sword", 2, enabled).unwrap().unwrap();
    assert_eq!(plan.materials, [("iron".to_owned(), 10)]);
    assert_eq!(plan.targets, [("blade".to_owned(), 2)]);
    assert!(plan_craft(&c, &s, "listed", 1, enabled).unwrap().is_ok());
}

#[test]
fn quality_dice_and_experience_follow_the_manager_rolls() {
    use vindictus_server::domain::manufacture::{craft_experience, roll_quality};
    let dice = [(1, 50), (2, 650), (3, 295), (4, 5)];
    // No dice rows: quality 2 without a roll.
    assert_eq!(roll_quality(&[], 0, 0, &mut Script(vec![])), Ok(2));
    assert_eq!(roll_quality(&dice, 0, 0, &mut Script(vec![49])), Ok(1));
    assert_eq!(roll_quality(&dice, 0, 0, &mut Script(vec![50])), Ok(2));
    assert_eq!(roll_quality(&dice, 0, 0, &mut Script(vec![999])), Ok(4));
    // Experience 3099 over a requirement of 49: ((3100 / 50) - 1) / 30 = 2 extra chances
    // of quality 4, checked against the remainder after the table.
    assert_eq!(
        roll_quality(&dice, 3099, 49, &mut Script(vec![1000])),
        Ok(4)
    );
    assert_eq!(
        roll_quality(&dice, 3099, 49, &mut Script(vec![1001])),
        Ok(4)
    );
    // The bonus is checked against what is left of the roll, also when it stopped early.
    assert_eq!(roll_quality(&dice, 3099, 49, &mut Script(vec![1])), Ok(4));
    assert_eq!(roll_quality(&dice, 3099, 49, &mut Script(vec![2])), Ok(1));
    // Far below the requirement: Next(1000) >= 25 then Next(100) >= 70 adds Next(2) + 1.
    assert_eq!(
        craft_experience(99, 10, 50, 2, 1, &mut Script(vec![25, 70, 1])),
        Ok((Some(4), 14))
    );
    assert_eq!(
        craft_experience(99, 10, 50, 2, 1, &mut Script(vec![24])),
        Ok((Some(2), 12))
    );
    // Near the requirement: Next(1000) >= 50 and Next(100) >= 80 add one.
    assert_eq!(
        craft_experience(99, 50, 45, 2, 1, &mut Script(vec![50, 80])),
        Ok((Some(3), 53))
    );
    assert_eq!(
        craft_experience(99, 50, 45, 2, 1, &mut Script(vec![49])),
        Ok((Some(2), 52))
    );
    // Clamped to the grade maximum; far above the requirement or at the cap: nothing.
    assert_eq!(
        craft_experience(99, 85, 80, 20, 1, &mut Script(vec![0])),
        Ok((Some(14), 99))
    );
    assert_eq!(
        craft_experience(99, 60, 10, 2, 1, &mut Script(vec![])),
        Ok((None, 60))
    );
    assert_eq!(
        craft_experience(99, 95, 10, 2, 1, &mut Script(vec![])),
        Ok((None, 95))
    );
}
