//! PvP rewards and results (B5 PV4): PvpPlayer.GiveRewardItem with SlayerStamp
//! (RewardItemProcessor: "from>to" is DoChangeItem, one CraftItem per count, else DoGiveItem
//! of an ItemClassEx, both stopping at the first failure in one transaction), the
//! PvPReward_Success messages, heroes PvpResultInfoUpdate/PvpResultInfoGet and the
//! ArenaResultInfoMessage of a winner. GiveRewardItem without SlayerStamp (GiveItem with
//! ForceMail) is not used by EN-EU and is refused.
use super::{
    character_lobby::UserKey,
    minimum_assets::Assets,
    minimum_character::MinimumError,
    minimum_give::{self, Request},
};
use crate::{
    domain::{item_attribute, map_partition::OutgoingPacket},
    protocol::inventory as inv,
};
use sqlx::PgPool;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// RewardItemProcessor of one reward: the given (class, count) and the inventory packets.
pub async fn reward_item(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    class: &str,
    count: i32,
    ids: Vec<i64>,
) -> Result<(Option<(String, i32)>, Vec<OutgoingPacket>), MinimumError> {
    if count <= 0 {
        return Ok((None, Vec::new()));
    }
    let mut ids = ids.into_iter();
    let mut next = || ids.next().ok_or_else(fail);
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut slots = Vec::new();
    let mut given = 0;
    let parts: Vec<&str> = class.split('>').collect();
    let result_class;
    if parts.len() == 2 {
        // DoChangeItem: CraftItem(DestroyItemRequest(from, 1), GiveItemRequest(to, 1)).
        let (from, to) = (utf16(parts[0]), utf16(parts[1]));
        result_class = parts[1].to_owned();
        for _ in 0..count {
            let destroy = Request {
                class: &from,
                count: 1,
                bound: false,
                equipment: None,
                attributes: &item_attribute::EMPTY,
            };
            let give = Request {
                class: &to,
                count: 1,
                bound: false,
                equipment: None,
                attributes: &item_attribute::EMPTY,
            };
            match minimum_give::apply(&mut tx, owner, assets, cid, &[destroy], &[give], &mut next)
                .await?
            {
                Ok(applied) => {
                    slots.extend(applied.slots);
                    given += 1;
                }
                Err(_) => break,
            }
        }
    } else {
        // DoGiveItem: ItemClassExBuilder.Parse, GiveItemRequest(class, 1) per count.
        let (name, attributes) = item_attribute::parse_class_ex(class).map_err(|_| fail())?;
        let key = utf16(&name);
        result_class = class.to_owned();
        for _ in 0..count {
            let give = Request {
                class: &key,
                count: 1,
                bound: false,
                equipment: None,
                attributes: &attributes,
            };
            match minimum_give::apply(&mut tx, owner, assets, cid, &[], &[give], &mut next).await? {
                Ok(applied) => {
                    slots.extend(applied.slots);
                    given += 1;
                }
                Err(_) => break,
            }
        }
    }
    tx.commit().await?;
    let mut packets = Vec::new();
    if !slots.is_empty() {
        packets.push(OutgoingPacket {
            category: inv::Kind::UpdateInventoryInfo.category(),
            body: inv::encode(
                inv::Kind::UpdateInventoryInfo,
                Some(&inv::Message::UpdateInventoryInfo(Some(slots))),
                crate::protocol::DEFAULT_MAX_BODY,
            )
            .map_err(|_| fail())?
            .to_vec(),
        });
    }
    if given == 0 {
        return Ok((None, packets));
    }
    // GameUI_Heroes_PvPReward_Success of each given item.
    packets.push(super::minimum_shop::system_message(
        super::minimum_shop::SYSTEM,
        "GameUI_Heroes_PvPReward_Success",
        &[result_class.clone(), given.to_string()],
    )?);
    Ok((Some((result_class, given)), packets))
}

/// PvpResultInfoUpdate: Win and Lose add, MaxSuccessiveWinCount keeps the greater.
pub async fn result_update(
    pool: &PgPool,
    cid: i64,
    mode: &str,
    win: i32,
    lose: i32,
    successive: i32,
) -> Result<(), MinimumError> {
    sqlx::query(
        "INSERT INTO game_pvp_result_info(cid,pvp_mode,win,lose,max_successive_win_count) VALUES($1,$2,$3,$4,$5) \
         ON CONFLICT(cid,pvp_mode) DO UPDATE SET win=game_pvp_result_info.win+EXCLUDED.win, \
         lose=game_pvp_result_info.lose+EXCLUDED.lose, max_successive_win_count=CASE \
         WHEN game_pvp_result_info.max_successive_win_count IS NULL \
         OR game_pvp_result_info.max_successive_win_count < EXCLUDED.max_successive_win_count \
         THEN EXCLUDED.max_successive_win_count ELSE game_pvp_result_info.max_successive_win_count END",
    )
    .bind(cid)
    .bind(mode)
    .bind(win)
    .bind(lose)
    .bind(successive)
    .execute(pool)
    .await?;
    Ok(())
}

/// PvpResultInfoGet: Win, Lose, MaxSuccessiveWinCount.
pub async fn result_info(
    pool: &PgPool,
    cid: i64,
    mode: &str,
) -> Result<Option<(i32, i32, i32)>, MinimumError> {
    let row: Option<(i32, i32, Option<i32>)> = sqlx::query_as(
        "SELECT win,lose,max_successive_win_count FROM game_pvp_result_info WHERE cid=$1 AND pvp_mode=$2",
    )
    .bind(cid)
    .bind(mode)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(w, l, m)| (w, l, m.unwrap_or(0))))
}
