//! MemoryTicker — Phase 8B Phase D / T-D1 + T-D2.
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
//! 8B.7 (T-D2) replaces the 8B.6 stubs with real
//! [`TurnHook::on_turn_complete`] / [`TurnHook::on_session_end`]
//! implementations and exposes a synchronous [`MemoryTicker::flush_session`]
//! API for the future `flush_session` Tauri command.  Wiring of the ticker
//! into `commands/agent.rs::ConversationRuntime::with_turn_hook(...)` is
//! deferred until `commands/agent.rs` un-stashes — until then the hook
//! is constructed and held in [`crate::commands::AppState::memory_ticker`]
//! but is not yet driven by real turns.

#![allow(dead_code)] // wired into ConversationRuntime in a follow-up un-stash slice.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::modules::memory::compiler::{CompilePaths, MemoryCompiler};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::summary::rolling::RollingSummarizer;
use crate::modules::memory::summary::store::SessionSummaryStore;
use crate::modules::memory::MemoryError;
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
/// 8B.7 (T-D2) lands the real `notify_turn` / `notify_session_end`
/// implementations + the synchronous [`Self::flush_session`] API.
pub struct MemoryTicker {
    summarizer: Arc<RollingSummarizer>,
    compiler: Arc<MemoryCompiler>,
    summary_store: Arc<dyn SessionSummaryStore>,
    config: TickerConfig,
    /// Wrapped in `Arc<Mutex<…>>` rather than a bare `Mutex<…>` so the
    /// background `tokio::spawn` closures inside `on_turn_complete` /
    /// `on_session_end` can `clone` the handle and continue to mark
    /// `summary_in_progress` after the ticker reference is dropped.
    state: Arc<Mutex<TickerState>>,
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
    /// when [`TurnHook::on_turn_complete`] fires.
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
            state: Arc::new(Mutex::new(TickerState::default())),
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

    /// Resolve `<data_local_dir>/.if2ai/memory` per v2 §0.5 Δ-6.  Returns
    /// `None` when the OS does not expose a local data directory (rare;
    /// CI sandboxes occasionally hit this) — callers must downgrade to a
    /// `tracing::warn!` and skip the step.
    fn memory_root() -> Option<std::path::PathBuf> {
        dirs::data_local_dir().map(|d| d.join(".if2ai").join("memory"))
    }

    /// Spawn rolling_summary → compile_today → assemble in a background
    /// tokio task.  Errors are logged via `tracing` and never propagate.
    fn spawn_rolling_then_compile_today(
        &self,
        scope: MemoryExecutionScope,
        session_id: String,
        messages: Vec<ConversationMessage>,
    ) {
        match self.state.lock() {
            Ok(mut g) => {
                if g.summary_in_progress.contains(&session_id) {
                    tracing::debug!(
                        session_id = %session_id,
                        "rolling already in progress, skipping spawn"
                    );
                    return;
                }
                g.summary_in_progress.insert(session_id.clone());
            }
            Err(e) => {
                tracing::error!(error = %e, "ticker state mutex poisoned");
                return;
            }
        }

        let summarizer = self.summarizer.clone();
        let compiler = self.compiler.clone();
        let state = Arc::clone(&self.state);

        tokio::spawn(async move {
            if let Err(e) = summarizer
                .rolling_summary(&session_id, &scope, &messages)
                .await
            {
                tracing::error!(
                    session_id = %session_id,
                    error = %e,
                    "rolling_summary failed"
                );
            }
            if let Some(memory_root) = Self::memory_root() {
                let paths = CompilePaths::from_scope_root(&memory_root);
                if let Err(e) = compiler.compile_today(&scope, &paths).await {
                    tracing::warn!(error = %e, "compile_today failed in spawn");
                }
                if let Err(e) = compiler.assemble(&scope, &paths) {
                    tracing::warn!(error = %e, "assemble failed in spawn");
                }
            } else {
                tracing::warn!("data_local_dir unavailable; skipping compile_today + assemble");
            }
            finish_in_progress(&state, &session_id);
        });
    }

