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
