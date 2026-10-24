//! QuerySkillListProcessor for the selected first Fiona: SkillListMessage with
//! LearningSkillID "", CurrentAp 0 and SkillManager.ToSkillList.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        map_partition::OutgoingPacket,
        skill_list::{self, Character, Skill},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, BriefSkill, Kind, Message, SkillList},
    },
};
use sqlx::PgPool;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    if raw.len() > 100 || !raw.len().is_multiple_of(2) {
        return Err(fail());
    }
    String::from_utf16(
        &raw.as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}

/// Admission (selected InGame character) belongs to the frontend.
pub async fn query(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let (connected, level, class): (bool, i32, i32) = sqlx::query_as(
            "SELECT is_connected,level,class::integer FROM game_character_info WHERE id=$1",
        )
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
        if !connected {
            return Err(fail());
        }
        let packet = packet(&mut tx, owner, assets, cid, level, class).await?;
        tx.commit().await?;
        Ok(packet)
    })
    .await
    .map_err(|_| fail())?
}

/// QuerySkillList projection inside the caller's character transaction (also used
/// by the InGame entry, which forces the Skill observer sync).
pub(super) async fn packet(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    level: i32,
    class: i32,
) -> Result<OutgoingPacket, MinimumError> {
    // VocationManager.VocationClass: no stored vocation is VocationEnum.None.
    let vocation: Option<i32> =
        sqlx::query_scalar("SELECT vocation_class FROM game_vocations WHERE cid=$1")
            .bind(cid)
            .fetch_optional(&mut **tx)
            .await?;
    let contents = assets.skills.as_ref().ok_or_else(fail)?;
    let rows: Vec<(Vec<u8>, i32, i16, i32)> = sqlx::query_as(
            "SELECT skill_utf16,rank,locked,current_ap FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097",
        )
        .bind(cid)
        .fetch_all(&mut **tx)
        .await?;
    if rows.len() > 4096 {
        return Err(fail());
    }
    let skills = rows
        .into_iter()
        .map(|(id, rank, locked, current_ap)| {
            Ok(Skill {
                id: text(&id)?,
                rank,
                locked: u8::try_from(locked).map_err(|_| fail())?,
                current_ap,
            })
        })
        .collect::<Result<Vec<_>, MinimumError>>()?;
    // Every skill must be in the bound content subset; absent SkillInfo or rank
    // rows are valid source states only for content the profile covers.
    if skills.iter().any(|s| !contents.contains_key(&s.id)) {
        return Err(fail());
    }
    // ItemSkillRank comes from the equipped set bonus (validated by the Gear loader).
    let gear = super::minimum_equipment::load(tx, owner, assets, cid).await?;
    let list = skill_list::to_skill_list(
        &skills,
        contents,
        &Character {
            class,
            level,
            item_ranks: &gear.bonus.skills,
        },
    )
    .map_err(|_| fail())?;
    let mut list = list;
    list.extend(skill_list::vocation_entry(vocation));
    // ToBriefSkillInfo: Skill.Enhances (the SkillEnhanceQueryAll row of the skill).
    let enhances = super::minimum_skill_enhance::load(tx, cid).await?;
    let message = Message::SkillList(SkillList {
        learning: Some(String::new()),
        current_ap: 0,
        skills: Some(
            list.into_iter()
                .map(|b| {
                    let own: Vec<_> = enhances
                        .iter()
                        .filter(|(s, _)| *s == b.id)
                        .map(|(_, e)| Some(super::minimum_skill_enhance::brief(e)))
                        .collect();
                    Some(BriefSkill {
                        id: Some(b.id),
                        base_rank: b.base_rank,
                        final_rank: b.final_rank,
                        required_ap: b.required_ap,
                        is_locked: b.is_locked,
                        can_start_training: b.can_start_training,
                        current_ap: b.current_ap,
                        reset_ap: b.reset_ap,
                        untrain_ap: b.untrain_ap,
                        enhances: Some(own),
                    })
                })
                .collect(),
        ),
        reset_vocation: false,
    });
    Ok(OutgoingPacket {
        category: Kind::SkillList.category(),
        body: wire::encode(Kind::SkillList, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// LearnSkillProcessor: Learn + CompleteSkill + SubmitChanges, then the frontend
/// messages. `Ok(vec![])` is the source path where Learn returns false (no message).
pub async fn learn(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    skill_id: &str,
    ap: i32,
    arithmetic: crate::domain::character_stats::Arithmetic,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let (connected, level, class): (bool, i32, i32) = sqlx::query_as(
            "SELECT is_connected,level,class::integer FROM game_character_info WHERE id=$1",
        )
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
        if !connected {
            return Err(fail());
        }
        let contents = assets.skills.as_ref().ok_or_else(fail)?;
        let rows: Vec<(i64, Vec<u8>, i32, i16, i32)> = sqlx::query_as(
            "SELECT row_id,skill_utf16,rank,locked,current_ap FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097 FOR UPDATE",
        )
        .bind(cid)
        .fetch_all(&mut *tx)
        .await?;
        if rows.len() > 4096 {
            return Err(fail());
        }
        let mut row_ids = Vec::with_capacity(rows.len());
        let mut skills = Vec::with_capacity(rows.len());
        for (row_id, id, rank, locked, current_ap) in rows {
            row_ids.push(row_id);
            skills.push(Skill {
                id: text(&id)?,
                rank,
                locked: u8::try_from(locked).map_err(|_| fail())?,
                current_ap,
            });
        }
        // GetSkill == null: FailMessage only.
        let Some(index) = skills.iter().position(|s| s.id == skill_id) else {
            return Ok(Vec::new());
        };
        if !contents.contains_key(skill_id) {
            return Err(fail());
        }
        let gear = super::minimum_equipment::load(&mut tx, owner, assets, cid).await?;
        // Validates the AP manager profile (BonusAPTime 0) and reads the stored AP.
        let before = super::minimum_ap::current(&mut tx, owner, assets, cid, arithmetic).await?;
        let character = Character {
            class,
            level,
            item_ranks: &gear.bonus.skills,
        };
        let Some(learned) =
            skill_list::learn(&skills, contents, &character, skill_id, before.ap, ap)
                .map_err(|_| fail())?
        else {
            return Ok(Vec::new());
        };
        if learned.completed && learned.locked == 1 {
            // TryUnlockStory (story service) is not migrated; nothing is written.
            return Err(fail());
        }
        let meditation = learned.completed && skills[index].id == MEDITATION;
        let stats = if learned.completed {
            stat_before(&mut tx, owner, assets, cid, skill_id, arithmetic).await?
        } else {
            None
        };
        sqlx::query("UPDATE game_skills SET rank=$2,locked=$3,current_ap=$4 WHERE row_id=$1")
            .bind(row_ids[index])
            .bind(learned.rank)
            .bind(i16::from(learned.locked))
            .bind(learned.current_ap)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE game_character_info SET ap=$2,total_used_ap=total_used_ap+$3 WHERE id=$1")
            .bind(cid)
            .bind(learned.ap)
            .bind(learned.used)
            .execute(&mut *tx)
            .await?;
        let list = packet(&mut tx, owner, assets, cid, level, class).await?;
        let stat_packets = stat_after(&mut tx, owner, assets, cid, stats, arithmetic).await?;
        let ap_packet = super::minimum_ap::packet(super::minimum_ap::current(&mut tx, owner, assets, cid, arithmetic).await?)?;
        let mut packets = Vec::new();
        if learned.completed {
            // CompleteSkill → OnSkillRankChanged pushes the list before SubmitChanges;
            // a meditation rank first recalculates BonusAPTime (UpdateAP already ran in
            // UseAP) and pushes the APMessage.
            if meditation {
                packets.push(OutgoingPacket {
                    category: ap_packet.category,
                    body: ap_packet.body.clone(),
                });
            }
            packets.push(OutgoingPacket {
                category: list.category,
                body: list.body.clone(),
            });
            // Stat.OnSkillRankChanged runs after SkillManager's handler.
            packets.extend(stat_packets);
        }
        // LearnSkillProcessor after SubmitChanges: PushAPMessage, PushSkillList and,
        // for a completed rank, SkillCompletionMessage.
        packets.push(ap_packet);
        packets.push(list);
        if learned.completed {
            let completion = Message::SkillCompletion(wire::SkillAp {
                skill: Some(skill_id.to_owned()),
                value: learned.rank,
            });
            packets.push(OutgoingPacket {
                category: Kind::SkillCompletion.category(),
                body: wire::encode(Kind::SkillCompletion, Some(&completion), DEFAULT_MAX_BODY)
                    .map_err(|_| fail())?
                    .to_vec(),
            });
        }
        tx.commit().await?;
        Ok(packets)
    })
    .await
    .map_err(|_| fail())?
}

fn learn_new_result(code: i32) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: Kind::LearnNewSkillResult.category(),
        body: wire::encode(
            Kind::LearnNewSkillResult,
            Some(&Message::LearnNewSkillResult(code)),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    })
}

