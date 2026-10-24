use vindictus_server::domain::{
    enchant::{
        Context, EnchantInfo, Protection, Random, StatRow, do_enchant, is_available_to, load,
        result, success_ratio,
    },
    item_stats::ItemStat,
};

struct Owner;
impl Context for Owner {
    fn level(&self) -> i32 {
        30
    }
    fn class(&self) -> i32 {
        2
    }
    fn skill_rank(&self, skill: Option<&str>) -> i32 {
        if skill == Some("skill_str_mastery") {
            3
        } else {
            0
        }
    }
    fn title_acquired(&self, title: i32) -> bool {
        title == 410017
    }
    fn selected_title(&self) -> Option<i32> {
        Some(7)
    }
}
fn info(level: i32, destruction: i32) -> EnchantInfo {
    EnchantInfo {
        class: "keen".into(),
        is_prefix: true,
        level,
        min_arg: 0,
        max_arg: 0,
        constraint: Some("CLOTH,WEAPON & longsword_iron2".into()),
        min_ratio: 50,
        max_ratio: 80,
        destruction,
        mini_game: 100,
        duration: 0,
        ignore_max_armor: false,
    }
}
fn row(order: i32, condition: Option<&str>, stat: &str) -> StatRow {
    StatRow {
        order,
        condition: condition.map(str::to_owned),
        stat: Some(stat.into()),
    }
}
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn get(stat: &ItemStat, name: &str) -> i32 {
    stat.get(Some(&units(name)))
}

#[test]
fn enchant_bonuses_follow_their_conditions_and_arguments() {
    let rows = [
        row(1, None, "STR+1,DEX+2"),
        row(2, Some("skillrank>=(skill_str_mastery,3)"), "ATK+{0}"),
        row(3, Some("level>=(31)"), "DEF+9"),
        row(
            4,
            Some("title_acquired(410017)&class(2)"),
            "@enchant_poison+3;HP-4",
        ),
        row(5, Some("equip_class(SWORD,LONGSWORD)"), "Balance+5"),
        row(6, Some("unknown(1)"), "INT+1"),
        row(7, Some("skillrank<=(skill_str_mastery,9)"), "WILL+2"),
    ];
    let info = info(1, 0);
    let loaded = load(
        Some(&info),
        Some(&rows),
        12,
        0,
        0,
        Some("LONGSWORD"),
        &Owner,
    )
    .unwrap();
    assert_eq!(
        loaded.status,
        [
            (1, true),
            (2, true),
            (3, false),
            (4, true),
            (5, true),
            (6, false),
            (7, false)
        ]
    );
    let stat = loaded.stat.unwrap();
    assert_eq!(
        (get(&stat, "STR"), get(&stat, "DEX"), get(&stat, "ATK")),
        (1, 2, 12)
    );
    assert_eq!(
        (get(&stat, "DEF"), get(&stat, "HP"), get(&stat, "Balance")),
        (0, -4, 5)
    );
    assert_eq!(
        stat.status_entries().map(|e| e.value).collect::<Vec<_>>(),
        [3]
    );
    assert_eq!(loaded.level, 1);
    // A timed enchant past its Arg2 second is disabled: no Stat, no status, level 0.
    let expired = load(Some(&info), Some(&rows), 0, 100, 200, None, &Owner).unwrap();
    assert!(expired.expired && expired.stat.is_none() && expired.status.is_empty());
    // Unknown class: empty Stat; info without rows, a repeated Order or a format beyond
    // {0} are refused like the source exceptions.
    let unknown = load(None, None, 0, 0, 0, None, &Owner).unwrap();
    assert_eq!((unknown.level, unknown.status.len()), (0, 0));
    assert!(load(Some(&info), None, 0, 0, 0, None, &Owner).is_err());
    assert!(
        load(
            Some(&info),
            Some(&[row(1, None, "A+1"), row(1, None, "B+1")]),
            0,
            0,
            0,
            None,
            &Owner
        )
        .is_err()
    );
    assert!(
        load(
            Some(&info),
            Some(&[row(1, None, "A+{1}")]),
            0,
            0,
            0,
            None,
            &Owner
        )
        .is_err()
    );
}

#[test]
fn constraints_and_ratios_follow_the_item_contents() {
    let i = info(1, 0);
    // Every '&' group needs a token containing the class, category or EquipClass.
    assert!(is_available_to(
        &i,
        "longsword_iron",
        "WEAPON",
        Some("LONGSWORD")
    ));
    assert!(!is_available_to(
        &i,
        "boots_x",
        "LIGHT_ARMOR",
        Some("LIGHT_ARMOR")
    ));
    assert!(!is_available_to(&i, "longsword_iron", "TUNIC", None));
    let open = EnchantInfo {
        constraint: None,
        ..i.clone()
    };
    assert!(is_available_to(&open, "x", "y", None));
    // (Min + (Max-Min) × current/goal) / 100, capped at 1; past the goal only Min.
    assert_eq!(success_ratio(&i, 0, 100, 0), 0.5);
    assert_eq!(success_ratio(&i, 50, 100, 0), 0.65);
    assert_eq!(success_ratio(&i, 101, 100, 0), 0.5);
    assert_eq!(success_ratio(&i, 100, 100, 50), 1.0);
}

struct Script(Vec<f64>, Vec<i32>);
impl Random for Script {
    fn next_double(&mut self) -> f64 {
        self.0.remove(0)
    }
    fn next(&mut self, maximum: i32) -> Option<i32> {
        (maximum >= 0).then(|| self.1.remove(0).min(maximum.max(1) - 1))
    }
}

#[test]
fn do_enchant_takes_the_protection_and_failure_branches() {
    let run = |level: i32, destruction: i32, protection, doubles: Vec<f64>, ints: Vec<i32>| {
        do_enchant(
            &info(level, destruction),
            0.5,
            protection,
            true,
            &mut Script(doubles, ints),
        )
        .unwrap()
    };
    let o = run(7, 0, Protection::Perfect, vec![], vec![]);
    assert_eq!(
        (o.result, o.consume_rune, o.bind),
        (result::ENCHANT_SUCCESS, true, true)
    );
    let o = run(1, 0, Protection::Unstable, vec![0.5], vec![]);
    assert_eq!(
        (o.result, o.consume_rune, o.bind),
        (result::ENCHANT_SUCCESS, true, false)
    );
    let o = run(1, 0, Protection::Item, vec![0.9], vec![]);
    assert_eq!(
        (o.result, o.consume_scroll, o.consume_rune),
        (result::ITEM_PROTECTED, true, true)
    );
    let o = run(1, 0, Protection::Scroll, vec![0.9], vec![]);
    assert_eq!(
        (o.result, o.consume_scroll),
        (result::SCROLL_PROTECTED, false)
    );
    // DestructionRatio 30: a second roll ≤ 0.3f destroys the item.
    let o = run(5, 30, Protection::None, vec![0.9, 0.2], vec![]);
    assert_eq!((o.result, o.destroyed), (result::ITEM_DESTROYED, true));
    // Otherwise Next(100) >= 30 - 2(level-1) loses (1 + Next(level or 2×level)) × 100.
    let o = run(5, 30, Protection::None, vec![0.9, 0.5], vec![22, 3]);
    assert_eq!(
        (o.result, o.max_durability_loss),
        (result::ENCHANT_FAILED, 400)
    );
    let o = run(8, 0, Protection::None, vec![0.9], vec![16, 15]);
    assert_eq!(o.max_durability_loss, 1600);
    let o = run(1, 0, Protection::None, vec![0.9], vec![29]);
    assert_eq!(o.max_durability_loss, 0);
}
