//! MemoryTicker — Phase 8B Phase D / T-D1.
//!
//! Turn-based scheduler that drives the rolling summary + compile pipeline:
//!
//! - Per-turn (8B.7): every `turns_per_summary` user turns, kick off
//!   `RollingSummarizer::rolling_summary` and
//!   `MemoryCompiler::compile_today` in the background.
//! - Session end (8B.7): force a final rolling summary then run
//!   `compile_today` and assemble.
//! - Daily (8B.8): once per logical day, run the full
//!   `compile_today` / week / longterm / facts / assemble pipeline
//!   (each step idempotent + JobRunner-throttled).
//! - Startup recovery (8B.9): catch up rolling summaries for any
//!   `(session, mtime)` pair that has changed since the last
//!   persisted summary.
//!
//! Per v2 §0.5 Δ-8, [`crate::modules::runtime::conversation::ConversationRuntime`]
//! invokes this scheduler via the
//! [`crate::modules::runtime::conversation::TurnHook`] trait — the
//! `commands/agent.rs` glue does not import this module.
//!
//! Per v2 §0.5 Δ-3, every audit emission is via
//! `MemoryAuditEmitter::*` free functions (no
//! `Arc<MemoryAuditEmitter>` field).
//!
//! 8B.6 lands the structural scaffolding only — both
//! [`TurnHook`] methods are no-op stubs.  The real `notify_turn` /
//! `notify_session_end` implementations ship in 8B.7 once
//! `spawn_rolling_then_compile_today` and `maybe_run_daily` are wired
//! up.

#![allow(dead_code)] // first real producers land in 8B.7+ (notify_turn / notify_session_end).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::modules::memory::compiler::MemoryCompiler;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::summary::rolling::RollingSummarizer;
use crate::modules::memory::summary::store::SessionSummaryStore;
use crate::modules::runtime::conversation::TurnHook;
use crate::modules::runtime::session::ConversationMessage;

/// Per-runtime tunables for the ticker.  All fields are simple
/// `Copy` types so the struct can be cheaply cloned + transported
/// through `MemoryConfigOverrides` (8B.x) without bringing in extra
/// allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickerConfig {
    /// Roll a summary every N user turns (default `6`, mirrors
    /// openhanako).
    pub turns_per_summary: u32,
    /// Backup timer interval for the daily compile cycle in seconds
    /// (default `3600` = 1h).  The daily job is also kicked off
    /// opportunistically by `notify_turn` when the logical day rolls
    /// over, so the timer only catches truly idle sessions.
    pub daily_check_interval_secs: u64,
    /// When `true` (default), `notify_session_end` also extracts
    /// session experiences via the future
    /// `experience::extractor` (Phase 8D).
    pub experience_enabled: bool,
}

impl Default for TickerConfig {
    fn default() -> Self {
        Self {
            turns_per_summary: 6,
            daily_check_interval_secs: 3600,
            experience_enabled: true,
        }
    }
}

/// Sub-tasks of the daily compile cycle.  Tracked individually inside
/// [`TickerState::daily_steps_completed`] so a partial failure
/// (e.g. LLM-throttled longterm) doesn't force the entire cycle to
/// re-run.
///
/// The 6 variants form a topological order:
/// `Today → Week → Longterm → Facts → Assemble → DeepMemory`.  The
/// future `do_daily` (8B.8) walks this list, checks completion, and
/// re-tries only what's missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DailyStep {
    /// Compile today's daily summary from rolling summaries.
    Today,
    /// Compile this week's summary from daily summaries.
    Week,
    /// Compile long-term summary from weekly summaries.
    Longterm,
    /// Extract structured facts from rolling + daily summaries.
    Facts,
    /// Assemble `compiled.md` from all higher-tier summaries + facts.
    Assemble,
    /// Synthesize deep-memory diary entries (Phase 8D).
    DeepMemory,
}

/// Mutable scheduler state, guarded by [`MemoryTicker::state`] Mutex.
///
/// Kept out of [`MemoryTicker`] itself so the surrounding methods can
/// take `&self` and the ticker can live behind `Arc` without an
/// outer wrapper.
#[derive(Debug, Default)]
pub struct TickerState {
    /// `session_id → number of user turns observed since last roll`.
    pub turn_counts: HashMap<String, u32>,
    /// Set of `session_id`s currently mid-roll, prevents
    /// double-spawn.
    pub summary_in_progress: HashSet<String>,
    /// Last logical day on which `do_daily` ran to completion.
    pub last_daily_job_date: Option<NaiveDate>,
    /// Sub-steps of the *current* daily cycle that have completed.
    pub daily_steps_completed: HashSet<DailyStep>,
    /// Logical day the partial daily-step set belongs to.  Resets
    /// the completed set when the day rolls over.
    pub daily_steps_date: Option<NaiveDate>,
    /// `true` while `do_daily` is running — prevents reentrancy.
    pub daily_running: bool,
}

