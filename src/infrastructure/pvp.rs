//! Free duels (B5 PvP PV1): FreeMatchReportProcessor resolves the MicroPlay host's winner and
//! loser tags to the play's players, then IncreaseFreeMatchWinCount and
//! IncreaseFreeMatchLoseCount add one to CharacterInfo.FreeMatchWinCount/LoseCount (kept at
//! int.MaxValue) with FreeMatch_Detail on. Nothing is sent back; the counts reach the
//! CharacterSummary read from the character row.
use super::minimum_assets::Assets;
use super::minimum_character::MinimumError;
use sqlx::PgPool;

/// FreeMatchReportProcessor: the CIDs of the winner and loser tags among `players`
/// (MicroPlay.Players: slot → CID); `None` when either tag is not a player.
pub fn resolve(players: &[(i32, i64)], winner_tag: i32, loser_tag: i32) -> Option<(i64, i64)> {
    let cid = |tag: i32| players.iter().find(|(s, _)| *s == tag).map(|(_, c)| *c);
    Some((cid(winner_tag)?, cid(loser_tag)?))
}

/// IncreaseFreeMatchWinCount (`win`) or IncreaseFreeMatchLoseCount of `cid`; false (a
/// LogicalFail) with FreeMatch_Detail off or no such character.
pub async fn increase(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
    win: bool,
) -> Result<bool, MinimumError> {
    let enabled = assets
        .npc_policy()
        .map(|p| p.is_enabled(Some("FreeMatch_Detail")))
        .transpose()
        .map_err(|_| MinimumError::Conflict)?
        .unwrap_or(false);
    if !enabled {
        return Ok(false);
    }
    increase_count(pool, cid, win).await
}

/// The CharacterInfo write of `increase` (no FeatureMatrix check); false for no character.
pub async fn increase_count(pool: &PgPool, cid: i64, win: bool) -> Result<bool, MinimumError> {
    let query = if win {
        "UPDATE game_character_info SET free_match_win_count=free_match_win_count+1 \
         WHERE id=$1 AND free_match_win_count<2147483647"
    } else {
        "UPDATE game_character_info SET free_match_lose_count=free_match_lose_count+1 \
         WHERE id=$1 AND free_match_lose_count<2147483647"
    };
    sqlx::query(query).bind(cid).execute(pool).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_character_info WHERE id=$1)")
            .bind(cid)
            .fetch_one(pool)
            .await?;
    Ok(exists)
}
