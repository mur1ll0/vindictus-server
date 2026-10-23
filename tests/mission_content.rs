use std::collections::VecDeque;
use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    mission::{
        Consumable, ItemSlotInfo, MissionContents, MissionError, MissionRandom, MissionTables,
        SlotSource, battle_inventory, multiplier,
    },
    mission_goal::{
        self as goal, Achieved, GoalInfo, GoalTrigger, RewardFacts, RewardLists, TargetTracker,
    },
};

/// Scripted System.Random: queued values, then the given fallbacks.
#[derive(Default)]
struct Script {
    next: VecDeque<i32>,
    range: VecDeque<i32>,
    double: VecDeque<f64>,
}
impl MissionRandom for Script {
    fn next(&mut self) -> i32 {
        self.next.pop_front().unwrap_or(0)
    }
    fn next_range(&mut self, min: i32, max: i32) -> i32 {
        let v = self.range.pop_front().unwrap_or(min);
        assert!(
            min == max || (min..max).contains(&v),
            "{v} outside {min}..{max}"
        );
        v
    }
    fn next_double(&mut self) -> f64 {
        self.double.pop_front().unwrap_or(0.5)
    }
}

fn policy() -> FeaturePolicy {
    FeaturePolicy::npc_source([
        ("CurrentVer".to_owned(), Some("3.09.00".to_owned())),
        ("Gathering".to_owned(), Some("2.02".to_owned())),
        ("GatheringRare".to_owned(), Some("2.02.01".to_owned())),
        ("Off".to_owned(), None),
        // As in the exported policy: a test-only feature with an empty value (disabled).
        ("DropRequiredTest".to_owned(), Some(String::new())),
    ])
    .unwrap()
}
fn attrs(pairs: &[(&str, &str)]) -> Vec<(Option<String>, Option<String>)> {
    pairs
        .iter()
        .map(|(k, v)| (Some((*k).to_owned()), Some((*v).to_owned())))
        .collect()
}
fn s(v: &str) -> Option<String> {
    Some(v.to_owned())
}
fn tables() -> MissionTables {
    MissionTables {
        quests: vec![
            (
                s("quest_a"),
                3,
                30,
                4,
                9,
                false,
                None,
                Some(5),
                2,
                None,
                None,
                4,
                false,
                false,
                None,
            ),
            (
                s("quest_tower"),
                1,
                30,
                0,
                0,
                false,
                Some(10),
                None,
                0,
                None,
                None,
                4,
                true,
                false,
                None,
            ),
        ],
        goals: vec![
            (
                s("quest_a"),
                1,
                0,
                s("kill npc_gnoll (boss|chief)"),
                None,
                None,
            ),
            (s("quest_a"), 7, 2, None, s("SoloMode"), Some(2)),
            (s("quest_a"), 8, 2, None, s("TimeLimit"), Some(12)),
            (s("quest_a"), 8, 2, None, s("HardMode"), None),
            (s("quest_a"), 9, 0, s("potion_.*"), s("ItemLimit"), Some(1)),
            (s("quest_a"), 2, 1, s("get item potion"), None, None),
            (s("quest_a"), 3, 1, s("player_dead"), None, None),
            (s("quest_a"), 4, 1, s("kill (?(?=x)y|z)"), None, None),
        ],
        sectors: vec![
            (s("quest_a"), 1, 10, s("01a"), 30, 0, 0),
            (s("quest_a"), 1, 11, s("01b"), 70, 100, 10),
            (s("quest_a"), 2, 12, s("01_boss"), 100, 0, 0),
            (s("quest_a"), 0, 13, s("story"), 100, 0, 0),
            (s("quest_tower"), 1, 14, s("t"), 100, 0, 0),
        ],
        sector_groups: vec![],
        tower_quests: vec!["quest_tower".to_owned()],
        entities: vec![
            (
                10,
                attrs(&[("classname", "info_player_start"), ("origin", "0 0 0")]),
            ),
            (
                11,
                attrs(&[("classname", "trigger_once"), ("targetname", "t1")]),
            ),
            (
                11,
                attrs(&[
                    ("classname", "npc_template_maker"),
                    ("targetname", "maker_a"),
                    ("TemplateName", "npc_gnoll normal"),
                    ("MaxNPCCount", "4"),
                    ("GroupNumber", "3"),
                    ("probability", "0.7"),
                    ("StartDisabled", "1"),
                ]),
            ),
            (
                11,
                attrs(&[
                    ("classname", "npc_template_maker"),
                    ("Feature", "Off"),
                    ("TemplateName", "npc_gnoll normal"),
                ]),
            ),
            (
                11,
                attrs(&[
                    ("classname", "treasure_box"),
                    ("entityname", "box1"),
                    ("itemname", "tb"),
                ]),
            ),
            (
                11,
                attrs(&[("classname", "func_breakable"), ("entityname", "barrel")]),
            ),
            (
                11,
                attrs(&[("classname", "func_breakable"), ("entityname", "barrel")]),
            ),
            (
                11,
                attrs(&[
                    ("classname", "random_box"),
                    ("entityname", "rb1"),
                    ("boxnumber", "1"),
                    ("boxweight", "-2"),
                ]),
            ),
            (
                11,
                attrs(&[
                    ("classname", "random_box"),
                    ("entityname", "rb2"),
                    ("boxnumber", "1"),
                    ("boxweight", "3"),
                ]),
            ),
            (
                11,
                attrs(&[
                    ("classname", "gathering_rock"),
                    ("entityname", "rock_rare"),
                    ("RandomSpawn", "1"),
                ]),
            ),
            (
                11,
                attrs(&[
                    ("classname", "event_of_trial"),
                    ("targetname", "trial_1"),
                    ("EventProbability", "40"),
                    ("TrialMod_1", "5"),
                    ("TrialMod_2", "6"),
                    ("EffectiveGroup", "3"),
                ]),
            ),
            (
                12,
                attrs(&[
                    ("classname", "unique_npc_template_maker"),
                    ("TemplateName", "npc_gnoll chief"),
                    ("MaxNPCCount", "1"),
                ]),
            ),
            (
                12,
                attrs(&[
                    ("classname", "math_counter"),
                    ("targetname", "mc"),
                    ("max", "2"),
                ]),
            ),
            (
                13,
                attrs(&[("classname", "info_player_start"), ("origin", "1 2 3")]),
            ),
        ],
        named_props: vec!["barrel".to_owned()],
        named_prop_items: Vec::new(),
        named_prop_amounts: Vec::new(),
        settings: vec![(1, Some(1008), s("Tower"), s(""))],
        setting_conditions: vec![(1008, s("QuestPrefix"), s("quest_tower"))],
        monsters: vec![
            (11, s("npc_gnoll"), s("normal"), -1, 50, 300, 100),
            (12, s("npc_gnoll"), s("chief"), 1, 400, 900, 100),
            (13, s("npc_gnoll"), s("chief"), 2, 800, 1800, 100),
        ],
        monster_drops: vec![
            // The exported npc_gnoll table carries Regina's Sealed Power as a 90 %
            // absolute row behind DropRequiredTest; GetTable must leave it out.
            (
                11,
                None,
                1,
                s("combine_wpart1_lv90_regina_5_1"),
                900_000,
                true,
                false,
                false,
                s("DropRequiredTest"),
            ),
            (
                11,
                None,
                1,
                s("cloth_lvl1"),
                7000,
                false,
                false,
                false,
                None,
            ),
            (
                11,
                s("quest_other"),
                1,
                s("other_lvl1"),
                7000,
                false,
                false,
                false,
                None,
            ),
            (11, None, 1, s("rare_gem"), 3000, false, false, false, None),
            (11, None, 1, s("lance_only"), 9000, false, true, false, None),
        ],
        quest_difficulties: vec![(s("quest_a"), 1, 1, 2, None)],
        drop_classes: vec![
            (s("cloth_lvl1"), 1, 2047, None),
            (s("rare_gem"), 6, 2047, None),
            (s("lance_only"), 1, 4, None),
        ],
        prop_tables: vec![(s("quest_a"), s("ruin"), None)],
        prop_items: vec![
            (1, s("ruin"), s("WOOD"), s("INSTANT"), s("hp 65"), 700, None),
            (
                2,
                s("ruin"),
                s("WOOD"),
                s("SERVER"),
                s("life_erg_crystal"),
                300,
                None,
            ),
            (
                3,
                s("ruin"),
                s("WOOD"),
                s("SERVER"),
                s("off_erg"),
                999,
                s("Off"),
            ),
        ],
        prop_expectations: vec![(s("ruin"), s("WOOD"), 1)],
        material_groups: vec![(4, s("WOOD"))],
        effects: vec![
            (
                s("phoenix_feather"),
                s("PlayerRevived"),
                s("TargetDead"),
                1,
                vec![
                    s("ReviveByClient 50"),
                    None,
                    None,
                    s("GetStatusByClient 1 revive_invincible_normal"),
                    None,
                ],
            ),
            (
                s("elixir"),
                s("useconsumable"),
                s("Self"),
                1,
                vec![s("GainHealth 30"), s("Bogus 1")],
            ),
            (
                s("elixir"),
                s("UseConsumable"),
                s("None"),
                0,
                vec![s("ReduceMinClearTime 5")],
            ),
        ],
        goal_infos: vec![
            GoalInfo {
                id: 1,
                target_count: Some(1),
                is_positive: 1,
                is_party: 1,
                gold: 100,
                xp: 2000,
                max_ap: 10,
                min_ap: 2,
                step_ap: 3,
                ap_regen_time: 60,
                ap_regen_max: 8,
                item: s("reward_box;bonus_box"),
                item_num: 2,
                item_multiplier_feature: s("Gathering"),
                item_multiplier: 3,
                weight: 50,
            },
            GoalInfo {
                id: 2,
                target_count: Some(2),
                is_positive: 1,
                xp: 500,
                gold: 40,
                max_ap: 4,
                min_ap: 4,
                item: s("sub_item"),
                item_num: 1,
                weight: 20,
                ..GoalInfo::default()
            },
            GoalInfo {
                id: 3,
                target_count: Some(1),
                is_party: 1,
                xp: 300,
                weight: 10,
                ..GoalInfo::default()
            },
            GoalInfo {
                id: 4,
                target_count: Some(1),
                is_party: 1,
                ..GoalInfo::default()
            },
            GoalInfo {
                id: 7,
                ..GoalInfo::default()
            },
            GoalInfo {
                id: 8,
                is_party: 1,
                ..GoalInfo::default()
            },
            GoalInfo {
                id: 9,
                ..GoalInfo::default()
            },
        ],
        difficulty_rewards: vec![(s("quest_a"), 1, 15000, 5000, 40, None)],
        quest_rewards: vec![
            (s("quest_a"), Some(3), true),
            (s("quest_tower"), Some(0), false),
        ],
    }
}