/// LearnNewSkillProcessor for the selected first Fiona: rank-1 checks, the gold
/// DestroyItem (ItemService Save and Sync), GiveSkill and the result message. The
/// gold and the skill are written in one transaction.
#[allow(clippy::too_many_arguments)]
pub async fn learn_new(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    skill_id: &str,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
    character_journal: i64,
    arithmetic: crate::domain::character_stats::Arithmetic,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    use crate::domain::skill_list::{LEARN_NEW_SUCCEEDED, LearnNewRefusal};
    let flags = log.flags;
    // DestroyItemProcessor constructor: GetJournalID; ItemImpl: AddItemJournal.
    let operation = super::minimum_item_logs::operation_journal(pool, flags).await;
    let journal = super::minimum_item_logs::destroy_item_journal(pool, flags).await;
    let mut ids = entity_ids.into_iter();
    let (packets, paid, learned) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let (connected, level, class): (bool, i32, i32) = sqlx::query_as(
            "SELECT is_connected,level,class::integer FROM game_character_info WHERE id=$1",
        )
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
        if !connected {
            return Err(fail());
        }
        let contents = assets.skills.as_ref().ok_or_else(fail)?;
        let rows: Vec<(Vec<u8>, i32, i16, i32)> = sqlx::query_as(
            "SELECT skill_utf16,rank,locked,current_ap FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097 FOR UPDATE",
        )
        .bind(cid)
        .fetch_all(&mut *tx)
        .await?;
        if rows.len() > 4096 {
            return Err(fail());
        }
        let skills = rows
            .into_iter()
            .map(|(id, rank, locked, current_ap)| {
                Ok(Skill {
                    id: text(&id)?,
                    rank,
                    locked: u8::try_from(locked).map_err(|_| fail())?,
                    current_ap,
                })
            })
            .collect::<Result<Vec<_>, MinimumError>>()?;
        let gear = super::minimum_equipment::load(&mut tx, owner, assets, cid).await?;
        let character = Character {
            class,
            level,
            item_ranks: &gear.bonus.skills,
        };
        let new = match skill_list::learn_new(&skills, contents, &character, skill_id)
            .map_err(|_| fail())?
        {
            Ok(new) => new,
            Err(refusal) => return Ok((vec![learn_new_result(refusal as i32)?], None, false)),
        };
        if new.locked == 1 {
            // TryUnlockStory needs the story service, which is not migrated.
            return Err(fail());
        }
        let mut packets = Vec::new();
        let mut paid = None;
        if new.price != 0 {
            let gold = assets.inventory.gold_class().to_vec();
            let applied = super::minimum_give::apply(
                &mut tx,
                owner,
                assets,
                cid,
                &[super::minimum_give::Request {
                    class: &gold,
                    count: new.price,
                    bound: false,
                    equipment: None,
                    attributes: &crate::domain::item_attribute::EMPTY,
                }],
                &[],
                &mut || ids.next().ok_or_else(fail),
            )
            .await?;
            let Ok(applied) = applied else {
                // DestroyItem false: Not_Enough_Gold, nothing is written.
                return Ok((
                    vec![learn_new_result(LearnNewRefusal::NotEnoughGold as i32)?],
                    None,
                    false,
                ));
            };
            // ItemOwner.Save → Sync: the gold slot update reaches the client first.
            packets.push(OutgoingPacket {
                category: Kind::UpdateInventoryInfo.category(),
                body: wire::encode(
                    Kind::UpdateInventoryInfo,
                    Some(&Message::UpdateInventoryInfo(Some(applied.slots.clone()))),
                    DEFAULT_MAX_BODY,
                )
                .map_err(|_| fail())?
                .to_vec(),
            });
            paid = Some((applied, gold, new.price));
        }
        let stats = stat_before(&mut tx, owner, assets, cid, skill_id, arithmetic).await?;
        // GiveSkill: Skill row with RowID = MakeEntityID, rank 1, RecalculateRequiredAP.
        let key: Vec<u8> = skill_id.encode_utf16().flat_map(u16::to_le_bytes).collect();
        sqlx::query("INSERT INTO game_skills(row_id,cid,skill_utf16,rank,locked,current_ap) VALUES($1,$2,$3,1,$4,0)")
            .bind(ids.next().ok_or_else(fail)?)
            .bind(cid)
            .bind(&key)
            .bind(i16::from(new.locked))
            .execute(&mut *tx)
            .await?;
        if new.active {
            // SetSpSkillToEmptySlot: the first SPSkill whose SkillID is null.
            sqlx::query("UPDATE game_sp_skills SET skill_utf16=$2 WHERE row_id=(SELECT row_id FROM game_sp_skills WHERE cid=$1 AND skill_utf16 IS NULL ORDER BY row_id LIMIT 1)")
                .bind(cid)
                .bind(&key)
                .execute(&mut *tx)
                .await?;
        }
        // SkillRankChanged → OnSkillRankChanged → PushSkillList (before SubmitChanges).
        packets.push(packet(&mut tx, owner, assets, cid, level, class).await?);
        // Stat.OnSkillRankChanged (item/stat skills) after SkillManager's handler.
        packets.extend(stat_after(&mut tx, owner, assets, cid, stats, arithmetic).await?);
        packets.push(learn_new_result(LEARN_NEW_SUCCEEDED)?);
        tx.commit().await?;
        Ok((packets, paid, true))
    })
    .await
    .map_err(|_| fail())??;
    if learned {
        // GiveSkill: AddSkillLog(CharacterSkillEvent.GiveSkill, skillID, 1, 0).
        let key: Vec<u8> = skill_id.encode_utf16().flat_map(u16::to_le_bytes).collect();
        super::character_initialization_logs::skill_log(pool, character_journal, 1, &key, 1, 0)
            .await;
    }
    if let Some((applied, gold, price)) = paid {
        super::minimum_item_logs::write_give(
            pool,
            cid,
            operation,
            super::minimum_item_logs::DESTROY_ITEM,
            &applied,
            log,
        )
        .await;
        super::minimum_item_logs::write_destroy_ledger(pool, cid, journal, &gold, price, flags)
            .await;
    }
    Ok(packets)
}

