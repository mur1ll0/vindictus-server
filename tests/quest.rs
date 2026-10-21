use serde_json::Value;
use vindictus_server::domain::{
    character_stats::Arithmetic,
    feature_policy::FeaturePolicy,
    item_colors::ColorRandom,
    quest::{
        Constraint, Context, FIRST_QUEST, GiveQuest, NEVER_PLAYED, QuestContents, QuestInfo,
        QuestOwner, QuestTables, UNITS_PER_DAY, days_from_civil, framework_string_hash,
        year_and_day,
    },
};

#[test]
fn string_hash_and_random_match_the_framework_oracle() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-string-hash.json")).unwrap();
    for (arch, arithmetic) in [
        ("x86", Arithmetic::FrameworkX86),
        ("x64", Arithmetic::FrameworkX64),
    ] {
        let cases = fixture[arch].as_array().unwrap();
        assert_eq!(cases.len(), 149);
        for case in cases {
            let text = case["text"].as_str().unwrap();
            let hash = framework_string_hash(text, arithmetic);
            assert_eq!(
                i64::from(hash),
                case["hash"].as_i64().unwrap(),
                "{arch} {text:?}"
            );
            let mut random = ColorRandom::from_seed(hash);
            let next: Vec<i64> = [1, 2, 3, 7, 40, 190]
                .into_iter()
                .map(|n| i64::from(random.next(n).unwrap()))
                .collect();
            let expected: Vec<i64> = case["next"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect();
            assert_eq!(next, expected, "{arch} {text:?}");
        }
        // The oracle's last texts are Year + DayOfYear of 2026-01-01 plus 7-day steps.
        let first = days_from_civil(2026, 1, 1);
        for (i, case) in cases[91..].iter().enumerate() {
            let (year, day) = year_and_day(first + 7 * i as i64);
            assert_eq!(format!("{year}{day}"), case["text"].as_str().unwrap());
        }
    }
    assert_eq!(days_from_civil(1990, 1, 1) * UNITS_PER_DAY, NEVER_PLAYED);
    assert_eq!(year_and_day(days_from_civil(2024, 12, 31)), (2024, 366));
    assert_eq!(year_and_day(0), (1900, 1));
}

fn info(id: &str, level: i32, set: i32, feature: Option<&str>) -> QuestInfo {
    QuestInfo {
        id: id.to_owned(),
        auto_give_level: level,
        quest_set: set,
        service_coin: 0,
        max_member_count: 4,
        max_play_count: None,
        enable_today_quest: Some("Enable".to_owned()),
        season3_today_group: None,
        auto_reveal: false,
        feature: feature.map(str::to_owned),
        is_season2: false,
    }
}
fn policy() -> FeaturePolicy {
    FeaturePolicy::npc_source(
        [
            ("CurrentVer", "3.09.00"),
            ("Enable", "1"),
            ("TodayQuest", "1"),
            ("TodayQuest_InitTimeAdder", "1"),
            ("EP3", "1"),
            ("enEU", "1"),
            ("Event_PlayCount_Plus", "1"),
        ]
        .map(|(k, v)| (k.to_owned(), Some(v.to_owned()))),
    )
    .unwrap()
}
fn contents(p: &FeaturePolicy) -> QuestContents {
    QuestContents::load(
        QuestTables {
            quests: vec![
                info(FIRST_QUEST, 1, 1, None),
                info("quest_shadowfang", 1, 1, Some("EP3")),
                info("quest_ambermane", 1, 2, None),
                info("quest_late", 5, 2, None),
                info("quest_hidden", 1, 3, Some("EP9")),
            ],
            goals: vec![
                (10, Some(FIRST_QUEST.to_owned()), 0, 100),
                (20, Some("quest_shadowfang".to_owned()), 0, 60),
                (21, Some("quest_shadowfang".to_owned()), 1, 40),
            ],
            constraints: vec![(
                Some("quest_ambermane".to_owned()),
                Some("quest_shadowfang".to_owned()),
                100,
            )],
            difficulties: vec![
                (Some(FIRST_QUEST.to_owned()), 2, 1, None),
                (Some("quest_ambermane".to_owned()), 2, 3, None),
                (
                    Some("quest_shadowfang".to_owned()),
                    2,
                    1,
                    Some("EP9".to_owned()),
                ),
            ],
            periods: vec![
                (
                    Some(FIRST_QUEST.to_owned()),
                    None,
                    24,
                    Some(5),
                    Some("enEU".to_owned()),
                ),
                (
                    Some(FIRST_QUEST.to_owned()),
                    None,
                    24,
                    Some(9),
                    Some("koKR".to_owned()),
                ),
            ],
        },
        p,
    )
    .unwrap()
}