#[test]
fn digest_applies_the_swear_settings_and_item_limit() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let d = contents.digest("quest_a", -1).unwrap();
    assert_eq!(
        (
            d.quest_level,
            d.time_limit,
            d.item_count_min,
            d.item_count_max
        ),
        (3, 30, 4, 9)
    );
    assert_eq!((d.init_game_time, d.sector_move_game_time), (-1, 5));
    assert!(!d.solo_mode() && !d.hard_mode);
    let mut solo = contents.digest("quest_a", 7).unwrap();
    assert_eq!(
        (solo.solo_sector, solo.item_count_min, solo.item_count_max),
        (2, 2, 4)
    );
    let hard = contents.digest("quest_a", 8).unwrap();
    assert_eq!((hard.time_limit, hard.hard_mode), (12, true));
    contents.item_limit(&mut solo, 7, 1);
    assert_eq!(solo.item_limit, "potion_.*");
    contents.item_limit(&mut solo, 7, 2);
    assert_eq!(solo.item_limit, "");
    assert_eq!(
        contents.check_supported("quest_tower", 1),
        Err(MissionError::Unsupported)
    );
    assert!(contents.check_supported("quest_a", 1).is_ok());
    assert!(contents.builder("quest_tower").is_none());
}

#[test]
fn sequence_info_carries_entity_strings_connections_and_trials() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let builder = contents.builder("quest_a").unwrap();
    // Without SectorGroupInfo rows, AddBaseConnection links consecutive groups.
    assert_eq!(builder.groups.len(), 2);
    assert_eq!(builder.groups[0].1[0].connections.len(), 1);
    let digest = contents.digest("quest_a", -1).unwrap();
    let mut random = Script {
        // Group seeds: 50 % 100 selects 01b (30 ≤ 50 < 100); group 2 has one sector.
        next: [50, 0].into(),
        // Sector 01b: DecideSpawnMonster, trial roll (30 - 40 ≤ 0 sends to the host),
        // trial mod index 1; sector 01_boss: DecideSpawnMonster; then the gathering
        // pick and the random box roll (2: past rb1's weight 2, inside rb2).
        range: [0, 30, 1, 0, 0, 2].into(),
        // Spawner probability 0.7 ≥ 0.5 keeps it; boss spawner has no probability.
        double: [0.5].into(),
    };
    let sequence = contents
        .sector_sequence(&digest, 1, false, false, &mut random)
        .unwrap();
    let info = contents.info("quest_a", &sequence, false).unwrap();
    assert_eq!((info.current_group, info.starting_group), (-1, 1));
    assert_eq!(info.sectors.len(), 2);
    let a = &info.sectors[0];
    assert_eq!((a.group_id, a.bsp.as_deref()), (1, Some("01b")));
    // Base string: the trigger entity, CRLF, then a blank line per entity.
    assert!(a.entities[0].starts_with("{\r\n\t\"classname\" \"trigger_once\"\r\n"));
    assert!(a.entities[0].ends_with("}\r\n\r\n"));
    let spawner = a
        .entities
        .iter()
        .find(|e| e.contains("NPCTargetname"))
        .unwrap();
    assert!(spawner.starts_with("{\r\n\t\"templateName\" \"npc_gnoll normal\"\r\n"));
    assert!(spawner.contains("\t\"CurrentSpawnCount\" \"4\"\r\n\t\"RespawnSpawnCount\" \"4\"\r\n\t\"RespawnTime\" \"300\"\r\n"));
    assert!(!spawner.contains("probability") && !spawner.contains("TemplateName"));
    assert!(spawner.contains("\t\"StartDisabled\" \"1\"\r\n"));
    // Named props are unique per sector; the second barrel gets a suffix.
    assert!(
        a.entities
            .iter()
            .any(|e| e.ends_with("\t\"entityname\" \"barrel\"\r\n}\r\n"))
    );
    assert!(
        a.entities
            .iter()
            .any(|e| e.contains("\"entityname\" \"barrel_\""))
    );
    // The disabled-feature spawner and the weighted-out random box are absent.
    assert_eq!(
        a.entities
            .iter()
            .filter(|e| e.contains("NPCTargetname"))
            .count(),
        1
    );
    assert!(a.entities.iter().any(|e| e.contains("\"rb2\"")));
    assert!(!a.entities.iter().any(|e| e.contains("\"rb1\"")));
    assert!(a.entities.iter().any(|e| e.contains("\"rock_rare\"")));
    assert_eq!(a.trial_factors, [(6, 3, Some("trial_1".to_owned()), 1)]);
    assert_eq!(a.treasure_boxes, [(1, Some("box1".to_owned()))]);
    assert_eq!(a.connections[0].to, 2);
    let boss = &info.sectors[1];
    assert!(boss.entities[0].contains("\"math_counter\""));
    let target = boss
        .entities
        .iter()
        .find(|e| e.contains("NPCTargetname"))
        .unwrap();
    // The goal target regex marks the boss spawner.
    assert!(target.contains("/Target\"\r\n"));
    assert!(target.contains("\t\"RespawnSpawnCount\" \"0\"\r\n"));
}

