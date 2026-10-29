//! Filled list only for the explicit native minimum marker;legacy/source rows remain refused.
use super::{
    character_lobby::{self, QueryConfig, UserKey},
    fiona_character_state as state,
    fiona_initializer_profile::Initializers,
    minimum_character::MinimumError,
};
use crate::{
    application::lobby_costume,
    domain::legacy_user_data::SqlDateTime2,
    protocol::{
        DEFAULT_MAX_BODY,
        character_list::{self, Summary},
    },
};
use sqlx::{PgPool, Row};
use zeroize::Zeroizing;
fn text(b: Option<Vec<u8>>) -> Result<Option<Zeroizing<String>>, MinimumError> {
    b.map(|b| {
        if !b.len().is_multiple_of(2) {
            return Err(MinimumError::Conflict);
        }
        String::from_utf16(
            &b.as_chunks::<2>()
                .0
                .iter()
                .map(|a| u16::from_le_bytes(*a))
                .collect::<Vec<_>>(),
        )
        .map(Zeroizing::new)
        .map_err(|_| MinimumError::Conflict)
    })
    .transpose()
}
/// Complete recovered arithmetic,including two UTC reads when DeleteTime is null.
pub fn delete_wait(
    delay: i32,
    deleted: Option<SqlDateTime2>,
    mut utc: impl FnMut() -> Result<SqlDateTime2, MinimumError>,
) -> Result<i32, MinimumError> {
    let first = utc()?;
    let second = match deleted {
        Some(t) => t,
        None => utc()?,
    };
    let elapsed = ((first.ticks() - second.ticks()) as f64 / 10_000_000.0).trunc();
    let elapsed = if (i32::MIN as f64..2147483648.0).contains(&elapsed) {
        elapsed as i32
    } else {
        i32::MIN
    };
    Ok(delay.wrapping_mul(60).wrapping_sub(elapsed).max(0))
}
pub async fn query(
    pool: &PgPool,
    owner: &UserKey<'_>,
    profiles: Initializers<'_>,
    progression: Option<&crate::domain::progression::Progression>,
    config: QueryConfig,
    delete_delay: i32,
    mut utc: impl FnMut() -> Result<SqlDateTime2, MinimumError>,
) -> Result<Zeroizing<Vec<u8>>, MinimumError> {
    let mut tx = state::owned_account(pool, owner).await?;
    let ids = listed(&mut *tx, owner).await?;
    for c in &ids {
        // Same ownership lock for every listed character; class 10 (Delia) is listed for client
        // tests. Status 2 (waiting for deletion) is listed with its remaining time.
        let owned: Option<i64> = sqlx::query_scalar("SELECT id FROM game_character_info WHERE id=$1 AND uid=$2 AND class BETWEEN 0 AND 27 AND level>=1 AND NOT use_preset AND NOT is_premium AND status=$3 FOR UPDATE")
            .bind(c.cid)
            .bind(owner.uid)
            .bind(c.status)
            .fetch_optional(&mut *tx)
            .await?;
        if owned.is_none() {
            return Err(MinimumError::Conflict);
        }
    }
    let ids = ids.iter().map(|c| c.cid).collect::<Vec<_>>();
    let bytes = load_many(
        &mut tx,
        owner,
        profiles,
        progression,
        config,
        delete_delay,
        &mut utc,
        &ids,
    )
    .await?;
    tx.commit().await?;
    Ok(bytes)
}
/// One row of the list: CharacterInfo.IsVisible is status 0, or 2 while waiting for deletion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Listed {
    pub cid: i64,
    pub status: i32,
}
impl Listed {
    /// CharacterSummary.IsDeleting.
    pub fn deleting(self) -> bool {
        self.status == 2
    }
}
/// The account's visible characters in list order, bounded by the slot maximum. GetCharacters
/// orders by COALESCE(OrderNum,255),ID; QueryCharacters keeps the IsVisible rows.
pub(super) async fn listed(
    executor: impl sqlx::PgExecutor<'_>,
    owner: &UserKey<'_>,
) -> Result<Vec<Listed>, MinimumError> {
    let rows: Vec<(i64, i32)> = sqlx::query_as(
        "SELECT id,status FROM game_character_info WHERE uid=$1 AND status IN (0,2) ORDER BY COALESCE(order_num,255),id LIMIT 17",
    )
    .bind(owner.uid)
    .fetch_all(executor)
    .await?;
    if rows.len() > 16 {
        return Err(MinimumError::Conflict);
    }
    Ok(rows
        .into_iter()
        .map(|(cid, status)| Listed { cid, status })
        .collect())
}
/// The list row the client selects by index.
pub(super) async fn characters(
    pool: &PgPool,
    owner: &UserKey<'_>,
) -> Result<Vec<Listed>, MinimumError> {
    listed(pool, owner).await
}
/// Builds the summary while the caller retains ownership and character locks.
#[allow(clippy::too_many_arguments)]
pub(super) async fn load(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    profiles: Initializers<'_>,
    progression: Option<&crate::domain::progression::Progression>,
    config: QueryConfig,
    delete_delay: i32,
    utc: impl FnMut() -> Result<SqlDateTime2, MinimumError>,
    cid: i64,
) -> Result<Zeroizing<Vec<u8>>, MinimumError> {
    load_many(
        tx,
        owner,
        profiles,
        progression,
        config,
        delete_delay,
        utc,
        &[cid],
    )
    .await
}
/// The texts a summary borrows: name, quote and free title.
type Texts = [Option<Zeroizing<String>>; 3];
#[allow(clippy::too_many_arguments)]
async fn load_many(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    profiles: Initializers<'_>,
    progression: Option<&crate::domain::progression::Progression>,
    config: QueryConfig,
    delete_delay: i32,
    mut utc: impl FnMut() -> Result<SqlDateTime2, MinimumError>,
    cids: &[i64],
) -> Result<Zeroizing<Vec<u8>>, MinimumError> {
    let mut texts: Vec<Texts> = Vec::with_capacity(cids.len());
    let mut summaries: Vec<Summary<'static>> = Vec::with_capacity(cids.len());
    for &cid in cids {
        let (t, summary) = entry(
            tx,
            owner,
            profiles,
            progression,
            delete_delay,
            &mut utc,
            cid,
        )
        .await?;
        texts.push(t);
        summaries.push(summary);
    }
    let (purchased, premium, prologue, preset) = character_lobby::filled_header(
        tx,
        owner.uid,
        QueryConfig {
            max_free: config.max_free,
            care_clock: config.care_clock,
        },
    )
    .await
    .map_err(|_| MinimumError::Conflict)?;
    let characters = texts
        .iter()
        .zip(summaries)
        .map(|([name, quote, free_title], s)| {
            Some(Summary {
                character_id: name.as_deref().map(|s| s.as_str()),
                quote: quote.as_deref().map(|s| s.as_str()),
                free_title_name: free_title.as_deref().map(|s| s.as_str()),
                ..s
            })
        })
        .collect::<Vec<_>>();
    let states = vec![0; characters.len()];
    let bytes = character_list::encode(
        Some(&character_list::Message {
            characters: Some(characters),
            max_free: config.max_free,
            max_purchased: purchased,
            max_premium: premium,
            prologue_played: u8::from(prologue),
            preset_used: preset,
            // CharacterList.OnEnter adds one None (0) state per character even
            // with CombatRejoinSystem disabled. The native lobby indexes by row.
            party_states: Some(states),
        }),
        DEFAULT_MAX_BODY,
    )
    .map_err(|_| MinimumError::Conflict)?;
    Ok(bytes)
}
/// One character's summary; its texts are returned apart and borrowed by the caller.
async fn entry(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    profiles: Initializers<'_>,
    progression: Option<&crate::domain::progression::Progression>,
    delete_delay: i32,
    utc: &mut impl FnMut() -> Result<SqlDateTime2, MinimumError>,
    cid: i64,
) -> Result<(Texts, Summary<'static>), MinimumError> {
    let marker:Option<(i32,Vec<u8>,bool)>=sqlx::query_as("SELECT uid,profile_sha256,initialized FROM game_minimum_character_initialization WHERE cid=$1 FOR SHARE").bind(cid).fetch_optional(&mut **tx).await?;
    // The marker records the initializer of the character's class.
    let profile = match &marker {
        Some((uid, fingerprint, true)) if *uid == owner.uid => profiles
            .by_fingerprint(fingerprint)
            .ok_or(MinimumError::Conflict)?,
        _ => return Err(MinimumError::Conflict),
    };
    super::character_vocation::load_first(tx, cid)
        .await
        .map_err(|_| MinimumError::Conflict)?;
    // CharacterDataContext's list summary passes TitleCount 0; the in-game summary
    // (minimum_selection::view) counts the acquired titles.
    let info=sqlx::query("SELECT name_utf16,character_sn,class,level,exp,selected_title,total_used_ap,quote_utf16,is_premium,free_match_win_count,free_match_lose_count,free_title_name_utf16,delete_time_units300,status FROM game_character_info WHERE id=$1 FOR SHARE").bind(cid).fetch_one(&mut **tx).await?;
    let name = text(info.try_get("name_utf16")?)?;
    let quote = text(info.try_get("quote_utf16")?)?;
    let free_title = text(info.try_get("free_title_name_utf16")?)?;
    let delete: Option<i64> = info.try_get("delete_time_units300")?;
    let delete = delete
        .map(crate::domain::legacy_user_data::SqlDateTime::from_units300)
        .transpose()
        .and_then(|d| d.map(|d| d.framework_date_time()).transpose())
        .map_err(|_| MinimumError::Conflict)?;
    let wait = delete_wait(delete_delay, delete, &mut *utc)?;
    let micro:Option<(i32,i32)>=sqlx::query_as("SELECT daily_micro_play_count,daily_free_play_count FROM game_micro_play_status WHERE cid=$1").bind(cid).fetch_optional(&mut **tx).await?;
    let (daily, free) = micro.unwrap_or((0, 0));
    let set = super::character_costumes::load_for_constructor(tx, cid)
        .await
        .map_err(|_| MinimumError::Conflict)?;
    let costume = lobby_costume::project(&set).map_err(|_| MinimumError::Conflict)?;
    // Optional guild/pet/event/arena services are explicitly deferred by this native profile.
    // These are initial native fields,not a general projection of existing legacy characters.
    let summary = Summary {
        cid,
        nexon_sn: owner.uid,
        character_id: None,
        character_sn: info.try_get("character_sn")?,
        base_character: u8::try_from(info.try_get::<i16, _>("class")?)
            .map_err(|_| MinimumError::Conflict)?,
        level: info.try_get("level")?,
        exp: info.try_get::<i64, _>("exp")? as i32,
        // CharacterEntity.LevelUpExp: levelUpExpDic of the stored level.
        level_up_exp: match info.try_get::<i32, _>("level")? {
            1 => profile.level_up_exp(),
            level => progression
                .and_then(|p| p.level_exp(level))
                .ok_or(MinimumError::Conflict)?,
        },
        title: info
            .try_get::<Option<i32>, _>("selected_title")?
            .ok_or(MinimumError::Conflict)?,
        title_count: 0,
        costume: Some(costume),
        daily_micro_play_count: daily,
        daily_free_play_count: free,
        quote: None,
        total_used_ap: info.try_get("total_used_ap")?,
        guild_id: 0,
        guild_name: Some(""),
        cafe_type: 0,
        is_premium: info.try_get("is_premium")?,
        has_bonus_effect: false,
        is_return: false,
        is_deleting: info.try_get::<i32, _>("status")? == 2,
        delete_wait_left_sec: wait,
        is_should_name_change: false,
        vip_code: 0,
        vocation_class: -1,
        vocation_level: 0,
        vocation_exp: 0,
        vocation_level_up_exp: -1,
        vocation_skill_point_available: 0,
        free_match_win_count: info.try_get("free_match_win_count")?,
        free_match_lose_count: info.try_get("free_match_lose_count")?,
        is_event_jumping: false,
        free_title_name: None,
        pattern: 0,
        arena_win_count: 0,
        arena_lose_count: 0,
        arena_successive_win_count: 0,
    };
    Ok(([name, quote, free_title], summary))
}
/// CharacterList SelectCharacter: true when the row at `index` is waiting for deletion
/// (CharacterSummary.IsDeleting), which the source ignores without a response.
pub(super) async fn deleting_at(
    pool: &PgPool,
    owner: &UserKey<'_>,
    index: i32,
) -> Result<bool, MinimumError> {
    let rows = characters(pool, owner).await?;
    Ok(usize::try_from(index)
        .ok()
        .and_then(|i| rows.get(i))
        .is_some_and(|c| c.deleting()))
}
