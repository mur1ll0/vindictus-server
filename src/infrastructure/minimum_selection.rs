//! Selection for the explicit initialized first-Fiona laboratory profile.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::{
    domain::{
        character_load::{EmptyGear, State},
        character_stats::{self, Arithmetic, Stats},
        map_partition::{self, Channel, OutgoingPacket},
        skill_runtime::Cache,
    },
    protocol::{
        DEFAULT_MAX_BODY, character_list,
        minimum_map::{self as wire, Message},
    },
};
use sqlx::{PgPool, Row};

pub struct Selected {
    pub cid: i64,
    pub packets: Vec<OutgoingPacket>,
    pub look: OutgoingPacket,
}
fn packet(message: Message<'_>) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: message.kind().category(),
        body: wire::encode(message.kind(), Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| MinimumError::Conflict)?
            .to_vec(),
    })
}
/// Password/session admission is checked by the frontend before this storage boundary.
/// Stat arithmetic follows the explicit CLR profile, independently of body quantization.
pub async fn select(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    index: i32,
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<Selected, MinimumError> {
    if index != 0 {
        return Err(MinimumError::Conflict);
    }
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM game_character_info WHERE uid=$1 ORDER BY character_sn LIMIT 2",
    )
    .bind(owner.uid)
    .fetch_all(pool)
    .await?;
    if ids.len() != 1 {
        return Err(MinimumError::Conflict);
    }
    let cid = ids[0];
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let bytes = super::minimum_character_list::load(
        &mut tx,
        owner,
        &assets.initializer,
        config,
        assets
            .initializer
            .policy()
            .get_integer("DeleteCharacterDelay")?,
        || super::lab_clock::utc_now().map_err(|_| MinimumError::Conflict),
        cid,
    )
    .await?;
    let row = sqlx::query("SELECT dirty,str,dex,int,will,luck,hp,stamina,region_code,map_state_info,is_connected FROM game_character_info WHERE id=$1").bind(cid).fetch_one(&mut *tx).await?;
    let mut values = [0; 7];
    for (i, key) in ["str", "dex", "int", "will", "luck", "hp", "stamina"]
        .iter()
        .enumerate()
    {
        values[i] = row.try_get(*key)?;
    }
    let base = assets.initializer.base_stats([0; 7]);
    if row.try_get::<bool, _>("dirty")?
        || row.try_get::<bool, _>("is_connected")?
        || values != *base.values()
    {
        return Err(MinimumError::Conflict);
    }
    let skills: Vec<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT skill_utf16,rank FROM game_skills WHERE cid=$1 ORDER BY row_id FOR SHARE",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    if skills.len() != assets.initializer.skills().len() {
        return Err(MinimumError::Conflict);
    }
    let mut cache = Cache::default();
    for ((key, rank), expected) in skills.iter().zip(assets.initializer.skills()) {
        if key != expected.key() || *rank != expected.rank() {
            return Err(MinimumError::Conflict);
        }
        cache
            .load_base(key, *rank, Some(expected.arguments()))
            .map_err(|_| MinimumError::Conflict)?;
    }
    let gear = assets.appearance.initial_gear(&assets.catalog)?;
    let items = gear
        .item_stats(&cache)
        .map_err(|_| MinimumError::Conflict)?;
    let mut load = State::new(gear.set_bonus().clone());
    load.recalculate(
        Some(gear.set_bonus().clone()),
        Some(EmptyGear),
        &cache,
        arithmetic,
        || Ok::<_, MinimumError>(assets.initializer.base_load()),
    )?;
    let total = Stats::from_base(&base).add(&items.stats);
    let stats = character_stats::apply_skills(&total, &items.armor_def, &cache, false, arithmetic)
        .map_err(|_| MinimumError::Conflict)?;
    let l = load.snapshot();
    let dictionary =
        character_stats::stat_dictionary(&stats.stats, items.weight, l.medium, l.heavy, l.penalty);
    let summary = character_list::decode(&bytes, DEFAULT_MAX_BODY)
        .map_err(|_| MinimumError::Conflict)?
        .and_then(|m| m.characters)
        .and_then(|mut c| c.pop())
        .flatten()
        .ok_or(MinimumError::Conflict)?;
    let look =
        Channel::encode_look_summary(cid, Some(&summary)).map_err(|_| MinimumError::Conflict)?;
    let packets = vec![
        packet(Message::LoginOk {
            region_code: map_partition::login_ok_region_code(row.try_get("region_code")?),
            time: map_partition::utility_epoch_seconds(
                super::lab_clock::utc_now().map_err(|_| MinimumError::Conflict)?,
            ),
            limited: 0,
            facebook_token: Some(""),
            // UserInfo defaults / UserCareType.Nothing / UserCareState.Nothing.
            // Zero means CareUser + Promise and incorrectly enters the event lobby.
            user_care_type: -1,
            user_care_next_state: -1,
            map_state_info: u8::try_from(row.try_get::<i16, _>("map_state_info")?)
                .map_err(|_| MinimumError::Conflict)?,
        })?,
        packet(Message::NoticeGameEnvironment {
            cafe_type: 0,
            is_otp: false,
        })?,
        packet(Message::CharacterCommonInfo {
            query_id: 0,
            info: Some(summary),
        })?,
        packet(Message::UpdateStat {
            stat: Some(dictionary),
        })?,
    ];
    sqlx::query("INSERT INTO game_user_info(nexon_sn,last_login_cid,prologue_played) VALUES($1,$2,false) ON CONFLICT(nexon_sn) DO UPDATE SET last_login_cid=EXCLUDED.last_login_cid").bind(owner.uid).bind(cid).execute(&mut *tx).await?;
    sqlx::query("UPDATE game_character_info SET is_connected=true WHERE id=$1")
        .bind(cid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE game_micro_play_status SET current_quest_utf16=NULL WHERE cid=$1")
        .bind(cid)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Selected { cid, packets, look })
}
pub async fn disconnect(pool: &PgPool, cid: i64) -> Result<(), MinimumError> {
    sqlx::query("UPDATE game_character_info SET is_connected=false,channel=NULL WHERE id=$1")
        .bind(cid)
        .execute(pool)
        .await?;
    Ok(())
}
