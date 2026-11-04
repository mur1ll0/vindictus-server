use vindictus_server::domain::guild::{self as rules, open_error, user_type};
use vindictus_server::protocol::guild::rank;

#[test]
fn guild_names_follow_the_naming_rule_and_its_length_limits() {
    for ok in ["Night_Owls", "abcd", "ab_cd", "ABCDEFGHIJKLMNOPQRST"] {
        assert_eq!(rules::check_name(ok), Ok(()), "{ok}");
    }
    for (name, err) in [
        ("abc", open_error::NAMING_RULE),
        ("a_bcd", open_error::NAMING_RULE),
        ("ab__cd", open_error::NAMING_RULE),
        ("ab_cd_ef", open_error::NAMING_RULE),
        ("Night0wls", open_error::NAMING_RULE),
        ("", open_error::NAMING_RULE),
        ("ABCDEFGHIJKLMNOPQRSTU", open_error::NAMING_RULE_MAX_BYTES),
        // Outside [a-zA-Z0-9_] the limit is half: 10 UTF-16 units.
        ("Ünïcødé", open_error::NAMING_RULE),
        ("Ünïcødé_long", open_error::NAMING_RULE_MAX_BYTES),
        ("night owls", open_error::NAMING_RULE),
    ] {
        assert_eq!(rules::check_name(name), Err(err), "{name}");
    }
    assert_eq!(rules::check_id(""), Err(open_error::EMPTY_ID));
    assert_eq!(rules::check_id("x"), Ok(()));
    assert_eq!(
        rules::truncate_utf16(&"a".repeat(30), rules::ID_MAX).len(),
        24
    );
    assert_eq!(rules::truncate_utf16("日本語", 2), "日本");
}

#[test]
fn guild_ranks_limits_and_pages_follow_the_guild_api() {
    for (t, r) in [
        (user_type::MASTER, rank::MASTER),
        (user_type::OPERATOR, rank::OPERATOR),
        (user_type::MEMBER, rank::MEMBER),
        (user_type::WAIT, rank::WAIT),
    ] {
        assert_eq!(rules::rank_of(t), r);
        assert_eq!(rules::user_type_of(r), t);
    }
    assert_eq!(rules::rank_of(7), rank::IGNORABLE);
    assert_eq!(rules::user_type_of(rank::UNKNOWN), user_type::UNKNOWN);
    assert!(rules::is_regular(rank::MEMBER) && !rules::is_regular(rank::WAIT));
    assert_eq!(rules::max_member_limit(0), 100);
    assert_eq!(rules::max_member_limit(300), 300);
    assert_eq!(rules::total_pages(0, 0), Some(1));
    assert_eq!(rules::total_pages(21, 10), Some(3));
    assert_eq!(rules::total_pages(20, 10), Some(2));
    assert_eq!(rules::total_pages(5, 0), None);
    assert!(rules::search_key_valid("ab") && !rules::search_key_valid("a"));
    assert!(!rules::search_key_valid(&"a".repeat(21)));
    assert_eq!(rules::search_name_key("[Ni ght]!"), "Night");
    let day = 300 * 86_400;
    assert_eq!(rules::days_since(10 * day + 5, 3 * day), 7);
    assert_eq!(rules::days_since(day - 1, 0), 0);
}

fn levels() -> rules::Levels {
    rules::Levels::new([
        (1, 100, Some("capsule_ap[VALUE:100]".to_owned())),
        (2, 130, Some("capsule_ap[VALUE:110]".to_owned())),
        (3, 160, None),
    ])
    .unwrap()
}

