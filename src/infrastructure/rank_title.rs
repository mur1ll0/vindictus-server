//! Rank titles (B5 RK3): RankTitleManager keeps the weekly HDV_GetRank rows within
//! RankTitleLimit (queried at start and after each RankPeriodInfo 2 boundary, three seconds
//! late), and GuildInfoManager.UpdateRankTitles / TitleManager.UpdateRankTitle give the
//! character the RankTitle of each rank it (or its guild) holds. Rank titles live in memory,
//! as in the source; GiveTitle stores a Known row for a title the character never owned.
//! InformSelectedTitleToItemService and the character AddLog lines are not migrated.
use super::character_lobby::UserKey;
use super::minimum_assets::Assets;
use super::minimum_character::MinimumError;
use crate::domain::map_partition::OutgoingPacket;
use crate::domain::rank::{self as rules, HighRank, TitleUpdate};
use crate::protocol::DEFAULT_MAX_BODY;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::RwLock;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// CharacterTitleEvent.GiveTitle, SelectTitle and GiveRankTitle.
const GIVE_TITLE: i32 = 2;
const SELECT_TITLE: i32 = 3;
const GIVE_RANK_TITLE: i32 = 8;
/// SystemMessageCategory.Telepathy.
const TELEPATHY: u8 = 2;
/// The weekly PeriodType of RankTitleManager (RankPeriodInfo 2, QueryRank(2, limit)).
const WEEKLY: i16 = 2;
/// Bound of the kept rows (every event within the limit).
const MAX_ROWS: i64 = 100_000;

/// RankTitleManager.highRankList, shared by every connection.
#[derive(Debug, Default)]
pub struct HighRanks {
    list: RwLock<Vec<HighRank>>,
}
impl HighRanks {
    pub fn new() -> Self {
        Self::default()
    }
    /// GetHighRankList.
    pub fn get(&self) -> Vec<HighRank> {
        match self.list.read() {
            Ok(list) => list.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
    fn set(&self, rows: Vec<HighRank>) {
        match self.list.write() {
            Ok(mut list) => *list = rows,
            Err(poisoned) => *poisoned.into_inner() = rows,
        }
    }
    /// UpdateHighRank's QueryRank(2, RankTitleLimit) → HDV_GetRank for both IsGuild values:
    /// the weekly rows whose n4Rank and n4Order are within the limit, of the ranked events,
    /// by EventID and n4Order. Nothing changes with RankTitle off.
    pub async fn refresh(&self, pool: &PgPool, assets: &Assets) -> Result<(), MinimumError> {
        let (Some(policy), Some(economy)) = (assets.npc_policy(), assets.economy.as_ref()) else {
            return Ok(());
        };
        if !policy.is_enabled(Some("RankTitle")).map_err(|_| fail())? {
            return Ok(());
        }
        let limit = policy.get_integer("RankTitleLimit").map_err(|_| fail())?;
        let described = |e: i32| economy.rank_descriptions().iter().any(|d| d.event_id == e);
        let rows = load_rows(pool, limit).await?;
        self.set(rows.into_iter().filter(|h| described(h.event_id)).collect());
        Ok(())
    }
}

/// HDV_GetRank(2, limit) without the RankDescription join: the weekly rows whose n4Rank and
/// n4Order are within `limit`, by EventID and n4Order.
pub async fn load_rows(pool: &PgPool, limit: i32) -> Result<Vec<HighRank>, MinimumError> {
    let rows: Vec<(i32, i32, i64)> = sqlx::query_as(
        "SELECT event_id,n4_rank,cid FROM game_rank_result \
         WHERE period_type=$1 AND n4_rank<=$2 AND n4_order<=$2 \
         ORDER BY event_id,n4_order LIMIT $3",
    )
    .bind(WEEKLY)
    .bind(limit)
    .bind(MAX_ROWS)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(event_id, rank, entity)| HighRank {
            event_id,
            rank,
            entity,
        })
        .collect())
}

/// CharacterContents.IsGuildRank(EventID): its RankDescription has IsGuild.
fn is_guild_event(assets: &Assets, event: i32) -> bool {
    assets.economy.as_ref().is_some_and(|e| {
        e.rank_descriptions()
            .iter()
            .any(|d| d.event_id == event && d.is_guild == Some(1))
    })
}

