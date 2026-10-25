//! Hot spring processors of the selected first Fiona over `game_channel_buffs`
//! (heroesShare ChannelBuff): HotSpringRequestInfoProcessor, HotSpringAddPotionProcessor
//! and HotSpringRequestPotionEffectProcessor. The channel broadcast of a new potion and its
//! announcement are returned to the frontend, which owns the channel and client fan-out.
//! ChannelBuffSet compares against the server clock instead of SQL Server GETUTCDATE().
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::map_partition::OutgoingPacket,
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{
            self as wire, HotSpringAddPotion, HotSpringInfos, HotSpringPotionEffectInfo,
            HotSpringRequestPotionEffect, Kind, Message,
        },
    },
};
use sqlx::{PgPool, Postgres, Transaction};

/// HotSpringAddPotionResult values.
pub mod result {
    pub const NOT_USABLE_CHANNEL: i32 = 1;
    pub const NO_ITEM: i32 = 2;
    pub const NO_POTION_INFO: i32 = 3;
    pub const OTHER_PLAYER_USE_IN_TIME: i32 = 6;
}
/// HotSpringPotionInfo.Level of the premium powder (clears the other effects).
const PREMIUM: i32 = 99;
/// DateTime ticks of the POSIX epoch (Extention.ToInt32).
const EPOCH_TICKS: i64 = 621_355_968_000_000_000;
const TICKS_PER_SECOND: i64 = 10_000_000;

