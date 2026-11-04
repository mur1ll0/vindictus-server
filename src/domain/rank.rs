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
