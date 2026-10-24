//! APManager of the selected first Fiona: QueryAPProcessor and GiveAP with the
//! APMessage projection. BonusAPTime is SkillEffect.CalculateAPTime of the current
//! meditation rank and final INT; UpdateAP runs on the stored AP and LastAPUpdate.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        character_ap::{self, ApError, Snapshot, State},
        character_stats::Arithmetic,
        legacy_user_data::{SqlDateTime, SqlDateTime2},
        map_partition::OutgoingPacket,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Ap, Kind, Message},
    },
};
use sqlx::{PgPool, Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// The request's clock, APMax and BonusAPTime for the recovered UpdateAP.
struct Inputs {
    now: SqlDateTime2,
    maximum: i32,
    interval: i32,
}
impl character_ap::Inputs for Inputs {
    fn utc(&mut self) -> Result<SqlDateTime2, ApError> {
        Ok(self.now)
    }
    fn maximum(&mut self) -> Result<i32, ApError> {
        Ok(self.maximum)
    }
    fn interval(&mut self) -> Result<i32, ApError> {
        Ok(self.interval)
    }
}

/// SkillEffect.CalculateAPTime(Stat[INT]); untrained meditation returns 0 before
/// reading the stats or StatRatio.
async fn bonus_interval(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    arithmetic: Arithmetic,
) -> Result<i32, MinimumError> {
    let cache = super::minimum_selection::skill_cache(tx, assets, cid).await?;
    let reduction = character_ap::meditation_reduction(&cache);
    if reduction == -1 {
        return Ok(0);
    }
    let int =
        super::minimum_selection::final_int(tx, owner, assets, cid, &cache, arithmetic).await?;
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let ratio = character_ap::feature_float(npc.policy.get_string("StatRatio"));
    Ok(character_ap::ap_interval(reduction, int, ratio, arithmetic))
}

/// UpdateAP (persisted when it advances AP or LastAPUpdate) and the APInfo/APMessage
/// values returned to the frontend.
pub(super) async fn current(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    arithmetic: Arithmetic,
) -> Result<Ap, MinimumError> {
    let interval = bonus_interval(tx, owner, assets, cid, arithmetic).await?;
    let (ap, units): (i32, i64) =
        sqlx::query_as("SELECT ap,last_ap_update_units300 FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    let maximum = assets.initializer.maximum_ap()?;
    // LINQ to SQL reads the datetime column as a millisecond-rounded DateTime.
    let last = SqlDateTime::from_units300(units)
        .and_then(SqlDateTime::framework_date_time)
        .map_err(|_| fail())?;
    let now = super::lab_clock::utc_now().map_err(|_| fail())?;
    let mut state = State::restore(Snapshot {
        ap,
        last_update: last,
        bonus_seconds: interval,
    });
    state
        .update(&mut Inputs {
            now,
            maximum,
            interval,
        })
        .map_err(|_| fail())?;
    let s = state.snapshot();
    if s.ap != ap || s.last_update != last {
        set(tx, cid, s.ap, s.last_update).await?;
    }
    let (next_bonus_ms, bonus_interval) = character_ap::timing(s.last_update, interval, now);
    Ok(Ap {
        next_bonus_ms,
        ap: s.ap,
        max_ap: maximum,
        bonus_interval,
    })
}

/// SubmitChanges of CharacterInfo.AP/LastAPUpdate (SqlDbType.DateTime parameter).
async fn set(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    ap: i32,
    last: SqlDateTime2,
) -> Result<(), MinimumError> {
    let units = last.sql_parameter().map_err(|_| fail())?.units300();
    sqlx::query("UPDATE game_character_info SET ap=$2,last_ap_update_units300=$3 WHERE id=$1")
        .bind(cid)
        .bind(ap)
        .bind(units)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// APManager.UseAP: UpdateAP, then `None` when AP is short, else AP - value written.
pub(super) async fn use_ap(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    arithmetic: Arithmetic,
    value: i32,
) -> Result<Option<Ap>, MinimumError> {
    let before = current(tx, owner, assets, cid, arithmetic).await?;
    if value > before.ap {
        return Ok(None);
    }
    let ap = before.ap.wrapping_sub(value);
    sqlx::query("UPDATE game_character_info SET ap=$2 WHERE id=$1")
        .bind(cid)
        .bind(ap)
        .execute(&mut **tx)
        .await?;
    Ok(Some(Ap { ap, ..before }))
}

/// CalculateBonusAPTime when meditation is first learned: the old BonusAPTime is 0, so
/// LastAPUpdate restarts at UtcNow before the new interval applies.
pub(super) async fn restart_bonus(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<(), MinimumError> {
    let ap: i32 = sqlx::query_scalar("SELECT ap FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(&mut **tx)
        .await?;
    let now = super::lab_clock::utc_now().map_err(|_| fail())?;
    set(tx, cid, ap, now).await
}

pub(super) fn packet(ap: Ap) -> Result<OutgoingPacket, MinimumError> {
    let message = Message::Ap(ap);
    Ok(OutgoingPacket {
        category: Kind::Ap.category(),
        body: wire::encode(Kind::Ap, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// CharacterImpl.GiveAP(ap, allowExceed: true): unchecked Int32 addition, as in
/// APManager.GiveAP. The growth log is not written by this slice.
pub(super) async fn give_exceeding(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    arithmetic: Arithmetic,
    amount: i32,
) -> Result<Ap, MinimumError> {
    let before = current(tx, owner, assets, cid, arithmetic).await?;
    let ap = before.ap.wrapping_add(amount);
    sqlx::query("UPDATE game_character_info SET ap=$2 WHERE id=$1")
        .bind(cid)
        .bind(ap)
        .execute(&mut **tx)
        .await?;
    Ok(Ap { ap, ..before })
}

/// APManager.GiveAP(ap, allowExceed: false): at most MaxAP − AP (never negative), the
/// source's Int32 overflow guard never fires, so the addition wraps.
pub(super) async fn give_capped(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    arithmetic: Arithmetic,
    amount: i32,
) -> Result<Ap, MinimumError> {
    let before = current(tx, owner, assets, cid, arithmetic).await?;
    let given = if before.max_ap >= 0 {
        amount.min(before.max_ap.wrapping_sub(before.ap)).max(0)
    } else {
        amount
    };
    let ap = before.ap.wrapping_add(given);
    sqlx::query("UPDATE game_character_info SET ap=$2 WHERE id=$1")
        .bind(cid)
        .bind(ap)
        .execute(&mut **tx)
        .await?;
    Ok(Ap { ap, ..before })
}

/// QueryAPMessage → QueryAP → APMessage. Admission belongs to the frontend.
pub async fn query(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    arithmetic: Arithmetic,
) -> Result<OutgoingPacket, MinimumError> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let connected: bool =
            sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?;
        if !connected {
            return Err(fail());
        }
        let ap = current(&mut tx, owner, assets, cid, arithmetic).await?;
        tx.commit().await?;
        packet(ap)
    })
    .await
    .map_err(|_| fail())?
}
