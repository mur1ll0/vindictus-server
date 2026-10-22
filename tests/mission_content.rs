use std::collections::VecDeque;
use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    mission::{MissionContents, MissionError, MissionRandom, MissionTables},
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
        .sector_sequence(&digest, false, false, &mut random)
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
        .sector_sequence(&solo, true, false, &mut random)
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
        .sector_sequence(&digest, true, false, &mut random)
        .unwrap();
    let info = contents.info("quest_a", &sequence, true).unwrap();
    assert!(
        !info.sectors[0]
            .entities
            .iter()
            .any(|e| e.contains("entityname"))
    );
}
