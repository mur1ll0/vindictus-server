//! The AdminClient event scheduler (AdminClientServiceCore EventDataContext and
//! AdminClientService.UpdateFeatureMatrix/ProcessScript) as a laboratory panel feature
//! (operator decision of 03/11/2026): registered events from EventTemplate rows start and end
//! by their times and daily period, turn FeatureMatrix keys on (`Key` or `Key[Value]`) and
//! back, and run the `droptable mod`/`reset` commands; the `*_refresh` commands are what the
//! game's content reload already does.
use std::collections::BTreeMap;

/// An Event row (the heroes Event table) with times in local seconds (DateTime.Now).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub name: String,
    pub feature: Option<String>,
    pub start_script: Option<String>,
    pub end_script: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    /// Seconds of the local day.
    pub period_begin: Option<i64>,
    pub period_end: Option<i64>,
    pub start_message: Option<String>,
    pub notify_message: Option<String>,
    pub end_message: Option<String>,
    pub notify_interval: Option<i32>,
}

/// What ProcessEvent does with each registered event at `now`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// StartEvent (a running event is not started again).
    pub start: Vec<String>,
    /// EndEvent: outside the period or before StartTime.
    pub end: Vec<String>,
    /// RemoveEvent: past EndTime (ended and deleted).
    pub remove: Vec<String>,
}

const DAY: i64 = 86_400;

/// ProcessEvent's lists (StartCount events, never registered by the panel, are left alone).
pub fn plan(events: &[Event], now: i64) -> Plan {
    let mut out = Plan::default();
    let time_of_day = now.rem_euclid(DAY);
    for e in events {
        if e.start_time.is_some_and(|s| now < s) {
            out.end.push(e.name.clone());
            continue;
        }
        if e.end_time.is_some_and(|t| now >= t) {
            out.remove.push(e.name.clone());
            continue;
        }
        // No StartTime: running until EndTime, whatever the period.
        if e.start_time.is_none() {
            out.start.push(e.name.clone());
            continue;
        }
        let in_period = e
            .period_begin
            .is_none_or(|b| time_of_day >= b && e.period_end.is_none_or(|p| time_of_day < p));
        if in_period {
            out.start.push(e.name.clone());
        } else {
            out.end.push(e.name.clone());
        }
    }
    out
}

/// UpdateFeatureMatrix: `Key[Value]` or `Key` (value "0") separated by ';'.
pub fn features(feature: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for part in feature.split(';') {
        let (key, value) = match (part.find('['), part.find(']')) {
            (Some(a), Some(b)) if b > a => (&part[..a], &part[a + 1..b]),
            _ => (part, "0"),
        };
        if key.is_empty() {
            continue;
        }
        // dictionary[text] = value: a repeated key keeps the last value.
        match out.iter_mut().find(|(k, _)| k == key) {
            Some(row) => row.1 = value.to_owned(),
            None => out.push((key.to_owned(), value.to_owned())),
        }
    }
    out
}

/// One command of a StartScript/EndScript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// `droptable mod <ItemClassEX> <percent> …`.
    DropMod(Vec<(String, i32)>),
    /// `droptable reset <ItemClassEX> …`.
    DropReset(Vec<String>),
    /// monster_refresh, quest_refresh, goalevent_refresh, sector_refresh: the content reload.
    Refresh,
    /// A command the laboratory does not run (freetoken, droptable mod_fishing, …).
    Unsupported(String),
}

/// ProcessScript: commands separated by ';', each "name args".
pub fn script(text: &str) -> Vec<Command> {
    let mut out = Vec::new();
    for raw in text.split(';') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let (name, args) = raw.split_once(' ').unwrap_or((raw, ""));
        let mut tokens = args.split_whitespace();
        out.push(match name {
            "monster_refresh" | "quest_refresh" | "goalevent_refresh" | "sector_refresh" => {
                Command::Refresh
            }
            "droptable" => match tokens.next() {
                Some("mod") => {
                    let rest: Vec<&str> = tokens.collect();
                    // CmdTokenizer pairs; a missing or non-integer value aborts the command.
                    let pairs: Option<Vec<(String, i32)>> = rest
                        .chunks(2)
                        .map(|c| Some((c[0].to_owned(), c.get(1)?.parse().ok()?)))
                        .collect();
                    match pairs {
                        Some(p) => Command::DropMod(p),
                        None => Command::Unsupported(raw.to_owned()),
                    }
                }
                Some("reset") => Command::DropReset(tokens.map(str::to_owned).collect()),
                _ => Command::Unsupported(raw.to_owned()),
            },
            _ => Command::Unsupported(raw.to_owned()),
        });
    }
    out
}

