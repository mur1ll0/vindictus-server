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
    let written = basis_write(
        &mut tx, cid, event_id, score_type, ascending, score, entity_id, now,
    )
    .await?;
    tx.commit().await?;
    Ok(written)
}

/// UpdateRankBasis now from inside another transaction, in a savepoint: a failed request
/// leaves the caller's work untouched, as the original's separate RankService connection.
pub async fn update_rank_basis_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assets: &Assets,
    cid: i64,
    rank_id: &str,
    score: i64,
    entity_id: i64,
) -> Result<bool, MinimumError> {
    let Some((event_id, score_type, ascending)) = event(assets, rank_id)? else {
        return Ok(false);
    };
    let mut savepoint = sqlx::Acquire::begin(&mut **tx).await?;
    let written = basis_write(
        &mut savepoint,
        cid,
        event_id,
        score_type,
        ascending,
        score,
        entity_id,
        now_seconds(),
    )
    .await?;
    savepoint.commit().await?;
    Ok(written)
}

#[allow(clippy::too_many_arguments)]
async fn basis_write(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cid: i64,
    event_id: i32,
    score_type: ScoreType,
    ascending: bool,
    score: i64,
    entity_id: i64,
    now: i64,
) -> Result<bool, MinimumError> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT row_id,score,last_update_seconds FROM game_rank_basis \
         WHERE cid=$1 AND event_id=$2 ORDER BY row_id DESC LIMIT 1 FOR UPDATE",
    )
    .bind(cid)
    .bind(event_id)
    .fetch_optional(&mut **tx)
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
            .execute(&mut **tx)
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
            .execute(&mut **tx)
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
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "UPDATE game_rank_basis SET score=$2,last_update_seconds=$3,entity_id=$4 WHERE row_id=$1",
            )
            .bind(row_id)
            .bind(new.score)
            .bind(new.last_update)
            .bind(entity_id)
            .execute(&mut **tx)
            .await?;
            true
        }
        Update::Keep => false,
    };
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

use crate::protocol::rank::RankResult;

/// EventID, n4Rank, n4Rank_Prev, Score and the character's name.
type ResultRow = (i32, i32, i32, i64, Option<Vec<u8>>);

/// The published rows of one period as RankResultInfo, with or without the character name
/// (GetRank* fill it; GetRankByCID leaves it "").
async fn results(
    pool: &PgPool,
    filter: &str,
    bind_i64: i64,
    period_type: i32,
    named: bool,
    limit: i64,
) -> Result<Vec<Option<RankResult>>, MinimumError> {
    let rows: Vec<ResultRow> = sqlx::query_as(&format!(
        "SELECT r.event_id,r.n4_rank,r.n4_rank_prev,r.score,c.name_utf16 FROM game_rank_result r \
         JOIN game_character_info c ON c.id=r.cid AND c.status=0 \
         WHERE r.period_type=$2 AND {filter} ORDER BY r.event_id,r.n4_order LIMIT $3"
    ))
    .bind(bind_i64)
    .bind(period_type as i16)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(event_id, rank, rank_prev, score, name)| {
            let character_name = if named {
                name.as_deref()
                    .map(super::minimum_party::decode)
                    .transpose()?
                    .or_else(|| Some(String::new()))
            } else {
                Some(String::new())
            };
            Ok(Some(RankResult {
                event_id,
                rank,
                rank_prev,
                score,
                character_name,
            }))
        })
        .collect()
}

/// GetRankByCID: every published row of the character in the period.
pub async fn by_cid(
    pool: &PgPool,
    cid: i64,
    period_type: i32,
) -> Result<Vec<Option<RankResult>>, MinimumError> {
    results(pool, "r.cid=$1", cid, period_type, false, 10_000).await
}

/// GetRankByEventID: the first 100 of one event, with names.
pub async fn by_event(
    pool: &PgPool,
    event_id: i32,
    period_type: i32,
) -> Result<Vec<Option<RankResult>>, MinimumError> {
    results(
        pool,
        "r.event_id=$1::integer AND r.n4_rank<=100 AND r.n4_order<=100",
        i64::from(event_id),
        period_type,
        true,
        100,
    )
    .await
}

/// GetRank(P, RankLimit 1): the first of every event, with names.
pub async fn firsts(
    pool: &PgPool,
    period_type: i32,
) -> Result<Vec<Option<RankResult>>, MinimumError> {
    results(
        pool,
        "$1=$1 AND r.n4_rank<=1 AND r.n4_order<=1",
        0,
        period_type,
        true,
        10_000,
    )
    .await
}

/// GetRankFavorites: the character's favorite events with its published row (0 when it is
/// not ranked); RankPrev is 0.
pub async fn favorites(
    pool: &PgPool,
    cid: i64,
    period_type: i32,
) -> Result<Vec<Option<RankResult>>, MinimumError> {
    let rows: Vec<(i32, Option<i32>, Option<i64>)> = sqlx::query_as(
        "SELECT f.event_id,r.n4_rank,r.score FROM game_rank_favorite f \
         LEFT JOIN game_rank_result r ON r.event_id=f.event_id AND r.cid=f.cid AND r.period_type=$2 \
         WHERE f.cid=$1 ORDER BY f.event_id LIMIT 1000",
    )
    .bind(cid)
    .bind(period_type as i16)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(event_id, rank, score)| {
            Some(RankResult {
                event_id,
                rank: rank.unwrap_or(0),
                rank_prev: 0,
                score: score.unwrap_or(0),
                character_name: Some(String::new()),
            })
        })
        .collect())
}

/// RankResultHeavyFavorites: the most favorited events (Score 0).
pub async fn heavy_favorites(pool: &PgPool) -> Result<Vec<Option<RankResult>>, MinimumError> {
    let rows: Vec<(i32, i32, i32)> = sqlx::query_as(
        "SELECT event_id,n4_rank,n4_rank_prev FROM game_rank_heavy_favorite ORDER BY n4_rank,event_id LIMIT 100",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(event_id, rank, rank_prev)| {
            Some(RankResult {
                event_id,
                rank,
                rank_prev,
                score: 0,
                character_name: Some(String::new()),
            })
        })
        .collect())
}

/// UpdateRankFavorite (RankFavoriteInsert/Delete) of an enabled RankID.
pub async fn set_favorite(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
    rank_id: &str,
    addition: bool,
) -> Result<(), MinimumError> {
    let Some((event_id, ..)) = event(assets, rank_id)? else {
        return Ok(());
    };
    if addition {
        sqlx::query(
            "INSERT INTO game_rank_favorite(cid,event_id,last_update_seconds) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
        )
        .bind(cid)
        .bind(event_id)
        .bind(now_seconds())
        .execute(pool)
        .await?;
    } else {
        sqlx::query("DELETE FROM game_rank_favorite WHERE cid=$1 AND event_id=$2")
            .bind(cid)
            .bind(event_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// GetMyRank: the rows that entered the first 100 (from outside it or unranked before) and
/// were not read yet, marked read. (PeriodType, EventID, Rank).
pub async fn alarms(pool: &PgPool, cid: i64) -> Result<Vec<(i32, i32, i32)>, MinimumError> {
    let rows: Vec<(i16, i32, i32)> = sqlx::query_as(
        "UPDATE game_rank_result SET is_read=true WHERE cid=$1 AND NOT is_read AND n4_rank<=100 \
         AND (n4_rank_prev>100 OR n4_rank_prev=0) RETURNING period_type,event_id,n4_rank",
    )
    .bind(cid)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(p, e, r)| (i32::from(p), e, r))
        .collect())
}
