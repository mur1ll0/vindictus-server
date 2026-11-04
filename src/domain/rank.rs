//! RankService rules (FeatureMatrix CharacterRank on in EN-EU): the RankBasisUpdate procedure
//! of heroesRank (study_heroesRank.schema.json). A Count score adds to the stored one; a Time
//! score keeps the best of the current UTC day, and the first time of a new day replaces the
//! row whatever it is, moving the previous day's best to RankBasis_History.

/// RankDescription.ScoreType.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreType {
    Count,
    Time,
}
impl ScoreType {
    /// The procedure compares the text: anything but "TIME" counts.
    pub fn parse(text: Option<&str>) -> Self {
        if text.is_some_and(|t| t.eq_ignore_ascii_case("time")) {
            Self::Time
        } else {
            Self::Count
        }
    }
}

/// RankDescription.SortType: true for ASCENDING (a lower time is better).
pub fn ascending(sort: Option<&str>) -> bool {
    sort.is_some_and(|s| s.eq_ignore_ascii_case("ascending"))
}

/// The stored RankBasis row of a character and event: score and last update (Unix seconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Basis {
    pub score: i64,
    pub last_update: i64,
}

/// What RankBasisUpdate writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    Insert(Basis),
    /// The row's new score and last update.
    Set(Basis),
    /// A new day's first time: the previous row goes to RankBasis_History.
    Rollover {
        previous: Basis,
        new: Basis,
    },
    /// A worse time of the same day: nothing changes.
    Keep,
}

/// RankBasisUpdate on the latest row of (CID, EventID); `now` in Unix seconds (GETUTCDATE).
pub fn update(
    current: Option<Basis>,
    score: i64,
    score_type: ScoreType,
    ascending: bool,
    now: i64,
) -> Update {
    let Some(row) = current else {
        return Update::Insert(Basis {
            score,
            last_update: now,
        });
    };
    match score_type {
        ScoreType::Count => Update::Set(Basis {
            score: row.score.wrapping_add(score),
            last_update: now,
        }),
        ScoreType::Time => {
            let today = now.div_euclid(86_400) * 86_400;
            if row.last_update < today {
                return Update::Rollover {
                    previous: row,
                    new: Basis {
                        score,
                        last_update: now,
                    },
                };
            }
            let better = if ascending {
                score < row.score
            } else {
                score > row.score
            };
            if better {
                Update::Set(Basis {
                    score,
                    last_update: now,
                })
            } else {
                Update::Keep
            }
        }
    }
}

/// Rank result period types (RankResultCharacter, Weekly, Monthly).
pub mod period {
    pub const DAILY: i32 = 1;
    pub const WEEKLY: i32 = 2;
    pub const MONTHLY: i32 = 3;
}

/// The publication hour of every period (07:00 UTC, as RankPeriodInfo's DateForm).
pub const RESET_SECONDS: i64 = 7 * 3600;
/// RankPeriodInfo enEU: PeriodType 2 from 2011-11-19 07:00 UTC (a Saturday), 10080 minutes.
pub const WEEK_REFERENCE: i64 = 1_321_686_000;
pub const WEEK_SECONDS: i64 = 10_080 * 60;

/// The latest 07:00 UTC boundary at or before `now` (Unix seconds).
pub fn daily_boundary(now: i64) -> i64 {
    (now - RESET_SECONDS).div_euclid(86_400) * 86_400 + RESET_SECONDS
}

/// Days since 1970-01-01 → (year, month, day) (the inverse of quest::days_from_civil).
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The window start of each period published at `boundary`: the day that ended, the week
/// since the latest Saturday 07:00 strictly before it, and the month since the latest day 1
/// at 07:00 strictly before it (a boundary that closes a week or a month publishes the whole
/// finished period).
pub fn period_starts(boundary: i64) -> [(i32, i64); 3] {
    let week =
        WEEK_REFERENCE + (boundary - 1 - WEEK_REFERENCE).div_euclid(WEEK_SECONDS) * WEEK_SECONDS;
    let (year, month, _) = civil_from_days((boundary - 1 - RESET_SECONDS).div_euclid(86_400));
    let month_start =
        // quest::days_from_civil counts from 1900-01-01 (25,567 days before 1970).
        (crate::domain::quest::days_from_civil(year, month, 1) - 25_567) * 86_400 + RESET_SECONDS;
    [
        (period::DAILY, boundary - 86_400),
        (period::WEEKLY, week),
        (period::MONTHLY, month_start),
    ]
}

/// One published row: CID, score, n4Order (ROW_NUMBER) and n4Rank (RANK).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ranked {
    pub cid: i64,
    pub score: i64,
    pub order: i32,
    pub rank: i32,
}

/// Orders the scores by the SortType (ties by CID), numbering rows and giving equal scores
/// the same rank.
pub fn rank_scores(mut scores: Vec<(i64, i64)>, ascending: bool) -> Vec<Ranked> {
    scores.sort_by(|a, b| {
        let by_score = if ascending {
            a.1.cmp(&b.1)
        } else {
            b.1.cmp(&a.1)
        };
        by_score.then(a.0.cmp(&b.0))
    });
    let mut out: Vec<Ranked> = Vec::with_capacity(scores.len());
    for (i, (cid, score)) in scores.into_iter().enumerate() {
        let order = i32::try_from(i + 1).unwrap_or(i32::MAX);
        let rank = match out.last() {
            Some(prev) if prev.score == score => prev.rank,
            _ => order,
        };
        out.push(Ranked {
            cid,
            score,
            order,
            rank,
        });
    }
    out
}