#[test]
fn solo_and_practice_modes_follow_the_original_branches() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let solo = contents.digest("quest_a", 7).unwrap();
    let mut random = Script::default();
    let sequence = contents
        .sector_sequence(&solo, 1, true, false, &mut random)
        .unwrap();
    let info = contents.info("quest_a", &sequence, true).unwrap();
    assert_eq!(info.starting_group, 2);
    assert_eq!(info.sectors.len(), 1);
    assert_eq!(info.sectors[0].group_id, 2);
    // Practice mode keeps spawners but drops treasure/named/gathering/random-box strings.
    let digest = contents.digest("quest_a", -1).unwrap();
    let mut random = Script {
        next: [99, 0].into(),
        ..Script::default()
    };
    let sequence = contents
        .sector_sequence(&digest, 1, true, false, &mut random)
        .unwrap();
    let info = contents.info("quest_a", &sequence, true).unwrap();
    assert!(
        !info.sectors[0]
            .entities
            .iter()
            .any(|e| e.contains("entityname"))
    );
}

fn item(class: &str, stack: i32, sub: bool, usable: i32) -> SlotSource {
    SlotSource::Item {
        info: Some(ItemSlotInfo {
            class: class.to_owned(),
            sub_weapon: sub,
            quick_slot_max_stack: stack,
            draft: 2,
            alchemist_box: false,
        }),
        usable,
    }
}

