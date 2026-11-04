//! RankService UpdateRankBasis (B5 ranking RK1): the RankBasis rows of heroesRank (0064) kept
//! by RankBasisUpdate. UpdateRankBasisUtility sends only with FeatureMatrix CharacterRank on;
//! the processor answers Ok without writing for a RankID whose Feature is off or whose
//! EventID is −1, and the ScoreLimit is read but unused, as in the procedure.
use super::minimum_assets::Assets;
use super::minimum_character::MinimumError;
use crate::domain::rank::{self as rules, Basis, ScoreType, Update};
use sqlx::PgPool;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// RankContents.IsEnableRankID and the score rules of a RankID: (EventID, ScoreType,
/// ascending), None when the ranking does not count it.
pub fn event(
    assets: &Assets,
    rank_id: &str,
) -> Result<Option<(i32, ScoreType, bool)>, MinimumError> {
    let (Some(policy), Some(economy)) = (assets.npc_policy(), assets.economy.as_ref()) else {
        return Ok(None);
    };
    if !policy
        .is_enabled(Some("CharacterRank"))
        .map_err(|_| fail())?
    {
        return Ok(None);
    }
    let Some(row) = economy
        .rank_descriptions()
        .iter()
        .find(|r| r.rank_id.as_deref() == Some(rank_id))
    else {
        return Ok(None);
    };
    if row.event_id == -1
        || !policy
            .is_enabled(row.feature.as_deref())
            .map_err(|_| fail())?
    {
        return Ok(None);
    }
    Ok(Some((
        row.event_id,
        ScoreType::parse(row.score_type.as_deref()),
        rules::ascending(row.sort_type.as_deref()),
    )))
}

/// UpdateRankBasis(CID, RankID, Score, EntityID) at `now` (Unix seconds); true when a row was
/// written.
pub async fn update_rank_basis_at(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
    rank_id: &str,
    score: i64,
    entity_id: i64,
    now: i64,
) -> Result<bool, MinimumError> {
    let Some((event_id, score_type, ascending)) = event(assets, rank_id)? else {
        return Ok(false);
    };
    basis_update(
        pool, cid, event_id, score_type, ascending, score, entity_id, now,
    )
    .await
}

/// The RankBasisUpdate procedure for one event; true when a row was written.
#[allow(clippy::too_many_arguments)]
pub async fn basis_update(
    pool: &PgPool,
    cid: i64,
    event_id: i32,
    score_type: ScoreType,
    ascending: bool,
    score: i64,
    entity_id: i64,
    now: i64,
) -> Result<bool, MinimumError> {
    let mut tx = pool.begin().await?;
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT row_id,score,last_update_seconds FROM game_rank_basis \
         WHERE cid=$1 AND event_id=$2 ORDER BY row_id DESC LIMIT 1 FOR UPDATE",
    )
    .bind(cid)
    .bind(event_id)
    .fetch_optional(&mut *tx)
    .await?;
    let current = row.map(|(_, score, last_update)| Basis { score, last_update });
    let written = match rules::update(current, score, score_type, ascending, now) {
        Update::Insert(b) => {
            sqlx::query(
                "INSERT INTO game_rank_basis(cid,event_id,score,last_update_seconds,entity_id) VALUES($1,$2,$3,$4,$5)",
            )
            .bind(cid)
            .bind(event_id)
            .bind(b.score)
            .bind(b.last_update)
            .bind(entity_id)
            .execute(&mut *tx)
            .await?;
            true
        }
        Update::Set(b) => {
            let (row_id, ..) = row.ok_or_else(fail)?;
            sqlx::query(
                "UPDATE game_rank_basis SET score=$2,last_update_seconds=$3,entity_id=$4 WHERE row_id=$1",
            )
            .bind(row_id)
            .bind(b.score)
            .bind(b.last_update)
            .bind(entity_id)
            .execute(&mut *tx)
            .await?;
            true
        }
        Update::Rollover { previous, new } => {
            let (row_id, ..) = row.ok_or_else(fail)?;
            sqlx::query(
                "INSERT INTO game_rank_basis_history(row_id,score,last_update_seconds,entity_id) VALUES($1,$2,$3,$4)",
            )
            .bind(row_id)
            .bind(previous.score)
            .bind(previous.last_update)
            .bind(entity_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE game_rank_basis SET score=$2,last_update_seconds=$3,entity_id=$4 WHERE row_id=$1",
            )
            .bind(row_id)
            .bind(new.score)
            .bind(new.last_update)
            .bind(entity_id)
            .execute(&mut *tx)
            .await?;
            true
        }
        Update::Keep => false,
    };
    tx.commit().await?;
    Ok(written)
}

/// UpdateRankBasis now.
pub async fn update_rank_basis(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
    rank_id: &str,
    score: i64,
    entity_id: i64,
) -> Result<bool, MinimumError> {
    update_rank_basis_at(pool, assets, cid, rank_id, score, entity_id, now_seconds()).await
}
