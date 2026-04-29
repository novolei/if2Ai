//! FEAT-SH-001 — Backwards-compatibility shim.
//!
//! The original 135 LOC of bespoke watchdog logic moved into
//! [`crate::modules::runtime::daemon`]. This module is kept as a
//! shim so existing callers — which import
//! [`record_tool_outcome`] and [`spawn_self_repair_watchdog`] — keep
//! compiling without churn.
//!
//! New code should depend on
//! [`crate::modules::runtime::daemon::spawn_self_healing_daemon`]
//! and the per-tool outcome accounting below.

use std::sync::Arc;

use crate::modules::memory::MemoryTicker;
use crate::modules::runtime::daemon;

#[allow(unused_imports)]
#[doc(inline)]
pub use crate::modules::runtime::daemon::{
    spawn_self_healing_daemon, DaemonState, HealthCheck, HealthCheckRegistry, HealthStatus,
    RecoveryAction, RecoveryOutcome, DAEMON_DISABLE_ENV,
};

/// Record a single tool outcome from the agent loop. On success the
/// per-tool counter is cleared; on failure it is incremented. The
/// daemon's `BrokenToolStreakHealthCheck` consumes this counter on
/// each poll and triggers `RecoveryAction::ClearBrokenStreak` once
/// the streak crosses the configured threshold.
///
/// API preserved verbatim from the pre-FEAT-SH-001 implementation
/// (Pack contract I1: signature-stable shim).
pub fn record_tool_outcome(tool_name: &str, ok: bool) {
    let streaks = daemon::global_tool_fail_streaks();
    let Ok(mut g) = streaks.lock() else {
        return;
    };
    if ok {
        g.remove(tool_name);
        return;
    }
    *g.entry(tool_name.to_string()).or_insert(0) += 1;
}

/// Legacy spawn entry point. Now a thin alias for
/// [`spawn_self_healing_daemon`]; signature preserved per Pack
/// contract I5 (no caller churn).
pub fn spawn_self_repair_watchdog(memory_ticker: Arc<MemoryTicker>) {
    spawn_self_healing_daemon(memory_ticker);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_tool_outcome_resets_on_success() {
        record_tool_outcome("bash", false);
        record_tool_outcome("bash", false);
        record_tool_outcome("bash", true);
        let streaks = daemon::global_tool_fail_streaks();
        let g = streaks.lock().unwrap();
        assert!(!g.contains_key("bash"));
    }
}
