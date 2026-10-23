use std::collections::BTreeMap;
use vindictus_server::domain::skill_list::{
    Brief, Character, RankInfo, Skill, SkillContent, to_skill_list,
};

fn rank(level: i32, required_ap: i32) -> RankInfo {
    RankInfo {
        level,
        required_ap,
        require_unlock: false,
        feature_enabled: true,
    }
}
fn content() -> SkillContent {
    let mut c = SkillContent {
        class_restriction: Some(2),
        ..Default::default()
    };
    for (r, ap) in [(0, 0), (1, 0), (2, 30), (3, 130), (4, 170)] {
        c.ranks.insert(r, rank(1, ap));
    }
    c
}
fn skill(id: &str, rank: i32, locked: u8, current_ap: i32) -> Skill {
    Skill {
        id: id.to_owned(),
        rank,
        locked,
        current_ap,
    }
}
fn list(skills: &[Skill], contents: &BTreeMap<String, SkillContent>, level: i32) -> Vec<Brief> {
    let items = BTreeMap::new();
    to_skill_list(
        skills,
        contents,
        &Character {
            class: 1,
            level,
            item_ranks: &items,
        },
    )
    .unwrap()
}

#[test]
fn initial_rank_one_skill_can_train_next_rank() {
    let contents = BTreeMap::from([("skill_a".to_owned(), content())]);
    let b = &list(&[skill("skill_a", 1, 0, 0)], &contents, 1)[0];
    assert_eq!(
        (
            b.base_rank,
            b.final_rank,
            b.required_ap,
            b.can_start_training
        ),
        (1, 1, 30, 1)
    );
    // Rank 1 cannot be untrained; reset returns only the current AP.
    assert_eq!((b.untrain_ap, b.reset_ap, b.current_ap), (-1, 0, 0));
}

#[test]
fn higher_ranks_sum_reset_and_untrain_ap() {
    let contents = BTreeMap::from([("skill_a".to_owned(), content())]);
    let b = &list(&[skill("skill_a", 3, 0, 7)], &contents, 1)[0];
    assert_eq!(b.required_ap, 170);
    assert_eq!(b.untrain_ap, 130 + 7);
    assert_eq!(b.reset_ap, 7 + 130 + 30);
    // No next rank: RequiredAP -1 and training unavailable.
    let top = &list(&[skill("skill_a", 4, 0, 0)], &contents, 1)[0];
    assert_eq!((top.required_ap, top.can_start_training), (-1, 0));
}

#[test]
fn constraint_failures_disable_training_in_source_order() {
    let mut contents = BTreeMap::from([("skill_a".to_owned(), content())]);
    // Locked skills (1 or 2) never start training.
    assert_eq!(
        list(&[skill("skill_a", 1, 1, 0)], &contents, 1)[0].can_start_training,
        0
    );
    assert_eq!(
        list(&[skill("skill_a", 1, 2, 0)], &contents, 1)[0].is_locked,
        2
    );
    // Level constraint of the next rank.
    contents
        .get_mut("skill_a")
        .unwrap()
        .ranks
        .insert(2, rank(5, 30));
    assert_eq!(
        list(&[skill("skill_a", 1, 0, 0)], &contents, 4)[0].can_start_training,
        0
    );
    assert_eq!(
        list(&[skill("skill_a", 1, 0, 0)], &contents, 5)[0].can_start_training,
        1
    );
    // Disabled feature.
    contents.get_mut("skill_a").unwrap().ranks.insert(
        2,
        RankInfo {
            feature_enabled: false,
            ..rank(1, 30)
        },
    );
    assert_eq!(
        list(&[skill("skill_a", 1, 0, 0)], &contents, 1)[0].can_start_training,
        0
    );
    contents
        .get_mut("skill_a")
        .unwrap()
        .ranks
        .insert(2, rank(1, 30));
    // Prerequisite skill rank, then satisfied by another loaded skill.
    contents
        .get_mut("skill_a")
        .unwrap()
        .constraints
        .insert(2, vec![("skill_b".to_owned(), 2)]);
    contents.insert("skill_b".to_owned(), content());
    assert_eq!(
        list(&[skill("skill_a", 1, 0, 0)], &contents, 1)[0].can_start_training,
        0
    );
    let both = list(
        &[skill("skill_a", 1, 0, 0), skill("skill_b", 2, 0, 0)],
        &contents,
        1,
    );
    assert_eq!(both[0].can_start_training, 1);
    // Class restriction without the Fiona bit, and a missing SkillInfo row.
    contents.get_mut("skill_b").unwrap().class_restriction = Some(1);
    assert_eq!(
        list(&[skill("skill_b", 1, 0, 0)], &contents, 1)[0].can_start_training,
        0
    );
    contents.get_mut("skill_b").unwrap().class_restriction = None;
    assert_eq!(
        list(&[skill("skill_b", 1, 0, 0)], &contents, 1)[0].can_start_training,
        0
    );
}