/// SkillEffect.itemSkills and statSkills: their rank change recalculates item/final
/// stats (StatManager.OnSkillRankChanged → PushUpdateStat).
const STAT_CHANGING_SKILLS: [&str; 31] = [
    "skill_cloth_mastery",
    "skill_light_armor_mastery",
    "skill_heavy_armor_mastery",
    "skill_plate_armor_mastery",
    "skill_weight_mastery",
    "skill_fiona_longsword_mastery",
    "skill_fiona_shield_mastery",
    "skill_fiona_largeshield_mastery",
    "skill_fiona_longhammer_mastery",
    "skill_lethita_dualsword_mastery",
    "skill_lethita_dualspear_mastery",
    "skill_evy_staff_mastery",
    "skill_evy_scythe_mastery",
    "skill_kalok_battlepillar_mastery",
    "skill_kalok_blaster_mastery",
    "skill_kay_bow_mastery",
    "skill_kay_crossgun_mastery",
    "skill_vella_dualblade_mastery",
    "skill_hurk_greatsword_mastery",
    "skill_lynn_battleglaive_mastery",
    "skill_defence_mastery",
    "skill_stamina_mastery",
    "skill_hp_mastery",
    "skill_str_mastery",
    "skill_dex_mastery",
    "skill_int_mastery",
    "skill_will_mastery",
    "skill_evy_magic_mastery",
    "skill_stone_skin",
    "skill_critical_hit",
    "skill_evy_magic_critical",
];

