//! FEAT-SH-003 — Browser session health state machine + recovery plan.
//!
//! Pure functions over [`crate::modules::browser::session::SessionHeartbeat`]
//! observations. Built as a separate module so:
//!
//! 1. The chromiumoxide-heavy `BrowserSession` does not need to depend
//!    on the daemon framework (avoids a cyclic / heavy import graph).
//! 2. Tests can exhaust every state-machine transition without launching
//!    a real browser — the input is just `Option<Instant>` + a crashed
//!    flag + a reconnect-failure counter.
//! 3. The same rules can later drive a frontend projection without
//!    re-deriving the contract.

use std::time::{Duration, Instant};

use crate::modules::runtime::daemon::{HealthCheck, HealthStatus, RecoveryAction};

/// Heartbeat older than this is considered stale.
pub const STALE_HEARTBEAT_THRESHOLD: Duration = Duration::from_secs(30);

/// Reconnect attempts beyond this trigger cloud escalation.
pub const RECONNECT_FAILURE_ESCALATION_THRESHOLD: u32 = 3;

/// Five-state lifecycle for a single browser session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserHealthStatus {
    /// Heartbeat fresh; session is interactive.
    Connected,
    /// Heartbeat older than [`STALE_HEARTBEAT_THRESHOLD`] but no
    /// crash signal — likely network blip or page transition.
    Stale,
    /// Underlying CDP transport lost (no heartbeat ever observed
    /// after construction or watcher reported `disconnected`).
    Disconnected,
    /// A recovery plan is currently executing.
    Recovering,
    /// Chromium child died; full process restart required.
    Crashed,
}

/// One step of the recovery plan a daemon (or future supervisor)
/// will execute on the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserRecoveryAction {
    /// Soft fix: re-issue CDP `attach` against the same browser.
    Reconnect,
    /// Tear down the chromium process and respawn it with the same
    /// profile. Used when [`BrowserHealthStatus::Crashed`].
    RestartProcess,
    /// Hand off to the cloud browser pool (FEAT-BR-XXX). Triggered
    /// only after `RECONNECT_FAILURE_ESCALATION_THRESHOLD` failed
    /// reconnects in a row.
    EscalateToCloud,
    /// No action needed; session is healthy.
    NoAction,
}

/// Bundle the agreed action with a tracing reason and the daemon
/// `RecoveryAction` to invoke. Wrapping `RecoveryAction` keeps this
/// module independent from the per-Pack list of recovery variants —
/// here we always emit `RestartMcpServer`-like markers via a future
/// `RecoveryAction::ResetBrowserSession`. Until that variant is added
/// (out of scope for this Pack), recovery callers can still
/// pattern-match on [`BrowserRecoveryAction`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserRecoveryPlan {
    pub action: BrowserRecoveryAction,
    pub reason: String,
}

impl BrowserHealthStatus {
    /// Pure transition function. Inputs:
    /// - `now`: the wall-clock instant the daemon poll ran.
    /// - `last_heartbeat`: most recent CDP-observed heartbeat.
    /// - `crashed`: sticky crash flag from the chromium process watcher.
    ///
    /// Returns the new status. Pure / deterministic / cheap → safe
    /// to call inside `HealthCheck::check`.
    #[must_use]
    pub fn from_observation(now: Instant, last_heartbeat: Option<Instant>, crashed: bool) -> Self {
        if crashed {
            return BrowserHealthStatus::Crashed;
        }
        match last_heartbeat {
            None => BrowserHealthStatus::Disconnected,
            Some(t) if now.saturating_duration_since(t) > STALE_HEARTBEAT_THRESHOLD => {
                BrowserHealthStatus::Stale
            }
            Some(_) => BrowserHealthStatus::Connected,
        }
    }

    /// Map the lifecycle state onto a daemon [`HealthStatus`].
    #[must_use]
    pub fn to_health_status(self) -> HealthStatus {
        match self {
            BrowserHealthStatus::Connected => HealthStatus::Healthy,
            BrowserHealthStatus::Stale => HealthStatus::Degraded {
                reason: "browser heartbeat stale (> 30s)".to_string(),
            },
            BrowserHealthStatus::Disconnected => HealthStatus::Failed {
                reason: "browser CDP disconnected (no heartbeat)".to_string(),
            },
            BrowserHealthStatus::Recovering => HealthStatus::Degraded {
                reason: "browser session recovery in flight".to_string(),
            },
            BrowserHealthStatus::Crashed => HealthStatus::Failed {
                reason: "browser child process crashed".to_string(),
            },
        }
    }
}

