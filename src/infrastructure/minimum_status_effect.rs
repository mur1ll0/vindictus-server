//! StatusEffectManager over `game_status_effects` for the selected first Fiona: the effects
//! of the exported StatusEffectGroupInfo subset (the HomeCafeBonus ones) are loaded, added,
//! removed, summed into StatusEffectStat and announced with StatusEffectUpdated. Rows of
//! other types (preserved legacy effects) are left untouched and carry no stats here.
use super::minimum_character::MinimumError;
use crate::{
    domain::{
        map_partition::OutgoingPacket,
        status_effect::{Active, Contents, Kind, seconds_to_ticks},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Message},
        mission::StatusEffect,
    },
};
use sqlx::{Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn text(b: &[u8]) -> Result<String, MinimumError> {
    let units: Vec<u16> = b
        .as_chunks::<2>()
        .0
        .iter()
        .map(|a| u16::from_le_bytes(*a))
        .collect();
    String::from_utf16(&units).map_err(|_| fail())
}

pub(super) fn now_ticks() -> Result<i64, MinimumError> {
    Ok(super::lab_clock::utc_now().map_err(|_| fail())?.ticks())
}

type Row = (i64, Vec<u8>, i32, Option<i32>, Option<i64>, Option<i16>);

/// The stored effects: (effects of the exported subset, whether other rows exist).
/// StatusEffectData.LoadDB drops an expired known effect; a second effect of one Type or
/// IDTag is dropped too (both removed from the rows, like the source constructor).
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    contents: &Contents,
    cid: i64,
) -> Result<(Vec<Active>, bool), MinimumError> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT row_id,type_utf16,level,remain_time,expire_ticks,remain_micro FROM game_status_effects WHERE cid=$1 ORDER BY row_id LIMIT 257",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 256 {
        return Err(fail());
    }
    let now = now_ticks()?;
    let mut active: Vec<Active> = Vec::new();
    let mut other = false;
    let mut dropped = Vec::new();
    for (row_id, type_utf16, level, remain, expire, micro) in rows {
        let name = text(&type_utf16)?;
        let Some(info) = contents.get(&name, level) else {
            other = true;
            continue;
        };
        if !matches!(info.kind, Kind::UseExpireTime | Kind::UseRemainTime) {
            dropped.push(row_id);
            continue;
        }
        // RemainTime without ExpireTime counts from the login.
        let expire = match (remain, expire) {
            (Some(r), None) => Some(now + seconds_to_ticks(r)),
            (_, e) => e,
        };
        let effect = Active {
            row_id,
            info: info.clone(),
            expire_ticks: expire,
            remain_micro: micro,
        };
        let duplicate = active.iter().any(|a| {
            a.info.type_name == effect.info.type_name
                || (effect.info.id_tag.is_some() && a.info.id_tag == effect.info.id_tag)
        });
        if effect.is_expired(now) || duplicate {
            dropped.push(row_id);
        } else {
            active.push(effect);
        }
    }
    if !dropped.is_empty() {
        sqlx::query("DELETE FROM game_status_effects WHERE cid=$1 AND row_id=ANY($2)")
            .bind(cid)
            .bind(&dropped)
            .execute(&mut **tx)
            .await?;
    }
    Ok((active, other))
}

