//! APManager of the selected first Fiona: QueryAPProcessor and GiveAP with the
//! APMessage projection. Only the untrained-meditation profile (BonusAPTime 0, no
//! time-based regeneration) is admitted; trained meditation is refused.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{character_ap, map_partition::OutgoingPacket},
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Ap, Kind, Message},
    },
};
use sqlx::{PgPool, Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// SkillEffect.CalculateAPTime for the loaded skill cache. A zero interval makes
/// UpdateAP a no-op and APMessage reports -1 for both timing fields.
async fn bonus_interval(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
) -> Result<i32, MinimumError> {
    let cache = super::minimum_selection::skill_cache(tx, assets, cid).await?;
    let interval = character_ap::untrained_interval(&cache).map_err(|_| fail())?;
    if interval != 0 {
        return Err(fail());
    }
    Ok(interval)
}

fn message(ap: i32, max_ap: i32, interval: i32) -> Ap {
    // PushAPMessage/QueryAPProcessor: a non-positive interval reports -1/-1.
    debug_assert!(interval <= 0);
    Ap {
        next_bonus_ms: -1,
        ap,
        max_ap,
        bonus_interval: -1,
    }
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

/// UpdateAP (no-op at interval 0) and the APInfo returned to the frontend.
pub(super) async fn current(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
) -> Result<Ap, MinimumError> {
    let interval = bonus_interval(tx, assets, cid).await?;
    let ap: i32 = sqlx::query_scalar("SELECT ap FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(&mut **tx)
        .await?;
    Ok(message(ap, assets.initializer.maximum_ap()?, interval))
}

/// CharacterImpl.GiveAP(ap, allowExceed: true): unchecked Int32 addition, as in
/// APManager.GiveAP. The growth log is not written by this slice.
pub(super) async fn give_exceeding(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
    amount: i32,
) -> Result<Ap, MinimumError> {
    let before = current(tx, assets, cid).await?;
    let ap = before.ap.wrapping_add(amount);
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
        let ap = current(&mut tx, assets, cid).await?;
        tx.commit().await?;
        packet(ap)
    })
    .await
    .map_err(|_| fail())?
}
