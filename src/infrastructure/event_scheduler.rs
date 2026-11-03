//! The panel's event schedule (laboratory extension of AdminClientServiceCore
//! EventDataContext, operator decision of 03/11/2026): the registered events of
//! `game_event_schedules`, the ProcessEvent pass at each minute boundary and RegisterEvent's
//! checks. A start or end changes the scheduler state, reloads the game content with its
//! keys and drop modifiers, and sends the Notice messages to every connected player.
use crate::domain::event_schedule::{Event, State};
use sqlx::PgPool;

/// RegisterEvent refusals (AdminReportNotifyMessage errors of the source).
#[derive(Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// "Event is Already Running !".
    Running,
    /// "Cannot Find Event Template !".
    NoTemplate,
    /// "Reservation Time Error": a start or end in the past, or a bad period.
    Time,
    Invalid,
}

/// A registration from the panel (times in UTC seconds; the period in local seconds).
#[derive(Clone, Debug, Default)]
pub struct Registration {
    pub name: String,
    pub template: Option<String>,
    pub feature: Option<String>,
    pub start_script: Option<String>,
    pub end_script: Option<String>,
    pub start_at: Option<i64>,
    pub end_at: Option<i64>,
    pub period_begin: Option<i32>,
    pub period_end: Option<i32>,
    pub start_message: Option<String>,
    pub notify_message: Option<String>,
    pub end_message: Option<String>,
    pub notify_interval: Option<i32>,
}

/// A registered event as the panel lists it.
#[derive(Clone, Debug)]
pub struct Listed {
    pub registration: Registration,
    pub running: bool,
}

type Row = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i32>,
    Option<i32>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i32>,
);

const COLUMNS: &str = "name,template,feature,start_script,end_script,
    extract(epoch FROM start_at)::bigint,extract(epoch FROM end_at)::bigint,
    period_begin_sec,period_end_sec,start_message,notify_message,end_message,notify_interval";

fn registration(r: Row) -> Registration {
    Registration {
        name: r.0,
        template: r.1,
        feature: r.2,
        start_script: r.3,
        end_script: r.4,
        start_at: r.5,
        end_at: r.6,
        period_begin: r.7,
        period_end: r.8,
        start_message: r.9,
        notify_message: r.10,
        end_message: r.11,
        notify_interval: r.12,
    }
}

async fn rows(pool: &PgPool) -> Result<Vec<Registration>, sqlx::Error> {
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM game_event_schedules ORDER BY created_at, name LIMIT 1000"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(registration).collect())
}

/// The registered events with their running state.
pub async fn list(
    pool: &PgPool,
    state: &std::sync::Mutex<State>,
) -> Result<Vec<Listed>, sqlx::Error> {
    let regs = rows(pool).await?;
    let state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(regs
        .into_iter()
        .map(|r| Listed {
            running: state.running.contains_key(&r.name),
            registration: r,
        })
        .collect())
}

/// The domain view of a registration at the server's civil offset.
fn event(r: &Registration, offset: i64) -> Event {
    Event {
        name: r.name.clone(),
        feature: r.feature.clone(),
        start_script: r.start_script.clone(),
        end_script: r.end_script.clone(),
        start_time: r.start_at.map(|t| t + offset),
        end_time: r.end_at.map(|t| t + offset),
        period_begin: r.period_begin.map(i64::from),
        period_end: r.period_end.map(i64::from),
        start_message: r.start_message.clone(),
        notify_message: r.notify_message.clone(),
        end_message: r.end_message.clone(),
        notify_interval: r.notify_interval,
    }
}