#[test]
fn battle_inventory_caps_shares_counts_and_empties_like_add_player() {
    let settings = vec![
        ("SubWeaponMultiplier".to_owned(), "2".to_owned()),
        ("TutorStamp".to_owned(), "Basic".to_owned()),
    ];
    assert_eq!(
        multiplier(&settings, "SubWeaponMultiplier", "SetSubWeaponLimit"),
        Ok(2.0)
    );
    assert_eq!(
        multiplier(&settings, "QuickSlotMultiplier", "SetQuickSlotLimit"),
        Ok(1.0)
    );
    let limited = vec![("SetQuickSlotLimit".to_owned(), "3".to_owned())];
    assert_eq!(
        multiplier(&limited, "QuickSlotMultiplier", "SetQuickSlotLimit"),
        Ok(0.0)
    );
    let limit = regex::RegexBuilder::new("elixir")
        .case_insensitive(true)
        .build()
        .unwrap();
    let rows = battle_inventory(
        vec![
            (100, "potion".to_owned(), item("potion", 5, false, 7)),
            // The same class shares the loaded count: only 2 of 7 remain.
            (101, "potion".to_owned(), item("potion", 5, false, 7)),
            (102, "javelin".to_owned(), item("javelin", 3, true, 10)),
            (103, "ELIXIR".to_owned(), item("ELIXIR", 5, false, 4)),
            (
                104,
                "unknown".to_owned(),
                SlotSource::Item {
                    info: None,
                    usable: 0,
                },
            ),
            (
                105,
                "skill_guard".to_owned(),
                SlotSource::Skill {
                    info: Some(true),
                    rank: Some(3),
                },
            ),
            (
                106,
                "skill_passive".to_owned(),
                SlotSource::Skill {
                    info: Some(false),
                    rank: Some(1),
                },
            ),
            (107, "none".to_owned(), item("none", 5, false, 0)),
        ],
        1.0,
        2.0,
        Some(&limit),
    )
    .unwrap();
    let c = |class: &str, bring, draft| Consumable {
        class: class.to_owned(),
        bring,
        draft,
    };
    // SetConsumable on a .NET Dictionary: the second potion slot evicts part 100 (same
    // class) and reuses its entry; the limited ELIXIR becomes an empty consumable that the
    // later empty slot 107 evicts and replaces in the same entry, after skill_guard.
    assert_eq!(
        rows,
        [
            (101, c("potion", 2, 2)),
            (102, c("javelin", 6, 2)),
            (107, c("", 0, 0)),
            (105, c("skill_guard", 3, 0)),
        ]
    );
    assert_eq!(
        battle_inventory(
            vec![(
                1,
                "skill_x".to_owned(),
                SlotSource::Skill {
                    info: None,
                    rank: Some(1)
                }
            )],
            1.0,
            1.0,
            None,
        ),
        Err(MissionError::Content)
    );
}

#[test]
fn move_next_follows_the_starting_group_then_trigger_connections() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let digest = contents.digest("quest_a", -1).unwrap();
    let mut sequence = contents
        .sector_sequence(&digest, 1, true, false, &mut Script::default())
        .unwrap();
    // The first move must target the starting group (or -1).
    assert!(!contents.move_next("quest_a", &mut sequence, "", 2).unwrap());
    assert_eq!(sequence.current, None);
    assert!(
        contents
            .move_next("quest_a", &mut sequence, "", -1)
            .unwrap()
    );
    assert_eq!(
        (sequence.current, sequence.last_used_spawn_point.as_deref()),
        (Some(1), Some(""))
    );
    // An unknown trigger falls back to the "" connection of AddBaseConnection.
    assert!(
        !contents
            .move_next("quest_a", &mut sequence, "exit_x", 3)
            .unwrap()
    );
    assert!(
        contents
            .move_next("quest_a", &mut sequence, "exit_x", 2)
            .unwrap()
    );
    assert_eq!((sequence.current, sequence.latest_cleared), (Some(2), 0));
    // The last sector has no connection.
    assert!(
        !contents
            .move_next("quest_a", &mut sequence, "", -1)
            .unwrap()
    );
    assert_eq!(sequence.passed.len(), 2);
}

