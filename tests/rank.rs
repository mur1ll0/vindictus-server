use vindictus_server::domain::rank::{self as rules, Basis, ScoreType, Update};

#[test]
fn rank_basis_update_adds_counts_and_keeps_the_best_time_of_the_day() {
    let day = 86_400;
    let now = 10 * day + 3600;
    assert_eq!(ScoreType::parse(Some("Time")), ScoreType::Time);
    assert_eq!(ScoreType::parse(Some("Count")), ScoreType::Count);
    assert_eq!(ScoreType::parse(None), ScoreType::Count);
    assert!(rules::ascending(Some("ASCENDING")) && !rules::ascending(Some("DESCENDING")));
    let b = |score, last_update| Basis { score, last_update };
    assert_eq!(
        rules::update(None, 5, ScoreType::Count, false, now),
        Update::Insert(b(5, now))
    );
    assert_eq!(
        rules::update(Some(b(5, 0)), 3, ScoreType::Count, false, now),
        Update::Set(b(8, now))
    );
    // Same day: a better (lower, ascending) time replaces, a worse one is kept out.
    assert_eq!(
        rules::update(Some(b(9000, 10 * day)), 8000, ScoreType::Time, true, now),
        Update::Set(b(8000, now))
    );
    assert_eq!(
        rules::update(Some(b(9000, 10 * day)), 9500, ScoreType::Time, true, now),
        Update::Keep
    );
    assert_eq!(
        rules::update(Some(b(9000, 10 * day)), 9500, ScoreType::Time, false, now),
        Update::Set(b(9500, now))
    );
    // A new UTC day: the first time replaces whatever it is and the old row goes to history.
    assert_eq!(
        rules::update(
            Some(b(9000, 9 * day + 50)),
            99_999,
            ScoreType::Time,
            true,
            now
        ),
        Update::Rollover {
            previous: b(9000, 9 * day + 50),
            new: b(99_999, now)
        }
    );
}

#[test]
fn rank_periods_start_at_07_utc_on_saturdays_and_month_firsts() {
    use rules::period;
    // 2026-11-04 (Wednesday) 10:00 UTC.
    let wed = 1_793_786_400;
    assert_eq!(rules::civil_from_days(wed / 86_400), (2026, 11, 4));
    let boundary = rules::daily_boundary(wed);
    assert_eq!(boundary, wed - 3 * 3600);
    assert_eq!(rules::daily_boundary(boundary - 1), boundary - 86_400);
    let starts = rules::period_starts(boundary);
    assert_eq!(starts[0], (period::DAILY, boundary - 86_400));
    // The week since Saturday 2026-10-31 07:00 and the month since 2026-11-01 07:00.
    assert_eq!(starts[1], (period::WEEKLY, boundary - 4 * 86_400));
    assert_eq!(starts[2], (period::MONTHLY, boundary - 3 * 86_400));
    // A boundary that closes the week or the month publishes the whole finished period.
    let saturday = boundary + 3 * 86_400;
    assert_eq!(rules::period_starts(saturday)[1].1, saturday - 7 * 86_400);
    let december = rules::daily_boundary(1_796_112_000 + 8 * 3600); // 2026-12-01 08:00
    assert_eq!(rules::civil_from_days(december / 86_400), (2026, 12, 1));
    assert_eq!(rules::period_starts(december)[2].1, december - 30 * 86_400);
}

#[test]
fn rank_scores_number_rows_and_share_ranks() {
    let ranked = rules::rank_scores(vec![(3, 10), (1, 30), (2, 10), (4, 5)], false);
    let rows: Vec<_> = ranked.iter().map(|r| (r.cid, r.order, r.rank)).collect();
    assert_eq!(rows, [(1, 1, 1), (2, 2, 2), (3, 3, 2), (4, 4, 4)]);
    let times = rules::rank_scores(vec![(1, 9000), (2, 8000)], true);
    assert_eq!((times[0].cid, times[0].rank), (2, 1));
}

#[test]
fn rank_titles_follow_the_holder_and_the_update_type() {
    use rules::{HighRank, TitleUpdate};
    let high = [
        HighRank {
            event_id: 60001,
            rank: 1,
            entity: 7,
        },
        HighRank {
            event_id: 60003,
            rank: 1,
            entity: 2,
        },
        HighRank {
            event_id: 60003,
            rank: 2,
            entity: 1,
        },
        HighRank {
            event_id: 60003,
            rank: 3,
            entity: 9,
        },
    ];
    let titles = [(60001, 1, 420034), (60003, 1, 420040), (60003, 2, 420041)];
    let guild = |e: i32| e == 60001;
    // A guild ranking is held by the guild; a rank without a RankTitle row gives nothing.
    assert_eq!(
        rules::rank_titles(&high, &titles, guild, 1, Some(7)),
        [420034, 420041]
    );
    assert_eq!(rules::rank_titles(&high, &titles, guild, 1, None), [420041]);
    assert!(rules::rank_titles(&high, &titles, guild, 9, Some(8)).is_empty());

    let is_guild_title = |t: i32| t == 420034;
    let is_rank_title = |t: i32| (420034..=420041).contains(&t);
    // Character: only the non-guild slots are pushed; repeated or invalid titles are skipped.
    let c = rules::update_rank_titles(
        &[420040],
        &[420034, 420041, 420041, 999],
        |t| t != 999,
        is_guild_title,
        is_rank_title,
        TitleUpdate::Character,
        0,
    );
    assert_eq!(c.held, [420034, 420041]);
    assert_eq!(c.given, [420034, 420041]);
    assert_eq!(c.acquired_slots, [420041]);
    assert_eq!(c.removed_slots, [420040]);
    assert!(!c.unselect);
    let g = rules::update_rank_titles(
        &[],
        &[420034, 420041],
        |_| true,
        is_guild_title,
        is_rank_title,
        TitleUpdate::Guild,
        0,
    );
    assert_eq!(g.acquired_slots, [420034]);
    // The selected rank title no longer held is cleared; another title is kept.
    let lost = rules::update_rank_titles(
        &[420041],
        &[],
        |_| true,
        is_guild_title,
        is_rank_title,
        TitleUpdate::Both,
        420041,
    );
    assert!(lost.unselect && lost.held.is_empty());
    assert_eq!(lost.removed_slots, [420041]);
    let kept = rules::update_rank_titles(
        &[],
        &[],
        |_| true,
        is_guild_title,
        is_rank_title,
        TitleUpdate::Both,
        1001,
    );
    assert!(!kept.unselect);
}

#[test]
fn rank_titles_refresh_after_each_weekly_start() {
    // Saturday 2026-10-31 07:00 UTC opens a week of RankPeriodInfo 2.
    let saturday = 1_793_430_000;
    assert_eq!((saturday - rules::WEEK_REFERENCE) % rules::WEEK_SECONDS, 0);
    assert_eq!(rules::next_week_start(saturday - 1), saturday);
    assert_eq!(
        rules::next_week_start(saturday),
        saturday + rules::WEEK_SECONDS
    );
}
