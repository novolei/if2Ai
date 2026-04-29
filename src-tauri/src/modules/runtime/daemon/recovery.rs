//! FEAT-SH-001 — Recovery actions executed by the self-healing daemon.
//!
//! Each [`RecoveryAction`] is **idempotent**: invoking it twice on a
//! steady-state system produces the same observable effect as once.
//! That property is load-bearing — the daemon may double-fire a
//! recovery if its `HealthCheck` continues reporting `Failed` after
//! a previous repair attempt.
//!
//! New recovery variants are intentionally enum-shaped (not a trait
//! object) so the daemon loop can pattern-match for tracing without
//! a downcast. Add variants here; do not add ad-hoc closures.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::modules::memory::MemoryTicker;

/// Result of attempting a single recovery action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryOutcome {
    /// Action ran successfully (no-op also counts).
    Repaired,
    /// Action skipped because its preconditions were not met
    /// (e.g. the streak counter was already zero).
    NoOpSkipped,
    /// Action failed; daemon should mark the check `Failed` and
    /// surface a tracing warning. The `String` is human-readable.
    Failed(String),
}

/// Closed enum of recovery primitives. New variants land in their
/// own Pack; this Pack ports the two existing repairs from
/// `runtime/self_repair.rs`.
///
/// FEAT-SH-002: extended with `DegradeGracefully` (provider circuit
/// breaker tripped) and `RestartMcpServer` (MCP stdio child died).
/// Both new variants are *marker-only* in this Pack — the actual
/// degrade / restart logic lands in a follow-up wiring Pack so SH-002
/// can deliver the contract surface without touching provider /
/// process-management internals.
#[derive(Clone)]
pub enum RecoveryAction {
    /// Clear the `daily_running` stuck flag on a [`MemoryTicker`].
    /// Idempotent: clearing an already-clear flag is a no-op.
    ClearStuckState(Arc<MemoryTicker>),
    /// Reset the per-tool failure streak counter so the agent can
    /// retry. Idempotent: resetting a missing/zero counter no-ops.
    ClearBrokenStreak {
        tool_name: String,
        streaks: Arc<Mutex<HashMap<String, u32>>>,
    },
    /// FEAT-SH-002: provider circuit breaker — emit a tracing warning
    /// so the agent loop can fall back to a tighter retry budget.
    /// Marker-only in this Pack (`attempt()` always returns
    /// `Repaired`); real degrade routing lands in a future wiring Pack.
    DegradeGracefully { provider_name: String },
    /// FEAT-SH-002: MCP stdio child process is dead — emit a warning.
    /// Marker-only in this Pack (`attempt()` returns `NoOpSkipped`);
    /// the actual respawn lives in a follow-up Pack so this one stays
    /// inside its `Files` write list.
    RestartMcpServer { server_name: String },
}

impl RecoveryAction {
    /// Stable identifier used in tracing.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            RecoveryAction::ClearStuckState(_) => "clear_stuck_state",
            RecoveryAction::ClearBrokenStreak { .. } => "clear_broken_streak",
            RecoveryAction::DegradeGracefully { .. } => "degrade_gracefully",
            RecoveryAction::RestartMcpServer { .. } => "restart_mcp_server",
        }
    }

    /// Execute the action. Never panics; on lock-poisoning it
    /// returns `Failed` so the daemon can keep going.
    pub fn attempt(&self) -> RecoveryOutcome {
        match self {
            RecoveryAction::ClearStuckState(ticker) => {
                ticker.repair_clear_stuck_daily();
                RecoveryOutcome::Repaired
            }
            RecoveryAction::ClearBrokenStreak { tool_name, streaks } => {
                let Ok(mut g) = streaks.lock() else {
                    return RecoveryOutcome::Failed(
                        "broken_streak map mutex poisoned".to_string(),
                    );
                };
                if g.remove(tool_name).is_some() {
                    RecoveryOutcome::Repaired
                } else {
                    RecoveryOutcome::NoOpSkipped
                }
            }
            RecoveryAction::DegradeGracefully { provider_name } => {
                tracing::warn!(
                    provider = %provider_name,
                    "[daemon] provider circuit tripped; downstream layers should fall back"
                );
                RecoveryOutcome::Repaired
            }
            RecoveryAction::RestartMcpServer { server_name } => {
                tracing::warn!(
                    server = %server_name,
                    "[daemon] MCP server process is dead; restart-logic deferred to follow-up Pack"
                );
                RecoveryOutcome::NoOpSkipped
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_action_idempotent() {
        let streaks = Arc::new(Mutex::new(HashMap::new()));
        streaks.lock().unwrap().insert("bash".to_string(), 5);

        let action = RecoveryAction::ClearBrokenStreak {
            tool_name: "bash".to_string(),
            streaks: streaks.clone(),
        };

        assert_eq!(action.attempt(), RecoveryOutcome::Repaired);
        assert_eq!(action.attempt(), RecoveryOutcome::NoOpSkipped);
        assert_eq!(action.attempt(), RecoveryOutcome::NoOpSkipped);
        assert!(streaks.lock().unwrap().is_empty());
    }

    #[test]
    fn recovery_action_name_is_stable() {
        let streaks = Arc::new(Mutex::new(HashMap::new()));
        let a = RecoveryAction::ClearBrokenStreak {
            tool_name: "x".into(),
            streaks,
        };
        assert_eq!(a.name(), "clear_broken_streak");
    }
}