#[test]
fn login_gives_the_first_quest_revealed_and_level_auto_gives_once() {
    let p = policy();
    let c = contents(&p);
    let mut owner = QuestOwner::load(vec![], vec![], vec![], &c).unwrap();
    assert_eq!(owner.login_openings(&c, 1), 4);
    owner.login(&c, 1);
    let ids: Vec<(&str, i32)> = owner
        .quests
        .iter()
        .map(|q| (q.quest.as_str(), q.revealed))
        .collect();
    assert_eq!(
        ids,
        [
            (FIRST_QUEST, 1),
            ("quest_shadowfang", 0),
            ("quest_ambermane", 0),
            ("quest_hidden", 0)
        ]
    );
    assert!(
        owner
            .quests
            .iter()
            .all(|q| q.row_id.is_none() && q.last_play == NEVER_PLAYED)
    );
    // A relogin at the same level gives nothing; level 5 adds its quest.
    let mut again = QuestOwner::load(owner.quests.clone(), vec![], vec![], &c).unwrap();
    assert_eq!(again.login_openings(&c, 1), 0);
    assert_eq!(again.login_openings(&c, 5), 1);
    again.login(&c, 5);
    assert_eq!(again.quests.len(), 5);
    // give_quest with reveal: owned quests are revealed and still succeed.
    assert_eq!(
        again.give_quest(&c, "quest_ambermane", true),
        (GiveQuest::Success, 0)
    );
    assert_eq!(again.owned("quest_ambermane").unwrap().revealed, 1);
    assert_eq!(
        again.give_quest(&c, "quest_ambermane", false),
        (GiveQuest::AlreadyOwned, 0)
    );
    assert_eq!(
        again.give_quest(&c, "missing", true),
        (GiveQuest::NoSuchQuest, 0)
    );
}

#[test]
fn progress_reports_constraints_play_counts_features_and_goal_weights() {
    let p = policy();
    let c = contents(&p);
    let goals = vec![vindictus_server::domain::quest::GoalAchievement {
        goal: 20,
        status: 4,
        clear_count: 1,
        regened_ap: 0,
        last_clear: NEVER_PLAYED,
    }];
    let mut owner = QuestOwner::load(vec![], goals, vec![], &c).unwrap();
    owner.login(&c, 1);
    let mut owner = QuestOwner::load(owner.quests, owner.goals, vec![], &c).unwrap();
    owner.init_status(&c);
    owner.level = 1;
    let ctx = Context {
        contents: &c,
        policy: &p,
        cafe: false,
        raid_coupons: 0,
    };
    // InitQuestStatus sums the weights of achieved goals.
    assert_eq!(owner.progress_of("quest_shadowfang"), 60);
    assert_eq!(owner.progress_of("quest_none"), -1);
    assert_eq!(owner.achieved_goal_ids(&c, "quest_shadowfang"), [20]);
    // Difficulty 2 of shadowfang is disabled by feature: NoSuchDifficulty.
    assert_eq!(
        owner
            .check_constraint(ctx, "quest_shadowfang", 2, false)
            .unwrap(),
        Constraint::NoSuchDifficulty
    );
    // Ambermane needs level 3 at difficulty 2, and shadowfang progress 100.
    assert_eq!(
        owner
            .check_constraint(ctx, "quest_ambermane", 2, false)
            .unwrap(),
        Constraint::LevelConstraintFail
    );
    assert_eq!(
        owner
            .check_constraint(ctx, "quest_ambermane", 0, false)
            .unwrap(),
        Constraint::RequiredStatusFail
    );
    assert_eq!(
        owner
            .check_constraint(ctx, "quest_hidden", 0, false)
            .unwrap(),
        Constraint::InvalidQuest
    );
    let now = days_from_civil(2026, 10, 21) * UNITS_PER_DAY + 12 * 300 * 3600;
    let progress = owner.progress(ctx, now, Arithmetic::FrameworkX64).unwrap();
    // The disabled-feature quest is not listed; the enEU period sets MaxPlayCount 5 (+1 event).
    let ids: Vec<&str> = progress.iter().map(|q| q.id.as_str()).collect();
    assert_eq!(ids, [FIRST_QUEST, "quest_shadowfang", "quest_ambermane"]);
    assert_eq!(progress[0].max_play_count, 6);
    assert!(progress[0].available && progress[0].revealed && progress[0].appear_named_monster);
    assert_eq!(progress[1].max_play_count, -1);
    assert!(!progress[2].available);
    // One quest per QuestSet of enabled quests is today's quest (set 3 is disabled).
    let today = c.today_quests(&p, now, Arithmetic::FrameworkX64).unwrap();
    assert_eq!(today.len(), 2);
}