/// Background scheduler.  See module docs for the full lifecycle.
///
/// 8B.6 lands construction + [`TurnHook`] no-op stubs only; full
/// orchestration ships in 8B.7+.
pub struct MemoryTicker {
    summarizer: Arc<RollingSummarizer>,
    compiler: Arc<MemoryCompiler>,
    summary_store: Arc<dyn SessionSummaryStore>,
    config: TickerConfig,
    state: Mutex<TickerState>,
}

impl std::fmt::Debug for MemoryTicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryTicker")
            .field("config", &self.config)
            .finish()
    }
}

impl MemoryTicker {
    /// Build a new ticker.  Construction is cheap — heavy work begins
    /// when [`TurnHook::on_turn_complete`] fires (real implementation
    /// in 8B.7).
    pub fn new(
        summarizer: Arc<RollingSummarizer>,
        compiler: Arc<MemoryCompiler>,
        summary_store: Arc<dyn SessionSummaryStore>,
        config: TickerConfig,
    ) -> Self {
        Self {
            summarizer,
            compiler,
            summary_store,
            config,
            state: Mutex::new(TickerState::default()),
        }
    }

    /// Read-only access to the runtime config.
    pub fn config(&self) -> &TickerConfig {
        &self.config
    }

    /// Snapshot of the current scheduler state for tests + future
    /// `MemoryJobsStatusCard` UI introspection.
    ///
    /// Returns `(active_session_count, in_progress_count,
    /// daily_running)`.
    ///
    /// # Errors
    ///
    /// Returns a string description if the internal mutex is
    /// poisoned (only happens when another thread panicked while
    /// holding the lock).
    pub fn snapshot_state(&self) -> Result<(usize, usize, bool), String> {
        let guard = self
            .state
            .lock()
            .map_err(|e| format!("ticker state mutex poisoned: {e}"))?;
        Ok((
            guard.turn_counts.len(),
            guard.summary_in_progress.len(),
            guard.daily_running,
        ))
    }
}

impl TurnHook for MemoryTicker {
    /// 8B.7 stub: a real implementation will (a) increment
    /// `turn_counts[session_id]`, (b) when the count hits
    /// `config.turns_per_summary`, spawn a tokio task that calls
    /// `summarizer.rolling_summary` + `compiler.compile_today`,
    /// (c) opportunistically call `do_daily` if the logical day
    /// has rolled over.  All errors are logged via `tracing` — the
    /// hook never returns a `Result`, never panics.
    fn on_turn_complete(
        &self,
        _scope: &MemoryExecutionScope,
        _session_id: &str,
        _messages: &[ConversationMessage],
    ) {
        // TODO(8B.7): wire turn_count + spawn rolling_summary + compile_today.
        tracing::trace!("MemoryTicker.on_turn_complete (8B.6 stub — no-op)");
    }

    /// 8B.7 stub: real implementation will force a final rolling
    /// summary, then `compile_today` + `assemble` synchronously, then
    /// — if `config.experience_enabled` — extract session experiences
    /// (Phase 8D).
    fn on_session_end(
        &self,
        _scope: &MemoryExecutionScope,
        _session_id: &str,
        _messages: &[ConversationMessage],
    ) {
        // TODO(8B.7): force final rolling + compile_today + assemble.
        tracing::trace!("MemoryTicker.on_session_end (8B.6 stub — no-op)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_config_default_values() {
        let c = TickerConfig::default();
        assert_eq!(c.turns_per_summary, 6);
        assert_eq!(c.daily_check_interval_secs, 3600);
        assert!(c.experience_enabled);
    }

    #[test]
    fn ticker_state_init_is_empty() {
        let s = TickerState::default();
        assert!(s.turn_counts.is_empty());
        assert!(s.summary_in_progress.is_empty());
        assert!(s.last_daily_job_date.is_none());
        assert!(s.daily_steps_completed.is_empty());
        assert!(s.daily_steps_date.is_none());
        assert!(!s.daily_running);
    }

    #[test]
    fn daily_step_hashset_membership() {
        let mut set: HashSet<DailyStep> = HashSet::new();
        set.insert(DailyStep::Today);
        set.insert(DailyStep::Week);
        assert!(set.contains(&DailyStep::Today));
        assert!(!set.contains(&DailyStep::Longterm));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn daily_step_serializes_stable_string() {
        let json = serde_json::to_string(&DailyStep::Longterm).expect("DailyStep serializes");
        // serde's default for unit enums is the variant name; that's
        // fine for TelemetryDrawer / MemoryJobsStatusCard which only
        // need a stable identifier.
        assert!(json.contains("Longterm") || json.contains("longterm"));
    }

    #[test]
    fn turn_hook_stubs_are_no_op_and_dont_panic() {
        // We can't construct a real MemoryTicker here without a
        // JobRunner + SessionSummaryStore + ProviderManager; the
        // compile-time symbol assertion is enough for 8B.6.  Real
        // wiring + behavioural tests land in 8B.7 with mock
        // collaborators.
        fn assert_impl<T: TurnHook>() {}
        assert_impl::<MemoryTicker>();
    }
}