/// One-shot snapshot used by [`check_session_health`] callers and the
/// HealthCheck implementation.
#[derive(Debug, Clone)]
pub struct BrowserSessionObservation {
    pub now: Instant,
    pub last_heartbeat: Option<Instant>,
    pub crashed: bool,
    pub reconnect_failures: u32,
}

/// Compute the daemon HealthStatus for the given observation.
#[must_use]
pub fn check_session_health(obs: &BrowserSessionObservation) -> HealthStatus {
    BrowserHealthStatus::from_observation(obs.now, obs.last_heartbeat, obs.crashed)
        .to_health_status()
}

/// Compute the recovery plan paired with the current observation.
#[must_use]
pub fn build_recovery_plan(obs: &BrowserSessionObservation) -> BrowserRecoveryPlan {
    let status = BrowserHealthStatus::from_observation(obs.now, obs.last_heartbeat, obs.crashed);
    match status {
        BrowserHealthStatus::Connected | BrowserHealthStatus::Recovering => BrowserRecoveryPlan {
            action: BrowserRecoveryAction::NoAction,
            reason: "session healthy or already recovering".to_string(),
        },
        BrowserHealthStatus::Stale | BrowserHealthStatus::Disconnected => {
            if obs.reconnect_failures >= RECONNECT_FAILURE_ESCALATION_THRESHOLD {
                BrowserRecoveryPlan {
                    action: BrowserRecoveryAction::EscalateToCloud,
                    reason: format!(
                        "reconnect failed {} ≥ threshold {}",
                        obs.reconnect_failures, RECONNECT_FAILURE_ESCALATION_THRESHOLD
                    ),
                }
            } else {
                BrowserRecoveryPlan {
                    action: BrowserRecoveryAction::Reconnect,
                    reason: format!("status={status:?}; reconnect attempt"),
                }
            }
        }
        BrowserHealthStatus::Crashed => BrowserRecoveryPlan {
            action: BrowserRecoveryAction::RestartProcess,
            reason: "chromium process crashed; respawn required".to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// Daemon HealthCheck wrapper
// ---------------------------------------------------------------------------

type ObservationFn = Box<dyn Fn() -> BrowserSessionObservation + Send + Sync>;

/// HealthCheck wrapper that polls a closure returning a fresh
/// observation. Closure-based to avoid coupling daemon ↔ smart_browser
/// concrete types and so tests can drive arbitrary observation
/// sequences.
pub struct BrowserSessionLivenessCheck {
    session_id: String,
    check_id: String,
    observe: ObservationFn,
}

impl BrowserSessionLivenessCheck {
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        observe: impl Fn() -> BrowserSessionObservation + Send + Sync + 'static,
    ) -> Self {
        let session_id: String = session_id.into();
        let check_id = format!("browser_session_liveness:{session_id}");
        Self {
            session_id,
            check_id,
            observe: Box::new(observe),
        }
    }
}

impl HealthCheck for BrowserSessionLivenessCheck {
    fn name(&self) -> &str {
        &self.check_id
    }
    fn check(&self) -> HealthStatus {
        let obs = (self.observe)();
        check_session_health(&obs)
    }
    fn recovery(&self) -> Option<RecoveryAction> {
        let obs = (self.observe)();
        let plan = build_recovery_plan(&obs);
        match plan.action {
            // Until the daemon adds a dedicated `ResetBrowserSession`
            // variant (deferred Pack), reuse `RestartMcpServer` as the
            // generic "external subsystem needs respawn" marker — the
            // string carries the plan reason for downstream tracing.
            BrowserRecoveryAction::Reconnect
            | BrowserRecoveryAction::RestartProcess
            | BrowserRecoveryAction::EscalateToCloud => Some(RecoveryAction::RestartMcpServer {
                server_name: format!("browser:{}:{}", self.session_id, plan.reason),
            }),
            BrowserRecoveryAction::NoAction => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_observation_classifies_basic_cases() {
        let base = Instant::now();
        // Crashed always wins.
        assert_eq!(
            BrowserHealthStatus::from_observation(base, Some(base), true),
            BrowserHealthStatus::Crashed
        );
        // No heartbeat → Disconnected.
        assert_eq!(
            BrowserHealthStatus::from_observation(base, None, false),
            BrowserHealthStatus::Disconnected
        );
        // Fresh → Connected.
        assert_eq!(
            BrowserHealthStatus::from_observation(base, Some(base), false),
            BrowserHealthStatus::Connected
        );
    }
}