fn fail() -> MinimumError {
    MinimumError::Conflict
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
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
/// DateTime.ToInt32: whole seconds since the POSIX epoch, truncated to Int32.
fn to_int32(ticks: i64) -> i32 {
    ((ticks - EPOCH_TICKS) / TICKS_PER_SECOND) as i32
}
/// (short)Channel and (byte)TownID of the source casts.
fn keys(channel: i32, town: i32) -> (i16, i16) {
    (channel as i16, i16::from(town as u8))
}

type Buff = (String, i64, Option<i64>, Option<i64>);

/// ChannelBuffGet(channel, town) in primary key order.
async fn buffs(
    tx: &mut Transaction<'_, Postgres>,
    channel: i16,
    town: i16,
) -> Result<Vec<Buff>, MinimumError> {
    Ok(sqlx::query_as(
        "SELECT item_class,cid,expire_ticks,change_ticks FROM game_channel_buffs WHERE channel=$1 AND town=$2 ORDER BY item_class LIMIT 1001",
    )
    .bind(channel)
    .bind(town)
    .fetch_all(&mut **tx)
    .await?)
}

/// ChannelBuffSet: 0 (nothing changes) while the stored row of the class is before its
/// ChangeTime, otherwise the row is merged.
#[allow(clippy::too_many_arguments)]
async fn set(
    tx: &mut Transaction<'_, Postgres>,
    channel: i16,
    town: i16,
    class: &str,
    cid: i64,
    expire: i64,
    change: i64,
    now: i64,
) -> Result<bool, MinimumError> {
    Ok(sqlx::query(
        "INSERT INTO game_channel_buffs(channel,town,item_class,cid,expire_ticks,change_ticks) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(channel,town,item_class) DO UPDATE SET cid=EXCLUDED.cid,expire_ticks=EXCLUDED.expire_ticks,change_ticks=EXCLUDED.change_ticks WHERE NOT COALESCE($7<game_channel_buffs.change_ticks,false)",
    )
    .bind(channel)
    .bind(town)
    .bind(class)
    .bind(cid)
    .bind(expire)
    .bind(change)
    .bind(now)
    .execute(&mut **tx)
    .await?
    .rows_affected()
        == 1)
}

/// HotSpringRequestInfoProcessor: HotSpringRequestInfoResultMessage of the unexpired buffs
/// with their characters' names (no guild names). None when a stored time is missing
/// (the source logs the exception and sends nothing).
pub async fn request_info(
    pool: &PgPool,
    channel: i32,
    town: i32,
) -> Result<Option<OutgoingPacket>, MinimumError> {
    let (c, t) = keys(channel, town);
    let mut tx = pool.begin().await?;
    let now = super::minimum_status_effect::now_ticks()?;
    let mut infos = Vec::new();
    for (class, cid, expire, change) in buffs(&mut tx, c, t).await? {
        if expire.is_some_and(|e| e < now) {
            continue;
        }
        let (Some(expire), Some(change)) = (expire, change) else {
            return Ok(None);
        };
        let name: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT name_utf16 FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();
        infos.push(Some(HotSpringPotionEffectInfo {
            potion_item_class: Some(class),
            character_name: Some(name.as_deref().map(text).transpose()?.unwrap_or_default()),
            guild_name: Some(String::new()),
            expired_time: to_int32(expire),
            other_potion_usable_time: to_int32(change),
        }));
    }
    tx.commit().await?;
    Ok(Some(packet(
        Kind::HotSpringRequestInfoResult,
        Message::HotSpringRequestInfoResult(HotSpringInfos {
            infos: Some(infos),
            town_id: town,
        }),
    )?))
}

/// HotSpringAddPotionProcessor outcome.
pub struct AddPotion {
    /// Frontend messages to the requester: the failure result or the inventory update.
    pub reply: Vec<OutgoingPacket>,
    /// ChannelHotSpringInfo → HotSpringRequestInfoResultChannelMessage for the channel.
    pub channel: Option<OutgoingPacket>,
    /// AllAnnounce NotifyClient (Telepathy, town only) for a requester at or above
    /// RandomItem_AnnounceLevel (BroadcastToAllClients).
    pub announce: Option<OutgoingPacket>,
}

fn add_result(result: i32, town: i32) -> Result<AddPotion, MinimumError> {
    Ok(AddPotion {
        reply: vec![packet(
            Kind::HotSpringAddPotionResult,
            Message::HotSpringAddPotionResult(wire::HotSpringAddPotionResult {
                result,
                town_id: town,
                prev_potion_item_class: None,
                info: None,
            }),
        )?],
        channel: None,
        announce: None,
    })
}

/// HotSpringAddPotionProcessor. The previous-potion checks (BeforeChangeTime,
/// LowerPotionLevel) are unreachable in the source: its lookup only runs for rows without
/// a potion info, and then throws.
pub async fn add_potion(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    message: &HotSpringAddPotion,
) -> Result<AddPotion, MinimumError> {
    let town = message.town_id;
    if message.channel == -1 {
        return add_result(result::NOT_USABLE_CHANNEL, town);
    }
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let (c, t) = keys(message.channel, town);
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let item: Option<(Vec<u8>, i32)> =
        sqlx::query_as("SELECT item_class_utf16,count FROM game_items WHERE id=$1 AND owner_id=$2")
            .bind(message.item_id)
            .bind(cid)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((class, count)) = item else {
        return add_result(result::NO_ITEM, town);
    };
    let class = text(&class)?;
    let Some(potion) = economy.hot_spring(&class) else {
        return add_result(result::NO_POTION_INFO, town);
    };
    // The previous-potion loop dereferences a missing potion info.
    for (stored, ..) in buffs(&mut tx, c, t).await? {
        if economy.hot_spring(&stored).is_none() {
            return Err(fail());
        }
    }
    let now = super::minimum_status_effect::now_ticks()?;
    if potion.level == PREMIUM {
        for (stored, ..) in buffs(&mut tx, c, t).await? {
            set(&mut tx, c, t, &stored, cid, now, now, now).await?;
        }
    }
    let expire = now + i64::from(potion.bath_time_sec) * TICKS_PER_SECOND;
    let change = now + i64::from(potion.overlap_min_sec) * TICKS_PER_SECOND;
    if !set(&mut tx, c, t, &class, cid, expire, change, now).await? {
        // The premium resets above are kept, as the source commits each call.
        tx.commit().await?;
        return add_result(result::OTHER_PLAYER_USE_IN_TIME, town);
    }
    let (name, character_level): (Option<Vec<u8>>, i32) =
        sqlx::query_as("SELECT name_utf16,level FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    let name = name.as_deref().map(text).transpose()?.unwrap_or_default();
    let channel = packet(
        Kind::HotSpringRequestInfoResultChannel,
        Message::HotSpringRequestInfoResultChannel(HotSpringInfos {
            infos: Some(vec![Some(HotSpringPotionEffectInfo {
                potion_item_class: Some(class.clone()),
                character_name: Some(name.clone()),
                guild_name: Some(String::new()),
                expired_time: to_int32(expire),
                other_potion_usable_time: to_int32(change),
            })]),
            town_id: town,
        }),
    )?;
    let announce = if potion.all_announce == 1 {
        let level = assets
            .catalog
            .policy
            .get_integer("RandomItem_AnnounceLevel")
            .unwrap_or(0);
        (level == 0 || character_level >= level)
            .then(|| {
                super::minimum_shop::system_message(
                    2,
                    &format!("GameUI_Heroes_HotSpring_HighClassPotion_Notify_{town}"),
                    &[message.channel.to_string(), name, class.clone()],
                )
            })
            .transpose()?
    } else {
        None
    };
    // useItem.Num-- and Save (the item journal and ledger are not migrated).
    let deleted = count <= 1;
    if deleted {
        sqlx::query("DELETE FROM game_item_attributes WHERE item_id=$1")
            .bind(message.item_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
            .bind(message.item_id)
            .bind(cid)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE game_items SET count=count-1 WHERE id=$1 AND owner_id=$2")
            .bind(message.item_id)
            .bind(cid)
            .execute(&mut *tx)
            .await?;
    }
    let slot = if deleted {
        super::minimum_move::deleted_slot(message.item_id)
    } else {
        super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
            .await?
            .slots
            .as_ref()
            .and_then(|s| s.iter().flatten().find(|s| s.id == message.item_id))
            .cloned()
            .ok_or_else(fail)?
    };
    tx.commit().await?;
    Ok(AddPotion {
        reply: vec![packet(
            Kind::UpdateInventoryInfo,
            Message::UpdateInventoryInfo(Some(vec![Some(slot)])),
        )?],
        channel: Some(channel),
        announce,
    })
}

/// HotSpringRequestPotionEffectProcessor: the effect of a stored buff of the class (its
/// expiry is not checked). The premium powder removes the other hot spring effects;
/// another potion is refused while a premium effect is active. Save sends UpdateStat
/// (the effects carry StatAdder), then StatusEffectUpdated.
pub async fn request_effect(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    message: &HotSpringRequestPotionEffect,
    arithmetic: crate::domain::character_stats::Arithmetic,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let contents = &assets
        .cash_shop
        .as_ref()
        .ok_or_else(fail)?
        .customize()
        .status_effects;
    let Some(class) = message.potion_item_class.as_deref() else {
        return Ok(Vec::new());
    };
    let (c, t) = keys(message.channel, message.town_id);
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    if !buffs(&mut tx, c, t).await?.iter().any(|b| b.0 == class) {
        return Ok(Vec::new());
    }
    let Some(potion) = economy.hot_spring(class) else {
        return Ok(Vec::new());
    };
    let effects = economy.hot_spring_effects();
    if potion.level == PREMIUM {
        for effect in &effects {
            super::minimum_status_effect::remove(&mut tx, contents, cid, effect).await?;
        }
    } else {
        let (active, _) = super::minimum_status_effect::load(&mut tx, contents, cid).await?;
        if active
            .iter()
            .any(|a| effects.contains(&a.info.type_name.as_str()) && a.info.level == PREMIUM)
        {
            tx.commit().await?;
            return Ok(Vec::new());
        }
    }
    super::minimum_status_effect::add(
        &mut tx,
        contents,
        cid,
        &potion.status_effect,
        potion.level,
        None,
    )
    .await?;
    let mut packets = vec![
        super::minimum_selection::update_stat(&mut tx, owner, assets, cid, arithmetic)
            .await?
            .0,
    ];
    packets.push(super::minimum_status_effect::updated_packet(&mut tx, contents, cid).await?);
    tx.commit().await?;
    Ok(packets)
}
