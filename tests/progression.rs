use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    progression::{
        AcquiredTitle, GivenTitle, Growth, NoticedTitle, Proceeded, Progression, ProgressionTables,
        STAT_NAMES, StoredTitle, TitleSlotValue,
    },
};

fn policy(max_level: &str) -> FeaturePolicy {
    FeaturePolicy::npc_source(
        [
            ("CurrentVer", "3.09.00"),
            ("MaxLevel", max_level),
            ("EP3", "1"),
            ("AccountTitleStatRatio", "80"),
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
        title_stats: vec![
            (410017, Some("STR".to_owned()), 2),
            (410017, Some("hp".to_owned()), 15),
            (410019, Some("DEX".to_owned()), 5),
            (410021, Some("WILL".to_owned()), 7),
            // A row of an unknown title is skipped, as titleStats.TryGetValue.
            (999, Some("STR".to_owned()), 50),
        ],
        title_sharing: vec![
            (410017, 1, false),
            (410018, 1, false),
            (410019, 5, true),
            (410020, 1, true),
            (410021, 1, true),
            (410022, 1, false),
        ],
        title_goal_targets: Vec::new(),
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
    assert_eq!(p.class_stats(1, 4), p.class_stats(1, 1));
}

#[test]
fn level_up_stat_increase_and_titles_follow_the_source_order() {
    let p = Progression::load(tables(), &policy("90")).unwrap();
    let increase = p.stat_increase(1, 1, 3).unwrap();
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

#[test]
fn title_stats_add_own_and_shared_account_titles_like_get_title_stats() {
    let p = Progression::load(tables(), &policy("90")).unwrap();
    // STR DEX INT WILL LUCK HP STAMINA.
    assert_eq!(p.title_stats(&[], &[], 1, 1), Some([0; 7]));
    assert_eq!(
        p.title_stats(&[410017], &[410017], 1, 1),
        Some([2, 0, 0, 0, 0, 15, 0])
    );
    // Account titles: 410021 (shared, level 1) counts at 80 % truncated (7 → 5);
    // 410019 needs level 5; 410020 is shared but not for the Fiona's class.
    assert_eq!(
        p.title_stats(&[410017], &[410017, 410019, 410020, 410021], 1, 1),
        Some([2, 0, 0, 5, 0, 15, 0])
    );
    assert_eq!(
        p.title_stats(&[410017], &[410019, 410021, 410021], 1, 5),
        Some([2, 4, 0, 5, 0, 15, 0])
    );
    // A title acquired here is not counted again from the account list.
    assert_eq!(
        p.title_stats(&[410021], &[410021], 1, 1),
        Some([0, 0, 0, 7, 0, 0, 0])
    );
    // An unknown stat name throws in the source: the profile is refused.
    let mut bad = tables();
    bad.title_stats.push((410017, Some("SPEED".to_owned()), 1));
    assert!(Progression::load(bad, &policy("90")).is_err());
}

#[test]
fn title_goals_track_match_and_proceed_like_the_title_manager() {
    let mut t = tables();
    // 410017: goals 1 (kick a spider, 1) and 2 (any gnoll, 2, party); 410018 (HideUntil
    // 5): goal 3 (kick anything, 10).
    t.title_goal_targets = vec![
        (
            1,
            Some("kill npc_spider [a-z_]* [a-z0-9_]*kick[a-z0-9_]*".to_owned()),
            1,
            false,
        ),
        (2, Some("KILL NPC_GNOLL".to_owned()), 2, true),
        (3, Some("kill.+kick".to_owned()), 10, false),
    ];
    let p = Progression::load(t, &policy("90")).unwrap();
    let owned = [(410017, false), (410018, false), (410019, true)];
    // GetTrackingGoalID: unacquired owned titles, progress not -1 (no row is 0).
    assert_eq!(p.tracking_goals(&owned, &[(2, -1)]), [1, 3]);
    let tracked = p.tracking_goals(&owned, &[]);
    assert_eq!(tracked, [1, 2, 3]);
    // IgnoreCase regex; a non-party goal needs the event's own actor.
    let kick = "kill npc_spider normal player_kick_01";
    assert_eq!(p.matching_goals(&tracked, kick, true), [1, 3]);
    assert_eq!(p.matching_goals(&tracked, kick, false), Vec::<i32>::new());
    assert_eq!(
        p.matching_goals(&tracked, "kill npc_gnoll chief x", false),
        [2]
    );
    // Hidden 410018 (HideUntil 5) counts without a slot until the progress reveals it;
    // UpdateTitleCount off.
    assert_eq!(
        p.proceed(3, &owned, &[(3, 3)], 1, false),
        Some(Proceeded {
            title: 410018,
            goal: 3,
            progress: 4,
            completed: false,
            acquired: false,
            slot: None
        })
    );
    // Reaching HideUntil discovers it (Known).
    assert_eq!(
        p.proceed(3, &owned, &[(3, 4)], 1, false).unwrap().slot,
        Some((1, vec![(3, 5)]))
    );
    // A visible title reports every count only with UpdateTitleCount.
    assert_eq!(
        p.proceed(3, &owned, &[(3, 5)], 1, false).unwrap().slot,
        None
    );
    assert_eq!(
        p.proceed(3, &owned, &[(3, 5)], 1, true).unwrap().slot,
        Some((1, vec![(3, 6)]))
    );
    // Completing goal 1 with goal 2 open: completed (-1), not yet acquirable.
    let first = p.proceed(1, &owned, &[], 1, true).unwrap();
    assert_eq!(
        (first.progress, first.completed, first.acquired),
        (-1, true, false)
    );
    assert_eq!(first.slot, Some((1, vec![(1, -1), (2, 0)])));
    // The last goal acquires the title: every goal reports -1.
    let last = p.proceed(2, &owned, &[(1, -1), (2, 1)], 1, false).unwrap();
    assert!(last.acquired);
    assert_eq!(last.slot, Some((2, vec![(1, -1), (2, -1)])));
    // Completed goals, acquired, unowned or unknown titles change nothing.
    assert_eq!(p.proceed(1, &owned, &[(1, -1)], 1, true), None);
    assert_eq!(p.proceed(1, &[(410017, true)], &[], 1, true), None);
    assert_eq!(p.proceed(1, &[], &[], 1, true), None);
    assert_eq!(p.proceed(99, &owned, &[], 1, true), None);
}

#[test]
fn title_list_reports_visible_titles_and_shared_account_titles() {
    let mut t = tables();
    t.title_goal_targets = vec![
        (1, Some("a".to_owned()), 1, false),
        (2, Some("b".to_owned()), 2, false),
        (3, Some("c".to_owned()), 10, false),
    ];
    let p = Progression::load(t, &policy("90")).unwrap();
    let row = |id, acquired, ticks: Option<i64>, quest: Option<&str>| StoredTitle {
        id,
        acquired,
        acquired_ticks: ticks,
        quest: quest.map(str::to_owned),
        expire_ticks: None,
    };
    // 410017 Known (HideUntil 0), 410018 Counting (HideUntil 5, progress 3: left out),
    // 410019 acquired, 410022 feature-disabled and unacquired: left out.
    let own = [
        row(410017, false, None, None),
        row(410018, false, None, None),
        row(410019, true, Some(77), Some("quest_a")),
        row(410022, false, None, None),
    ];
    // Account: 410019 (AllowShare) from another character and this one (own wins),
    // 410017 (no AllowShare) left out, 410020 (Lethita only) left out for Fiona.
    let account = [
        (2, row(410019, true, Some(1), Some("other"))),
        (1, row(410019, true, Some(77), Some("quest_a"))),
        (2, row(410017, true, Some(5), None)),
        (2, row(410020, true, Some(6), None)),
    ];
    let (shared, titles) = p.title_list(1, 1, &own, &[(1, 1), (3, 3)], &account);
    let acquired = TitleSlotValue {
        id: 410019,
        status: 2,
        time_stamp: 77,
        quest: "quest_a".to_owned(),
        goals: Vec::new(),
        expire: None,
    };
    assert_eq!(shared, std::slice::from_ref(&acquired));
    assert_eq!(
        titles,
        [
            TitleSlotValue {
                id: 410017,
                status: 1,
                time_stamp: 0,
                quest: String::new(),
                goals: vec![(1, 1), (2, 0)],
                expire: None,
            },
            acquired,
        ]
    );
}
