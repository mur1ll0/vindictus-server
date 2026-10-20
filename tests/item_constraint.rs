use std::collections::BTreeMap;
use vindictus_server::domain::item_constraint::{Character, Constraint, Equipped, Failure, check};

fn constraint() -> Constraint {
    Constraint {
        cafe_only: 0,
        required_level: 1,
        level_upper_bound: 0,
        class_restriction: 2,
        required_guild_level: 0,
        required_skill: None,
        required_enhance: 0,
        required_enchant: 0,
    }
}
fn fiona(skills: &BTreeMap<String, i32>) -> Character<'_> {
    Character {
        level: 1,
        class: 1,
        cafe_type: 0,
        cafe: false,
        guild_level: None,
        skills,
        equipped: Some(&[]),
    }
}

#[test]
fn initial_fiona_equipment_profile_is_satisfied() {
    let skills = BTreeMap::new();
    assert_eq!(check(&fiona(&skills), &constraint()), None);
    // 2047 admits every class bit in the recovered contents.
    let all = Constraint {
        class_restriction: 2047,
        ..constraint()
    };
    assert_eq!(check(&fiona(&skills), &all), None);
}

#[test]
fn refusals_follow_source_order() {
    let skills = BTreeMap::new();
    let c = fiona(&skills);
    let cases = [
        (
            Constraint {
                cafe_only: 1,
                required_level: 99,
                ..constraint()
            },
            Failure::NeedCafeAuth,
        ),
        (
            Constraint {
                cafe_only: 2,
                ..constraint()
            },
            Failure::NeedCafeAuth,
        ),
        (
            Constraint {
                cafe_only: 3,
                ..constraint()
            },
            Failure::NeedVvipAuth,
        ),
        (
            Constraint {
                level_upper_bound: -1,
                required_level: 99,
                ..constraint()
            },
            Failure::HighLevel,
        ),
        (
            Constraint {
                required_level: 2,
                class_restriction: 1,
                ..constraint()
            },
            Failure::LowLevel,
        ),
        (
            Constraint {
                class_restriction: 1,
                ..constraint()
            },
            Failure::InvalidClass,
        ),
        (
            Constraint {
                required_skill: Some(("absent".to_owned(), 0)),
                ..constraint()
            },
            Failure::LowSkillRank,
        ),
    ];
    for (k, expected) in cases {
        assert_eq!(check(&c, &k), Some(expected));
    }
    // Missing skill rank is -1, so a negative requirement is satisfied.
    let k = Constraint {
        required_skill: Some(("absent".to_owned(), -1)),
        ..constraint()
    };
    assert_eq!(check(&c, &k), None);
}

#[test]
fn guild_and_enhance_requirements_depend_on_loaded_state() {
    let skills = BTreeMap::new();
    let guild = Constraint {
        required_guild_level: 10,
        ..constraint()
    };
    assert_eq!(check(&fiona(&skills), &guild), None);
    let loaded = Character {
        guild_level: Some(9),
        ..fiona(&skills)
    };
    assert_eq!(check(&loaded, &guild), Some(Failure::LowLevel));
    let enhance = Constraint {
        required_enhance: 3,
        required_enchant: 2,
        ..constraint()
    };
    // No equipped items: the enhance failure is reported first.
    assert_eq!(
        check(&fiona(&skills), &enhance),
        Some(Failure::EnhanceLevel)
    );
    let items = [
        Equipped {
            enhance: 3,
            prefix: 0,
            suffix: 0,
        },
        Equipped {
            enhance: 0,
            prefix: 0,
            suffix: 1,
        },
    ];
    let partial = Character {
        equipped: Some(&items),
        ..fiona(&skills)
    };
    assert_eq!(check(&partial, &enhance), Some(Failure::EnchantLevel));
    let both = [
        items[0],
        Equipped {
            enhance: 0,
            prefix: 2,
            suffix: 0,
        },
    ];
    let satisfied = Character {
        equipped: Some(&both),
        ..fiona(&skills)
    };
    assert_eq!(check(&satisfied, &enhance), None);
    let unloaded = Character {
        equipped: None,
        ..fiona(&skills)
    };
    assert_eq!(check(&unloaded, &enhance), None);
    // Only one positive requirement disables the equipped-item check.
    let single = Constraint {
        required_enchant: 0,
        ..enhance
    };
    assert_eq!(check(&fiona(&skills), &single), None);
    assert_eq!(Failure::NeedVvipAuth.name(), "ConstraintFail_NeedVVIPAuth");
}
