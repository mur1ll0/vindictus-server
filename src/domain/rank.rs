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

/// UpdateRankTitleType: which of the given rank titles' slots UpdateRankTitle pushes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleUpdate {
    Character,
    Guild,
    Both,
}

/// One HDV_GetRank row the RankTitleManager keeps (weekly, n4Rank and n4Order within
/// RankTitleLimit): the event, n4Rank and the ranked entity (a CID, or a GID for a guild
/// ranking).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighRank {
    pub event_id: i32,
    pub rank: i32,
    pub entity: i64,
}

/// GuildInfoManager.UpdateRankTitles' list: the TitleID of each high rank with a RankTitle
/// row (EventID, Rank) held by the character (`cid`) or, for a guild ranking, by its guild.
pub fn rank_titles(
    high: &[HighRank],
    titles: &[(i32, i32, i32)],
    is_guild: impl Fn(i32) -> bool,
    cid: i64,
    guild: Option<i64>,
) -> Vec<i32> {
    let mut list = Vec::new();
    for h in high {
        let Some(&(_, _, title)) = titles
            .iter()
            .find(|(e, r, _)| *e == h.event_id && *r == h.rank)
        else {
            continue;
        };
        let holder = if is_guild(h.event_id) {
            guild == Some(h.entity)
        } else {
            h.entity == cid
        };
        if holder {
            list.push(title);
        }
    }
    list
}

/// The outcome of TitleManager.UpdateRankTitle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RankTitleChange {
    /// RankTitles after the update.
    pub held: Vec<i32>,
    /// Titles GiveRankTitle gave (each logs GiveRankTitle; GiveTitle when not owned).
    pub given: Vec<i32>,
    /// Acquired slots pushed (the given titles the update type selects).
    pub acquired_slots: Vec<i32>,
    /// Known slots pushed for the previously held titles not in the new list.
    pub removed_slots: Vec<i32>,
    /// SelectedTitle is a rank title the character no longer holds: it becomes 0.
    pub unselect: bool,
}

/// TitleManager.UpdateRankTitle(rankTitles, type): RankTitles is cleared and each listed
/// title given again (a repeated one, or one whose TitleInfo is unknown or disabled — `valid`
/// false — is not); the selected rank title no longer held is cleared.
pub fn update_rank_titles(
    previous: &[i32],
    list: &[i32],
    valid: impl Fn(i32) -> bool,
    is_guild_title: impl Fn(i32) -> bool,
    is_rank_title: impl Fn(i32) -> bool,
    kind: TitleUpdate,
    selected: i32,
) -> RankTitleChange {
    let mut change = RankTitleChange::default();
    let mut remaining: Vec<i32> = previous.to_vec();
    for &title in list {
        if !change.held.contains(&title) && valid(title) {
            change.held.push(title);
            change.given.push(title);
            let pushed = match kind {
                TitleUpdate::Character => !is_guild_title(title),
                TitleUpdate::Guild => is_guild_title(title),
                TitleUpdate::Both => true,
            };
            if pushed {
                change.acquired_slots.push(title);
            }
        }
        if let Some(i) = remaining.iter().position(|t| *t == title) {
            remaining.remove(i);
        }
    }
    change.unselect =
        (!change.held.contains(&selected) || !list.contains(&selected)) && is_rank_title(selected);
    change.removed_slots = remaining;
    change
}

/// RankTitleManager.UpdateHighRank's next query: the start of the weekly period after the one
/// containing `now` (RankPeriodInfo 2 from WEEK_REFERENCE), in Unix seconds.
pub fn next_week_start(now: i64) -> i64 {
    WEEK_REFERENCE + ((now - WEEK_REFERENCE).div_euclid(WEEK_SECONDS) + 1) * WEEK_SECONDS
}

/// UpdateRankBasisUtility.UpdateRankBasis_GetItem: the RankID of a given item class and its
/// score (the amount for gold, 1 otherwise), None when no ranking counts the class. The
/// tests run in the source's order; `GetItem_silk` has no RankDescription row in 1.69.
pub fn get_item(class: &str, count: i64) -> Option<(&'static str, i64)> {
    let rank_id = if class == "gold" {
        return Some(("GetItem_gold", count));
    } else if class.ends_with("_erg_crystal") {
        "GetItem_erg_crystal"
    } else if class.starts_with("cloth_lvl") {
        "GetItem_cloth"
    } else if class.starts_with("silk_lvl") {
        "GetItem_silk"
    } else if class.starts_with("skin_lvl") {
        "GetItem_skin"
    } else if class.starts_with("iron_ore_lvl") {
        "GetItem_iron_ore"
    } else if class.contains("sign_lvl") {
        "GetItem_sign"
    } else if class.starts_with("enhance_stone_lvl") {
        "GetItem_enhance_stone"
    } else {
        return None;
    };
    Some((rank_id, 1))
}

/// UpdateRankBasisUtility.UpdateRankBasis_UseItem: the RankID of a consumable used in a
/// non-practice play (score 1).
pub fn use_item(class: &str) -> Option<&'static str> {
    [
        ("javelin", "UseItem_javelin"),
        ("handbomb", "UseItem_handbomb"),
        ("flashbang", "UseItem_flashbang"),
        ("mining_bomb", "UseItem_mining_bomb"),
        ("sticky_bomb", "UseItem_sticky_bomb"),
    ]
    .into_iter()
    .find(|(prefix, _)| class.starts_with(prefix))
    .map(|(_, rank_id)| rank_id)
}

/// ManufactureManager.Craft: a cooking recipe counts GamePlay_produce_cook, any other
/// GamePlay_produce_equipment.
pub fn produce(manufacture_id: &str) -> &'static str {
    if manufacture_id.starts_with("cooking") {
        "GamePlay_produce_cook"
    } else {
        "GamePlay_produce_equipment"
    }
}

/// NamedProp.BreakNamedProp: breaking a named prop whose EntityName contains "ore_" outside
/// practice counts GamePlay_success_mining for the breaker.
pub fn is_mining(entity_name: &str) -> bool {
    entity_name.contains("ore_")
}
