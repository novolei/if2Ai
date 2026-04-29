//! FEAT-SH-001 — Health-check primitives for the self-healing daemon.
//!
//! A `HealthCheck` is a tiny named probe that returns a [`HealthStatus`].
//! Recovery is a *separate* concern (see [`super::recovery`]) so the
//! same probe can drive multiple recovery strategies in future Packs.
//!
//! Design intent:
//! - Probes are cheap, sync, and Send + Sync so the daemon loop can
//!   poll them on a single Tokio interval without spawning per-check
//!   tasks. If a future Pack needs an async probe (e.g. provider HTTP
//!   ping in FEAT-SH-002), we'll add a parallel `AsyncHealthCheck`
//!   trait rather than gating this one behind `async fn`.
//! - The registry is `Arc<RwLock<...>>`-friendly: callers may register
//!   checks at startup or at any time after the daemon is spawned.

use std::sync::Arc;

use super::recovery::RecoveryAction;

/// One probe outcome. `Degraded` means the subsystem is unhealthy but
/// still serving requests (recovery should be attempted opportunistically);
/// `Failed` means it is wedged and recovery should run *now*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Failed { reason: String },
}

impl HealthStatus {
    /// `true` when the subsystem needs attention (degraded or failed).
    #[must_use]
    pub fn needs_recovery(&self) -> bool {
        !matches!(self, HealthStatus::Healthy)
    }
}

/// One probe registered with the daemon.
///
/// Every check carries an optional [`RecoveryAction`] so the daemon
/// loop can attempt repair without a second lookup. `recovery = None`
/// is valid (probe is observability-only).
pub trait HealthCheck: Send + Sync {
    /// Stable identifier used in tracing + dashboards.
    fn name(&self) -> &str;
    /// Run the probe. MUST be cheap (≤ a few µs) and never block.
    fn check(&self) -> HealthStatus;
    /// Recovery action paired with this probe. Returning `None` means
    /// "log only, do not heal".
    fn recovery(&self) -> Option<RecoveryAction> {
        None
    }
}

/// Append-only registry of health checks.
///
/// Append-only because removal would race with the polling loop;
/// FEAT-SH-002 will introduce a `disable(name)` flag instead.
#[derive(Default, Clone)]
pub struct HealthCheckRegistry {
    checks: Vec<Arc<dyn HealthCheck>>,
}

impl HealthCheckRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Register a new probe. Returns the registry for builder-style chaining.
    pub fn register_check(&mut self, check: Arc<dyn HealthCheck>) -> &mut Self {
        self.checks.push(check);
        self
    }

    /// Number of registered probes (for diagnostics + tests).
    #[must_use]
    pub fn len(&self) -> usize {
        self.checks.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    /// Poll every check and return `(name, status)` pairs in
    /// registration order. Allocates a Vec — fine for the watchdog
    /// rate (1/min by default) and keeps callers free to filter by
    /// status before allocating recovery work.
    #[must_use]
    pub fn poll_all(&self) -> Vec<(String, HealthStatus)> {
        self.checks
            .iter()
            .map(|c| (c.name().to_string(), c.check()))
            .collect()
    }

    /// Borrowing accessor used by the daemon loop to invoke recovery
    /// without copying the trait object.
    #[must_use]
    pub fn checks(&self) -> &[Arc<dyn HealthCheck>] {
        &self.checks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingCheck {
        name: &'static str,
        calls: AtomicUsize,
        status_after_first: HealthStatus,
    }

    impl HealthCheck for CountingCheck {
        fn name(&self) -> &str {
            self.name
        }
        fn check(&self) -> HealthStatus {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                HealthStatus::Healthy
            } else {
                self.status_after_first.clone()
            }
        }
    }

    #[test]
    fn health_check_registry() {
        let mut registry = HealthCheckRegistry::new();
        assert!(registry.is_empty());
        let probe = Arc::new(CountingCheck {
            name: "test_probe",
            calls: AtomicUsize::new(0),
            status_after_first: HealthStatus::Failed {
                reason: "boom".to_string(),
            },
        });
        registry.register_check(probe.clone());
        assert_eq!(registry.len(), 1);

        let first = registry.poll_all();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].0, "test_probe");
        assert_eq!(first[0].1, HealthStatus::Healthy);

        let second = registry.poll_all();
        assert!(second[0].1.needs_recovery());
    }

    #[test]
    fn health_status_needs_recovery_classification() {
        assert!(!HealthStatus::Healthy.needs_recovery());
        assert!(HealthStatus::Degraded { reason: "x".into() }.needs_recovery());
        assert!(HealthStatus::Failed { reason: "x".into() }.needs_recovery());
    }
}
