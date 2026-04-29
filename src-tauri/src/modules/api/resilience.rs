//! FEAT-SH-002 — Provider liveness primitives.
//!
//! Exposes a [`HealthCheck`] implementation that wraps a provider
//! "ping" closure and tracks consecutive failures via
//! [`ProviderCircuitState`]. Three consecutive failures escalate from
//! `Degraded` to `Failed`, at which point the daemon fires
//! [`RecoveryAction::DegradeGracefully`].
//!
//! Why a closure rather than a baked-in HTTP call?
//! - Each provider transport has a different "cheap" probe (Anthropic
//!   `GET /v1/models`, OpenAI `GET /v1/models`, local llama-cpp HEAD,
//!   …). A pluggable closure keeps `resilience.rs` provider-agnostic.
//! - Tests inject deterministic ping closures without any network I/O.
//!
//! No new dependency is introduced — the closure-based design works on
//! `std::sync` + the existing daemon traits.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use crate::modules::runtime::daemon::{HealthCheck, HealthStatus, RecoveryAction};

/// Three-strike threshold before the circuit trips. Documented as a
/// constant so the matching test (`provider_liveness_three_strike`)
/// can pin the contract value.
pub const PROVIDER_CIRCUIT_BREAKER_THRESHOLD: u32 = 3;

/// Process-wide circuit state for one provider. Cheap to share across
/// the agent loop (which records request outcomes) and the daemon
/// (which polls + decides recovery).
#[derive(Debug, Default)]
pub struct ProviderCircuitState {
    consecutive_failures: AtomicU32,
}

impl ProviderCircuitState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            consecutive_failures: AtomicU32::new(0),
        }
    }

    /// Reset the streak (call on a successful provider request).
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
    }

    /// Increment the failure streak; returns the new total.
    pub fn record_failure(&self) -> u32 {
        self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Read the current streak length.
    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::SeqCst)
    }

    /// `true` once the streak has hit
    /// [`PROVIDER_CIRCUIT_BREAKER_THRESHOLD`].
    #[must_use]
    pub fn is_tripped(&self) -> bool {
        self.consecutive_failures() >= PROVIDER_CIRCUIT_BREAKER_THRESHOLD
    }
}

// ---------------------------------------------------------------------------
// Module C (truth-loop iter-7) — process-wide ProviderCircuitState singleton.
// ---------------------------------------------------------------------------
//
// `provider/resilience.rs::StreamCircuitState` is per-stream and not
// observable from outside the streaming task. Aggregating its
// success/failure signals into one process-wide
// [`ProviderCircuitState`] gives the self-healing daemon a real
// failure source to poll: previously the only provider-side health
// signal was the env-var-only [`make_provider_api_key_probe`].
//
// The singleton is a no-op until `provider/resilience.rs` calls
// [`global_provider_circuit`] to record its outcomes, so adding the
// OnceLock is a strictly additive change.

static GLOBAL_PROVIDER_CIRCUIT: OnceLock<Arc<ProviderCircuitState>> = OnceLock::new();

/// Process-wide [`ProviderCircuitState`] used by the self-healing
/// daemon's chat-provider liveness probe.
///
/// Lazily initialised on first access; cheap to clone. Anyone who
/// observes a provider success / failure (today: `StreamCircuitState`
/// in `provider/resilience.rs`) SHOULD mirror the call here so the
/// daemon's [`make_provider_circuit_probe`] sees the streak.
#[must_use]
pub fn global_provider_circuit() -> Arc<ProviderCircuitState> {
    Arc::clone(GLOBAL_PROVIDER_CIRCUIT.get_or_init(|| Arc::new(ProviderCircuitState::new())))
}

/// HealthCheck wrapper that reads an existing [`ProviderCircuitState`]
/// streak directly — does NOT call any ping closure. Useful when the
/// real success/failure signal already comes from the stream pipeline
/// (`provider/resilience.rs`) and a periodic active probe would only
/// double-count or reset the streak unintentionally.
///
/// Status table:
///
/// | streak           | status                                      |
/// |------------------|---------------------------------------------|
/// | 0                | `Healthy`                                   |
/// | 1..=THRESHOLD-1  | `Degraded`                                  |
/// | >= THRESHOLD     | `Failed` (recovery: `DegradeGracefully`)    |
pub struct ProviderCircuitProbe {
    name: String,
    state: Arc<ProviderCircuitState>,
}

impl ProviderCircuitProbe {
    #[must_use]
    pub fn new(name: impl Into<String>, state: Arc<ProviderCircuitState>) -> Self {
        Self {
            name: name.into(),
            state,
        }
    }
}

impl HealthCheck for ProviderCircuitProbe {
    fn name(&self) -> &str {
        &self.name
    }

    fn check(&self) -> HealthStatus {
        let streak = self.state.consecutive_failures();
        if streak == 0 {
            HealthStatus::Healthy
        } else if streak >= PROVIDER_CIRCUIT_BREAKER_THRESHOLD {
            HealthStatus::Failed {
                reason: format!(
                    "provider {} consecutive failure streak={streak} ≥ threshold {}",
                    self.name, PROVIDER_CIRCUIT_BREAKER_THRESHOLD
                ),
            }
        } else {
            HealthStatus::Degraded {
                reason: format!("provider {} consecutive failure streak={streak}", self.name),
            }
        }
    }

    fn recovery(&self) -> Option<RecoveryAction> {
        if self.state.is_tripped() {
            Some(RecoveryAction::DegradeGracefully {
                provider_name: self.name.clone(),
            })
        } else {
            None
        }
    }
}

