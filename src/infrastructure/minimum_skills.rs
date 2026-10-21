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