#[test]
fn item_ranks_raise_final_rank_and_duplicates_are_refused() {
    let contents = BTreeMap::from([("skill_a".to_owned(), content())]);
    let items = BTreeMap::from([("skill_a".to_owned(), 2)]);
    let character = Character {
        class: 1,
        level: 1,
        item_ranks: &items,
    };
    let b = &to_skill_list(&[skill("skill_a", 1, 0, 0)], &contents, &character).unwrap()[0];
    assert_eq!((b.base_rank, b.final_rank), (1, 3));
    assert!(
        to_skill_list(
            &[skill("skill_a", 1, 0, 0), skill("skill_a", 2, 0, 0)],
            &contents,
            &character
        )
        .is_err()
    );
}

#[test]
fn learning_spends_ap_completes_ranks_and_refunds_excess() {
    use vindictus_server::domain::skill_list::{Learned, learn};
    let contents = BTreeMap::from([("skill_a".to_owned(), content())]);
    let items = BTreeMap::new();
    let c = Character {
        class: 1,
        level: 1,
        item_ranks: &items,
    };
    let s = [skill("skill_a", 1, 0, 0)];
    // Partial training keeps the rank and accumulates CurrentAP.
    assert_eq!(
        learn(&s, &contents, &c, "skill_a", 50, 20).unwrap(),
        Some(Learned {
            ap: 30,
            used: 20,
            rank: 1,
            current_ap: 20,
            locked: 0,
            completed: false
        })
    );
    // Excess AP above RequiredAP is given back and the rank completes.
    let partial = [skill("skill_a", 1, 0, 20)];
    assert_eq!(
        learn(&partial, &contents, &c, "skill_a", 30, 30).unwrap(),
        Some(Learned {
            ap: 20,
            used: 10,
            rank: 2,
            current_ap: 0,
            locked: 0,
            completed: true
        })
    );
    // Source false paths: no AP, non-positive AP, unknown skill.
    assert_eq!(learn(&s, &contents, &c, "skill_a", 10, 20).unwrap(), None);
    assert_eq!(learn(&s, &contents, &c, "skill_a", 50, 0).unwrap(), None);
    assert_eq!(learn(&s, &contents, &c, "skill_x", 50, 10).unwrap(), None);
    // A next rank that requires unlocking locks the skill after completion.
    let mut locking = contents.clone();
    locking.get_mut("skill_a").unwrap().ranks.insert(
        3,
        RankInfo {
            require_unlock: true,
            ..rank(1, 130)
        },
    );
    let done = learn(&s, &locking, &c, "skill_a", 30, 30).unwrap().unwrap();
    assert_eq!((done.rank, done.locked, done.completed), (2, 1, true));
}

#[test]
fn vocation_entries_follow_the_source() {
    use vindictus_server::domain::skill_list::vocation_entry;
    assert!(vocation_entry(None).is_none());
    assert!(vocation_entry(Some(-1)).is_none());
    let paladin = vocation_entry(Some(0)).unwrap();
    assert_eq!(
        (
            paladin.id.as_str(),
            paladin.base_rank,
            paladin.required_ap,
            paladin.can_start_training
        ),
        ("skill_voc_pl_transform00", 1, -1, 0)
    );
    assert_eq!(
        vocation_entry(Some(1)).unwrap().id,
        "skill_voc_dk_transform00"
    );
}