/// FinalStat[INT] and BonusAPTime before an item/stat skill's rank changes; UpdateAP
/// with the old interval runs here (CalculateBonusAPTime's first step).
pub(super) struct StatBefore {
    interval: i32,
    int: i32,
}
pub(super) async fn stat_before(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    skill_id: &str,
    arithmetic: crate::domain::character_stats::Arithmetic,
) -> Result<Option<StatBefore>, MinimumError> {
    if !STAT_CHANGING_SKILLS.contains(&skill_id) {
        return Ok(None);
    }
    let ap = super::minimum_ap::current(tx, owner, assets, cid, arithmetic).await?;
    let (_, int) =
        super::minimum_selection::update_stat(tx, owner, assets, cid, arithmetic).await?;
    Ok(Some(StatBefore {
        interval: ap.bonus_interval,
        int,
    }))
}
/// Stat.OnSkillRankChanged after the change: CalculateItemStat/CalculateFinalStat; a
/// changed INT runs OnIntStatChanged (CalculateBonusAPTime, PushAPMessage) before
/// PushUpdateStat.
pub(super) async fn stat_after(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    before: Option<StatBefore>,
    arithmetic: crate::domain::character_stats::Arithmetic,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let Some(before) = before else {
        return Ok(Vec::new());
    };
    let (stat, int) =
        super::minimum_selection::update_stat(tx, owner, assets, cid, arithmetic).await?;
    let mut packets = Vec::new();
    if int != before.int {
        if before.interval <= 0 {
            // BonusAPTime 0: LastAPUpdate restarts at UtcNow.
            super::minimum_ap::restart_bonus(tx, cid).await?;
        }
        let ap = super::minimum_ap::current(tx, owner, assets, cid, arithmetic).await?;
        packets.push(super::minimum_ap::packet(ap)?);
    }
    packets.push(stat);
    Ok(packets)
}

