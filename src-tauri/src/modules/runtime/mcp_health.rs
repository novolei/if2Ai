//! Truth-loop iter-8 — MCP liveness snapshot cache.
//!
//! ## Why a snapshot?
//!
//! - The self-healing daemon's [`HealthCheck::check`] is **synchronous**
//!   and runs on tokio worker threads. It cannot
//!   `tokio::Mutex::blocking_lock`, and cannot `await`.
//! - The only persistent stdio MCP manager today is
//!   `smart_browser::runtime::BROWSER_USE_MCP_MANAGER`, an
//!   `Arc<tokio::sync::Mutex<McpServerManager>>`. Probes therefore
//!   cannot read it directly.
//! - Solution: a process-wide [`McpHealthSnapshot`] of last-known
//!   `(server_name → alive)` is refreshed by an async background
//!   task and read by the sync daemon probe.
//!
//! ## What this module does
//!
//! - Owns the [`global_mcp_health`] singleton.
//! - Exposes [`McpHealthSnapshot::record`] for the refresher and
//!   [`McpHealthSnapshot::sample`] for the probe.
//! - Provides factory [`make_browser_use_liveness_probe`] that wraps
//!   the existing [`crate::modules::runtime::daemon::McpServerLivenessCheck`]
//!   pattern (so daemon registration + recovery action stay shared with
//!   SH-002).
//!
//! ## What this module does NOT do
//!
//! - Spawn the refresher task (that is `desktop_host::setup`'s job —
//!   it owns the tokio runtime + the `BROWSER_USE_MCP_MANAGER` handle).
//! - Cover MCP servers users configure in `settings.json` outside the
//!   browser-use subset; those have no persistent child today and
//!   require a separate Pack to introduce a long-lived manager for
//!   the full config map.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default freshness budget. A sample older than this is treated as
/// `Degraded` (snapshot stale) rather than authoritative.  Sized to be
/// **3× the refresher interval** so a single skipped tick does not
/// cause a false `Degraded`.
pub const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(90);

/// One server's last-observed liveness reading.
#[derive(Debug)]
pub struct LivenessSample {
    alive: AtomicBool,
    /// Unix milliseconds at the time of the last refresh.  `0` until
    /// the refresher has written this sample at least once.
    last_checked_unix_ms: AtomicI64,
}

impl LivenessSample {
    fn new() -> Self {
        Self {
            alive: AtomicBool::new(true),
            last_checked_unix_ms: AtomicI64::new(0),
        }
    }

    /// Latest observation. `(alive, last_checked_unix_ms)`.
    /// `last_checked_unix_ms == 0` means "never refreshed".
    pub fn snapshot(&self) -> (bool, i64) {
        (
            self.alive.load(Ordering::Acquire),
            self.last_checked_unix_ms.load(Ordering::Acquire),
        )
    }

    fn store(&self, alive: bool, unix_ms: i64) {
        self.alive.store(alive, Ordering::Release);
        self.last_checked_unix_ms.store(unix_ms, Ordering::Release);
    }
}

/// Process-wide MCP liveness snapshot.  Cheap to clone (it's an
/// `Arc<...>` under the hood once you go through [`global_mcp_health`]).
#[derive(Debug)]
pub struct McpHealthSnapshot {
    samples: RwLock<HashMap<String, Arc<LivenessSample>>>,
    stale_after: Duration,
}

impl McpHealthSnapshot {
    #[must_use]
    pub fn new(stale_after: Duration) -> Self {
        Self {
            samples: RwLock::new(HashMap::new()),
            stale_after,
        }
    }

    /// Refresher API — record one observation. Called from an async
    /// background task; never blocks longer than the inner write
    /// lock + a couple of atomic writes.
    pub fn record(&self, server_name: &str, alive: bool) {
        let now = unix_ms_now();
        // Fast path: sample exists → just update atomics under read lock.
        if let Ok(g) = self.samples.read() {
            if let Some(sample) = g.get(server_name) {
                sample.store(alive, now);
                return;
            }
        }
        // Slow path: insert.
        if let Ok(mut g) = self.samples.write() {
            let entry = g
                .entry(server_name.to_string())
                .or_insert_with(|| Arc::new(LivenessSample::new()));
            entry.store(alive, now);
        }
    }

