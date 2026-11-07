//! HideCostumeProcessor for the selected first Fiona in town: the "hide helmet" flag
//! is CostumeManager.LoadExtraData(ExtraKey.HideHeadCostume, 0/1), a base costume row
//! (Type 2, Key 7), followed by PushCostumeUpdate and the look observers. With AvatarHide, the
//! avatar part 54-58 of the request sets ExtraKey.HideAvatarHelm..Boots (15-19) to its state; the
//! gear then skips that avatar's look (Gear.ToEquippedItemInfos) and is sent again.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::{
    domain::{character_stats::Arithmetic, map_partition::OutgoingPacket},
    protocol::{DEFAULT_MAX_BODY, inventory::HideCostume},
};
use sqlx::PgPool;

/// ExtraKey.HideHeadCostume.
const HIDE_HEAD_KEY: i16 = 7;
/// ExtraKey.HideAvatarHelm; Tunic, Pants, Gloves and Boots follow (parts 54-58).
const HIDE_AVATAR_HELM_KEY: i32 = 15;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// CostumeUpdate and the refreshed look when the costume changed (bCostumeUpdated);
/// `None` when the request changed nothing (OkMessage only, no frontend message).
pub struct Applied {
    pub packets: Vec<OutgoingPacket>,
    pub look: OutgoingPacket,
}

/// CostumeManager.LoadExtraData(key, value) on a base row (Type 2): an existing row takes CostumeSN
/// (C1-C3 and Gloss cleared), otherwise a new one. Duplicate rows make the source dictionary
/// choice observable and are refused. Returns whether the stored value changed.
async fn set_extra(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cid: i64,
    key: i16,
    value: i32,
) -> Result<bool, MinimumError> {
    let rows: Vec<(i64, Option<i32>)> = sqlx::query_as(
        "SELECT row_id,costume_sn FROM game_costumes WHERE cid=$1 AND is_base AND costume_type=2 AND costume_key=$2 ORDER BY row_id LIMIT 2 FOR UPDATE",
    )
    .bind(cid)
    .bind(key)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 1 {
        return Err(fail());
    }
    if rows.first().is_some_and(|(_, sn)| *sn == Some(value)) {
        return Ok(false);
    }
    match rows.first() {
        Some((row_id, _)) => {
            sqlx::query("UPDATE game_costumes SET costume_sn=$2,c1=NULL,c2=NULL,c3=NULL,gloss=NULL WHERE row_id=$1")
                .bind(row_id)
                .bind(value)
                .execute(&mut **tx)
                .await?;
        }
        None => {
            sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect) VALUES(nextval('game_costume_row_id'),$1,true,2,$2,$3,NULL,NULL,NULL,NULL,NULL)")
                .bind(cid)
                .bind(key)
                .bind(value)
                .execute(&mut **tx)
                .await?;
        }
    }
    Ok(true)
}

pub async fn hide_costume(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: HideCostume,
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<Option<Applied>, MinimumError> {
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
        // CostumeInfo.HideHeadCostume is CostumeSN == 1 (no row: false); the source compares
        // before writing, so an unchanged flag writes nothing.
        let head: Option<Option<i32>> = sqlx::query_scalar(
            "SELECT costume_sn FROM game_costumes WHERE cid=$1 AND is_base AND costume_type=2 AND costume_key=$2 ORDER BY row_id LIMIT 1",
        )
        .bind(cid)
        .bind(HIDE_HEAD_KEY)
        .fetch_optional(&mut *tx)
        .await?;
        let mut updated = false;
        if head.flatten().is_some_and(|sn| sn == 1) != request.hide_head {
            updated |= set_extra(&mut tx, cid, HIDE_HEAD_KEY, i32::from(request.hide_head)).await?;
        }
        // FeatureMatrix AvatarHide: parts 54-58 are ExtraKey.HideAvatarHelm..Boots (15-19), then
        // Gear.ResetSlot and ItemOwner.Save, which the gear refresh below stands for.
        let mut avatar = false;
        if (54..=58).contains(&request.avatar_part) && assets.appearance.avatar_hide_enabled()? {
            let key = i16::try_from(request.avatar_part - 54 + HIDE_AVATAR_HELM_KEY)
                .map_err(|_| fail())?;
            set_extra(&mut tx, cid, key, request.avatar_state).await?;
            updated = true;
            avatar = true;
        }
        if !updated {
            tx.commit().await?;
            return Ok(None);
        }
        let (packets, look) = if avatar {
            // The avatar look of the part changes: equipment, stats and CostumeUpdate.
            let refreshed =
                super::minimum_equip::refresh_gear(&mut tx, owner, assets, cid, config, arithmetic)
                    .await?;
            (refreshed.packets, refreshed.look)
        } else {
            // PushCostumeUpdate sends CostumeInfo with the item costumes applied.
            let view = super::minimum_selection::view(
                &mut tx, owner, assets, cid, config, arithmetic, true, 0,
            )
            .await?;
            let update = OutgoingPacket {
                category: crate::protocol::character_list::COSTUME_UPDATE_CATEGORY,
                body: crate::protocol::character_list::encode_costume_update(
                    Some(view.costume.as_ref()),
                    DEFAULT_MAX_BODY,
                )
                .map_err(|_| fail())?
                .to_vec(),
            };
            (vec![update], view.look)
        };
        tx.commit().await?;
        Ok(Some(Applied { packets, look }))
    })
    .await
    .map_err(|_| fail())?
}