/// StatusEffectManager.AddStatusEffect(type, level, length): false (nothing changes) for
/// an unknown or client effect, or one that cannot overwrite the stored effect of its
/// Type or IDTag; otherwise those are removed and the new row stored.
pub(super) async fn add(
    tx: &mut Transaction<'_, Postgres>,
    contents: &Contents,
    cid: i64,
    type_name: &str,
    level: i32,
    length: Option<i32>,
) -> Result<bool, MinimumError> {
    let Some(info) = contents.get(type_name, level) else {
        return Ok(false);
    };
    if info.kind == Kind::Client {
        return Ok(false);
    }
    let (active, _) = load(tx, contents, cid).await?;
    let by_type = active.iter().find(|a| a.info.type_name == info.type_name);
    let by_tag = info
        .id_tag
        .as_ref()
        .and_then(|t| active.iter().find(|a| a.info.id_tag.as_ref() == Some(t)));
    if let Some(existing) = by_type.or(by_tag)
        && !info.can_overwrite(&existing.info)
    {
        return Ok(false);
    }
    let removed: Vec<i64> = [by_type, by_tag]
        .into_iter()
        .flatten()
        .map(|a| a.row_id)
        .collect();
    let duration = length.or(info.duration).unwrap_or(-1);
    // StatusEffectData.New: a non-positive duration other than -1 fails.
    if duration <= 0 && duration != -1 {
        return Ok(false);
    }
    // DeleteOnLogOut effects have no row and live only in the session (not migrated).
    if info.kind == Kind::DeleteOnLogOut {
        return Err(fail());
    }
    if !removed.is_empty() {
        sqlx::query("DELETE FROM game_status_effects WHERE cid=$1 AND row_id=ANY($2)")
            .bind(cid)
            .bind(&removed)
            .execute(&mut **tx)
            .await?;
    }
    let expire = (duration > 0)
        .then(|| now_ticks().map(|n| n + seconds_to_ticks(duration)))
        .transpose()?;
    // RemainMicroPlay = (byte?)CombatCount: -1 wraps to 255.
    let micro = i16::from(info.combat_count.to_le_bytes()[0]);
    sqlx::query("INSERT INTO game_status_effects(row_id,cid,type_utf16,level,remain_time,expire_ticks,remain_micro) VALUES(nextval('game_status_effect_row_id'),$1,$2,$3,$4,$5,$6)")
        .bind(cid)
        .bind(utf16(&info.type_name))
        .bind(info.level)
        .bind(duration)
        .bind(expire)
        .bind(micro)
        .execute(&mut **tx)
        .await?;
    Ok(true)
}

/// StatusEffectManager.RemoveStatusEffect(type) for a stored effect of the subset.
pub(super) async fn remove(
    tx: &mut Transaction<'_, Postgres>,
    contents: &Contents,
    cid: i64,
    type_name: &str,
) -> Result<bool, MinimumError> {
    let (active, _) = load(tx, contents, cid).await?;
    let ids: Vec<i64> = active
        .iter()
        .filter(|a| a.info.type_name == type_name)
        .map(|a| a.row_id)
        .collect();
    if ids.is_empty() {
        return Ok(false);
    }
    sqlx::query("DELETE FROM game_status_effects WHERE cid=$1 AND row_id=ANY($2)")
        .bind(cid)
        .bind(&ids)
        .execute(&mut **tx)
        .await?;
    Ok(true)
}

/// GetStatusEffectElementList: the unexpired effects as StatusEffectElement.
pub(super) fn elements(active: &[Active], now: i64) -> Vec<Option<StatusEffect>> {
    active
        .iter()
        .filter(|a| !a.is_expired(now))
        .map(|a| {
            let (kind, level, remain_time, combat_count) = a.element(now);
            Some(StatusEffect {
                kind: Some(kind),
                level,
                remain_time,
                combat_count,
            })
        })
        .collect()
}

/// StatusEffectManager.Save → StatusEffectUpdated(CharacterName, GetStatusEffectElementList).
pub(super) async fn updated_packet(
    tx: &mut Transaction<'_, Postgres>,
    contents: &Contents,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    let (active, _) = load(tx, contents, cid).await?;
    let name: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT name_utf16 FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    let message = Message::StatusEffectUpdated(wire::StatusEffectUpdated {
        character_name: name.as_deref().map(text).transpose()?,
        status_effects: Some(elements(&active, now_ticks()?)),
    });
    Ok(OutgoingPacket {
        category: wire::Kind::StatusEffectUpdated.category(),
        body: wire::encode(
            wire::Kind::StatusEffectUpdated,
            Some(&message),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    })
}