#[test]
fn learn_new_checks_rank_one_constraints_price_and_ownership_in_source_order() {
    use vindictus_server::domain::skill_list::{LearnNewRefusal, NewSkill, learn_new};
    let items = BTreeMap::new();
    let fiona = |level| Character {
        class: 1,
        level,
        item_ranks: &items,
    };
    let mut buyable = content();
    buyable.price = Some(1200);
    buyable.use_type = Some(1);
    buyable.ranks.insert(1, rank(3, 0));
    buyable
        .constraints
        .insert(1, vec![("skill_base".to_owned(), 2)]);
    let mut contents = BTreeMap::from([("skill_new".to_owned(), buyable)]);
    let learn = |skills: &[Skill], contents: &BTreeMap<String, SkillContent>, level| {
        learn_new(skills, contents, &fiona(level), "skill_new").unwrap()
    };
    let base = |r| vec![skill("skill_base", r, 0, 0)];
    assert_eq!(learn(&base(2), &contents, 2), Err(LearnNewRefusal::Level));
    assert_eq!(
        learn(&base(1), &contents, 3),
        Err(LearnNewRefusal::Constraint)
    );
    assert_eq!(
        learn(&base(2), &contents, 3),
        Ok(NewSkill {
            price: 1200,
            locked: 0,
            active: true
        })
    );
    let mut owned = base(2);
    owned.push(skill("skill_new", 1, 0, 0));
    assert_eq!(
        learn(&owned, &contents, 3),
        Err(LearnNewRefusal::AlreadyLearnt)
    );
    let entry = contents.get_mut("skill_new").unwrap();
    entry.ranks.get_mut(&2).unwrap().require_unlock = true;
    assert_eq!(learn(&base(2), &contents, 3).unwrap().locked, 1);
    let entry = contents.get_mut("skill_new").unwrap();
    entry.price = Some(-1);
    assert_eq!(
        learn(&base(2), &contents, 3),
        Err(LearnNewRefusal::CannotBuySkill)
    );
    let entry = contents.get_mut("skill_new").unwrap();
    entry.class_restriction = Some(4);
    assert_eq!(learn(&base(2), &contents, 3), Err(LearnNewRefusal::Class));
    let entry = contents.get_mut("skill_new").unwrap();
    entry.ranks.get_mut(&1).unwrap().feature_enabled = false;
    assert_eq!(
        learn(&base(2), &contents, 3),
        Err(LearnNewRefusal::NoSuchSkill)
    );
    assert_eq!(
        learn_new(&[], &contents, &fiona(3), "skill_missing").unwrap(),
        Err(LearnNewRefusal::NoSuchSkill)
    );
}

#[test]
fn story_give_checks_constraints_unless_preset_and_ignores_price() {
    use vindictus_server::domain::skill_list::{LearnNewRefusal, NewSkill, give};
    // GuidePlay skills cannot be bought (Price -1) but GiveSkill has no price check.
    let mut granted = content();
    granted.price = Some(-1);
    granted.ranks.insert(1, rank(5, 0));
    granted
        .constraints
        .insert(1, vec![("skill_base".to_owned(), 2)]);
    let contents = BTreeMap::from([("skill_story".to_owned(), granted)]);
    let ranks = |base: i32, story: i32| {
        move |id: &str| match id {
            "skill_base" => base,
            "skill_story" => story,
            _ => -1,
        }
    };
    let new = NewSkill {
        price: 0,
        locked: 0,
        active: false,
    };
    // ConstraintPassed: feature, class, level, constraints; then Already_Learnt.
    assert_eq!(
        give(&ranks(2, -1), &contents, 0, 5, false, "skill_story"),
        Err(LearnNewRefusal::Class)
    );
    assert_eq!(
        give(&ranks(2, -1), &contents, 1, 4, false, "skill_story"),
        Err(LearnNewRefusal::Level)
    );
    assert_eq!(
        give(&ranks(1, -1), &contents, 1, 5, false, "skill_story"),
        Err(LearnNewRefusal::Constraint)
    );
    assert_eq!(
        give(&ranks(2, -1), &contents, 1, 5, false, "skill_story"),
        Ok(new)
    );
    assert_eq!(
        give(&ranks(2, 0), &contents, 1, 5, false, "skill_story"),
        Err(LearnNewRefusal::AlreadyLearnt)
    );
    // ForceConstraintPassed for a preset character skips level and constraints.
    assert_eq!(
        give(&ranks(-1, -1), &contents, 1, 1, true, "skill_story"),
        Ok(new)
    );
    assert_eq!(
        give(&ranks(2, -1), &contents, 1, 5, false, "skill_missing"),
        Err(LearnNewRefusal::NoSuchSkill)
    );
}
