//! CharacterImpl.GiveReward(exp, ap) for the selected first Fiona: GiveExp with the
//! level-up chain (LevelUpMessage, SkillList, quest NotifyLevel, story NotifyLevelUp,
//! CalculateBaseStat/UpdateStat), PushCharacterInfo, then GiveAP and PushAPMessage.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{character_stats::Arithmetic, map_partition::OutgoingPacket},
    protocol::{DEFAULT_MAX_BODY, minimum_map},
};
use sqlx::{Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// Runtime values the level-up chain needs.
#[derive(Clone, Copy)]
pub struct GrowthContext {
    pub clock: super::minimum_quest::QuestClock,
    pub arithmetic: Arithmetic,
    pub config: super::character_lobby::QueryConfig,
    /// CharacterEntity.LogJournalID for the title log.
    pub journal: i64,
}

fn map_packet(message: &minimum_map::Message<'_>) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: message.kind().category(),
        body: minimum_map::encode(message.kind(), Some(message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// GiveReward in the caller's transaction (the original SubmitChanges boundary). EXP
/// beyond MaxLevel is ignored, as GiveExp returns early; AP uses allowExceed false.
#[allow(clippy::too_many_arguments)]
pub(super) async fn give_reward(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    exp: i64,
    ap: i32,
    ctx: GrowthContext,
    ids: &mut (dyn Iterator<Item = i64> + Send),
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let progression = npc.progression.as_ref().ok_or_else(fail)?;
    let mut packets = Vec::new();
    if exp > 0 {
        let (stored_exp, level, class): (i64, i32, i32) = sqlx::query_as(
            "SELECT exp,level,class::integer FROM game_character_info WHERE id=$1 FOR UPDATE",
        )
        .bind(cid)
        .fetch_one(&mut **tx)
        .await?;
        if let Some(growth) = progression.give_exp(stored_exp, level, exp) {
            sqlx::query("UPDATE game_character_info SET exp=$2,level=$3 WHERE id=$1")
                .bind(cid)
                .bind(i64::from(growth.exp))
                .bind(growth.level)
                .execute(&mut **tx)
                .await?;
            if growth.leveled() {
                level_up(
                    tx,
                    owner,
                    assets,
                    cid,
                    class,
                    growth,
                    ctx,
                    ids,
                    &mut packets,
                )
                .await?;
            }
            // GiveReward: PushCharacterInfo (CharacterCommonInfoMessage, QueryID 0).
            let view = super::minimum_selection::view(
                tx,
                owner,
                assets,
                cid,
                ctx.config,
                ctx.arithmetic,
                false,
                0,
            )
            .await?;
            packets.push(view.common);
        }
    }
    if ap > 0 {
        let given =
            super::minimum_ap::give_capped(tx, owner, assets, cid, ctx.arithmetic, ap).await?;
        packets.push(super::minimum_ap::packet(given)?);
    }
    Ok(packets)
}

#[allow(clippy::too_many_arguments)]
async fn level_up(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    class: i32,
    growth: crate::domain::progression::Growth,
    ctx: GrowthContext,
    ids: &mut (dyn Iterator<Item = i64> + Send),
    packets: &mut Vec<OutgoingPacket>,
) -> Result<(), MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let progression = npc.progression.as_ref().ok_or_else(fail)?;
    let (from, to) = (growth.previous_level, growth.level);
    // LevelUpMessage(Level, GetCharacterStats(new) − GetCharacterStats(old)).
    let increase = progression.stat_increase(from, to).ok_or_else(fail)?;
    let rows: Vec<(&str, i32)> = increase.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    packets.push(map_packet(&minimum_map::Message::LevelUp {
        level: to,
        stat_increased: Some(rows),
    })?);
    // Skills.OnLevelChanged: TryUnlockStory for locked skills, then PushSkillList.
    let locked: i64 =
        sqlx::query_scalar("SELECT count(*) FROM game_skills WHERE cid=$1 AND locked<>0")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    if locked != 0 {
        return Err(fail());
    }
    packets.push(super::minimum_skills::packet(tx, owner, assets, cid, to, class).await?);
    // QuestProxy.NotifyLevel: auto-give from the previous CharacterLevel, Save, push.
    if let Some(contents) = npc.quests.as_ref() {
        let original = super::minimum_quest::load(tx, cid, contents).await?;
        let mut quests = original.clone();
        quests.init_status(contents);
        quests.level = from;
        quests.login(contents, to);
        super::minimum_quest::save(tx, cid, &original, &quests, ids).await?;
        let coupons = super::minimum_quest::raid_coupons(tx, cid).await?;
        let quest_ctx = crate::domain::quest::Context {
            contents,
            policy: &npc.policy,
            cafe: false,
            raid_coupons: coupons,
        };
        packets.push(super::minimum_quest::progress_packet(
            &quests,
            quest_ctx,
            ctx.clock,
            super::minimum_quest::now_units()?,
        )?);
    }
    // StoryProxy.NotifyLevelUp: storylines auto-given at the new levels.
    packets.extend(super::minimum_story::level_up(tx, assets, cid, from, to, class, ids).await?);
    // Stats.CalculateBaseStat (class stats of the level plus title stats) is written
    // here so the summary below reads a consistent row; UpdateStat keeps its source
    // position after the titles.
    calculate_base_stat(tx, assets, cid).await?;
    // Titles.GiveTitleByLevelUp → PushTitleUpdate (UpdateTitle, CharacterCommonInfo).
    let owned: Vec<i32> = sqlx::query_scalar("SELECT title_id FROM game_titles WHERE cid=$1")
        .bind(cid)
        .fetch_all(&mut **tx)
        .await?;
    let given = progression.titles_by_level(to, class, &owned);
    let mut slots = Vec::new();
    for t in &given {
        // AddNewTitle: TitleInsert(CID, TitleID, false, null, null, null); goal progress
        // stays in memory (0 per goal), as the source does not insert it here.
        sqlx::query("INSERT INTO game_titles(cid,title_id,acquired) VALUES($1,$2,false)")
            .bind(cid)
            .bind(t.id)
            .execute(&mut **tx)
            .await?;
        title_log(tx, ctx.journal, GIVE_TITLE, t.id).await?;
        if t.listed {
            slots.push(Some(crate::protocol::inventory::TitleSlot {
                title_id: t.id,
                // Known: visible with zero progress (HideUntil 0).
                status: 1,
                time_stamp: 0,
                quest_id: Some(String::new()),
                goal_progress: Some(t.goals.iter().map(|g| (*g, 0)).collect()),
                is_expireable: false,
                expire_date_time: -1,
            }));
        }
    }
    let update = crate::protocol::inventory::Message::UpdateTitle(Some(slots));
    packets.push(OutgoingPacket {
        category: crate::protocol::inventory::Kind::UpdateTitle.category(),
        body: crate::protocol::inventory::encode(
            crate::protocol::inventory::Kind::UpdateTitle,
            Some(&update),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    });
    let summary = super::minimum_selection::view(
        tx,
        owner,
        assets,
        cid,
        ctx.config,
        ctx.arithmetic,
        false,
        0,
    )
    .await?;
    packets.push(summary.common);
    // CalculateFinalStat → PushUpdateStat.
    let view = super::minimum_selection::view(
        tx,
        owner,
        assets,
        cid,
        ctx.config,
        ctx.arithmetic,
        false,
        0,
    )
    .await?;
    packets.push(view.stat);
    Ok(())
}

/// StatManager.CalculateBaseStat: the class stats of the stored level (level 1 from the
/// initializer profile, LUCK 100) plus Titles.GetTitleStats of the acquired titles and
/// the account's shared titles.
pub(super) async fn base_stats(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
) -> Result<crate::domain::character_base_stats::BaseStats, MinimumError> {
    let (level, class, uid): (i32, i32, i32) =
        sqlx::query_as("SELECT level,class::integer,uid FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    let own: Vec<i32> = sqlx::query_scalar(
        "SELECT title_id FROM game_titles WHERE cid=$1 AND acquired ORDER BY title_id",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    // TitleQueryAcquiredByAccount: acquired titles of every character of the account.
    let account: Vec<i32> = sqlx::query_scalar("SELECT DISTINCT t.title_id FROM game_titles t JOIN game_character_info c ON c.id=t.cid WHERE c.uid=$1 AND t.acquired ORDER BY t.title_id")
        .bind(uid)
        .fetch_all(&mut **tx)
        .await?;
    let progression = assets.npc.as_ref().and_then(|n| n.progression.as_ref());
    let titles = if own.is_empty() && account.is_empty() {
        [0; 7]
    } else {
        progression
            .and_then(|p| p.title_stats(&own, &account, class, level))
            .ok_or_else(fail)?
    };
    let base = if level == 1 {
        assets.initializer.base_stats(titles)
    } else {
        crate::domain::character_base_stats::BaseStats::compose(
            progression
                .and_then(|p| p.class_stats(level))
                .ok_or_else(fail)?,
            titles,
        )
    };
    Ok(base)
}

/// CalculateBaseStat's write: Dirty false and the seven CharacterInfo stat columns.
pub(super) async fn calculate_base_stat(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
) -> Result<(), MinimumError> {
    let v = *base_stats(tx, assets, cid).await?.values();
    sqlx::query("UPDATE game_character_info SET dirty=false,str=$2,dex=$3,int=$4,will=$5,luck=$6,hp=$7,stamina=$8 WHERE id=$1")
        .bind(cid)
        .bind(v[0])
        .bind(v[1])
        .bind(v[2])
        .bind(v[3])
        .bind(v[4])
        .bind(v[5])
        .bind(v[6])
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// CharacterTitleEvent.OpenTitle, GiveTitle and AcquireTitle.
const OPEN_TITLE: i32 = 1;
const GIVE_TITLE: i32 = 2;
const UPDATE_TITLE: i32 = 5;
const COMPLETE_GOAL: i32 = 6;
const ACQUIRE_TITLE: i32 = 7;

/// CharacterEntity.AddTitleLog(event, titleID, 0, 0) in the day's partition.
async fn title_log(
    tx: &mut Transaction<'_, Postgres>,
    journal: i64,
    event: i32,
    title: i32,
) -> Result<(), MinimumError> {
    title_log_value(tx, journal, event, title, 0, 0).await
}
/// CharacterEntity.AddTitleLog(event, titleID, goalID, value) in the day's partition.
async fn title_log_value(
    tx: &mut Transaction<'_, Postgres>,
    journal: i64,
    event: i32,
    title: i32,
    goal: i32,
    value: i32,
) -> Result<(), MinimumError> {
    use crate::domain::character_ledger::{LedgerPartition, partition};
    let units: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
    )
    .fetch_one(&mut **tx)
    .await?;
    let now =
        crate::domain::legacy_user_data::SqlDateTime::from_units300(units).map_err(|_| fail())?;
    let q = match partition(now) {
        LedgerPartition::One => {
            "INSERT INTO game_character_title_log01(journal_id,event,title_id,goal_id,value) VALUES($1,$3,$2,$4,$5)"
        }
        LedgerPartition::Two => {
            "INSERT INTO game_character_title_log02(journal_id,event,title_id,goal_id,value) VALUES($1,$3,$2,$4,$5)"
        }
    };
    sqlx::query(q)
        .bind(journal)
        .bind(title)
        .bind(event)
        .bind(goal)
        .bind(value)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Story `notice_title`: CharacterImpl.GiveReward(0, 0, null, {titleID: false}) →
/// Titles.NoticeTitle, then Stats.TitleChanged, PushUpdateStat and PushTitleUpdate
/// (UpdateTitle, CharacterCommonInfo) when the title was opened or revealed.
pub(super) async fn notice_title(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    title: i32,
    ctx: GrowthContext,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let progression = npc.progression.as_ref().ok_or_else(fail)?;
    let class: i32 =
        sqlx::query_scalar("SELECT class::integer FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    let acquired: Option<bool> =
        sqlx::query_scalar("SELECT acquired FROM game_titles WHERE cid=$1 AND title_id=$2")
            .bind(cid)
            .bind(title)
            .fetch_optional(&mut **tx)
            .await?;
    let stored: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT title_goal_id,progress FROM game_title_goal_progress WHERE cid=$1 LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if stored.len() > 4096 {
        return Err(fail());
    }
    let noticed = progression
        .notice_title(title, class, acquired.map(|a| (a, stored.as_slice())))
        .map_err(|_| fail())?;
    let Some(noticed) = noticed else {
        return Ok(Vec::new());
    };
    if noticed.new {
        sqlx::query("INSERT INTO game_titles(cid,title_id,acquired) VALUES($1,$2,false)")
            .bind(cid)
            .bind(title)
            .execute(&mut **tx)
            .await?;
        title_log(tx, ctx.journal, GIVE_TITLE, title).await?;
    }
    // TitleGoalProgress of the first goal: inserted when it was 0 (not stored), else updated.
    if let Some((goal, progress)) = noticed.goals.first().copied()
        && (progress != 0 || stored.iter().any(|(g, _)| *g == goal))
    {
        sqlx::query("INSERT INTO game_title_goal_progress(cid,title_goal_id,progress) VALUES($1,$2,$3) ON CONFLICT(cid,title_goal_id) DO UPDATE SET progress=EXCLUDED.progress")
            .bind(cid)
            .bind(goal)
            .bind(progress)
            .execute(&mut **tx)
            .await?;
    }
    title_log(tx, ctx.journal, OPEN_TITLE, title).await?;
    let slot = crate::protocol::inventory::TitleSlot {
        title_id: title,
        // TitleState.Known: visible and not acquired.
        status: 1,
        time_stamp: 0,
        quest_id: Some(String::new()),
        goal_progress: Some(noticed.goals.clone()),
        is_expireable: false,
        expire_date_time: -1,
    };
    title_changed(tx, owner, assets, cid, ctx, slot).await
}

/// Stats.TitleChanged (CalculateBaseStat, CalculateFinalStat, PushUpdateStat), GiveReward's
/// own PushUpdateStat and PushTitleUpdate (UpdateTitle with the slot, CharacterCommonInfo)
/// after GiveReward changed one title.
async fn title_changed(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    ctx: GrowthContext,
    slot: crate::protocol::inventory::TitleSlot,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    calculate_base_stat(tx, assets, cid).await?;
    let mut packets = Vec::new();
    let view = super::minimum_selection::view(
        tx,
        owner,
        assets,
        cid,
        ctx.config,
        ctx.arithmetic,
        false,
        0,
    )
    .await?;
    packets.push(view.stat.clone());
    packets.push(view.stat);
    let update = crate::protocol::inventory::Message::UpdateTitle(Some(vec![Some(slot)]));
    packets.push(OutgoingPacket {
        category: crate::protocol::inventory::Kind::UpdateTitle.category(),
        body: crate::protocol::inventory::encode(
            crate::protocol::inventory::Kind::UpdateTitle,
            Some(&update),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    });
    packets.push(view.common);
    Ok(packets)
}

/// Story `acquire_title`: CharacterImpl.GiveReward(0, 0, null, {titleID: true}) →
/// Titles.AcquireTitle (GiveTitle first when not owned; Acquired with AcquiredTime, goal
/// progress removed, AcquireTitle log), then TitleChanged and the title update packets.
/// Guild score and rank basis events are not migrated.
pub(super) async fn acquire_title(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    title: i32,
    ctx: GrowthContext,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let progression = npc.progression.as_ref().ok_or_else(fail)?;
    let class: i32 =
        sqlx::query_scalar("SELECT class::integer FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    let acquired: Option<bool> =
        sqlx::query_scalar("SELECT acquired FROM game_titles WHERE cid=$1 AND title_id=$2")
            .bind(cid)
            .bind(title)
            .fetch_optional(&mut **tx)
            .await?;
    let Some(done) = progression.acquire_title(title, class, acquired) else {
        return Ok(Vec::new());
    };
    if done.new {
        sqlx::query("INSERT INTO game_titles(cid,title_id,acquired) VALUES($1,$2,false)")
            .bind(cid)
            .bind(title)
            .execute(&mut **tx)
            .await?;
        title_log(tx, ctx.journal, GIVE_TITLE, title).await?;
    }
    let units: i64 = sqlx::query_scalar(
        "UPDATE game_titles SET acquired=true,acquired_time_units300=floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT WHERE cid=$1 AND title_id=$2 RETURNING acquired_time_units300",
    )
    .bind(cid)
    .bind(title)
    .fetch_one(&mut **tx)
    .await?;
    // ClearTitleProgress.
    sqlx::query("DELETE FROM game_title_goal_progress WHERE cid=$1 AND title_goal_id=ANY($2)")
        .bind(cid)
        .bind(&done.goals)
        .execute(&mut **tx)
        .await?;
    title_log(tx, ctx.journal, ACQUIRE_TITLE, title).await?;
    let ticks = crate::domain::legacy_user_data::SqlDateTime::from_units300(units)
        .and_then(|t| t.framework_date_time())
        .map_err(|_| fail())?
        .ticks();
    let slot = crate::protocol::inventory::TitleSlot {
        title_id: title,
        // TitleState.Acquired; every goal reports -1.
        status: 2,
        time_stamp: ticks,
        quest_id: Some(String::new()),
        goal_progress: Some(done.goals.iter().map(|g| (*g, -1)).collect()),
        is_expireable: false,
        expire_date_time: -1,
    };
    title_changed(tx, owner, assets, cid, ctx, slot).await
}

/// QueryPlayerInfo.TitleGoalIDs (Titles.GetTrackingGoalID) for MicroPlay.RegisterTitleTracker.
pub async fn tracking_goals(
    pool: &sqlx::PgPool,
    assets: &Assets,
    cid: i64,
) -> Result<Vec<i32>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let progression = npc.progression.as_ref().ok_or_else(fail)?;
    let mut tx = pool.begin().await?;
    let (owned, progress) = title_state(&mut tx, cid).await?;
    tx.commit().await?;
    Ok(progression.tracking_goals(&owned, &progress))
}

/// The character's titles (TitleID, Acquired) and goal progress rows.
async fn title_state(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<(Vec<(i32, bool)>, Vec<(i32, i32)>), MinimumError> {
    let owned: Vec<(i32, bool)> = sqlx::query_as(
        "SELECT title_id,acquired FROM game_titles WHERE cid=$1 ORDER BY title_id LIMIT 65537 FOR UPDATE",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let progress: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT title_goal_id,progress FROM game_title_goal_progress WHERE cid=$1 LIMIT 4097 FOR UPDATE",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if owned.len() > 65536 || progress.len() > 4096 {
        return Err(fail());
    }
    Ok((owned, progress))
}

/// InformMicroplayEventProcessor for one informer event: Titles.Proceed for each tracked
/// goal it matches (in tracker order, with the source's per-goal SubmitChanges), then
/// Stats.TitleChanged when a title was acquired and PushTitleUpdate (UpdateTitle,
/// CharacterCommonInfo) when any slot is reported. Rank basis is not migrated.
#[allow(clippy::too_many_arguments)]
pub async fn proceed_titles(
    pool: &sqlx::PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    tracked: &[i32],
    event: &str,
    own: bool,
    quest: &str,
    ctx: GrowthContext,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let progression = npc.progression.as_ref().ok_or_else(fail)?;
    let goals = progression.matching_goals(tracked, event, own);
    if goals.is_empty() {
        return Ok(Vec::new());
    }
    // MicroPlayInformer passes the item count for "get item <class> <count>", else 1.
    let count = event
        .strip_prefix("get item ")
        .and_then(|rest| rest.rsplit(' ').next())
        .and_then(|n| n.parse::<i32>().ok())
        .unwrap_or(1);
    let update_count = npc
        .policy
        .is_enabled(Some("UpdateTitleCount"))
        .map_err(|_| fail())?;
    let quest_key: Vec<u8> = quest.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut tx = pool.begin().await?;
    let (mut owned, mut progress) = title_state(&mut tx, cid).await?;
    let mut slots = Vec::new();
    let mut acquired_any = false;
    for goal in goals {
        let Some(p) = progression.proceed(goal, &owned, &progress, count, update_count) else {
            continue;
        };
        title_log_value(&mut tx, ctx.journal, UPDATE_TITLE, p.title, goal, count).await?;
        if p.completed {
            title_log_value(&mut tx, ctx.journal, COMPLETE_GOAL, p.title, goal, -1).await?;
        }
        // TitleGoalProgress.InsertIntoDatabase (from 0) or UpdateToDatabase.
        sqlx::query("INSERT INTO game_title_goal_progress(cid,title_goal_id,progress) VALUES($1,$2,$3) ON CONFLICT(cid,title_goal_id) DO UPDATE SET progress=EXCLUDED.progress")
            .bind(cid)
            .bind(goal)
            .bind(p.progress)
            .execute(&mut *tx)
            .await?;
        match progress.iter_mut().find(|(g, _)| *g == goal) {
            Some(row) => row.1 = p.progress,
            None => progress.push((goal, p.progress)),
        }
        let mut time_stamp = 0;
        let mut quest_id = String::new();
        if p.acquired {
            // Acquired, AcquiredQuest = where, AcquiredTime = UtcNow; ClearTitleProgress.
            let units: i64 = sqlx::query_scalar(
                "UPDATE game_titles SET acquired=true,acquired_quest_utf16=$3,acquired_time_units300=floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT WHERE cid=$1 AND title_id=$2 RETURNING acquired_time_units300",
            )
            .bind(cid)
            .bind(p.title)
            .bind(&quest_key)
            .fetch_one(&mut *tx)
            .await?;
            let title_goals: Vec<i32> = p
                .slot
                .as_ref()
                .map(|(_, list)| list.iter().map(|(g, _)| *g).collect())
                .unwrap_or_default();
            sqlx::query(
                "DELETE FROM game_title_goal_progress WHERE cid=$1 AND title_goal_id=ANY($2)",
            )
            .bind(cid)
            .bind(&title_goals)
            .execute(&mut *tx)
            .await?;
            progress.retain(|(g, _)| !title_goals.contains(g));
            if let Some(t) = owned.iter_mut().find(|(id, _)| *id == p.title) {
                t.1 = true;
            }
            title_log_value(&mut tx, ctx.journal, ACQUIRE_TITLE, p.title, goal, -1).await?;
            time_stamp = crate::domain::legacy_user_data::SqlDateTime::from_units300(units)
                .and_then(|t| t.framework_date_time())
                .map_err(|_| fail())?
                .ticks();
            quest_id = quest.to_owned();
            acquired_any = true;
        }
        if let Some((status, list)) = p.slot {
            slots.push(Some(crate::protocol::inventory::TitleSlot {
                title_id: p.title,
                status,
                time_stamp,
                quest_id: Some(quest_id),
                goal_progress: Some(list),
                is_expireable: false,
                expire_date_time: -1,
            }));
        }
    }
    let mut packets = Vec::new();
    if !slots.is_empty() {
        if acquired_any {
            // Stats.TitleChanged: CalculateBaseStat, CalculateFinalStat, PushUpdateStat.
            calculate_base_stat(&mut tx, assets, cid).await?;
        }
        let view = super::minimum_selection::view(
            &mut tx,
            owner,
            assets,
            cid,
            ctx.config,
            ctx.arithmetic,
            false,
            0,
        )
        .await?;
        if acquired_any {
            packets.push(view.stat);
        }
        let update = crate::protocol::inventory::Message::UpdateTitle(Some(slots));
        packets.push(OutgoingPacket {
            category: crate::protocol::inventory::Kind::UpdateTitle.category(),
            body: crate::protocol::inventory::encode(
                crate::protocol::inventory::Kind::UpdateTitle,
                Some(&update),
                DEFAULT_MAX_BODY,
            )
            .map_err(|_| fail())?
            .to_vec(),
        });
        packets.push(view.common);
    }
    tx.commit().await?;
    Ok(packets)
}

/// InGame TitleObserver.ForceSync → TitleListMessage(AccountTitles, Titles) from
/// QueryTitleProgress. None without the progression profile (the observer is not wired).
pub(super) async fn title_list_packet(
    tx: &mut Transaction<'_, Postgres>,
    assets: &Assets,
    cid: i64,
) -> Result<Option<OutgoingPacket>, MinimumError> {
    use crate::domain::progression::{StoredTitle, TitleSlotValue};
    let Some(progression) = assets.npc.as_ref().and_then(|n| n.progression.as_ref()) else {
        return Ok(None);
    };
    let (uid, class): (i32, i32) =
        sqlx::query_as("SELECT uid,class::integer FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    type Row = (i64, i32, bool, Option<i64>, Option<Vec<u8>>, Option<i64>);
    let convert = |r: Row| -> Result<(i64, StoredTitle), MinimumError> {
        let ticks =
            r.3.map(|u| {
                crate::domain::legacy_user_data::SqlDateTime::from_units300(u)
                    .and_then(|t| t.framework_date_time())
                    .map(|t| t.ticks())
            })
            .transpose()
            .map_err(|_| fail())?;
        let quest =
            r.4.map(|b| {
                let units: Vec<u16> = b
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
                String::from_utf16(&units).map_err(|_| fail())
            })
            .transpose()?;
        Ok((
            r.0,
            StoredTitle {
                id: r.1,
                acquired: r.2,
                acquired_ticks: ticks,
                quest,
                expire_ticks: r.5,
            },
        ))
    };
    let own: Vec<Row> = sqlx::query_as(
        "SELECT cid,title_id,acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks FROM game_titles WHERE cid=$1 ORDER BY title_id LIMIT 65537",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    // TitleQueryAcquiredByAccount: acquired titles of every character of the account.
    let account: Vec<Row> = sqlx::query_as(
        "SELECT t.cid,t.title_id,t.acquired,t.acquired_time_units300,t.acquired_quest_utf16,t.expire_ticks FROM game_titles t JOIN game_character_info c ON c.id=t.cid WHERE c.uid=$1 AND t.acquired ORDER BY t.cid,t.title_id LIMIT 65537",
    )
    .bind(uid)
    .fetch_all(&mut **tx)
    .await?;
    let progress: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT title_goal_id,progress FROM game_title_goal_progress WHERE cid=$1 LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if own.len() > 65536 || account.len() > 65536 || progress.len() > 4096 {
        return Err(fail());
    }
    let own: Vec<StoredTitle> = own
        .into_iter()
        .map(|r| convert(r).map(|(_, t)| t))
        .collect::<Result<_, _>>()?;
    let account: Vec<(i64, StoredTitle)> =
        account.into_iter().map(convert).collect::<Result<_, _>>()?;
    let (shared, titles) = progression.title_list(cid, class, &own, &progress, &account);
    let slot = |v: TitleSlotValue| {
        Some(crate::protocol::inventory::TitleSlot {
            title_id: v.id,
            status: v.status,
            time_stamp: v.time_stamp,
            quest_id: Some(v.quest),
            goal_progress: Some(v.goals),
            is_expireable: v.expire.is_some(),
            expire_date_time: v.expire.unwrap_or(-1),
        })
    };
    let message =
        crate::protocol::inventory::Message::TitleList(crate::protocol::inventory::TitleList {
            account: Some(shared.into_iter().map(slot).collect()),
            titles: Some(titles.into_iter().map(slot).collect()),
        });
    Ok(Some(OutgoingPacket {
        category: crate::protocol::inventory::Kind::TitleList.category(),
        body: crate::protocol::inventory::encode(
            crate::protocol::inventory::Kind::TitleList,
            Some(&message),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    }))
}
