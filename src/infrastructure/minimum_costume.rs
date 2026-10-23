//! HideCostumeProcessor for the selected first Fiona in town: the "hide helmet" flag
//! is CostumeManager.LoadExtraData(ExtraKey.HideHeadCostume, 0/1), a base costume row
//! (Type 2, Key 7), followed by PushCostumeUpdate and the look observers.
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

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// CostumeUpdate and the refreshed look when the costume changed (bCostumeUpdated);
/// `None` when the request changed nothing (OkMessage only, no frontend message).
pub struct Applied {
    pub packets: Vec<OutgoingPacket>,
    pub look: OutgoingPacket,
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
        // AvatarHide parts (54-58) also reset the equipped slot and resynchronize its
        // item (ItemOwner.Save); that item path is not migrated, so nothing is written.
        if request.avatar_part >= 0
            && assets.catalog.policy.is_enabled(Some("AvatarHide"))?
            && (54..=58).contains(&request.avatar_part)
        {
            tracing::warn!(
                avatar_part = request.avatar_part,
                avatar_state = request.avatar_state,
                "minimum avatar part hide is not migrated; nothing written"
            );
            return Err(fail());
        }
        // CostumeManager.GetCostume(isBase: true, 2, HideHeadCostume); duplicate keys
        // would make the source dictionary choice observable, so they are refused.
        let rows: Vec<(i64, Option<i32>)> = sqlx::query_as(
            "SELECT row_id,costume_sn FROM game_costumes WHERE cid=$1 AND is_base AND costume_type=2 AND costume_key=$2 ORDER BY row_id LIMIT 2 FOR UPDATE",
        )
        .bind(cid)
        .bind(HIDE_HEAD_KEY)
        .fetch_all(&mut *tx)
        .await?;
        if rows.len() > 1 {
            return Err(fail());
        }
        // CostumeInfo.Apply: HideHeadCostume = CostumeSN == 1 (no row: false).
        let current = rows.first().is_some_and(|(_, sn)| *sn == Some(1));
        if current == request.hide_head {
            tx.commit().await?;
            return Ok(None);
        }
        let value = i32::from(request.hide_head);
        // SetCostume: an existing row takes CostumeSN, C1-C3 and Gloss; otherwise a new
        // base row (Effect stays null).
        match rows.first() {
            Some((row_id, _)) => {
                sqlx::query("UPDATE game_costumes SET costume_sn=$2,c1=NULL,c2=NULL,c3=NULL,gloss=NULL WHERE row_id=$1")
                    .bind(row_id)
                    .bind(value)
                    .execute(&mut *tx)
                    .await?;
            }
            None => {
                sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect) VALUES(nextval('game_costume_row_id'),$1,true,2,$2,$3,NULL,NULL,NULL,NULL,NULL)")
                    .bind(cid)
                    .bind(HIDE_HEAD_KEY)
                    .bind(value)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        // PushCostumeUpdate sends CostumeInfo with the item costumes applied.
        let view =
            super::minimum_selection::view(&mut tx, owner, assets, cid, config, arithmetic, true, 0)
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
        tx.commit().await?;
        Ok(Some(Applied {
            packets: vec![update],
            look: view.look,
        }))
    })
    .await
    .map_err(|_| fail())?
}