#[test]
fn guild_gp_follows_the_daily_limit_and_levels_up() {
    use rules::gain::{BY_CHEAT, BY_PVE, BY_PVP};
    assert_eq!(rules::daily_limit("200;60", BY_PVE), 2000);
    assert_eq!(rules::daily_limit("200;60", BY_PVP), 600);
    assert_eq!(rules::daily_limit("200;60", BY_CHEAT), -1);
    assert_eq!(rules::daily_limit("200;60", 0), 0);
    assert_eq!(rules::daily_limit("-1;x", BY_PVE), -1);
    assert_eq!(rules::daily_limit("-1;x", BY_PVP), 0);
    assert_eq!(rules::daily_limit("", BY_PVE), -1);
    assert!(rules::Levels::new([(1, 1, None), (1, 2, None)]).is_err());
    let l = levels();
    assert_eq!(
        (l.max_level(), l.required(2), l.reward(3)),
        (Some(4), Some(1300), None)
    );
    // 2500 GP with 2000 left today: 2000 given, level 1 → 2 with 1000 left over.
    let g = rules::give_gp(&l, 1, 0, None, 2000, 2500, BY_PVE).unwrap();
    assert_eq!((g.given, g.daily, g.point_before), (2000, Some(2000), 0));
    assert_eq!((g.level, g.point, g.level_ups.clone()), (2, 1000, vec![2]));
    // The limit is reached.
    assert_eq!(
        rules::give_gp(&l, 2, 1000, Some(2000), 2000, 10, BY_PVE),
        None
    );
    // Two levels at once; the last level leaves the point at 0.
    let g = rules::give_gp(&l, 2, 1200, None, -1, 1800, BY_PVE).unwrap();
    assert_eq!((g.level, g.point, g.level_ups.clone()), (4, 0, vec![3, 4]));
    assert_eq!(rules::give_gp(&l, 4, 0, None, -1, 10, BY_PVE), None);
    // A cheat gain skips the daily limit and the GuildGainGP push.
    let g = rules::give_gp(&l, 1, 0, None, 0, 50, BY_CHEAT).unwrap();
    assert_eq!((g.given, g.daily, g.point), (50, None, 50));
    // Negative GP never goes below today's gain.
    let g = rules::give_gp(&l, 1, 500, Some(30), 2000, -100, BY_PVE).unwrap();
    assert_eq!((g.given, g.daily, g.point), (-30, Some(0), 470));
    assert_eq!(rules::give_gp(&l, 1, 0, None, 2000, 0, BY_PVE), None);
    // The previous reset: 07:00 UTC today, or yesterday's before 07:00.
    let day = 86_400;
    assert_eq!(
        rules::prev_reset(10 * day + 8 * 3600, 7),
        10 * day + 7 * 3600
    );
    assert_eq!(
        rules::prev_reset(10 * day + 6 * 3600, 7),
        9 * day + 7 * 3600
    );
    assert_eq!(rules::prev_reset(10 * day + 6 * 3600, 30), 10 * day);
}

#[test]
fn guild_level_bonus_takes_the_last_row_at_or_below_the_level() {
    let rows: Vec<(i32, String, i32)> = [
        (1, "BattleExp", 1),
        (2, "DyePrice", 1),
        (3, "TradePrice", 1),
        (4, "RepairPrice", 1),
        (5, "DyePrice", 2),
        (6, "EnhancePrice", 1),
        (7, "BattleExp", 3),
        (8, "TradePrice", 2),
        (9, "VocationExp", 5),
        (10, "CampFire", 1),
        (11, "RepairPrice", 2),
        (12, "EnhancePrice", 2),
        (13, "BattleExp", 5),
        (14, "DyePrice", 3),
        (15, "TradePrice", 3),
        (16, "BattleAP", 1),
        (17, "VocationExp", 10),
        (18, "EnhancePrice", 3),
        (19, "RepairPrice", 3),
        (20, "AdvancedFeather", 1),
    ]
    .into_iter()
    .map(|(l, k, a)| (l, k.to_owned(), a))
    .collect();
    let at13 = rules::level_bonus(&rows, 13);
    let expected: Vec<(&str, i32)> = vec![
        ("BattleExp", 5),
        ("CampFire", 1),
        ("DyePrice", 2),
        ("EnhancePrice", 2),
        ("RepairPrice", 2),
        ("TradePrice", 2),
        ("VocationExp", 5),
    ];
    assert_eq!(
        at13.iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(rules::level_bonus(&rows, 1).len(), 1);
    assert_eq!(
        rules::level_bonus(&rows, 20).get("AdvancedFeather"),
        Some(&1)
    );
    // Level 0 and the level after the highest row have no bonus.
    assert!(rules::level_bonus(&rows, 0).is_empty());
    assert!(rules::level_bonus(&rows, 21).is_empty());
    assert!(rules::level_bonus(&[], 5).is_empty());
}
