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
    let message = Message::SkillList(SkillList {
        learning: Some(String::new()),
        current_ap: 0,
        skills: Some(
            list.into_iter()
                .map(|b| {
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
                        // Skill enhances are not migrated: no stored rows exist.
                        enhances: Some(Vec::new()),
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
        let before = super::minimum_ap::current(&mut tx, assets, cid).await?;
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
        if learned.completed
            && (learned.locked == 1
                || crate::domain::skill_multicast::is_meditation(&skills[index].id.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()))
        {
            // TryUnlockStory (story service) and meditation's BonusAPTime change are
            // not migrated; nothing is written.
            return Err(fail());
        }
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
        let ap_packet = super::minimum_ap::packet(super::minimum_ap::current(&mut tx, assets, cid).await?)?;
        let mut packets = Vec::new();
        if learned.completed {
            // CompleteSkill → OnSkillRankChanged pushes the list before SubmitChanges.
            packets.push(OutgoingPacket {
                category: list.category,
                body: list.body.clone(),
            });
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
pub async fn learn_new(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    skill_id: &str,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    use crate::domain::skill_list::{LEARN_NEW_SUCCEEDED, LearnNewRefusal};
    let flags = log.flags;
    // DestroyItemProcessor constructor: GetJournalID; ItemImpl: AddItemJournal.
    let operation = super::minimum_item_logs::operation_journal(pool, flags).await;
    let journal = super::minimum_item_logs::destroy_item_journal(pool, flags).await;
    let mut ids = entity_ids.into_iter();
    let (packets, paid) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
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
            Err(refusal) => return Ok((vec![learn_new_result(refusal as i32)?], None)),
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
        packets.push(learn_new_result(LEARN_NEW_SUCCEEDED)?);
        tx.commit().await?;
        Ok((packets, paid))
    })
    .await
    .map_err(|_| fail())??;
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