#[test]
fn monster_kills_count_once_and_accumulate_monster_exp() {
    use vindictus_server::domain::mission::{Kill, monster_kill_exp};
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    // GetMonsterInfo: a single row answers every difficulty; "class variation weight"
    // drops the weight; a missing difficulty falls back to -1 (absent here).
    let normal = contents.monster_info("npc_gnoll normal 30", 3).unwrap();
    assert_eq!(normal.identifier, 11);
    assert_eq!(
        contents
            .monster_info("npc_gnoll chief", 2)
            .unwrap()
            .identifier,
        13
    );
    assert_eq!(
        contents
            .monster_info("npc_gnoll chief", -1)
            .unwrap()
            .identifier,
        12
    );
    assert!(contents.monster_info("npc_gnoll chief", 5).is_none());
    // 50 × 100/100 × 1.0 × 140 / 100 = 70, then ExpRate_Monster 100.
    assert_eq!(monster_kill_exp(normal, 1, 100), 70);
    assert_eq!(monster_kill_exp(normal, 2, 100), 56);
    assert_eq!(monster_kill_exp(normal, 6, 50), 0);
    let digest = contents.digest("quest_a", -1).unwrap();
    let mut sequence = contents
        .sector_sequence(&digest, 1, true, false, &mut Script::default())
        .unwrap();
    assert_eq!(contents.kill(&mut sequence, 1), Kill::NotInSector);
    assert!(contents.move_next("quest_a", &mut sequence, "", 1).unwrap());
    assert!(contents.move_next("quest_a", &mut sequence, "", 2).unwrap());
    let boss = contents
        .entities()
        .iter()
        .find(|e| e.get("TemplateName") == Some("npc_gnoll chief"))
        .unwrap()
        .id;
    assert_eq!(contents.kill(&mut sequence, boss + 1000), Kill::Unknown);
    let Kill::Killed {
        monster_type,
        is_boss,
        all_dead,
        ..
    } = contents.kill(&mut sequence, boss)
    else {
        panic!("boss kill")
    };
    assert_eq!(
        (monster_type.as_str(), is_boss, all_dead),
        ("npc_gnoll chief", true, true)
    );
    assert_eq!(contents.kill(&mut sequence, boss), Kill::AlreadyDead);
}

#[test]
fn drops_mark_item_drop_entities_and_cores_loot_like_evil_core() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let digest = contents.digest("quest_a", -1).unwrap();
    // Seeds pick 01b; the 0.7-probability spawner stays (0.5); every range draws its minimum,
    // so ItemCountMin (4) is capped to the single candidate (the normal gnoll, base drop 0
    // with a MonsterDropTable at difficulty 1).
    let mut random = Script {
        next: [50, 0].into(),
        ..Script::default()
    };
    let mut sequence = contents
        .sector_sequence(&digest, 1, false, false, &mut random)
        .unwrap();
    let info = contents.info("quest_a", &sequence, false).unwrap();
    let normal = contents
        .entities()
        .iter()
        .find(|e| e.get("TemplateName") == Some("npc_gnoll normal"))
        .unwrap()
        .id;
    assert_eq!(info.sectors[0].item_drop_entity, [(normal.to_string(), 1)]);
    // The boss has no base drop and no QuestDifficultyInfo boost.
    assert!(info.sectors[1].item_drop_entity.is_empty());
    assert!(contents.move_next("quest_a", &mut sequence, "", 1).unwrap());
    // Relational weights skip other quests' rows and the required class the Fiona
    // cannot use: roll 0 picks cloth_lvl1; without LuckyBonus there is no gold core.
    let extra = contents
        .make_evil_core(
            &mut sequence,
            &normal.to_string(),
            1,
            contents.monster_info("npc_gnoll normal", 1),
            false,
            "quest_a",
            1,
            150,
            &mut Script::default(),
        )
        .unwrap();
    assert!(extra.is_empty());
    let mut rare_roll = Script {
        range: [7000].into(),
        ..Script::default()
    };
    contents
        .make_evil_core(
            &mut sequence,
            &normal.to_string(),
            1,
            contents.monster_info("npc_gnoll normal", 1),
            false,
            "quest_a",
            1,
            150,
            &mut rare_roll,
        )
        .unwrap();
    assert_eq!(
        sequence.cores[0].items,
        [(1, vec!["cloth_lvl1".to_owned(), "rare_gem".to_owned()])]
    );
    // A rare kick takes the rare core; a normal kick the normal one; then nothing is left.
    // RareCoreEffectRarity is 0 in this policy: no core is rare.
    assert!(!contents.is_rare_core("rare_gem"));
    assert_eq!(
        contents
            .loot(&mut sequence, &normal.to_string(), 1, false)
            .unwrap()
            .as_deref(),
        Some("cloth_lvl1")
    );
    assert_eq!(
        contents
            .loot(&mut sequence, &normal.to_string(), 1, false)
            .unwrap()
            .as_deref(),
        Some("rare_gem")
    );
    assert_eq!(
        contents
            .loot(&mut sequence, &normal.to_string(), 1, false)
            .unwrap(),
        None
    );
    assert_eq!(
        contents.loot(&mut sequence, "unknown", 1, false).unwrap(),
        None
    );
}

