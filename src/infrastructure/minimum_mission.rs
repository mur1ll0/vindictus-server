//! Mission entry composition: the QueryMicroPlayInfo(InitItemDropEntities) answer as a
//! GameStartGranted message, from the digest and the generated sector sequence.
use crate::domain::mission::{
    MissionContents, MissionError, MissionRandom, QuestDigest, SectorInfos,
};
use crate::protocol::mission::{
    GameStartGranted, QuestSector, QuestSectors, SectorConnection, StrDict, TreasureBox,
    TrialFactor,
};
use rand_core::{OsRng, RngCore};

/// Uniform operating-system randomness for the unseeded System.Random of the builder.
pub struct OsMissionRandom;
impl MissionRandom for OsMissionRandom {
    fn next(&mut self) -> i32 {
        loop {
            let v = (OsRng.next_u32() >> 1) as i32;
            if v != i32::MAX {
                return v;
            }
        }
    }
    fn next_range(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = (i64::from(max) - i64::from(min)) as u64;
        // Rejection sampling keeps the draw uniform.
        let zone = u64::MAX - (u64::MAX % span);
        loop {
            let v = OsRng.next_u64();
            if v < zone {
                return (i64::from(min) + (v % span) as i64) as i32;
            }
        }
    }
    fn next_double(&mut self) -> f64 {
        (OsRng.next_u64() >> 11) as f64 * (1.0 / (1_u64 << 53) as f64)
    }
}

/// QuestSectorInfos on the wire (empty drop dictionaries, no altar or boss list entries).
pub fn sectors_wire(infos: &SectorInfos) -> QuestSectors {
    QuestSectors {
        current_group: infos.current_group,
        starting_group: infos.starting_group,
        sectors: Some(
            infos
                .sectors
                .iter()
                .map(|s| {
                    Some(QuestSector {
                        group_id: s.group_id,
                        bsp: s.bsp.clone(),
                        entities: Some(s.entities.iter().cloned().map(Some).collect()),
                        connections: Some(
                            s.connections
                                .iter()
                                .map(|c| {
                                    Some(SectorConnection {
                                        from: c.from,
                                        from_trigger: Some(c.from_trigger.clone()),
                                        to: c.to,
                                        to_spawn: Some(c.to_spawn.clone()),
                                    })
                                })
                                .collect(),
                        ),
                        item_drop_entity: StrDict(Some(Vec::new())),
                        weak_point_entity: StrDict(Some(Vec::new())),
                        trial_factors: Some(
                            s.trial_factors
                                .iter()
                                .map(|(m, g, name, sector)| {
                                    Some(TrialFactor {
                                        group_number: *g,
                                        trial_mod: *m,
                                        trial_name: name.clone(),
                                        sector_group_id: *sector,
                                    })
                                })
                                .collect(),
                        ),
                        treasure_boxes: Some(
                            s.treasure_boxes
                                .iter()
                                .map(|(g, name)| {
                                    Some(TreasureBox {
                                        group_id: *g,
                                        entity_name: name.clone(),
                                        is_visible: false,
                                        is_opened: false,
                                    })
                                })
                                .collect(),
                        ),
                        save_required: s.save_required,
                        last_used_spawn_point: None,
                    })
                })
                .collect(),
        ),
        treasure_box_max_count: 0,
        altar_status: Some(Vec::new()),
        boss_killed: Some(Vec::new()),
    }
}

/// BattleServiceProxy.StartGame (non-DS): GameStartGranted from QueryMicroPlayInfo.
pub fn game_start_granted(
    digest: &QuestDigest,
    infos: &SectorInfos,
    difficulty: i32,
    timer_decreasing: bool,
    player_count: i32,
) -> GameStartGranted {
    GameStartGranted {
        quest_level: digest.quest_level,
        quest_time: digest.time_limit,
        hard_mode: i32::from(digest.hard_mode),
        solo_sector: digest.solo_sector,
        sectors: Some(sectors_wire(infos)),
        is_hunting_quest: digest.is_hunting_quest,
        init_game_time: digest.init_game_time,
        sector_move_game_time: digest.sector_move_game_time,
        difficulty,
        is_timer_decreasing: timer_decreasing,
        quest_started_player_count: player_count,
        new_slot: -1,
        new_key: -1,
        is_user_dedicated: false,
    }
}

/// Digest, sequence and GameStartGranted of a solo start (no story drops, no today quest).
pub fn preview(
    contents: &MissionContents,
    quest: &str,
    swear_id: i32,
    difficulty: i32,
    practice: bool,
) -> Result<GameStartGranted, MissionError> {
    contents.check_supported(quest, difficulty)?;
    let mut digest = contents.digest(quest, swear_id)?;
    contents.item_limit(&mut digest, swear_id, difficulty);
    let sequence = contents.sector_sequence(&digest, practice, false, &mut OsMissionRandom)?;
    let infos = contents.info(quest, &sequence, practice)?;
    Ok(game_start_granted(&digest, &infos, difficulty, true, 1))
}
