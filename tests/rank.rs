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