/// Factory for [`ProviderCircuitProbe`] — mirrors the
/// [`provider_heartbeat_check_fn`] shape so call sites in
/// `desktop_host/setup.rs` compose the same way.
#[must_use]
pub fn make_provider_circuit_probe(
    name: impl Into<String>,
    state: Arc<ProviderCircuitState>,
) -> Arc<dyn HealthCheck> {
    Arc::new(ProviderCircuitProbe::new(name, state))
}

type PingFn = Box<dyn Fn() -> bool + Send + Sync>;

/// Provider heartbeat health check. Construct via
/// [`provider_heartbeat_check_fn`].
pub struct ProviderHeartbeatCheck {
    name: String,
    state: Arc<ProviderCircuitState>,
    ping: PingFn,
}

impl HealthCheck for ProviderHeartbeatCheck {
    fn name(&self) -> &str {
        &self.name
    }

    fn check(&self) -> HealthStatus {
        if (self.ping)() {
            self.state.record_success();
            return HealthStatus::Healthy;
        }
        let streak = self.state.record_failure();
        if streak >= PROVIDER_CIRCUIT_BREAKER_THRESHOLD {
            HealthStatus::Failed {
                reason: format!(
                    "provider {} consecutive failure streak={streak} ≥ threshold {}",
                    self.name, PROVIDER_CIRCUIT_BREAKER_THRESHOLD
                ),
            }
        } else {
            HealthStatus::Degraded {
                reason: format!("provider {} consecutive failure streak={streak}", self.name),
            }
        }
    }

    fn recovery(&self) -> Option<RecoveryAction> {
        if self.state.is_tripped() {
            Some(RecoveryAction::DegradeGracefully {
                provider_name: self.name.clone(),
            })
        } else {
            None
        }
    }
}

/// Factory: build a provider-heartbeat HealthCheck wrapping the given
/// `ping` closure + circuit `state`. Returned as `Arc<dyn HealthCheck>`
/// so callers can plug it straight into
/// [`crate::modules::runtime::daemon::HealthCheckRegistry::register_check`].
///
/// `ping` MUST return `true` on a healthy probe and `false` on
/// timeout / 4xx / 5xx; transport details are the closure's
/// responsibility.
#[must_use]
pub fn provider_heartbeat_check_fn(
    name: impl Into<String>,
    state: Arc<ProviderCircuitState>,
    ping: impl Fn() -> bool + Send + Sync + 'static,
) -> Arc<dyn HealthCheck> {
    Arc::new(ProviderHeartbeatCheck {
        name: name.into(),
        state,
        ping: Box::new(ping),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn provider_heartbeat_returns_health_status() {
        let healthy = Arc::new(AtomicBool::new(true));
        let healthy_for_closure = healthy.clone();
        let state = Arc::new(ProviderCircuitState::new());
        let probe = ProviderHeartbeatCheck {
            name: "test_provider".to_string(),
            state: state.clone(),
            ping: Box::new(move || healthy_for_closure.load(Ordering::SeqCst)),
        };

        state.record_failure();
        assert_eq!(state.consecutive_failures(), 1);
        assert_eq!(probe.check(), HealthStatus::Healthy);
        assert_eq!(state.consecutive_failures(), 0);

        healthy.store(false, Ordering::SeqCst);
        match probe.check() {
            HealthStatus::Degraded { .. } => {}
            other => panic!("expected Degraded, got {other:?}"),
        }
        assert!(probe.recovery().is_none());
    }

    #[test]
    fn provider_circuit_state_tracks_streak() {
        let s = ProviderCircuitState::new();
        assert!(!s.is_tripped());
        assert_eq!(s.record_failure(), 1);
        assert_eq!(s.record_failure(), 2);
        assert_eq!(s.record_failure(), 3);
        assert!(s.is_tripped());
        s.record_success();
        assert_eq!(s.consecutive_failures(), 0);
        assert!(!s.is_tripped());
    }

    /// Module C (truth-loop iter-7) — the streak-reading probe must
    /// promote `Degraded` → `Failed` once the threshold is hit and
    /// recommend `DegradeGracefully` for the daemon to act on. Run
    /// against an isolated `ProviderCircuitState` (NOT the global
    /// singleton) so parallel tests can't race on shared streak
    /// state.
    #[test]
    fn provider_circuit_probe_promotes_to_failed_at_threshold() {
        let state = Arc::new(ProviderCircuitState::new());
        let probe = ProviderCircuitProbe::new("chat", state.clone());

        assert_eq!(probe.check(), HealthStatus::Healthy);
        assert!(probe.recovery().is_none());

        state.record_failure();
        match probe.check() {
            HealthStatus::Degraded { reason } => {
                assert!(reason.contains("streak=1"), "got: {reason}");
            }
            other => panic!("expected Degraded, got {other:?}"),
        }
        assert!(probe.recovery().is_none());

        state.record_failure();
        state.record_failure();
        match probe.check() {
            HealthStatus::Failed { reason } => {
                assert!(reason.contains("≥ threshold"), "got: {reason}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        match probe.recovery() {
            Some(RecoveryAction::DegradeGracefully { provider_name }) => {
                assert_eq!(provider_name, "chat");
            }
            other => panic!("expected DegradeGracefully, got {other:?}"),
        }

        state.record_success();
        assert_eq!(probe.check(), HealthStatus::Healthy);
    }
}
