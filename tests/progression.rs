use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    progression::{
        AcquiredTitle, GivenTitle, Growth, NoticedTitle, Progression, ProgressionTables, STAT_NAMES,
    },
};

fn policy(max_level: &str) -> FeaturePolicy {
    FeaturePolicy::npc_source(
        [
            ("CurrentVer", "3.09.00"),
            ("MaxLevel", max_level),
            ("EP3", "1"),
        ]
        .map(|(k, v)| (k.to_owned(), Some(v.to_owned()))),
    )
    .unwrap()
}
fn tables() -> ProgressionTables {
    ProgressionTables {
        class_stats: vec![
            (1, 27, 44, 14, 23, 1020, 130, None),
            (2, 34, 48, 20, 27, 1048, 130, None),
            (3, 42, 52, 26, 31, 1075, 130, None),
            (4, 1, 1, 1, 1, 1, 1, Some("EP9".to_owned())),
        ],
        level_exp: vec![
            (1, 350, None),
            (2, 1148, None),
            (3, 1855, Some("EP3".to_owned())),
            (4, 2537, Some("EP9".to_owned())),
        ],
        titles: vec![
            (410017, Some(1), 2047, 0, None),
            (410018, Some(2), 2047, 5, None),
            (410019, Some(3), 2047, 0, None),
            (410020, Some(1), 1, 0, None),
            (410021, None, 2047, 0, None),
            (410022, Some(1), 2047, 0, Some("EP9".to_owned())),
        ],
        title_goals: vec![(1, 410017), (2, 410017), (3, 410018)],
    }
}

#[test]
fn give_exp_follows_the_source_loop_level_table_and_max_level() {
    let p = Progression::load(tables(), &policy("90")).unwrap();
    // Below the requirement: exp only.
    assert_eq!(
        p.give_exp(0, 1, 300),
        Some(Growth {
            previous_exp: 0,
            exp: 300,
            previous_level: 1,
            level: 1,
            level_up_exp: 350
        })
    );
    // Exactly the requirement stays (value >= exp breaks).
    assert_eq!(p.give_exp(0, 1, 350).unwrap().level, 1);
    // Two levels in one reward: 350 + 1148 consumed.
    let g = p.give_exp(100, 1, 1500).unwrap();
    assert_eq!((g.level, g.exp, g.level_up_exp), (3, 102, 1855));
    assert!(g.leveled());
    // A missing table row (level 4 is feature-disabled) resets the exp to 0.
    let g = p.give_exp(0, 3, 5000).unwrap();
    assert_eq!((g.level, g.exp, g.level_up_exp), (4, 0, 0));
    // MaxLevel reached: early return; reaching it while looping zeroes the exp.
    let capped = Progression::load(tables(), &policy("2")).unwrap();
    assert_eq!(capped.give_exp(0, 2, 10), None);
    let g = capped.give_exp(0, 1, 5000).unwrap();
    assert_eq!((g.level, g.exp, g.level_up_exp), (2, 0, 1148));
    // GetCharacterStats falls back to level 1 for a missing class row.
    assert_eq!(p.class_stats(4), p.class_stats(1));
}

#[test]
fn level_up_stat_increase_and_titles_follow_the_source_order() {
    let p = Progression::load(tables(), &policy("90")).unwrap();
    let increase = p.stat_increase(1, 3).unwrap();
    assert_eq!(increase.len(), STAT_NAMES.len());
    let get = |k: &str| increase.iter().find(|(n, _)| n == k).unwrap().1;
    assert_eq!(
        (get("STR"), get("HP"), get("LUCK"), get("ATK")),
        (15, 55, 0, 0)
    );
    // AutoGiveLevel ≤ level, enabled, class allowed (Fiona = 1), not owned.
    assert_eq!(
        p.titles_by_level(2, 1, &[410017]),
        [GivenTitle {
            id: 410018,
            goals: vec![3],
            listed: false
        }]
    );
    let ids: Vec<i32> = p
        .titles_by_level(3, 1, &[])
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(ids, [410017, 410018, 410019]);
}

#[test]
fn notice_title_opens_or_reveals_a_title_like_title_manager() {
    let p = Progression::load(tables(), &policy("90")).unwrap();
    // Not owned: GiveTitle, the first goal gets HideUntil (5) and the slot is Known.
    assert_eq!(
        p.notice_title(410018, 1, None).unwrap(),
        Some(NoticedTitle {
            id: 410018,
            new: true,
            goals: vec![(3, 5)]
        })
    );
    // Goals of a HideUntil 0 title stay at 0.
    assert_eq!(
        p.notice_title(410017, 1, None).unwrap().unwrap().goals,
        [(1, 0), (2, 0)]
    );
    // Unknown, feature-disabled and class-restricted titles change nothing.
    assert_eq!(p.notice_title(999, 1, None).unwrap(), None);
    assert_eq!(p.notice_title(410022, 1, None).unwrap(), None);
    assert_eq!(p.notice_title(410020, 1, None).unwrap(), None);
    // Owned and hidden (progress 2 < HideUntil 5): revealed, not inserted again.
    assert_eq!(
        p.notice_title(410018, 1, Some((false, &[(3, 2)]))).unwrap(),
        Some(NoticedTitle {
            id: 410018,
            new: false,
            goals: vec![(3, 5)]
        })
    );
    // Owned and already visible, or acquired: "already known", nothing changes.
    assert_eq!(
        p.notice_title(410018, 1, Some((false, &[(3, 5)]))).unwrap(),
        None
    );
    assert_eq!(p.notice_title(410018, 1, Some((true, &[]))).unwrap(), None);
    // A completed goal (-1) counts TargetCount, which the profile does not carry.
    assert!(
        p.notice_title(410018, 1, Some((false, &[(3, -1)])))
            .is_err()
    );
}

#[test]
fn acquire_title_gives_then_acquires_like_title_manager() {
    let p = Progression::load(tables(), &policy("90")).unwrap();
    // Not owned: GiveTitle then acquire; every goal is listed for the -1 slot.
    assert_eq!(
        p.acquire_title(410017, 1, None),
        Some(AcquiredTitle {
            id: 410017,
            new: true,
            goals: vec![1, 2]
        })
    );
    // Owned and not acquired: only acquired.
    assert!(!p.acquire_title(410018, 1, Some(false)).unwrap().new);
    // Already acquired, unknown, feature-disabled or class-restricted: nothing.
    assert_eq!(p.acquire_title(410018, 1, Some(true)), None);
    assert_eq!(p.acquire_title(999, 1, None), None);
    assert_eq!(p.acquire_title(410022, 1, None), None);
    assert_eq!(p.acquire_title(410020, 1, None), None);
}