#[test]
fn props_carry_erg_expectations_and_gold_then_break_into_pickable_ergs() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let digest = contents.digest("quest_a", -1).unwrap();
    let mut sequence = contents
        .sector_sequence(
            &digest,
            1,
            true,
            false,
            &mut Script {
                next: [50, 0].into(),
                ..Script::default()
            },
        )
        .unwrap();
    assert!(contents.move_next("quest_a", &mut sequence, "", 1).unwrap());
    // No shuffle movement (every range draws its minimum): prop 5 (wood) takes the single
    // WOOD erg expectation; GoldDropTable(100, 10) gives 2·2+1 = 5 amounts 8..=12 for
    // ⌊100/10⌋/5 = 2 props each, so props 6 and 7 get 8 gold.
    contents
        .register_props(
            "quest_a",
            &mut sequence,
            &[(5, 4), (6, 9), (7, 9)],
            false,
            &mut Script::default(),
        )
        .unwrap();
    let sector = &sequence.sectors[0];
    assert_eq!(
        sector
            .props
            .iter()
            .map(|p| (p.id, p.group.clone(), p.gold))
            .collect::<Vec<_>>(),
        [(5, Some("WOOD".to_owned()), 0), (6, None, 8), (7, None, 8)]
    );
    // Roll 700 skips the INSTANT hp row (700) and lands on the server erg; the disabled
    // feature row neither weighs nor drops.
    let ergs = contents
        .break_prop(
            "quest_a",
            &mut sequence,
            5,
            0,
            false,
            &mut Script {
                range: [700].into(),
                ..Script::default()
            },
        )
        .unwrap();
    assert_eq!(
        (
            ergs[0].class.as_str(),
            ergs[0].kind.as_str(),
            ergs[0].id,
            ergs[0].winner
        ),
        ("SERVER", "life_erg_crystal", 0, 0)
    );
    let gold = contents
        .break_prop(
            "quest_a",
            &mut sequence,
            6,
            0,
            false,
            &mut Script::default(),
        )
        .unwrap();
    assert_eq!(
        (gold[0].class.as_str(), gold[0].amount, gold[0].id),
        ("GOLD", 8, 1)
    );
    assert!(
        contents
            .break_prop(
                "quest_a",
                &mut sequence,
                6,
                0,
                false,
                &mut Script::default()
            )
            .is_none()
    );
    // Only the winner picks, once.
    assert!(contents.pick_erg(&mut sequence, 1, 3).is_none());
    assert_eq!(contents.pick_erg(&mut sequence, 1, 0).unwrap().amount, 8);
    assert!(contents.pick_erg(&mut sequence, 1, 0).is_none());
    // The second gold prop.
    let instant = contents
        .break_prop(
            "quest_a",
            &mut sequence,
            7,
            0,
            false,
            &mut Script::default(),
        )
        .unwrap();
    assert_eq!(instant[0].class, "GOLD");
}

#[test]
fn micro_play_effects_parse_like_the_element_loader() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let feather = contents.effects("phoenix_feather");
    assert_eq!((feather[0].operation, feather[0].target), (2, 2));
    assert_eq!(
        feather[0].effects,
        [
            (2, 50, None),
            (8, 1, Some("revive_invincible_normal".to_owned()))
        ]
    );
    // Case-insensitive enums; an unknown effect type is None (0).
    let elixir = contents.effects("elixir");
    assert_eq!((elixir[0].operation, elixir[0].effects[1].0), (1, 0));
    assert_eq!(elixir[1].effects, [(16, 5, None)]);
    assert!(contents.effects("missing").is_empty());
    let mut bad = tables();
    bad.effects.push((
        s("x"),
        s("UseConsumable"),
        s("Self"),
        1,
        vec![s("Revive many")],
    ));
    assert!(MissionContents::load(bad, &policy()).is_err());
}

#[test]
fn target_tracker_triggers_like_the_original() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let d = contents.digest("quest_a", -1).unwrap();
    assert_eq!((d.min_clear_time, d.is_free_quest), (3, true));
    let mut missing = tables();
    missing.quest_rewards[0].1 = None;
    let missing = MissionContents::load(missing, &policy()).unwrap();
    assert!(missing.digest("quest_a", -1).is_err());
    // Swear 8 has game settings: it becomes the setting swear, not a tracked target.
    let mut t: TargetTracker = contents.tracker("quest_a", 8, &[77]).unwrap();
    assert_eq!(t.essential_left, [1]);
    assert_eq!(t.setting_swear, Some((8, 2, true)));
    assert!(!t.is_single_boss);
    // The conditional pattern is rejected by the engine and never matches.
    assert_eq!(t.uncompiled, 1);
    // Patterns are case-sensitive.
    assert!(t.process(77, "kill npc_gnoll CHIEF").is_empty());
    let boss = t.process(5, "kill npc_gnoll chief");
    assert_eq!(
        boss,
        [GoalTrigger {
            cid: -1,
            goal: 1,
            relation: 0,
            is_party: true,
            success: true,
            main_goal_achieved: true
        }]
    );
    // Individual goals count only for their player and trigger once at the target.
    assert!(t.process(5, "get item potion").is_empty());
    assert!(t.process(77, "get item potion").is_empty());
    let sub = t.process(77, "get item potion_big");
    assert_eq!((sub[0].cid, sub[0].goal, sub[0].success), (77, 2, true));
    assert!(t.process(77, "get item potion").is_empty());
    // OnQuestFinished: the unmet negative goal succeeds, then the party swear.
    let finished = t.on_quest_finished();
    let ids: Vec<(i64, i32, bool)> = finished
        .iter()
        .map(|f| (f.cid, f.goal, f.success))
        .collect();
    assert_eq!(ids, [(-1, 3, true), (-1, 4, true), (-1, 8, true)]);
    // A negative goal reached fails at once.
    let dead = t.process(77, "player_dead");
    assert_eq!((dead[0].goal, dead[0].success), (3, false));
    assert!(t.on_quest_finished().iter().all(|f| f.goal != 3));
    // A tracked target without TargetCount throws in the original constructor.
    let mut bad = tables();
    bad.goal_infos[1].target_count = None;
    let bad = MissionContents::load(bad, &policy()).unwrap();
    assert!(bad.tracker("quest_a", -1, &[77]).is_err());
}

