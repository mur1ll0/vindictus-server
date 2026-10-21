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
        // Paladin appends a transform entry that is not migrated.
        let vocation: Option<i32> =
            sqlx::query_scalar("SELECT vocation_class FROM game_vocations WHERE cid=$1")
                .bind(cid)
                .fetch_optional(&mut *tx)
                .await?;
        if vocation == Some(0) {
            return Err(fail());
        }
        let contents = assets.skills.as_ref().ok_or_else(fail)?;
        let rows: Vec<(Vec<u8>, i32, i16, i32)> = sqlx::query_as(
            "SELECT skill_utf16,rank,locked,current_ap FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097",
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
        // Every skill must be in the bound content subset; absent SkillInfo or rank
        // rows are valid source states only for content the profile covers.
        if skills.iter().any(|s| !contents.contains_key(&s.id)) {
            return Err(fail());
        }
        // ItemSkillRank comes from the equipped set bonus (validated by the Gear loader).
        let gear = super::minimum_equipment::load(&mut tx, owner, assets, cid).await?;
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
        tx.commit().await?;
        Ok(OutgoingPacket {
            category: Kind::SkillList.category(),
            body: wire::encode(Kind::SkillList, Some(&message), DEFAULT_MAX_BODY)
                .map_err(|_| fail())?
                .to_vec(),
        })
    })
    .await
    .map_err(|_| fail())?
}