fn rank_title_rows(assets: &Assets) -> Vec<(i32, i32, i32)> {
    assets.economy.as_ref().map_or_else(Vec::new, |e| {
        e.rank_titles()
            .iter()
            .map(|r| (r.event_id, r.rank, r.title_id))
            .collect()
    })
}

/// CharacterContents.IsGuildRankByTitleID.
fn is_guild_title(assets: &Assets, rows: &[(i32, i32, i32)], title: i32) -> bool {
    rows.iter()
        .find(|(.., t)| *t == title)
        .is_some_and(|(e, ..)| is_guild_event(assets, *e))
}

fn telepathy(format: &str, title: i32) -> Result<OutgoingPacket, MinimumError> {
    super::minimum_shop::system_message(TELEPATHY, format, &[title.to_string()])
}

/// TitleManager.SendCharacterRankTitleUpdatedTelepathy (`guild` false) and
/// GuildInfoManager.SendGuildRankTitleUpdatedTelepathy (`guild` true): one Telepathy per held
/// rank title of that kind.
pub fn updated_telepathy(
    assets: &Assets,
    held: &[i32],
    guild: bool,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let rows = rank_title_rows(assets);
    held.iter()
        .filter(|t| is_guild_title(assets, &rows, **t) == guild)
        .map(|t| telepathy("GameUI_Heroes_NPC_Acquire_Title_Successful_NoStat", *t))
        .collect()
}

/// GuildInfoManager.UpdateRankTitles(type) with TitleManager.UpdateRankTitle: `held` is the
/// connection's RankTitles. Packets: the SelectTitle(0) reset with its Telepathy, then
/// PushUpdateStat, PushTitleUpdate (UpdateTitle, CharacterCommonInfo) and the Title observer
/// (TitleListMessage with the rank titles). Nothing with RankTitle off.
#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    growth: &super::minimum_growth::GrowthContext,
    cid: i64,
    high: &[HighRank],
    held: &mut Vec<i32>,
    kind: TitleUpdate,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let Some(policy) = assets.npc_policy() else {
        return Ok(Vec::new());
    };
    if !policy.is_enabled(Some("RankTitle")).map_err(|_| fail())? {
        return Ok(Vec::new());
    }
    let progression = assets.npc.as_ref().and_then(|n| n.progression.as_ref());
    let rows = rank_title_rows(assets);
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let (class, selected): (i32, i32) =
        sqlx::query_as("SELECT class::integer,selected_title FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    let (sn, ..) = super::guild::brief_on(&mut tx, cid).await?;
    let guild = (sn != 0).then_some(i64::from(sn));
    let list = rules::rank_titles(high, &rows, |e| is_guild_event(assets, e), cid, guild);
    let info = |t: i32| progression.and_then(|p| p.rank_title(t, class));
    let change = rules::update_rank_titles(
        held,
        &list,
        |t| info(t).is_some(),
        |t| is_guild_title(assets, &rows, t),
        |t| rows.iter().any(|(.., r)| *r == t),
        kind,
        selected,
    );
    let mut packets = apply_on(&mut tx, growth.journal, cid, &change, info, selected).await?;
    let view = super::minimum_selection::view(
        &mut tx,
        owner,
        assets,
        cid,
        growth.config,
        growth.arithmetic,
        false,
        0,
    )
    .await?;
    packets.push(view.stat);
    let slot = |title_id: i32, status: i32| {
        Some(crate::protocol::inventory::TitleSlot {
            title_id,
            status,
            time_stamp: 0,
            quest_id: Some(String::new()),
            goal_progress: Some(Vec::new()),
            is_expireable: false,
            expire_date_time: -1,
        })
    };
    // TitleSlotInfo(id, Acquired | Known, 0, "", {}, null).
    let slots = change
        .acquired_slots
        .iter()
        .map(|t| slot(*t, 2))
        .chain(change.removed_slots.iter().map(|t| slot(*t, 1)))
        .collect();
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
    if let Some(list) =
        super::minimum_growth::title_list_packet(&mut tx, assets, cid, &change.held).await?
    {
        packets.push(list);
    }
    tx.commit().await?;
    *held = change.held;
    Ok(packets)
}

