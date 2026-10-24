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
