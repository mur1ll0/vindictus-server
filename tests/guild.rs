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
