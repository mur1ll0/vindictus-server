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
