//! Mission entry composition: the QueryMicroPlayInfo(InitItemDropEntities) answer as a
//! GameStartGranted message, from the digest and the generated sector sequence.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::domain::{
    character_stats::Arithmetic,
    mission::{
        self as domain, ItemSlotInfo, MissionContents, MissionError, MissionRandom, QuestDigest,
        SectorInfos, SlotSource,
    },
};
use crate::protocol::{
    inventory::Dict,
    mission::{
        BattleInventory, Consumable, CostumeField, GameStartGranted, JoinMember, NoPet,
        QuestSector, QuestSectors, SectorConnection, StrDict, TreasureBox, TrialFactor,
    },
};
use rand_core::{OsRng, RngCore};
use regex::Regex;
use sqlx::PgPool;

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

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    let (pairs, rest) = raw.as_chunks::<2>();
    if !rest.is_empty() {
        return Err(fail());
    }
    String::from_utf16(
        &pairs
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}

/// MicroPlayer.MakeMemberInfo for a player admitted by AddPlayer: WhoIs (summary,
/// stats, skills, equipment maps), QueryPlayerInfo (equipped items, status-effect
/// and ability lists) and the battle inventory. Vocations, SP-skill enhancements,
/// guilds, pets and stored status effects are outside the migrated profile and refused.
#[allow(clippy::too_many_arguments)]
pub async fn member_info(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    slot: i32,
    key: i32,
    item_limit: Option<&Regex>,
    settings: &[(String, String)],
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<JoinMember, MinimumError> {
    let mut tx = pool.begin().await?;
    let view =
        super::minimum_selection::view(&mut tx, owner, assets, cid, config, arithmetic, false, 0)
            .await?;
    let vocation: Option<i32> =
        sqlx::query_scalar("SELECT vocation_class FROM game_vocations WHERE cid=$1")
            .bind(cid)
            .fetch_optional(&mut *tx)
            .await?;
    let effects: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_status_effects WHERE cid=$1")
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
    if vocation.is_some() || effects != 0 {
        return Err(fail());
    }
    // SkillManager.ToDictionary: base ranks without item skill ranks (refused by view).
    let skills: Vec<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT skill_utf16,rank FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    let sp: Vec<(i32, Option<Vec<u8>>)> = sqlx::query_as(
        "SELECT slot_id,skill_utf16 FROM game_sp_skills WHERE cid=$1 ORDER BY row_id LIMIT 257",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    if skills.len() > 4096 || sp.len() > 256 {
        return Err(fail());
    }
    let skills = skills
        .into_iter()
        .map(|(id, rank)| Ok((text(&id)?, rank)))
        .collect::<Result<Vec<_>, MinimumError>>()?;
    let mut sp_skills: Vec<(i32, Option<String>)> = Vec::new();
    for (slot_id, id) in sp {
        if sp_skills.iter().any(|(s, _)| *s == slot_id) {
            return Err(fail());
        }
        sp_skills.push((slot_id, id.map(|i| text(&i)).transpose()?));
    }
    // Gear.ToEquippedItemInfos: part → ItemClass of non-empty classes.
    let mut equipped: Vec<(i32, Option<String>)> = Vec::new();
    for (part, id) in &view.equipment {
        let class: Vec<u8> = sqlx::query_scalar(
            "SELECT item_class_utf16 FROM game_items WHERE id=$1 AND owner_id=$2",
        )
        .bind(id)
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
        let class = text(&class)?;
        if !class.is_empty() && !equipped.iter().any(|(p, _)| p == part) {
            equipped.push((*part, Some(class)));
        }
    }
    let quick = super::minimum_quickslot::load(&mut tx, assets, cid).await?;
    let held: Vec<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT item_class_utf16,count FROM game_items WHERE owner_id=$1 ORDER BY id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let contents = assets.skills.as_ref().ok_or_else(fail)?;
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let sub_weapon = utf16("SUBWEAPON");
    let mut slots = Vec::new();
    for (part, class) in super::minimum_quickslot::info(&quick)
        .classes
        .0
        .unwrap_or_default()
    {
        let Some(class) = class else { continue };
        let source = if class.starts_with("skill_") {
            SlotSource::Skill {
                info: contents
                    .get(&class)
                    .map(|c| c.use_type.is_some_and(|u| u != 0)),
                rank: skills.iter().find(|(id, _)| *id == class).map(|(_, r)| *r),
            }
        } else {
            let bytes = utf16(&class);
            let info = match assets.stats_index.class(Some(&bytes)) {
                None => None,
                Some(indexed) => {
                    let e = economy.get(indexed.source_index).ok_or_else(fail)?;
                    Some(ItemSlotInfo {
                        class: class.clone(),
                        sub_weapon: e.category.as_ref().map(|c| c.as_bytes())
                            == Some(&sub_weapon[..]),
                        quick_slot_max_stack: e.quick_slot_max_stack,
                        draft: e.draft,
                        alchemist_box: economy.is_alchemist_box(&bytes),
                    })
                }
            };
            let usable = held
                .iter()
                .filter(|(c, _)| *c == bytes)
                .try_fold(0_i32, |n, (_, count)| n.checked_add(*count))
                .ok_or_else(fail)?;
            SlotSource::Item { info, usable }
        };
        slots.push((part, class, source));
    }
    let consumables = domain::battle_inventory(
        slots,
        domain::multiplier(settings, "QuickSlotMultiplier", "SetQuickSlotLimit")
            .map_err(|_| fail())?,
        domain::multiplier(settings, "SubWeaponMultiplier", "SetSubWeaponLimit")
            .map_err(|_| fail())?,
        item_limit,
    )
    .map_err(|_| fail())?;
    let m = view.member;
    Ok(JoinMember {
        order: slot,
        tag: slot,
        key,
        name: m.name,
        base_class: m.base_class,
        level: m.level,
        exp: m.exp,
        level_up_exp: m.level_up_exp,
        title_id: m.title,
        title_count: m.title_count,
        stats: StrDict(Some(m.stats)),
        battle_inventory: Some(BattleInventory {
            consumables: Dict(Some(
                consumables
                    .into_iter()
                    .map(|(part, c)| {
                        (
                            part,
                            Some(Consumable {
                                item_class: Some(c.class),
                                bring: c.bring,
                                used: 0,
                                draft: c.draft,
                                inner: Some(Vec::new()),
                            }),
                        )
                    })
                    .collect(),
            )),
        }),
        costume: CostumeField(view.costume.clone()),
        pet: NoPet,
        skill_list: StrDict(Some(skills)),
        sp_skills: Dict(Some(sp_skills)),
        vocation_skills: StrDict(Some(Vec::new())),
        // MakeMemberInfo: TransformCoolDown is null without a vocation.
        transform_cool_down: -1,
        skill_enhances: StrDict(Some(Vec::new())),
        def_map: Dict(Some(m.def_map.into_iter().collect())),
        armor_hp_map: Dict(Some(m.armor_hp.into_iter().collect())),
        equipped_items: Dict(Some(equipped)),
        ability_list: Some(Vec::new()),
        status_effect_dict: StrDict(Some(Vec::new())),
        status_effects: Some(Vec::new()),
        destroyed_def: m.destroyed_def,
        is_teacher: false,
        is_assist: false,
        guild_level_bonus: StrDict(Some(Vec::new())),
        is_alive: true,
    })
}
