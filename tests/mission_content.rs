use std::collections::VecDeque;
use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    mission::{
        Consumable, ItemSlotInfo, MissionContents, MissionError, MissionRandom, MissionTables,
        SlotSource, battle_inventory, multiplier,
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
        ],
        sectors: vec![
            (s("quest_a"), 1, 10, s("01a"), 30),
            (s("quest_a"), 1, 11, s("01b"), 70),
            (s("quest_a"), 2, 12, s("01_boss"), 100),
            (s("quest_a"), 0, 13, s("story"), 100),
            (s("quest_tower"), 1, 14, s("t"), 100),
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
        ],
        named_props: vec!["barrel".to_owned()],
        settings: vec![(1, Some(1008), s("Tower"), s(""))],
        setting_conditions: vec![(1008, s("QuestPrefix"), s("quest_tower"))],
        monsters: vec![
            (11, s("npc_gnoll"), s("normal"), -1, 50, 300, 100),
            (12, s("npc_gnoll"), s("chief"), 1, 400, 900, 100),
            (13, s("npc_gnoll"), s("chief"), 2, 800, 1800, 100),
        ],
        monster_drops: vec![
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