/// SkillEffect.IsMeditation (case-sensitive equality).
const MEDITATION: &str = "skill_meditation";

/// Packets of a story skill change: `now` at the command's position, `later` after
/// the query (ReportToClient is scheduled), and the skill log written after commit.
pub(super) struct StorySkill {
    pub now: Vec<OutgoingPacket>,
    pub later: Vec<OutgoingPacket>,
    pub log: Option<(i32, Vec<u8>, i32, i32)>,
}

/// CharacterImpl.GiveSkill(skillID) for a grant `skill_list::give` accepted.
#[allow(clippy::too_many_arguments)]
pub(super) async fn give_story(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    skill_id: &str,
    new: skill_list::NewSkill,
    result: bool,
    arithmetic: crate::domain::character_stats::Arithmetic,
    ids: &mut (dyn Iterator<Item = i64> + Send),
) -> Result<StorySkill, MinimumError> {
    if new.locked == 1 {
        // TryUnlockStory is not migrated.
        tracing::warn!(
            skill = skill_id,
            "story skill grant outside the migrated profile"
        );
        return Err(fail());
    }
    let (level, class): (i32, i32) =
        sqlx::query_as("SELECT level,class::integer FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    let stats = stat_before(tx, owner, assets, cid, skill_id, arithmetic).await?;
    // GiveSkill: Skill row with RowID = MakeEntityID, rank 1, RecalculateRequiredAP.
    let key: Vec<u8> = skill_id.encode_utf16().flat_map(u16::to_le_bytes).collect();
    sqlx::query("INSERT INTO game_skills(row_id,cid,skill_utf16,rank,locked,current_ap) VALUES($1,$2,$3,1,$4,0)")
        .bind(ids.next().ok_or_else(fail)?)
        .bind(cid)
        .bind(&key)
        .bind(i16::from(new.locked))
        .execute(&mut **tx)
        .await?;
    if new.active {
        // SetSpSkillToEmptySlot: the first SPSkill whose SkillID is null.
        sqlx::query("UPDATE game_sp_skills SET skill_utf16=$2 WHERE row_id=(SELECT row_id FROM game_sp_skills WHERE cid=$1 AND skill_utf16 IS NULL ORDER BY row_id LIMIT 1)")
            .bind(cid)
            .bind(&key)
            .execute(&mut **tx)
            .await?;
    }
    // SkillRankChanged → OnSkillRankChanged: meditation recalculates BonusAPTime (from
    // 0, so LastAPUpdate restarts) and pushes the APMessage, then PushSkillList; a story
    // reward then sends LearnNewSkillResultMessage(Succeeced).
    let mut now = Vec::new();
    if skill_id == MEDITATION {
        super::minimum_ap::restart_bonus(tx, cid).await?;
        let ap = super::minimum_ap::current(tx, owner, assets, cid, arithmetic).await?;
        now.push(super::minimum_ap::packet(ap)?);
    }
    now.push(packet(tx, owner, assets, cid, level, class).await?);
    now.extend(stat_after(tx, owner, assets, cid, stats, arithmetic).await?);
    if result {
        now.push(learn_new_result(skill_list::LEARN_NEW_SUCCEEDED)?);
    }
    let completion = Message::SkillCompletion(wire::SkillAp {
        skill: Some(skill_id.to_owned()),
        value: 1,
    });
    let later = vec![OutgoingPacket {
        category: Kind::SkillCompletion.category(),
        body: wire::encode(Kind::SkillCompletion, Some(&completion), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    }];
    // AddSkillLog(CharacterSkillEvent.GiveSkill, skillID, rank, 0).
    Ok(StorySkill {
        now,
        later,
        log: Some((1, key, 1, 0)),
    })
}

/// A refused story reward GiveSkill: LearnNewSkillResultMessage(failReason).
pub(super) fn story_refusal(code: i32) -> Result<StorySkill, MinimumError> {
    Ok(StorySkill {
        now: vec![learn_new_result(code)?],
        later: Vec::new(),
        log: None,
    })
}

/// CharacterImpl.UnlockSkill(skillID, rank): only a locked skill at rank-1 unlocks;
/// otherwise it returns false and the story only logs UnlockSkillFail.
pub(super) async fn unlock_story(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    skill_id: &str,
    rank: i32,
) -> Result<StorySkill, MinimumError> {
    let key: Vec<u8> = skill_id.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let row: Option<(i32, i16, i32)> = sqlx::query_as(
        "SELECT rank,locked,current_ap FROM game_skills WHERE cid=$1 AND skill_utf16=$2 FOR UPDATE",
    )
    .bind(cid)
    .bind(&key)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((current, _, ap)) = row.filter(|r| r.1 != 0 && r.0 == rank.wrapping_sub(1)) else {
        return Ok(StorySkill {
            now: Vec::new(),
            later: Vec::new(),
            log: None,
        });
    };
    sqlx::query("UPDATE game_skills SET locked=0 WHERE cid=$1 AND skill_utf16=$2")
        .bind(cid)
        .bind(&key)
        .execute(&mut **tx)
        .await?;
    let (level, class): (i32, i32) =
        sqlx::query_as("SELECT level,class::integer FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    // SkillObservable.Updated(); PushSkillList; AddSkillLog(UnlockSkill, id, rank, AP).
    Ok(StorySkill {
        now: vec![packet(tx, owner, assets, cid, level, class).await?],
        later: Vec::new(),
        log: Some((5, key, current, ap)),
    })
}
