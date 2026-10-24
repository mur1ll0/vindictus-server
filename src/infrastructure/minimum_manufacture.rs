//! ManufactureManager state of the selected first Fiona (game_manufacture and
//! game_manufacture_recipes in load order), ManufactureInfoMessage (PushManufactureInfo,
//! also requested by ItemServiceProxy.Open) and the story changes AcquireManufacture,
//! LevelupManufacture and AcquireManufactureRecipe.
use super::minimum_character::MinimumError;
use crate::{
    domain::{
        manufacture::{Skill, State},
        map_partition::OutgoingPacket,
        npc_talk::ManufactureChange,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, ManufactureInfo, ManufactureLevelUp, Message},
        mission::StrDict,
    },
};
use sqlx::{Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes(*b))
        .collect();
    String::from_utf16(&units).map_err(|_| fail())
}
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// GetManufactureStatus and GetManufactureRecipes in row order.
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<State, MinimumError> {
    let skills: Vec<(Vec<u8>, i32, i32)> = sqlx::query_as(
        "SELECT manufacture_utf16,grade,experience_point FROM game_manufacture WHERE cid=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let recipes: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT recipe_utf16 FROM game_manufacture_recipes WHERE cid=$1 ORDER BY row_id LIMIT 65537",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if skills.len() > 4096 || recipes.len() > 65536 {
        return Err(fail());
    }
    let mut state = State::default();
    for (id, grade, experience) in skills {
        let id = text(&id)?;
        // ManufactureManager: EXP.Add throws on a repeated ManufacturelID.
        if state.skill(&id).is_some() {
            return Err(fail());
        }
        state.skills.push(Skill {
            id,
            grade,
            experience,
        });
    }
    state.recipes = recipes.iter().map(|r| text(r)).collect::<Result<_, _>>()?;
    Ok(state)
}

/// ManufactureInfoMessage(EXPDic, GradeDic, ManufactureRecipes).
pub(super) fn info_packet(state: &State) -> Result<OutgoingPacket, MinimumError> {
    let dict = |f: fn(&Skill) -> i32| {
        StrDict(Some(
            state.skills.iter().map(|s| (s.id.clone(), f(s))).collect(),
        ))
    };
    packet(
        Kind::ManufactureInfo,
        Message::ManufactureInfo(ManufactureInfo {
            exp: dict(|s| s.experience),
            grade: dict(|s| s.grade),
            recipes: Some(state.recipes.iter().cloned().map(Some).collect()),
        }),
    )
}

/// ItemServiceProxy.Open → QueryManufactureInfo.
pub(super) async fn query(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    info_packet(&load(tx, cid).await?)
}

/// Persists a story change decided by `npc_talk` and returns its frontend messages:
/// ManufactureLevelUpMessage (acquire and level-up) then ManufactureInfoMessage.
pub(super) async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    change: &ManufactureChange,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let mut packets = Vec::new();
    match change {
        ManufactureChange::Acquired(mid) => {
            sqlx::query(
                "INSERT INTO game_manufacture(cid,manufacture_utf16,grade,experience_point) VALUES($1,$2,0,0)",
            )
            .bind(cid)
            .bind(utf16(mid))
            .execute(&mut **tx)
            .await?;
            packets.push(level_up_packet(mid, 0)?);
        }
        ManufactureChange::LevelUp {
            id,
            grade,
            experience,
        } => {
            let updated = sqlx::query(
                "UPDATE game_manufacture SET grade=$3,experience_point=$4 WHERE cid=$1 AND manufacture_utf16=$2",
            )
            .bind(cid)
            .bind(utf16(id))
            .bind(grade)
            .bind(experience)
            .execute(&mut **tx)
            .await?;
            if updated.rows_affected() != 1 {
                return Err(fail());
            }
            packets.push(level_up_packet(id, *grade)?);
        }
        ManufactureChange::Recipe { id, new } => {
            if *new {
                // ManufactureRecipe.AcquireTime = DateTime.UtcNow.
                let ticks = super::lab_clock::utc_now().map_err(|_| fail())?.ticks();
                sqlx::query(
                    "INSERT INTO game_manufacture_recipes(cid,recipe_utf16,acquired_ticks) VALUES($1,$2,$3)",
                )
                .bind(cid)
                .bind(utf16(id))
                .bind(ticks)
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    packets.push(query(tx, cid).await?);
    Ok(packets)
}

fn level_up_packet(mid: &str, grade: i32) -> Result<OutgoingPacket, MinimumError> {
    packet(
        Kind::ManufactureLevelUp,
        Message::ManufactureLevelUp(ManufactureLevelUp {
            mid: Some(mid.to_owned()),
            grade,
        }),
    )
}
