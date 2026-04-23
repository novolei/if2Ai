//! P1-9 — Minimal observer surface (Noop / Log / Multi).
//!
//! Call sites use [`emit`] with a stable event name + small payload string.

use std::sync::{Arc, OnceLock};

/// Observer receives coarse-grained lifecycle / infra events.
pub trait Observer: Send + Sync {
    fn on_event(&self, name: &str, payload: &str);
}

/// No-op observer (default).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopObserver;

impl Observer for NoopObserver {
    fn on_event(&self, _name: &str, _payload: &str) {}
}

/// Writes `tracing::info!` with a dedicated target for log filtering.
#[derive(Debug, Default, Clone, Copy)]
pub struct LogObserver;

impl Observer for LogObserver {
    fn on_event(&self, name: &str, payload: &str) {
        tracing::info!(target: "if2ai_observer", event = %name, payload = %payload);
    }
}

/// Fan-out to multiple observers.
#[derive(Clone)]
pub struct MultiObserver {
    inner: Arc<Vec<Arc<dyn Observer>>>,
}

impl MultiObserver {
    #[must_use]
    pub fn new(inner: Vec<Arc<dyn Observer>>) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}

impl Observer for MultiObserver {
    fn on_event(&self, name: &str, payload: &str) {
        for o in self.inner.iter() {
            o.on_event(name, payload);
        }
    }
}

static GLOBAL: OnceLock<Arc<dyn Observer>> = OnceLock::new();

fn global() -> Arc<dyn Observer> {
    GLOBAL
        .get_or_init(|| {
            let mode = std::env::var("IF2AI_OBSERVER").unwrap_or_default();
            if mode.eq_ignore_ascii_case("log") || mode == "1" {
                Arc::new(LogObserver) as Arc<dyn Observer>
            } else {
                Arc::new(NoopObserver) as Arc<dyn Observer>
            }
        })
        .clone()
}

/// Emit one observer event (cheap; default is no-op unless `IF2AI_OBSERVER=log`).
pub fn emit(name: &str, payload: &str) {
    global().on_event(name, payload);
}