/// TitleManager.UpdateRankTitle's writes: GiveRankTitle of each given title (GiveTitle's
/// Known row when the character has none and its class may hold it; the GiveRankTitle log)
/// and the SelectedTitle reset, with its SelectTitle(0) and Telepathy.
pub async fn apply_on(
    tx: &mut Transaction<'_, Postgres>,
    journal: i64,
    cid: i64,
    change: &rules::RankTitleChange,
    info: impl Fn(i32) -> Option<crate::domain::progression::RankTitleInfo>,
    selected: i32,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    for &title in &change.given {
        // GiveRankTitle: GiveTitle when the character has no such title (a Known row; the
        // first goal's progress is HideUntil when not zero).
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM game_titles WHERE cid=$1 AND title_id=$2)",
        )
        .bind(cid)
        .bind(title)
        .fetch_one(&mut **tx)
        .await?;
        if let Some(t) = info(title).filter(|t| !owned && t.available) {
            sqlx::query("INSERT INTO game_titles(cid,title_id,acquired) VALUES($1,$2,false)")
                .bind(cid)
                .bind(title)
                .execute(&mut **tx)
                .await?;
            super::minimum_growth::title_log(tx, journal, GIVE_TITLE, title).await?;
            if let (Some(goal), true) = (t.goals.first(), t.hide_until != 0) {
                sqlx::query(
                    "INSERT INTO game_title_goal_progress(cid,title_goal_id,progress) VALUES($1,$2,$3) \
                     ON CONFLICT(cid,title_goal_id) DO UPDATE SET progress=EXCLUDED.progress",
                )
                .bind(cid)
                .bind(*goal)
                .bind(t.hide_until)
                .execute(&mut **tx)
                .await?;
            }
        }
        super::minimum_growth::title_log(tx, journal, GIVE_RANK_TITLE, title).await?;
    }
    let mut packets = Vec::new();
    if change.unselect {
        sqlx::query("UPDATE game_character_info SET selected_title=0 WHERE id=$1")
            .bind(cid)
            .execute(&mut **tx)
            .await?;
        super::minimum_growth::title_log(tx, journal, SELECT_TITLE, 0).await?;
        packets.push(select_title_packet(0)?);
        packets.push(telepathy(
            "GameUI_Heroes_NPC_Delete_Title_Successful",
            selected,
        )?);
    }
    Ok(packets)
}

/// SelectTitleMessage(title).
pub fn select_title_packet(title: i32) -> Result<OutgoingPacket, MinimumError> {
    use crate::protocol::title::{self as wire, Kind, Message};
    let kind = Kind::SelectTitle;
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&Message::SelectTitle(title)), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// SelectTitleProcessor: 0, an acquired title or a held rank title becomes SelectedTitle
/// (SelectTitle log); the frontend then echoes SelectTitleMessage. `None` for a title the
/// character does not hold (FailMessage: the frontend sends nothing).
pub async fn select(
    pool: &PgPool,
    owner: &UserKey<'_>,
    journal: i64,
    cid: i64,
    held: &[i32],
    title: i32,
) -> Result<Option<OutgoingPacket>, MinimumError> {
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let packet = select_on(&mut tx, journal, cid, held, title).await?;
    tx.commit().await?;
    Ok(packet)
}

/// SelectTitleProcessor in `tx` (see `select`).
pub async fn select_on(
    tx: &mut Transaction<'_, Postgres>,
    journal: i64,
    cid: i64,
    held: &[i32],
    title: i32,
) -> Result<Option<OutgoingPacket>, MinimumError> {
    let acquired: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_titles WHERE cid=$1 AND title_id=$2 AND acquired)",
    )
    .bind(cid)
    .bind(title)
    .fetch_one(&mut **tx)
    .await?;
    if title != 0 && !acquired && !held.contains(&title) {
        return Ok(None);
    }
    sqlx::query("UPDATE game_character_info SET selected_title=$2 WHERE id=$1")
        .bind(cid)
        .bind(title)
        .execute(&mut **tx)
        .await?;
    super::minimum_growth::title_log(tx, journal, SELECT_TITLE, title).await?;
    Ok(Some(select_title_packet(title)?))
}