/// RegisterEvent: a template fills Feature and both scripts, the given fields override it; a
/// running event cannot change; StartTime and EndTime must not be in the past; an existing
/// name is modified.
pub async fn register(
    pool: &PgPool,
    state: &std::sync::Mutex<State>,
    templates: &(dyn Fn(&str) -> Option<(Option<String>, Option<String>, Option<String>)> + Sync),
    mut r: Registration,
    account: uuid::Uuid,
    now: i64,
) -> Result<Result<(), RegisterError>, sqlx::Error> {
    let name_ok = !r.name.trim().is_empty() && r.name.chars().count() <= 100;
    if !name_ok {
        return Ok(Err(RegisterError::Invalid));
    }
    if state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .running
        .contains_key(&r.name)
    {
        return Ok(Err(RegisterError::Running));
    }
    if let Some(t) = r.template.as_deref().filter(|t| !t.is_empty()) {
        let Some((feature, start, end)) = templates(t) else {
            return Ok(Err(RegisterError::NoTemplate));
        };
        r.feature = r.feature.filter(|f| !f.is_empty()).or(feature);
        r.start_script = r.start_script.filter(|f| !f.is_empty()).or(start);
        r.end_script = r.end_script.filter(|f| !f.is_empty()).or(end);
    }
    if r.start_at.is_some_and(|t| t < now)
        || r.end_at.is_some_and(|t| t < now)
        || r.period_begin.is_some_and(|p| !(0..86_400).contains(&p))
        || r.period_end.is_some_and(|p| !(0..=86_400).contains(&p))
        || r.notify_interval.is_some_and(|i| i < 0)
    {
        return Ok(Err(RegisterError::Time));
    }
    let bounded = |v: &Option<String>, n: usize| v.as_ref().is_none_or(|s| s.chars().count() <= n);
    if !(bounded(&r.feature, 2000)
        && bounded(&r.start_script, 4000)
        && bounded(&r.end_script, 4000)
        && bounded(&r.start_message, 500)
        && bounded(&r.notify_message, 500)
        && bounded(&r.end_message, 500))
    {
        return Ok(Err(RegisterError::Invalid));
    }
    sqlx::query(
        "INSERT INTO game_event_schedules(name,template,feature,start_script,end_script,start_at,end_at,period_begin_sec,period_end_sec,start_message,notify_message,end_message,notify_interval,created_by)
         VALUES($1,$2,$3,$4,$5,to_timestamp($6),to_timestamp($7),$8,$9,$10,$11,$12,$13,$14)
         ON CONFLICT (name) DO UPDATE SET template=EXCLUDED.template,feature=EXCLUDED.feature,start_script=EXCLUDED.start_script,end_script=EXCLUDED.end_script,start_at=EXCLUDED.start_at,end_at=EXCLUDED.end_at,period_begin_sec=EXCLUDED.period_begin_sec,period_end_sec=EXCLUDED.period_end_sec,start_message=EXCLUDED.start_message,notify_message=EXCLUDED.notify_message,end_message=EXCLUDED.end_message,notify_interval=EXCLUDED.notify_interval",
    )
    .bind(&r.name)
    .bind(&r.template)
    .bind(&r.feature)
    .bind(&r.start_script)
    .bind(&r.end_script)
    .bind(r.start_at.map(|t| t as f64))
    .bind(r.end_at.map(|t| t as f64))
    .bind(r.period_begin)
    .bind(r.period_end)
    .bind(&r.start_message)
    .bind(&r.notify_message)
    .bind(&r.end_message)
    .bind(r.notify_interval)
    .bind(account)
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

/// RemoveEvent: the row goes; a running event ends at the next pass (returns whether it
/// existed).
pub async fn remove(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("DELETE FROM game_event_schedules WHERE name=$1")
            .bind(name)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}

/// What one ProcessEvent pass asks the runtime to do.
#[derive(Debug, Default)]
pub struct Pass {
    /// The keys or drop modifiers changed: reload the content.
    pub reload: bool,
    pub notices: Vec<String>,
    pub unsupported: Vec<String>,
}

/// ProcessEvent at `now` (UTC seconds) with the civil `offset`: the registered events, an
/// event removed from the table ends like one past its EndTime, and past events are deleted.
pub async fn process(
    pool: &PgPool,
    state: &std::sync::Mutex<State>,
    now: i64,
    offset: i64,
) -> Result<Pass, sqlx::Error> {
    let regs = rows(pool).await?;
    let events: Vec<Event> = regs.iter().map(|r| event(r, offset)).collect();
    let (effects, removed) = {
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut gone = crate::domain::event_schedule::Effects::default();
        let missing: Vec<String> = state
            .running
            .keys()
            .filter(|n| !events.iter().any(|e| &e.name == *n))
            .cloned()
            .collect();
        for name in missing {
            state.end(&name, &mut gone);
        }
        let (mut effects, removed) = state.process(&events, now + offset);
        gone.notices.append(&mut effects.notices);
        gone.unsupported.append(&mut effects.unsupported);
        gone.changed |= effects.changed;
        (gone, removed)
    };
    for name in &removed {
        remove(pool, name).await?;
    }
    Ok(Pass {
        reload: effects.changed,
        notices: effects.notices,
        unsupported: effects.unsupported,
    })
}

/// Milliseconds until the next minute boundary (ScheduleNextProcess).
pub fn until_next_minute(now_millis: i64) -> u64 {
    let rest = 60_000 - now_millis.rem_euclid(60_000);
    u64::try_from(if rest == 0 { 60_000 } else { rest }).unwrap_or(60_000)
}
