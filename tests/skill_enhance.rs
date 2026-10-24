use vindictus_server::domain::skill_enhance::{
    Enhance, Info, Material, Outcome, StartFacts, TYPE_PENALTY, TYPE_SUCCESS, can_use_material,
    parse_groups, roll, start, use_material,
};

fn info(max: i32) -> Info {
    Info {
        group: 2,
        key: 3,
        item_class: Some("stone_lv3".into()),
        max_success_ratio: max,
        max_durability: 100,
        repair_value: 30,
    }
}
fn facts<'a>(info: &'a Info, enhance: Option<Enhance>) -> StartFacts<'a> {
    StartFacts {
        feature_on: true,
        skill: Some((41, enhance)),
        stone: Some((7, "stone_lv3")),
        info: Some(info),
        groups: Some(vec![0, 2]),
    }
}
fn penalty(key: i32) -> Enhance {
    Enhance {
        skill_row_id: 41,
        group: 2,
        key,
        kind: TYPE_PENALTY,
        reduce_durability: 0,
        max_durability_bonus: 0,
    }
}

#[test]
fn sessions_start_only_for_an_available_group_and_key() {
    let i = info(60);
    let session = start("skill_a", facts(&i, None)).unwrap();
    assert_eq!(
        (
            session.skill_row_id,
            session.stone_item_id,
            session.max_success_ratio()
        ),
        (41, 7, 60)
    );
    let refused = |f: StartFacts<'_>| start("skill_a", f).unwrap_err();
    assert_eq!(
        refused(StartFacts {
            feature_on: false,
            ..facts(&i, None)
        }),
        Outcome::MakeSessionFailFeatureOff
    );
    assert_eq!(
        refused(StartFacts {
            skill: None,
            ..facts(&i, None)
        }),
        Outcome::MakeSessionFailCannotFindSkillData
    );
    assert_eq!(
        refused(StartFacts {
            stone: None,
            ..facts(&i, None)
        }),
        Outcome::MakeSessionFailCannotFindEnhanceStone
    );
    assert_eq!(
        refused(StartFacts {
            info: None,
            ..facts(&i, None)
        }),
        Outcome::MakeSessionFailCannotFindSkillEnhanceInfo
    );
    assert_eq!(
        refused(StartFacts {
            groups: Some(vec![1]),
            ..facts(&i, None)
        }),
        Outcome::MakeSessionFailNotAvailableEnhanceGroup
    );
    // A penalty with a higher key blocks a lower stone; an equal key does not.
    assert_eq!(
        refused(facts(&i, Some(penalty(4)))),
        Outcome::MakeSessionFailNewEnhanceKeyLowerThanEnhanceDataKey
    );
    assert!(start("skill_a", facts(&i, Some(penalty(3)))).is_ok());
    assert_eq!(parse_groups(Some(" 2;x;;4 ")), [2, 4]);
    assert!(parse_groups(None).is_empty());
}

#[test]
fn materials_raise_the_ratio_within_the_maximum() {
    let i = info(60);
    let mut session = start("skill_a", facts(&i, None)).unwrap();
    let ratio = Material {
        item_class: "erg".into(),
        kind: 0,
        min: 20,
        max: 30,
    };
    let cap = Material {
        item_class: "cap".into(),
        kind: 1,
        min: 50,
        max: 50,
    };
    let mut rolls = vec![25, 30, 30];
    let mut next = |min: i32, max: i32| {
        let v = rolls.remove(0);
        assert!((min..max).contains(&v));
        Some(v)
    };
    // Two items: 25 + 30 = 55, then 55 + 30 caps at 60.
    use_material(&mut session, &ratio, 2, &mut next).unwrap();
    assert_eq!(session.success_ratio, 55);
    use_material(&mut session, &ratio, 1, &mut next).unwrap();
    assert_eq!(session.success_ratio, 60);
    assert_eq!(
        can_use_material(&session, &ratio, 1),
        Err(Outcome::UseErgBiggerThanMaxRatio)
    );
    // A fixed MaxSuccessRatio material needs one item and applies once; the total stays at 100.
    assert_eq!(
        can_use_material(&session, &cap, 2),
        Err(Outcome::UseErgCannotDestroyMultipleItems)
    );
    use_material(&mut session, &cap, 1, &mut |_, _| None).unwrap();
    assert_eq!(
        (session.additional_max(), session.max_success_ratio()),
        (40, 100)
    );
    assert_eq!(
        can_use_material(&session, &cap, 1),
        Err(Outcome::UseErgAlreadyHasAdditionalMaxRatio)
    );
    let other = Material {
        kind: 9,
        ..ratio.clone()
    };
    assert_eq!(
        can_use_material(&session, &other, 1),
        Err(Outcome::UseErgNotImplementation)
    );
}

#[test]
fn rolls_follow_on_dice_success_and_fail() {
    let i = info(60);
    let mut session = start("skill_a", facts(&i, None)).unwrap();
    session.success_ratio = 40;
    // Success: the premium rune and the stone are consumed, a normal rune is kept.
    let s = roll(&session, 39, Some("skill_enhance_rune_cash_lv2"), false);
    assert!(s.success && s.consume_rune && s.consume_stone);
    assert_eq!(
        s.enhance.map(|e| (e.kind, e.group, e.key)),
        Some((TYPE_SUCCESS, 2, 3))
    );
    let s = roll(&session, 0, Some("Skill_Enhance_Rune_Gift_Lv1"), false);
    assert!(s.success && !s.consume_rune);
    // Fail without a rune: the stone is lost and the enhance becomes a penalty.
    let f = roll(&session, 40, None, false);
    assert!(!f.success && f.consume_stone && !f.consume_rune && !f.give_protected_stone);
    assert_eq!(f.enhance.map(|e| e.kind), Some(TYPE_PENALTY));
    // Fail with a normal rune on an unprotected stone: rune lost, a protected copy given,
    // the stone consumed and the enhance removed.
    let f = roll(&session, 99, Some("skill_enhance_rune_gift_lv1"), false);
    assert!(f.consume_rune && f.give_protected_stone && f.consume_stone);
    assert!(f.enhance.is_none() && f.additional_destroyed && f.stone_protected);
    // A premium rune on an already protected stone keeps the stone.
    let f = roll(&session, 99, Some("skill_enhance_rune_cash_lv2"), true);
    assert!(f.consume_rune && !f.give_protected_stone && !f.consume_stone && f.stone_protected);
    // A normal rune on a protected stone loses both and protects nothing more.
    let f = roll(&session, 99, Some("skill_enhance_rune_cash_lv1"), true);
    assert!(f.consume_stone && !f.give_protected_stone && !f.stone_protected);
}