    /// Synchronously flush a session: rolling summary + compile_today +
    /// assemble run inline in the caller's task.  Used by
    /// [`TurnHook::on_session_end`] (via spawn) and by the future
    /// `flush_session` Tauri command (await direct).
    ///
    /// Idempotent: a second concurrent call for the same `session_id`
    /// returns `Ok(())` without doing any work.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryError`] only when the rolling summary itself
    /// fails or the internal mutex is poisoned.  `compile_today` /
    /// `assemble` failures are downgraded to `tracing::warn!` so a
    /// stale fingerprint doesn't block session shutdown.
    pub async fn flush_session(
        &self,
        scope: &MemoryExecutionScope,
        session_id: &str,
        messages: &[ConversationMessage],
    ) -> Result<(), MemoryError> {
        {
            let mut g = self
                .state
                .lock()
                .map_err(|e| MemoryError::Generic(format!("ticker mutex poisoned: {e}")))?;
            if g.summary_in_progress.contains(session_id) {
                tracing::debug!(
                    session_id = %session_id,
                    "flush_session: already in progress, skipping"
                );
                return Ok(());
            }
            g.summary_in_progress.insert(session_id.to_string());
            g.turn_counts.remove(session_id);
        }

        let _guard = InProgressGuard {
            state: &self.state,
            session_id,
        };

        self.summarizer
            .rolling_summary(session_id, scope, messages)
            .await
            .inspect_err(|e| {
                tracing::error!(
                    session_id = %session_id,
                    error = %e,
                    "flush rolling_summary failed"
                );
            })?;

        if let Some(memory_root) = Self::memory_root() {
            let paths = CompilePaths::from_scope_root(&memory_root);
            if let Err(e) = self.compiler.compile_today(scope, &paths).await {
                tracing::warn!(error = %e, "flush compile_today failed (non-fatal)");
            }
            if let Err(e) = self.compiler.assemble(scope, &paths) {
                tracing::warn!(error = %e, "flush assemble failed (non-fatal)");
            }
        } else {
            tracing::warn!(
                "flush_session: data_local_dir unavailable; skipping compile_today + assemble"
            );
        }

        if self.config.experience_enabled {
            // Phase 8D experience extractor — wired separately.
            tracing::trace!(
                session_id = %session_id,
                "flush_session: experience extraction TBD in 8D"
            );
        }

        Ok(())
    }
}

/// Drop the `session_id` from `summary_in_progress`.  Used by the
/// background spawn closures (after the awaited work completes) and by
/// the [`InProgressGuard`] RAII helper that flush_session relies on.
fn finish_in_progress(state: &Arc<Mutex<TickerState>>, session_id: &str) {
    if let Ok(mut g) = state.lock() {
        g.summary_in_progress.remove(session_id);
    }
}

/// RAII guard: ensures `session_id` is removed from `summary_in_progress`
/// even if the awaited future is cancelled / panics part-way through
/// `flush_session`.
struct InProgressGuard<'a> {
    state: &'a Arc<Mutex<TickerState>>,
    session_id: &'a str,
}

impl<'a> Drop for InProgressGuard<'a> {
    fn drop(&mut self) {
        finish_in_progress(self.state, self.session_id);
    }
}

impl TurnHook for MemoryTicker {
    /// Increment per-session turn count; when the count is a positive
    /// multiple of [`TickerConfig::turns_per_summary`], spawn a
    /// background `rolling_summary → compile_today → assemble` task.
    ///
    /// Two early-exit conditions:
    /// - `session_id == "-"` — the 8A.7 fallback for "scope unavailable";
    ///   nothing to summarise.
    /// - `messages.is_empty()` — defensive; nothing to summarise.
    ///
    /// All errors are logged via `tracing` — this hook never panics
    /// and never returns a `Result`.
    fn on_turn_complete(
        &self,
        scope: &MemoryExecutionScope,
        session_id: &str,
        messages: &[ConversationMessage],
    ) {
        if session_id == "-" || messages.is_empty() {
            return;
        }

        let count = match self.state.lock() {
            Ok(mut g) => {
                let c = g.turn_counts.entry(session_id.to_string()).or_insert(0);
                *c = c.saturating_add(1);
                *c
            }
            Err(e) => {
                tracing::error!(error = %e, "ticker state mutex poisoned in on_turn_complete");
                return;
            }
        };

        let threshold = self.config.turns_per_summary;
        if threshold > 0 && count > 0 && count % threshold == 0 {
            self.spawn_rolling_then_compile_today(
                scope.clone(),
                session_id.to_string(),
                messages.to_vec(),
            );
        }

        // TODO(8B.8): self.maybe_run_daily(scope) — opportunistic daily kick
    }

