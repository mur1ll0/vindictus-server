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
    /// StoryEntity query state of the entered character.
    pub story: crate::domain::npc_talk::Session,
    /// ItemOwner.BuybackList (in memory for the entered character).
    pub buyback: Vec<super::minimum_shop::BuybackEntry>,
    /// CharacterEntity.LogJournalID (OpenLog; 0 until opened or when it failed).
    pub log_journal: i64,
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
        "SELECT dirty,region_code,map_state_info,is_connected,level,class::integer AS class FROM game_character_info WHERE id=$1",
    )
    .bind(cid)
    .fetch_one(&mut *tx)
    .await?;
    if row.try_get::<bool, _>("dirty")? || row.try_get::<bool, _>("is_connected")? {
        return Err(MinimumError::Conflict);
    }
    let view = view(&mut tx, owner, assets, cid, config, arithmetic, false, 0).await?;
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
    // InGame.OnEnter forces the Skill observer (SkillListMessage) and QueryAP
    // (APMessage) after the Stat observer, without a client query.
    if assets.skills.is_some() {
        let (level, class): (i32, i32) = (row.try_get("level")?, row.try_get("class")?);
        packets
            .push(super::minimum_skills::packet(&mut tx, owner, assets, cid, level, class).await?);
        packets.push(super::minimum_ap::packet(
            super::minimum_ap::current(&mut tx, assets, cid).await?,
        )?);
    }
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
    Ok(Selected {
        cid,
        packets,
        look,
        story: Default::default(),
        buyback: Vec::new(),
        log_journal: 0,
    })
}
/// Skill runtime cache of the initialized first Fiona: the initializer's skills come
/// first (in row order), followed by skills learned later (LearnNewSkill).
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
    let initial = assets.initializer.skills();
    if skills.len() < initial.len() || skills.len() > 4096 {
        return Err(MinimumError::Conflict);
    }
    let mut cache = Cache::default();
    for (index, (key, rank)) in skills.iter().enumerate() {
        if let Some(expected) = initial.get(index) {
            if key != expected.key() {
                return Err(MinimumError::Conflict);
            }
            if *rank == expected.rank() {
                cache
                    .load_base(key, *rank, Some(expected.arguments()))
                    .map_err(|_| MinimumError::Conflict)?;
                continue;
            }
        }
        // A trained rank or a learned skill: SkillEffect.UpdateSkillArgument reads
        // SkillArgumentInfo of the final rank (set-bonus ranks are refused by the Gear
        // loader); no rank info removes the arguments.
        let contents = assets.skills.as_ref().ok_or(MinimumError::Conflict)?;
        let id = String::from_utf16(
            &key.as_chunks::<2>()
                .0
                .iter()
                .map(|a| u16::from_le_bytes(*a))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| MinimumError::Conflict)?;
        let content = contents.get(&id).ok_or(MinimumError::Conflict)?;
        if !content.ranks.contains_key(rank) {
            cache
                .load_base(key, *rank, None)
                .map_err(|_| MinimumError::Conflict)?;
            continue;
        }
        let rows = content
            .arguments
            .get(rank)
            .map(|rows| {
                rows.iter()
                    .map(|(k, v)| {
                        crate::domain::skill_runtime::Argument::new(Some(k.clone()), *v)
                            .map_err(|_| MinimumError::Conflict)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        cache
            .load_base(key, *rank, Some(&rows))
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
#[allow(clippy::too_many_arguments)]
pub(super) async fn view(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    config: QueryConfig,
    arithmetic: Arithmetic,
    project: bool,
    query_id: i64,
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
            query_id,
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
/// InGame QueryCharacterCommonInfoMessage for the selected character's own CID:
/// WhoIs(Costume) → CharacterCommonInfoMessage with the request QueryID.
pub async fn common_info(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    query_id: i64,
    config: QueryConfig,
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
            return Err(MinimumError::Conflict);
        }
        let view = view(
            &mut tx, owner, assets, cid, config, arithmetic, false, query_id,
        )
        .await?;
        tx.commit().await?;
        Ok(view.common)
    })
    .await
    .map_err(|_| MinimumError::Conflict)?
}
pub async fn disconnect(pool: &PgPool, cid: i64) -> Result<(), MinimumError> {
    sqlx::query("UPDATE game_character_info SET is_connected=false,channel=NULL WHERE id=$1")
        .bind(cid)
        .execute(pool)
        .await?;
    Ok(())
}
