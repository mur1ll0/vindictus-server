//! Scheduled maintenance of the game server (B4.4c), the single authority that warns every
//! connection and stops the frontend. Follows FrontendServiceCore ShutDownProcessor: one
//! NoticeShutDownMessage{Delay} at once, then at the 300/60/15/10/5 second steps, and
//! service.ShutDown() when the delay ends; a newer schedule or a cancel (Delay -1) makes the
//! pending notices and the shutdown of the older one do nothing.
use std::sync::Mutex;
use tokio::sync::{broadcast, watch};
use tokio::time::{Duration, Instant};

/// The panel accepts up to one hour.
pub const MAX_DELAY_SECONDS: i32 = 3600;
const STEPS: [i32; 5] = [300, 60, 15, 10, 5];

/// (seconds after the start, Delay sent) of ShutDownProcessor.Run for `delay` seconds.
pub fn notice_schedule(delay: i32) -> Vec<(i32, i32)> {
    let mut out = vec![(0, delay)];
    let mut num = delay;
    for step in STEPS {
        if num % step != 0 && num > step {
            num -= num % step;
            out.push((delay - num, num));
        }
        while num > step {
            num -= step;
            out.push((delay - num, num));
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct Plan {
    pub id: u64,
    pub delay_seconds: i32,
    /// The content version the restart applies.
    pub version: i32,
    pub requested_by: String,
    /// Seconds left when read.
    pub remaining_seconds: i32,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum MaintenanceError {
    #[error("maintenance delay outside 0..=3600 seconds")]
    Delay,
    #[error("a maintenance is already scheduled")]
    Busy,
}

#[derive(Clone)]
struct Scheduled {
    plan: Plan,
    start: Instant,
}

pub struct Maintenance {
    current: watch::Sender<Option<Scheduled>>,
    notices: broadcast::Sender<i32>,
    next: Mutex<u64>,
}

impl Default for Maintenance {
    fn default() -> Self {
        Self {
            current: watch::Sender::new(None),
            notices: broadcast::Sender::new(16),
            next: Mutex::new(0),
        }
    }
}

impl Maintenance {
    /// Schedule the stop in `delay` seconds; only one at a time.
    pub fn schedule(
        &self,
        delay: i32,
        version: i32,
        requested_by: &str,
    ) -> Result<Plan, MaintenanceError> {
        if !(0..=MAX_DELAY_SECONDS).contains(&delay) {
            return Err(MaintenanceError::Delay);
        }
        let mut next = self.next.lock().map_err(|_| MaintenanceError::Busy)?;
        if self.current.borrow().is_some() {
            return Err(MaintenanceError::Busy);
        }
        *next += 1;
        let plan = Plan {
            id: *next,
            delay_seconds: delay,
            version,
            requested_by: requested_by.to_owned(),
            remaining_seconds: delay,
        };
        self.current.send_replace(Some(Scheduled {
            plan: plan.clone(),
            start: Instant::now(),
        }));
        Ok(plan)
    }
    /// Delay -1 of the original operation: the pending schedule does nothing.
    pub fn cancel(&self) -> bool {
        self.current.send_replace(None).is_some()
    }
    pub fn current(&self) -> Option<Plan> {
        self.current.borrow().as_ref().map(|s| {
            let elapsed = i32::try_from(s.start.elapsed().as_secs()).unwrap_or(i32::MAX);
            Plan {
                remaining_seconds: (s.plan.delay_seconds - elapsed).max(0),
                ..s.plan.clone()
            }
        })
    }
    /// The Delay values every connection sends as NoticeShutDownMessage.
    pub fn notices(&self) -> broadcast::Receiver<i32> {
        self.notices.subscribe()
    }
    /// Sends the notices of the current schedule and returns its plan when its delay ends.
    pub async fn wait_for_shutdown(&self) -> Plan {
        let mut watch = self.current.subscribe();
        loop {
            let scheduled = watch.borrow_and_update().clone();
            let Some(s) = scheduled else {
                if watch.changed().await.is_err() {
                    std::future::pending::<()>().await;
                }
                continue;
            };
            let mut cancelled = false;
            let delay = u64::try_from(s.plan.delay_seconds).unwrap_or(0);
            for (at, value) in notice_schedule(s.plan.delay_seconds) {
                let at = s.start + Duration::from_secs(u64::try_from(at).unwrap_or(0));
                tokio::select! {
                    () = tokio::time::sleep_until(at) => {
                        // No receiver yet only means no connection.
                        let _ = self.notices.send(value);
                    }
                    _ = watch.changed() => { cancelled = true; break; }
                }
            }
            if !cancelled {
                tokio::select! {
                    () = tokio::time::sleep_until(s.start + Duration::from_secs(delay)) => {
                        return s.plan;
                    }
                    _ = watch.changed() => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_follows_shutdown_processor_steps() {
        let five: Vec<i32> = notice_schedule(300).into_iter().map(|(_, d)| d).collect();
        assert_eq!(five, [300, 240, 180, 120, 60, 45, 30, 15, 10, 5]);
        assert_eq!(notice_schedule(0), [(0, 0)]);
        // 7 minutes: 420, then 300 and every 60 seconds below it.
        let seven = notice_schedule(420);
        assert_eq!(&seven[..3], &[(0, 420), (120, 300), (180, 240)]);
        assert!(seven.iter().all(|(at, d)| at + d == 420));
    }

    #[tokio::test(start_paused = true)]
    async fn notices_then_shutdown_and_cancel_stops_them() {
        let m = Maintenance::default();
        let mut rx = m.notices();
        assert_eq!(m.schedule(3601, 1, "admin"), Err(MaintenanceError::Delay));
        let plan = m.schedule(15, 1, "admin").unwrap();
        assert_eq!(m.schedule(5, 1, "admin"), Err(MaintenanceError::Busy));
        let done = m.wait_for_shutdown().await;
        assert_eq!(done, plan);
        let mut seen = Vec::new();
        while let Ok(d) = rx.try_recv() {
            seen.push(d);
        }
        assert_eq!(seen, [15, 10, 5]);
        assert!(m.cancel());
        assert!(!m.cancel());
        m.schedule(60, 2, "admin").unwrap();
        assert!(m.cancel());
        let waited = tokio::time::timeout(Duration::from_secs(120), m.wait_for_shutdown()).await;
        assert!(waited.is_err());
    }
}