#[test]
fn goal_rewards_scale_ratios_and_regenerate_ap() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let info = contents.goal_info(1).unwrap();
    // XP × GoalExpRatio / 10000 × 100 × ExpRate_Combat / 100 (integer steps).
    let r = goal::goal_reward(info, (15000, 5000), (100, 50, 100), None, false, 1_000);
    assert_eq!((r.exp, r.gold, r.ap), (300_000, 2_500, 10));
    assert_eq!(r.item.as_deref(), Some("reward_box;bonus_box"));
    // Achieved before: MaxAP − StepAP × ClearCount floored at MinAP plus the regeneration
    // capped by APRegenMax, then APRate_Combat.
    let minutes = |m: i64| m * 60 * 300;
    let achieved = Achieved {
        clear_count: 2,
        regened_ap: 1,
        last_clear: 0,
    };
    let r = goal::goal_reward(
        info,
        (15000, 5000),
        (100, 100, 50),
        Some(achieved),
        false,
        minutes(125),
    );
    // base 10 − 6 = 4, regen 1 + 2 = 3 → 7, × 50 %.
    assert_eq!(r.ap, 3);
    // A today quest takes MaxAP without the AP rate.
    let r = goal::goal_reward(
        info,
        (15000, 5000),
        (100, 100, 50),
        Some(achieved),
        true,
        minutes(125),
    );
    assert_eq!(r.ap, 10);
    assert_eq!(info.ap(5, 0, 0.0), 2);
    // AddClearCount: the regenerated AP is banked and the remainder kept on the time.
    let (mut count, mut regen, mut last) = (1_u8, 0_u8, 0_i64);
    info.add_clear_count(minutes(150), &mut count, &mut regen, &mut last);
    assert_eq!((count, regen, last), (2, 1, minutes(120)));
    // At MinAP the clear count stays and the banked AP pays the step.
    let (mut count, mut regen, mut last) = (3_u8, 5_u8, 0_i64);
    info.add_clear_count(minutes(10), &mut count, &mut regen, &mut last);
    // The time stays on the last regeneration boundary.
    assert_eq!((count, regen, last), (3, 2, 0));
    // Empty items and zero counts carry no item.
    let sub = goal::goal_reward(
        contents.goal_info(3).unwrap(),
        (10000, 10000),
        (100, 100, 100),
        None,
        false,
        0,
    );
    assert_eq!((sub.item, sub.item_num, sub.exp), (None, 0, 30_000));
}