    /// Spawn a background task that runs the full session-flush
    /// pipeline: rolling summary → compile_today → assemble (+ Phase 8D
    /// experience extraction when enabled).  Synchronous from the
    /// runtime's perspective — the caller does not await us.
    ///
    /// Re-entrancy is suppressed by the same `summary_in_progress`
    /// gate that [`Self::flush_session`] uses, so a concurrent
    /// `flush_session` Tauri command will short-circuit.
    fn on_session_end(
        &self,
        scope: &MemoryExecutionScope,
        session_id: &str,
        messages: &[ConversationMessage],
    ) {
        if session_id == "-" {
            return;
        }
        let scope_owned = scope.clone();
        let session_owned = session_id.to_string();
        let messages_owned = messages.to_vec();
        let summarizer = self.summarizer.clone();
        let compiler = self.compiler.clone();
        let experience_enabled = self.config.experience_enabled;
        let state = Arc::clone(&self.state);

        tokio::spawn(async move {
            {
                let mut g = match state.lock() {
                    Ok(g) => g,
                    Err(e) => {
                        tracing::error!(error = %e, "session_end mutex poisoned");
                        return;
                    }
                };
                if g.summary_in_progress.contains(&session_owned) {
                    return;
                }
                g.summary_in_progress.insert(session_owned.clone());
                g.turn_counts.remove(&session_owned);
            }

            if let Err(e) = summarizer
                .rolling_summary(&session_owned, &scope_owned, &messages_owned)
                .await
            {
                tracing::error!(
                    session_id = %session_owned,
                    error = %e,
                    "session_end rolling_summary failed"
                );
            }
            if let Some(memory_root) = MemoryTicker::memory_root() {
                let paths = CompilePaths::from_scope_root(&memory_root);
                if let Err(e) = compiler.compile_today(&scope_owned, &paths).await {
                    tracing::warn!(error = %e, "session_end compile_today failed");
                }
                if let Err(e) = compiler.assemble(&scope_owned, &paths) {
                    tracing::warn!(error = %e, "session_end assemble failed");
                }
            } else {
                tracing::warn!(
                    "session_end: data_local_dir unavailable; skipping compile_today + assemble"
                );
            }
            if experience_enabled {
                tracing::trace!(
                    session_id = %session_owned,
                    "session_end: experience extraction TBD in 8D"
                );
            }
            finish_in_progress(&state, &session_owned);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::memory::job_runner::JobRunner;
    use crate::modules::memory::llm::MockUtilityLlm;
    use crate::modules::memory::summary::store::NullSessionSummaryStore;
    use crate::modules::memory::UtilityLlm;
    use crate::modules::runtime::config::CompilerConfig;
    use crate::modules::runtime::session::{ContentBlock, MessageRole};

    fn make_ticker_with_config(config: TickerConfig) -> MemoryTicker {
        let store: Arc<dyn SessionSummaryStore> = Arc::new(NullSessionSummaryStore::new());
        let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
        let job_runner = Arc::new(
            JobRunner::open_in_memory_for_tests(3, 2).expect("in-mem JobRunner constructs"),
        );
        let summarizer = Arc::new(RollingSummarizer::new(
            store.clone(),
            llm.clone(),
            job_runner.clone(),
            None,
        ));
        let compiler = Arc::new(MemoryCompiler::new(
            store.clone(),
            llm,
            job_runner,
            CompilerConfig::default(),
        ));
        MemoryTicker::new(summarizer, compiler, store, config)
    }

    fn make_ticker() -> MemoryTicker {
        make_ticker_with_config(TickerConfig::default())
    }

    fn user_msg(text: &str) -> ConversationMessage {
        ConversationMessage {
            role: MessageRole::User,
            blocks: vec![ContentBlock::Text { text: text.into() }],
            usage: None,
            thinking: None,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            request_id: None,
        }
    }

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
        assert!(json.contains("Longterm") || json.contains("longterm"));
    }

    #[tokio::test]
    async fn turn_count_increments_per_call() {
        // Use a high threshold so we don't trip a spawn during the test.
        let ticker = make_ticker_with_config(TickerConfig {
            turns_per_summary: 1_000,
            ..TickerConfig::default()
        });
        let scope = MemoryExecutionScope::global();
        for _ in 0..3 {
            ticker.on_turn_complete(&scope, "sess-1", &[user_msg("hi")]);
        }
        let g = ticker.state.lock().expect("state lock");
        assert_eq!(g.turn_counts.get("sess-1").copied(), Some(3));
    }

    #[tokio::test]
    async fn session_id_dash_is_skipped() {
        let ticker = make_ticker();
        let scope = MemoryExecutionScope::global();
        ticker.on_turn_complete(&scope, "-", &[user_msg("hi")]);
        let g = ticker.state.lock().expect("state lock");
        assert!(
            g.turn_counts.is_empty(),
            "session_id='-' must not register a turn count"
        );
    }

    #[tokio::test]
    async fn empty_messages_are_skipped() {
        let ticker = make_ticker_with_config(TickerConfig {
            turns_per_summary: 1_000,
            ..TickerConfig::default()
        });
        ticker.on_turn_complete(&MemoryExecutionScope::global(), "sess-x", &[]);
        let g = ticker.state.lock().expect("state lock");
        assert!(g.turn_counts.is_empty());
    }

    #[tokio::test]
    async fn snapshot_state_reflects_turn_counts() {
        let ticker = make_ticker_with_config(TickerConfig {
            turns_per_summary: 1_000,
            ..TickerConfig::default()
        });
        let scope = MemoryExecutionScope::global();
        ticker.on_turn_complete(&scope, "sess-a", &[user_msg("x")]);
        ticker.on_turn_complete(&scope, "sess-b", &[user_msg("y")]);
        let (active, in_progress, daily) = ticker.snapshot_state().expect("snapshot succeeds");
        assert_eq!(active, 2);
        assert_eq!(in_progress, 0);
        assert!(!daily);
    }

    #[tokio::test]
    async fn flush_session_clears_turn_count() {
        let ticker = make_ticker_with_config(TickerConfig {
            turns_per_summary: 1_000,
            experience_enabled: false,
            ..TickerConfig::default()
        });
        let scope = MemoryExecutionScope::global();
        // Pre-register a turn count
        ticker.on_turn_complete(&scope, "sess-flush", &[user_msg("hi")]);
        {
            let g = ticker.state.lock().expect("state lock");
            assert_eq!(g.turn_counts.get("sess-flush").copied(), Some(1));
        }

        let res = ticker
            .flush_session(&scope, "sess-flush", &[user_msg("a"), user_msg("b")])
            .await;
        assert!(res.is_ok(), "flush_session should succeed: {res:?}");

        let g = ticker.state.lock().expect("state lock");
        assert!(
            !g.turn_counts.contains_key("sess-flush"),
            "flush must remove the per-session turn count"
        );
        assert!(
            !g.summary_in_progress.contains("sess-flush"),
            "in-progress guard must clear after flush"
        );
    }

    #[tokio::test]
    async fn flush_session_is_idempotent_under_inprogress_guard() {
        let ticker = make_ticker_with_config(TickerConfig {
            turns_per_summary: 1_000,
            experience_enabled: false,
            ..TickerConfig::default()
        });
        // Manually mark in-progress and confirm flush short-circuits.
        {
            let mut g = ticker.state.lock().expect("state lock");
            g.summary_in_progress.insert("sess-busy".to_string());
            g.turn_counts.insert("sess-busy".to_string(), 4);
        }
        let res = ticker
            .flush_session(
                &MemoryExecutionScope::global(),
                "sess-busy",
                &[user_msg("z")],
            )
            .await;
        assert!(res.is_ok(), "short-circuited flush returns Ok(())");
        let g = ticker.state.lock().expect("state lock");
        // Short-circuit must NOT remove the turn_count (we never started work).
        assert_eq!(g.turn_counts.get("sess-busy").copied(), Some(4));
        assert!(g.summary_in_progress.contains("sess-busy"));
    }

    #[test]
    fn turn_hook_impl_is_present() {
        fn assert_impl<T: TurnHook>() {}
        assert_impl::<MemoryTicker>();
    }
}
