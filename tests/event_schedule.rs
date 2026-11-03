use vindictus_server::domain::event_schedule::{Command, Event, State, features, plan, script};

const DAY: i64 = 86_400;
/// A local day start.
const T: i64 = 1_793_664_000;

fn event(name: &str) -> Event {
    Event {
        name: name.into(),
        feature: None,
        start_script: None,
        end_script: None,
        start_time: None,
        end_time: None,
        period_begin: None,
        period_end: None,
        start_message: None,
        notify_message: None,
        end_message: None,
        notify_interval: None,
    }
}

#[test]
fn templates_parse_like_update_feature_matrix_and_process_script() {
    // Key[Value], a bare Key ("0"), NULL kept as text, empty parts skipped, the last wins.
    assert_eq!(
        features("ExpRate_Combat[200];Event_xmas;;PvP[NULL];Event_xmas[1]"),
        [
            ("ExpRate_Combat".to_owned(), "200".to_owned()),
            ("Event_xmas".to_owned(), "1".to_owned()),
            ("PvP".to_owned(), "NULL".to_owned()),
        ]
    );
    assert_eq!(
        script(
            "droptable mod holy_sword 1000 gold_ore 300;monster_refresh; quest_refresh;freetoken on;droptable reset holy_sword gold_ore;droptable mod x;droptable mod_fishing a 1"
        ),
        [
            Command::DropMod(vec![("holy_sword".into(), 1000), ("gold_ore".into(), 300)]),
            Command::Refresh,
            Command::Refresh,
            Command::Unsupported("freetoken on".into()),
            Command::DropReset(vec!["holy_sword".into(), "gold_ore".into()]),
            Command::Unsupported("droptable mod x".into()),
            Command::Unsupported("droptable mod_fishing a 1".into()),
        ]
    );
}

#[test]
fn process_event_follows_times_periods_and_messages() {
    let mut always = event("always");
    always.end_time = Some(T + 2 * DAY);
    let mut evening = event("evening");
    evening.start_time = Some(T);
    evening.end_time = Some(T + 3 * DAY);
    evening.period_begin = Some(18 * 3600);
    evening.period_end = Some(22 * 3600);
    let mut later = event("later");
    later.start_time = Some(T + DAY);
    let mut old = event("old");
    old.start_time = Some(T - 2 * DAY);
    old.end_time = Some(T - DAY);
    let events = [always.clone(), evening.clone(), later.clone(), old.clone()];
    let p = plan(&events, T + 19 * 3600);
    assert_eq!(
        (p.start, p.end, p.remove),
        (
            vec!["always".into(), "evening".into()],
            vec!["later".to_owned()],
            vec!["old".to_owned()]
        )
    );
    assert_eq!(plan(&events, T + 23 * 3600).end, ["evening", "later"]);

    let mut xmas = event("xmas");
    xmas.feature = Some("Event_xmas;ExpRate_Combat[200]".into());
    xmas.start_script = Some("droptable mod gold_ore 300;monster_refresh;".into());
    xmas.end_script = Some("droptable reset gold_ore;".into());
    xmas.start_time = Some(T);
    xmas.end_time = Some(T + DAY);
    xmas.start_message = Some("start".into());
    xmas.end_message = Some("end".into());
    xmas.notify_message = Some("notify".into());
    xmas.notify_interval = Some(3600);
    let mut state = State::default();
    // 10:30: started, keys and modifier on, no notify off the hour.
    let (fx, removed) = state.process(std::slice::from_ref(&xmas), T + 10 * 3600 + 1800);
    assert!(fx.changed && removed.is_empty());
    assert_eq!(fx.notices, ["start"]);
    assert_eq!(
        state.overrides.get("Event_xmas").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        state.overrides.get("ExpRate_Combat").map(String::as_str),
        Some("200")
    );
    assert_eq!(state.drop_modifiers.get("gold_ore"), Some(&300));
    assert_eq!(state.first_income(), ["notify"]);
    // 11:00: running, the hourly notify, nothing changes.
    let (fx, _) = state.process(std::slice::from_ref(&xmas), T + 11 * 3600);
    assert_eq!((fx.changed, fx.notices), (false, vec!["notify".to_owned()]));
    // Past EndTime: ended and removed, everything back.
    let (fx, removed) = state.process(std::slice::from_ref(&xmas), T + DAY);
    assert_eq!(
        (fx.notices, removed),
        (vec!["end".to_owned()], vec!["xmas".to_owned()])
    );
    assert!(
        state.overrides.is_empty() && state.drop_modifiers.is_empty() && state.running.is_empty()
    );
}
