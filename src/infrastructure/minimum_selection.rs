//! Selection for the explicit initialized first-Fiona laboratory profile.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::domain::character_costume::CostumeSet;
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
    let row = sqlx::query(
        "SELECT dirty,region_code,map_state_info,is_connected FROM game_character_info WHERE id=$1",
    )
    .bind(cid)
    .fetch_one(&mut *tx)
    .await?;
    if row.try_get::<bool, _>("dirty")? || row.try_get::<bool, _>("is_connected")? {
        return Err(MinimumError::Conflict);
    }
    let view = view(&mut tx, owner, assets, cid, config, arithmetic, false).await?;
    let look = view.look;
    let equipment = view.equipment;
    let mut packets = vec![
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
        view.common,
        view.stat,
    ];
    packets
        .push(super::minimum_inventory::load(&mut tx, owner, assets, cid, Some(equipment)).await?);
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
/// Skill runtime cache of the initialized first Fiona; only the initializer's
/// skills at their initial ranks are admitted until skill training is migrated.
pub(super) async fn skill_cache(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assets: &Assets,
    cid: i64,
) -> Result<Cache, MinimumError> {
    let skills: Vec<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT skill_utf16,rank FROM game_skills WHERE cid=$1 ORDER BY row_id FOR SHARE",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
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
    Ok(cache)
}
pub(super) struct View {
    /// CharacterCommonInfo with the summary used by the lobby list.
    pub common: OutgoingPacket,
    /// UpdateStat after CalculateItemStat/CalculateFinalStat.
    pub stat: OutgoingPacket,
    pub look: OutgoingPacket,
    /// CostumeManager.CostumeInfo for CostumeUpdate.
    pub costume: Option<character_list::Costume>,
    /// Stored rows and rows after LoadItemCostumes, when projected.
    pub costumes: Option<(CostumeSet, CostumeSet)>,
    pub equipment: Vec<(i32, i64)>,
}
/// Character state derived from persisted base stats, skills, Gear and costumes.
/// `project` applies LoadItemCostumes even for an empty Gear (UpdateItemStat path).
pub(super) async fn view(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    config: QueryConfig,
    arithmetic: Arithmetic,
    project: bool,
) -> Result<View, MinimumError> {
    let bytes = super::minimum_character_list::load(
        tx,
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
    let row = sqlx::query(
        "SELECT dirty,str,dex,int,will,luck,hp,stamina FROM game_character_info WHERE id=$1",
    )
    .bind(cid)
    .fetch_one(&mut **tx)
    .await?;
    let mut values = [0; 7];
    for (i, key) in ["str", "dex", "int", "will", "luck", "hp", "stamina"]
        .iter()
        .enumerate()
    {
        values[i] = row.try_get(*key)?;
    }
    let base = assets.initializer.base_stats([0; 7]);
    if row.try_get::<bool, _>("dirty")? || values != *base.values() {
        return Err(MinimumError::Conflict);
    }
    let cache = skill_cache(tx, assets, cid).await?;
    let gear = super::minimum_equipment::load(tx, owner, assets, cid).await?;
    if !gear.bonus.skills.is_empty() {
        return Err(MinimumError::Conflict);
    }
    let items = gear
        .projection
        .item_stats(&gear.bonus.stats, &cache)
        .map_err(|_| MinimumError::Conflict)?;
    let total = Stats::from_base(&base).add(&items.stats);
    let armor_def = items.armor_def.clone();
    let weight = items.weight;
    let l = if gear.projection.equipment.is_empty() {
        let mut load = State::new(gear.bonus.stats.clone());
        load.recalculate(
            Some(gear.bonus.stats.clone()),
            Some(EmptyGear),
            &cache,
            arithmetic,
            || Ok::<_, MinimumError>(assets.initializer.base_load()),
        )?;
        load.snapshot().clone()
    } else {
        crate::domain::character_load::equipped_x64(
            items,
            gear.bonus.stats.clone(),
            &cache,
            assets.initializer.base_load(),
            arithmetic,
        )
        .map_err(|_| MinimumError::Conflict)?
    };
    let stats = character_stats::apply_skills(&total, &armor_def, &cache, false, arithmetic)
        .map_err(|_| MinimumError::Conflict)?;
    let dictionary =
        character_stats::stat_dictionary(&stats.stats, weight, l.medium, l.heavy, l.penalty);
    let mut summary = character_list::decode(&bytes, DEFAULT_MAX_BODY)
        .map_err(|_| MinimumError::Conflict)?
        .and_then(|m| m.characters)
        .and_then(|mut c| c.pop())
        .flatten()
        .ok_or(MinimumError::Conflict)?;
    let mut costumes = None;
    if project || !gear.projection.equipment.is_empty() {
        let before = super::character_costumes::load_for_constructor(tx, cid)
            .await
            .map_err(|_| MinimumError::Conflict)?;
        let mut after = before.clone();
        gear.projection
            .load_costumes_x64(&mut after)
            .map_err(|_| MinimumError::Conflict)?;
        summary.costume = Some(
            crate::application::lobby_costume::project(&after)
                .map_err(|_| MinimumError::Conflict)?,
        );
        costumes = Some((before, after));
    }
    let look =
        Channel::encode_look_summary(cid, Some(&summary)).map_err(|_| MinimumError::Conflict)?;
    Ok(View {
        common: packet(Message::CharacterCommonInfo {
            query_id: 0,
            info: Some(summary.clone()),
        })?,
        stat: packet(Message::UpdateStat {
            stat: Some(dictionary),
        })?,
        look,
        costume: summary.costume,
        costumes,
        equipment: gear.projection.equipment,
    })
}
pub async fn disconnect(pool: &PgPool, cid: i64) -> Result<(), MinimumError> {
    sqlx::query("UPDATE game_character_info SET is_connected=false,channel=NULL WHERE id=$1")
        .bind(cid)
        .execute(pool)
        .await?;
    Ok(())
}