    /// Probe API — read the most recent observation, or `None` if the
    /// refresher has never written this server.
    #[must_use]
    pub fn sample(&self, server_name: &str) -> Option<Arc<LivenessSample>> {
        self.samples.read().ok()?.get(server_name).cloned()
    }

    /// Test-only / introspection — number of distinct servers tracked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.read().map(|g| g.len()).unwrap_or(0)
    }

    /// `true` when no server has ever been recorded. Provided to
    /// satisfy `clippy::len_without_is_empty`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Stale window in effect. Exposed so the probe can format
    /// human-readable reasons.
    #[must_use]
    pub fn stale_after(&self) -> Duration {
        self.stale_after
    }
}

static GLOBAL_MCP_HEALTH: OnceLock<Arc<McpHealthSnapshot>> = OnceLock::new();

/// Process-wide [`McpHealthSnapshot`].  Lazily allocated with
/// [`DEFAULT_STALE_AFTER`].
#[must_use]
pub fn global_mcp_health() -> Arc<McpHealthSnapshot> {
    Arc::clone(
        GLOBAL_MCP_HEALTH.get_or_init(|| Arc::new(McpHealthSnapshot::new(DEFAULT_STALE_AFTER))),
    )
}

fn unix_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// HealthCheck factory — wraps the existing McpServerLivenessCheck contract.
// ---------------------------------------------------------------------------

use crate::modules::runtime::daemon::{HealthCheck, HealthStatus, RecoveryAction};

/// Internal probe that combines snapshot read + stale guard.  Public
/// only via [`make_browser_use_liveness_probe`] so callers cannot
/// construct an "always Healthy" stub by accident.
struct McpSnapshotLivenessCheck {
    server_name: String,
    check_id: String,
    snapshot: Arc<McpHealthSnapshot>,
}

impl HealthCheck for McpSnapshotLivenessCheck {
    fn name(&self) -> &str {
        &self.check_id
    }

    fn check(&self) -> HealthStatus {
        let Some(sample) = self.snapshot.sample(&self.server_name) else {
            // Refresher has not written yet (process just started, or
            // no MCP server has ever been observed). "Innocent until
            // proven dead" — do not let the daemon escalate.
            return HealthStatus::Healthy;
        };
        let (alive, last_ms) = sample.snapshot();
        if last_ms == 0 {
            return HealthStatus::Healthy;
        }
        let age_ms = unix_ms_now().saturating_sub(last_ms);
        if age_ms > i64::try_from(self.snapshot.stale_after().as_millis()).unwrap_or(i64::MAX) {
            return HealthStatus::Degraded {
                reason: format!(
                    "MCP server '{}' liveness snapshot stale by {}ms (refresher tardy?)",
                    self.server_name, age_ms
                ),
            };
        }
        if alive {
            HealthStatus::Healthy
        } else {
            HealthStatus::Failed {
                reason: format!(
                    "MCP server '{}' process not alive (snapshot age {}ms)",
                    self.server_name, age_ms
                ),
            }
        }
    }

    fn recovery(&self) -> Option<RecoveryAction> {
        Some(RecoveryAction::RestartMcpServer {
            server_name: self.server_name.clone(),
        })
    }
}

/// Build a HealthCheck that surfaces the snapshot reading for the
/// process-wide `BROWSER_USE_MCP_MANAGER` (the only persistent stdio
/// MCP child today).  The check id is `mcp_server_liveness:<name>`
/// to match the convention established by SH-002.
#[must_use]
pub fn make_browser_use_liveness_probe(snapshot: Arc<McpHealthSnapshot>) -> Arc<dyn HealthCheck> {
    use crate::modules::smart_browser::browser_use_mcp::BROWSER_USE_MCP_SERVER_NAME;

    let server_name = BROWSER_USE_MCP_SERVER_NAME.to_string();
    let check_id = format!("mcp_server_liveness:{server_name}");
    Arc::new(McpSnapshotLivenessCheck {
        server_name,
        check_id,
        snapshot,
    })
}

/// Pure decision function for the MCP config-presence probe.
/// Extracted so unit tests can pin both branches without depending
/// on the process-global `runtime::config::CURRENT_CONFIG` state
/// (which other lib tests may have already initialised).
#[must_use]
pub fn config_presence_status(initialised: bool) -> HealthStatus {
    if initialised {
        HealthStatus::Healthy
    } else {
        HealthStatus::Degraded {
            reason: "runtime config not initialised; \
                     MCP server map cannot be polled"
                .to_string(),
        }
    }
}

