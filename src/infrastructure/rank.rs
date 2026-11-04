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

/// The enabled ranking events: (EventID, ScoreType, ascending), in EventID order.
pub fn events(assets: &Assets) -> Result<Vec<(i32, ScoreType, bool)>, MinimumError> {
    let (Some(policy), Some(economy)) = (assets.npc_policy(), assets.economy.as_ref()) else {
        return Ok(Vec::new());
    };
    if !policy
        .is_enabled(Some("CharacterRank"))
        .map_err(|_| fail())?
    {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for row in economy.rank_descriptions() {
        if row.event_id != -1
            && policy
                .is_enabled(row.feature.as_deref())
                .map_err(|_| fail())?
        {
            out.push((
                row.event_id,
                ScoreType::parse(row.score_type.as_deref()),
                rules::ascending(row.sort_type.as_deref()),
            ));
        }
    }
    out.sort_by_key(|e| e.0);
    out.dedup_by_key(|e| e.0);
    Ok(out)
}

/// The decision D-S3 publication at `now`: when the latest 07:00 UTC boundary is newer than
/// the last published one, the cumulative Count scores are stored at the boundary and each
/// period's results (daily, week to date, month to date) replace the published ones, with
/// n4Rank_Prev from the previous publication; the heavy favorites follow. True when it
/// published.
pub async fn publish(
    pool: &PgPool,
    events: &[(i32, ScoreType, bool)],
    now: i64,
) -> Result<bool, MinimumError> {
    let boundary = rules::daily_boundary(now);
    let mut tx = pool.begin().await?;
    let last: Option<i64> = sqlx::query_scalar(
        "SELECT last_boundary_seconds FROM game_rank_publication WHERE singleton FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if last.is_some_and(|l| l >= boundary) {
        return Ok(false);
    }
    let counts: Vec<i32> = events
        .iter()
        .filter(|e| e.1 == ScoreType::Count)
        .map(|e| e.0)
        .collect();
    sqlx::query(
        "INSERT INTO game_rank_count_snapshot(boundary_seconds,cid,event_id,score) \
         SELECT $1,cid,event_id,score FROM game_rank_basis WHERE event_id=ANY($2) \
         ON CONFLICT DO NOTHING",
    )
    .bind(boundary)
    .bind(&counts)
    .execute(&mut *tx)
    .await?;
    for (period_type, start) in rules::period_starts(boundary) {
        for &(event_id, score_type, ascending) in events {
            let scores: Vec<(i64, i64)> = match score_type {
                ScoreType::Time => sqlx::query_as(&format!(
                    "SELECT x.cid,{}(x.score) FROM (\
                       SELECT cid,score,last_update_seconds FROM game_rank_basis WHERE event_id=$1 \
                       UNION ALL SELECT b.cid,h.score,h.last_update_seconds FROM game_rank_basis_history h \
                       JOIN game_rank_basis b ON b.row_id=h.row_id WHERE b.event_id=$1) x \
                     JOIN game_character_info c ON c.id=x.cid AND c.status=0 \
                     WHERE x.last_update_seconds>=$2 AND x.last_update_seconds<$3 GROUP BY x.cid LIMIT 100000",
                    if ascending { "min" } else { "max" }
                ))
                .bind(event_id)
                .bind(start)
                .bind(boundary)
                .fetch_all(&mut *tx)
                .await?,
                ScoreType::Count => sqlx::query_as(
                    "SELECT n.cid,n.score-COALESCE(o.score,0) FROM game_rank_count_snapshot n \
                     LEFT JOIN game_rank_count_snapshot o ON o.boundary_seconds=$2 AND o.event_id=n.event_id AND o.cid=n.cid \
                     JOIN game_character_info c ON c.id=n.cid AND c.status=0 \
                     WHERE n.boundary_seconds=$3 AND n.event_id=$1 AND n.score-COALESCE(o.score,0)>0 LIMIT 100000",
                )
                .bind(event_id)
                .bind(start)
                .bind(boundary)
                .fetch_all(&mut *tx)
                .await?,
            };
            let previous: std::collections::HashMap<i64, i32> = sqlx::query_as(
                "SELECT cid,n4_rank FROM game_rank_result WHERE period_type=$1 AND event_id=$2",
            )
            .bind(period_type as i16)
            .bind(event_id)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();
            sqlx::query("DELETE FROM game_rank_result WHERE period_type=$1 AND event_id=$2")
                .bind(period_type as i16)
                .bind(event_id)
                .execute(&mut *tx)
                .await?;
            for r in rules::rank_scores(scores, ascending) {
                sqlx::query(
                    "INSERT INTO game_rank_result(period_type,event_id,cid,score,n4_order,n4_rank,n4_rank_prev) \
                     VALUES($1,$2,$3,$4,$5,$6,$7)",
                )
                .bind(period_type as i16)
                .bind(event_id)
                .bind(r.cid)
                .bind(r.score)
                .bind(r.order)
                .bind(r.rank)
                .bind(previous.get(&r.cid).copied().unwrap_or(0))
                .execute(&mut *tx)
                .await?;
            }
        }
    }
    // RankProc_End HeavyFavorites: the most favorited events first (the procedure orders
    // ascending, read here as its intent), with the previous rank.
    let favorites: Vec<(i32, i64)> =
        sqlx::query_as("SELECT event_id,count(*) FROM game_rank_favorite GROUP BY event_id")
            .fetch_all(&mut *tx)
            .await?;
    let previous: std::collections::HashMap<i64, i32> =
        sqlx::query_as("SELECT event_id::bigint,n4_rank FROM game_rank_heavy_favorite")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();
    sqlx::query("DELETE FROM game_rank_heavy_favorite")
        .execute(&mut *tx)
        .await?;
    let ranked = rules::rank_scores(
        favorites
            .into_iter()
            .map(|(e, n)| (i64::from(e), n))
            .collect(),
        false,
    );
    for r in ranked {
        sqlx::query(
            "INSERT INTO game_rank_heavy_favorite(event_id,favorites,n4_rank,n4_rank_prev) VALUES($1,$2,$3,$4)",
        )
        .bind(i32::try_from(r.cid).map_err(|_| fail())?)
        .bind(r.score)
        .bind(r.rank)
        .bind(previous.get(&r.cid).copied().unwrap_or(0))
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "INSERT INTO game_rank_publication(singleton,last_boundary_seconds) VALUES(TRUE,$1) \
         ON CONFLICT (singleton) DO UPDATE SET last_boundary_seconds=EXCLUDED.last_boundary_seconds",
    )
    .bind(boundary)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}
