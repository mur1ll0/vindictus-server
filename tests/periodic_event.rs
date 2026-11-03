use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    periodic_event::{EventTables, Events, RewardType, StoryProgress},
};

fn policy(boost: Option<&str>, period: &str) -> FeaturePolicy {
    FeaturePolicy::npc_source([
        ("CurrentVer".to_owned(), Some("3.09.00".to_owned())),
        ("BoostEvent".to_owned(), boost.map(str::to_owned)),
        ("BoostEventPeriod".to_owned(), Some(period.to_owned())),
        ("Event_boost_eneu".to_owned(), Some("1".to_owned())),
        ("Off".to_owned(), None),
        ("enEU".to_owned(), Some("1".to_owned())),
    ])
    .unwrap()
}
fn s(v: &str) -> Option<String> {
    Some(v.to_owned())
}
fn events() -> Events {
    Events::load(EventTables {
        periodic: vec![
            (s("Gold"), None, 1000, 40, None, s("disable")),
            (s("ap"), None, 10, 30, None, None),
            (
                s("Exp"),
                None,
                500,
                20,
                s("GameUI_Heroes_GetitemEvent_Telepathy"),
                s(""),
            ),
            (
                s("ITEM"),
                s("capsule_ap50"),
                1,
                50,
                None,
                s("Event_boost_eneu"),
            ),
            (s("Unknown"), None, 1, 10, None, s("Off")),
        ],
        story_periods: vec![
            // 2010-01-19 00:00 UTC, daily, back to phase A.
            (s("Arbeit_get_fish"), 1_263_859_200, 24, s("A"), s("enEU")),
            (s("Arbeit_other"), 1_263_859_200, 24, s("A"), s("Off")),
            (s("Arbeit_missing"), 1_263_859_200, 48, s("Z"), None),
        ],
    })
    .unwrap()
}

#[test]
fn boost_rewards_need_boost_event_and_walk_the_enabled_rates() {
    let e = events();
    // PeriodicEventWaiting follows BoostEventPeriod alone (1800 in EN-EU).
    assert_eq!(Events::period(&policy(None, "1800")), Some(1800));
    assert_eq!(Events::period(&policy(None, "0")), None);
    // EN-EU keeps BoostEvent off: the timeline runs and gives nothing.
    let mut never = |_: i32| -> i32 { panic!("no draw without BoostEvent") };
    assert_eq!(e.pick(&policy(None, "1800"), &mut never).unwrap(), None);
    // Enabled: ap 30 + Exp 20 (empty Feature) + ITEM 50 = 100; Gold ("disable") and the
    // Off row are left out of the sum and the walk.
    let on = policy(Some("1"), "1800");
    let mut sums = Vec::new();
    let draw = |roll: i32| {
        move |n: i32| {
            assert_eq!(n, 100);
            roll
        }
    };
    for (roll, kind) in [
        (0, RewardType::Ap),
        (29, RewardType::Ap),
        (30, RewardType::Exp),
        (50, RewardType::Item),
        (99, RewardType::Item),
    ] {
        let got = e.pick(&on, &mut draw(roll)).unwrap().unwrap();
        assert_eq!(got.kind, kind, "roll {roll}");
        sums.push(got.amount);
    }
    assert_eq!(sums, [10, 10, 500, 1, 1]);
    let item = e.pick(&on, &mut draw(60)).unwrap().unwrap();
    assert_eq!(item.arg.as_deref(), Some("capsule_ap50"));
    assert_eq!(
        e.rewards()[2].announce.as_deref(),
        Some("GameUI_Heroes_GetitemEvent_Telepathy")
    );
    // Enum.Parse(ignoreCase): "Unknown" is None.
    assert_eq!(e.rewards()[4].kind, RewardType::None);
}

#[test]
fn story_periods_return_storylines_at_each_boundary() {
    let e = events();
    let policy = policy(None, "1800");
    let day = 86_400;
    let from = 1_263_859_200;
    // Day 100 of the period, 10:00.
    let now = from + 100 * day + 10 * 3600;
    let mut progress = vec![
        // Last update the day before: moves to A.
        StoryProgress {
            story_line: "Arbeit_get_fish".into(),
            phase: Some("C".into()),
            last_update: now - day,
        },
        // Off feature: untouched.
        StoryProgress {
            story_line: "Arbeit_other".into(),
            phase: Some("C".into()),
            last_update: now - day,
        },
        // Same 48-hour period (day 100 began it): untouched, though its phase is missing.
        StoryProgress {
            story_line: "Arbeit_missing".into(),
            phase: Some("B".into()),
            last_update: from + 100 * day,
        },
    ];
    let has = |line: &str, phase: &str| line == "Arbeit_get_fish" && phase == "A";
    let daily = e.daily_story(&policy, &mut progress, now, &has).unwrap();
    assert_eq!(daily.goto, [("Arbeit_get_fish".to_owned(), "A".to_owned())]);
    assert_eq!(progress[0].phase.as_deref(), Some("A"));
    assert_eq!(progress[0].last_update, now);
    assert_eq!(progress[1].phase.as_deref(), Some("C"));
    // The next boundary is the daily one at day 101 00:00.
    assert_eq!(daily.next, Some(from + 101 * day));
    // Run again the same day: nothing moves.
    let again = e
        .daily_story(&policy, &mut progress, now + 60, &has)
        .unwrap();
    assert!(again.goto.is_empty());
    // Two days later the 48-hour storyline turns but its phase Z does not exist (logged).
    let later = e
        .daily_story(&policy, &mut progress, now + 2 * day, &has)
        .unwrap();
    assert_eq!(later.goto, [("Arbeit_get_fish".to_owned(), "A".to_owned())]);
    assert_eq!(progress[2].phase.as_deref(), Some("B"));
    // A storyline without progress or phase is skipped.
    let mut none = vec![StoryProgress {
        story_line: "Arbeit_get_fish".into(),
        phase: None,
        last_update: 0,
    }];
    let skipped = e.daily_story(&policy, &mut none, now, &has).unwrap();
    assert_eq!((skipped.goto.len(), skipped.next), (0, None));
}