/// Truth-loop iter-8 — config-presence probe.
///
/// `Healthy` once `runtime::config::set_current` has installed a real
/// `RuntimeConfig` (regardless of whether any MCP server is actually
/// configured — zero MCP servers is a valid healthy steady state).
///
/// `Degraded` while the global config is still falling through to
/// `RuntimeConfig::empty`, so the operator knows the daemon's MCP
/// view is *not* authoritative yet.
#[must_use]
pub fn make_mcp_config_presence_probe() -> Arc<dyn HealthCheck> {
    struct McpConfigPresenceProbe;

    impl HealthCheck for McpConfigPresenceProbe {
        fn name(&self) -> &str {
            "mcp_config_presence"
        }

        fn check(&self) -> HealthStatus {
            config_presence_status(crate::modules::runtime::config::is_initialised())
        }
    }

    Arc::new(McpConfigPresenceProbe)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_snapshot() -> Arc<McpHealthSnapshot> {
        Arc::new(McpHealthSnapshot::new(DEFAULT_STALE_AFTER))
    }

    fn probe_for(snapshot: Arc<McpHealthSnapshot>, server: &str) -> McpSnapshotLivenessCheck {
        McpSnapshotLivenessCheck {
            server_name: server.to_string(),
            check_id: format!("mcp_server_liveness:{server}"),
            snapshot,
        }
    }

    #[test]
    fn sample_default_is_unknown_healthy() {
        let snap = fresh_snapshot();
        let probe = probe_for(snap, "test-server");
        // No record yet → daemon must NOT escalate.
        assert_eq!(probe.check(), HealthStatus::Healthy);
        // But recovery is still wired so a subsequent dead reading
        // can trigger a restart without re-registering the probe.
        assert!(matches!(
            probe.recovery(),
            Some(RecoveryAction::RestartMcpServer { .. })
        ));
    }

    #[test]
    fn recent_alive_true_yields_healthy() {
        let snap = fresh_snapshot();
        snap.record("test-server", true);
        let probe = probe_for(snap, "test-server");
        assert_eq!(probe.check(), HealthStatus::Healthy);
    }

    #[test]
    fn recent_alive_false_yields_failed_with_restart_recovery() {
        let snap = fresh_snapshot();
        snap.record("test-server", false);
        let probe = probe_for(snap.clone(), "test-server");
        match probe.check() {
            HealthStatus::Failed { reason } => {
                assert!(reason.contains("test-server"), "got: {reason}");
                assert!(reason.contains("not alive"), "got: {reason}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        match probe.recovery() {
            Some(RecoveryAction::RestartMcpServer { server_name }) => {
                assert_eq!(server_name, "test-server");
            }
            other => panic!("expected RestartMcpServer, got {other:?}"),
        }
    }

    #[test]
    fn stale_sample_yields_degraded() {
        // Build a snapshot whose stale window is microseconds, so we
        // can deterministically force the "stale" branch without
        // sleeping.
        let snap = Arc::new(McpHealthSnapshot::new(Duration::from_millis(1)));
        snap.record("test-server", true);
        // Spin briefly so unix_ms_now() advances past the 1ms window.
        std::thread::sleep(Duration::from_millis(10));
        let probe = probe_for(snap, "test-server");
        match probe.check() {
            HealthStatus::Degraded { reason } => {
                assert!(reason.contains("stale by"), "got: {reason}");
            }
            other => panic!("expected Degraded, got {other:?}"),
        }
    }

    #[test]
    fn config_presence_status_pins_both_branches() {
        // Pure decision function, decoupled from process-global
        // `CURRENT_CONFIG` state so this test stays deterministic
        // regardless of which other lib tests ran in the same
        // binary (and may have already called `set_current`).
        match config_presence_status(false) {
            HealthStatus::Degraded { reason } => {
                assert!(reason.contains("not initialised"), "got: {reason}");
            }
            other => panic!("expected Degraded for false, got {other:?}"),
        }
        assert_eq!(config_presence_status(true), HealthStatus::Healthy);
        // The wrapper probe never recommends recovery — it's an
        // observability-only signal.
        assert!(make_mcp_config_presence_probe().recovery().is_none());
    }
}
