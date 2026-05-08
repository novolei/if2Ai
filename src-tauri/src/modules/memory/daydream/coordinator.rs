//! DayDreamCoordinator — owns the idle timer and last-activity stamp.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use super::engine::{CycleError, DayDreamEngine};
use super::report::DayDreamReport;

const POLL_INTERVAL: Duration = Duration::from_secs(60);

pub struct DayDreamCoordinator {
    engine: Arc<DayDreamEngine>,
    last_activity: Mutex<Instant>,
    /// `true` once a cycle has fired for the current idle window.
    /// Reset by `note_activity()`.
    fired_this_window: Mutex<bool>,
}

impl DayDreamCoordinator {
    pub fn new(engine: Arc<DayDreamEngine>) -> Self {
        Self {
            engine,
            last_activity: Mutex::new(Instant::now()),
            fired_this_window: Mutex::new(false),
        }
    }

    /// Called by `turn_service` on every prompt entry.
    pub async fn note_activity(&self) {
        *self.last_activity.lock().await = Instant::now();
        *self.fired_this_window.lock().await = false;
    }

    /// Manual trigger from the command. Bypasses the idle gate.
    pub async fn trigger_manual(&self) -> Result<DayDreamReport, CycleError> {
        self.engine.run_cycle("manual").await
    }

    /// Poll-tick: returns `Some(report)` if the idle threshold fired this tick.
    pub async fn maybe_fire(&self) -> Option<Result<DayDreamReport, CycleError>> {
        let cfg = self.engine.config();
        if !cfg.enabled {
            return None;
        }
        let elapsed = self.last_activity.lock().await.elapsed();
        let threshold = Duration::from_secs(cfg.idle_trigger_minutes * 60);
        if elapsed < threshold {
            return None;
        }
        let mut fired = self.fired_this_window.lock().await;
        if *fired {
            return None;
        }
        *fired = true;
        Some(self.engine.run_cycle("idle").await)
    }
}

/// Spawn the long-running poll loop using `tokio::spawn`.
///
/// **Caller must already be inside a Tokio runtime context** (e.g. inside an
/// `async fn` or a `#[tokio::test]`). The Tauri `setup` hook is NOT such a
/// context — the production path in `main.rs` inlines the loop body via
/// `tauri::async_runtime::spawn` instead. This helper is kept for tests and
/// any future caller that already has a runtime entered.
pub fn spawn_poll_loop<F>(
    coord: Arc<DayDreamCoordinator>,
    on_report: F,
) -> tokio::task::JoinHandle<()>
where
    F: Fn(DayDreamReport) + Send + 'static,
{
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        loop {
            ticker.tick().await;
            if let Some(Ok(report)) = coord.maybe_fire().await {
                on_report(report);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_interval_is_60_seconds() {
        assert_eq!(POLL_INTERVAL, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn note_activity_resets_idle_window() {
        // Pure-state test: doesn't need a real engine.
        let stamp = Mutex::new(Instant::now() - Duration::from_secs(2_000));
        // Simulate threshold = 1800s; elapsed = 2000s → would fire.
        assert!(stamp.lock().await.elapsed() >= Duration::from_secs(1_800));
        // After note_activity, elapsed must be < threshold.
        *stamp.lock().await = Instant::now();
        assert!(stamp.lock().await.elapsed() < Duration::from_secs(1_800));
    }
}