#[test]
fn reward_calculation_sums_goals_multipliers_and_penalties() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let rate = (100, 100, 100);
    let mut main = vec![goal::goal_reward(
        contents.goal_info(1).unwrap(),
        (100, 100),
        rate,
        None,
        false,
        7,
    )];
    let mut sub = vec![goal::goal_reward(
        contents.goal_info(2).unwrap(),
        (10000, 10000),
        rate,
        None,
        false,
        8,
    )];
    let mut facts = RewardFacts {
        quest_id: "quest_a".into(),
        level: 30,
        max_level: 90,
        quest_level: 3,
        difficulty: 2,
        game_mode: 1,
        main_goal_achieved: true,
        extreme_mode: true,
        xe_play_rank: 2,
        ..RewardFacts::default()
    };
    let mut lists = RewardLists::new(&facts);
    // Quest level 3 < 30 × 0.87 at level 30: the 0 % level penalty.
    assert_eq!(lists.level_gold_penalty.as_ref().map(|p| p.value), Some(0));
    lists.add_bonuses(&facts);
    let enabled = |f: &str| f == "Gathering";
    let c = goal::calculate(true, &mut main, &mut sub, 1, 1000, &lists, &enabled).unwrap();
    // Goals: exp 2000 + 50000, main gold 100, sub gold 4000, ap 10 + 4.
    // EXP: (1000 + 52000) + XERank 50 % + DifficultyBonus 30 %.
    assert_eq!(c.exp, 53000 + 26500 + 15900);
    // Gold: 4100 + XERank 1 % + Difficulty 30 %, then the 0 % level penalty.
    assert_eq!(c.gold, 0);
    assert_eq!(c.ap, 14);
    assert_eq!(c.quest_goals, [(1, 7), (2, 8)]);
    assert_eq!(
        c.items,
        [
            ("reward_box".to_owned(), 6),
            ("bonus_box".to_owned(), 6),
            ("sub_item".to_owned(), 1)
        ]
    );
    assert_eq!(
        c.summary.cleared_goals,
        [(1, (2000, 100, 10)), (2, (50000, 4000, 4))]
    );
    assert_eq!(c.summary.battle_exp[0].desc, "MonsterKill");
    let etc_gold: Vec<(&str, i32)> = c
        .summary
        .etc_gold
        .iter()
        .map(|l| (l.desc.as_str(), l.value))
        .collect();
    assert_eq!(
        etc_gold,
        [
            ("XERankBonus", 41),
            ("DifficultyBonus", 1230),
            ("LevelPenalty", -5371)
        ]
    );
    // Goals already sent are not sent again; without the main goal only monsters count.
    let again = goal::calculate(
        true,
        &mut main,
        &mut sub,
        1,
        0,
        &RewardLists::default(),
        &enabled,
    )
    .unwrap();
    assert!(again.quest_goals.is_empty());
    let failed = goal::calculate(
        false,
        &mut main,
        &mut sub,
        1,
        70,
        &RewardLists::default(),
        &enabled,
    )
    .unwrap();
    assert_eq!((failed.exp, failed.gold, failed.ap), (70, 0, 0));
    assert!(failed.summary.cleared_goals.is_empty());
    // Practice of the camping penalty: today play count above 10 on a free quest.
    facts.free_quest = true;
    facts.today_play_count = 13;
    assert_eq!(
        RewardLists::new(&facts)
            .camping_gold_penalty
            .map(|p| p.value),
        Some(70)
    );
    // Progress: goals of this play count, else stored statuses (season 1: Status > 0).
    let goals = [(1, 50), (2, 20), (3, 10)];
    assert_eq!(
        goal::quest_progress(&goals, &[(3, 1)], Some(&[1]), false, 1),
        60
    );
    assert_eq!(
        goal::quest_progress(&goals, &[(3, 1), (2, 4)], None, true, 1),
        0
    );
    assert_eq!(goal::quest_progress(&goals, &[(2, 6)], None, true, 1), 20);
}

#[test]
fn story_sectors_keep_their_entity_strings() {
    let contents = MissionContents::load(tables(), &policy()).unwrap();
    let (bsp, entity) = contents.story_sector(13).unwrap();
    assert_eq!(bsp, "story");
    assert!(entity.contains("info_player_start"));
    assert!(contents.story_sector(10).is_none());
    // Story entities follow the sequence builders' entities.
    let last = contents.entities().last().unwrap();
    assert_eq!(last.get("origin"), Some("1 2 3"));
}

#[test]
fn named_props_break_into_named_drop_table_ergs() {
    let mut t = tables();
    // NamedPropItemInfo of "barrel" difficulty 1: an INSTANT heal, a server erg and a
    // disabled-feature row that neither weighs nor drops; 2..=3 ergs per break.
    t.named_prop_items = vec![
        (s("barrel"), 1, s("instant"), s("hp 50"), 100, None),
        (s("barrel"), 1, s("server"), s("erg_crystal"), 300, None),
        (s("barrel"), 1, s("server"), s("off_item"), 500, s("Off")),
    ];
    t.named_prop_amounts = vec![(s("barrel"), 2, 3)];
    let contents = MissionContents::load(t, &policy()).unwrap();
    let digest = contents.digest("quest_a", -1).unwrap();
    let mut sequence = contents
        .sector_sequence(
            &digest,
            1,
            false,
            false,
            &mut Script {
                next: [50, 0].into(),
                ..Script::default()
            },
        )
        .unwrap();
    assert!(contents.move_next("quest_a", &mut sequence, "", 1).unwrap());
    // Count 3 (Random.Next(2, 4)); rolls 50 (heal), 150 and 399 (server erg, total 400).
    let mut random = Script {
        range: [3, 50, 150, 399].into(),
        ..Script::default()
    };
    // Difficulty 99 uses the difficulty 1 table; the unique "barrel_" keeps "barrel".
    let ergs = contents
        .break_named_prop(&mut sequence, "barrel_", 0, 99, &mut random)
        .unwrap();
    assert_eq!(
        ergs.iter()
            .map(|e| (e.class.as_str(), e.kind.as_str(), e.amount, e.winner))
            .collect::<Vec<_>>(),
        [
            ("INSTANT", "HP", 50, -1),
            ("SERVER", "erg_crystal", 1, 0),
            ("SERVER", "erg_crystal", 1, 0)
        ]
    );
    // Only the server ergs become pickable sector ergs, with consecutive IDs.
    let picked: Vec<i32> = sequence.sectors[0].ergs.iter().map(|e| e.id).collect();
    assert_eq!(picked.len(), 2);
    assert_eq!(picked[1], picked[0] + 1);
    assert_eq!(ergs[1].id, picked[0]);
    // The prop left the sector; an unknown name is not a named prop.
    assert!(
        contents
            .break_named_prop(&mut sequence, "barrel_", 0, 1, &mut Script::default())
            .is_none()
    );
    assert!(
        contents
            .break_named_prop(&mut sequence, "crate", 0, 1, &mut Script::default())
            .is_none()
    );
}