/// The scheduler's in-memory state (FeatureMatrix.overrideMap, dropItemModifier and the
/// running events), rebuilt at start as the source's AdminClient service does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub overrides: BTreeMap<String, String>,
    pub drop_modifiers: BTreeMap<String, i32>,
    pub running: BTreeMap<String, Event>,
}

/// What a start or an end asks the frontends to show.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Notice broadcasts, in order.
    pub notices: Vec<String>,
    /// The commands the laboratory does not run.
    pub unsupported: Vec<String>,
    pub changed: bool,
}

impl State {
    fn run(&mut self, script: Option<&str>, effects: &mut Effects) {
        for command in script.map(self::script).unwrap_or_default() {
            match command {
                Command::DropMod(pairs) => {
                    for (class, m) in pairs {
                        self.drop_modifiers.insert(class, m);
                    }
                    effects.changed = true;
                }
                Command::DropReset(classes) => {
                    for class in classes {
                        self.drop_modifiers.remove(&class);
                    }
                    effects.changed = true;
                }
                Command::Refresh => {}
                Command::Unsupported(c) => effects.unsupported.push(c),
            }
        }
    }

    /// StartEvent: the start message, the keys, the start script.
    pub fn start(&mut self, event: &Event, effects: &mut Effects) {
        if self.running.contains_key(&event.name) {
            return;
        }
        self.running.insert(event.name.clone(), event.clone());
        if let Some(m) = event.start_message.as_deref().filter(|m| !m.is_empty()) {
            effects.notices.push(m.to_owned());
        }
        for (key, value) in features(event.feature.as_deref().unwrap_or("")) {
            self.overrides.insert(key, value);
            effects.changed = true;
        }
        self.run(event.start_script.as_deref(), effects);
    }

    /// EndEvent: the end message, the keys back, the end script.
    pub fn end(&mut self, name: &str, effects: &mut Effects) {
        let Some(event) = self.running.remove(name) else {
            return;
        };
        if let Some(m) = event.end_message.as_deref().filter(|m| !m.is_empty()) {
            effects.notices.push(m.to_owned());
        }
        for (key, _) in features(event.feature.as_deref().unwrap_or("")) {
            self.overrides.remove(&key);
            effects.changed = true;
        }
        self.run(event.end_script.as_deref(), effects);
    }

    /// ProcessEvent at `now` (local seconds): removes, ends, the notify messages due at
    /// this minute (NotifyInterval seconds, default 1800, rounded down to minutes), starts.
    /// Returns the effects and the names to delete.
    pub fn process(&mut self, events: &[Event], now: i64) -> (Effects, Vec<String>) {
        let plan = plan(events, now);
        let mut effects = Effects::default();
        for name in &plan.remove {
            self.end(name, &mut effects);
        }
        for name in &plan.end {
            self.end(name, &mut effects);
        }
        let minute_of_day = now.rem_euclid(DAY) / 60 * 60;
        for event in self.running.values() {
            let Some(m) = event.notify_message.as_deref().filter(|m| !m.is_empty()) else {
                continue;
            };
            let interval = event
                .notify_interval
                .filter(|i| *i >= 0)
                .map_or(1800, i64::from)
                / 60
                * 60;
            if interval != 0 && minute_of_day % interval == 0 {
                effects.notices.push(m.to_owned());
            }
        }
        for name in &plan.start {
            if let Some(event) = events.iter().find(|e| &e.name == name) {
                self.start(event, &mut effects);
            }
        }
        (effects, plan.remove)
    }

    /// RearrangeFirstIncomeMessage: the running events' notify messages, shown to each
    /// player 20 s after entering the game, one per second.
    pub fn first_income(&self) -> Vec<String> {
        self.running
            .values()
            .filter_map(|e| e.notify_message.clone().filter(|m| !m.is_empty()))
            .collect()
    }
}
