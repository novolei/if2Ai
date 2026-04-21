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

mod daily;
mod turn_hook;
mod types;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use types::{DailyStep, TickerConfig, TickerState};

use std::sync::{Arc, Mutex};

// HashSet + TurnHook + daily_step_name re-exported for `super::*` glob in tests.rs.
#[allow(unused_imports)]
use std::collections::HashSet;
#[allow(unused_imports)]
use crate::modules::runtime::conversation::TurnHook;
#[allow(unused_imports)]
use daily::daily_step_name;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter, RecoveredSummary};
use crate::modules::memory::compiler::{CompilePaths, MemoryCompiler};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::summary::rolling::RollingSummarizer;
use crate::modules::memory::summary::store::SessionSummaryStore;
use crate::modules::memory::MemoryError;
use crate::modules::runtime::session::ConversationMessage;

use daily::{finish_in_progress, run_daily_inline, InProgressGuard};

// TickerConfig + DailyStep + TickerState moved to types (GFR-T1-F-1).

/// MemoryTicker — schedules per-turn / session-end / daily memory work.
///
/// Holds the rolling summarizer, compiler, and ticker state.  Real
/// `TurnHook` impl is in [`turn_hook.rs`]; the daily pipeline runner is
/// in [`daily.rs`].  This struct + its inherent impl (constructors,
/// schedulers, flush_session API) stay in mod.rs.
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

    /// Run the full daily compile cycle.
    ///
    /// Idempotent: each step is tracked in
    /// [`TickerState::daily_steps_completed`] so re-running this
    /// function on the same logical day skips already-done steps.
    /// Cross-step failure is non-fatal — the function logs via
    /// `tracing` + emits a `memory_job_failed` audit event then
    /// continues with the remaining steps.
    ///
    /// Step order (topological):
    /// `Today → Week → Longterm → Facts → Assemble → DeepMemory`.
    /// When `Week` fails, `Longterm` is auto-skipped (its input
    /// doesn't exist).  `DeepMemory` is a no-op until 8D Phase E
    /// lands the FactExtractor.
    ///
    /// Reentrancy guard: [`TickerState::daily_running`] is set on
    /// entry and released via `scopeguard` even if a step panics.
    /// Day-rollover: when the logical day differs from
    /// [`TickerState::daily_steps_date`], the completed-steps set is
    /// cleared so the new day re-runs every step.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryError::Generic`] only when the internal
    /// state mutex is poisoned.  Per-step failures are recorded via
    /// audit + tracing and never short-circuit the function.
    pub async fn do_daily(
        &self,
        scope: &MemoryExecutionScope,
        paths: &CompilePaths,
    ) -> Result<(), MemoryError> {
        run_daily_inline(&self.state, &self.compiler, scope, paths).await
    }

    /// Opportunistic daily kick-off: spawn `do_daily` in a
    /// background task if the logical day has rolled over since
    /// [`TickerState::last_daily_job_date`] and no daily run is
    /// currently in flight.  Cheap to call from `on_turn_complete`
    /// — most calls return immediately without spawning.
    ///
    /// Errors during the spawned daily run are logged via `tracing`
    /// and never propagate back to the caller.
    pub fn maybe_run_daily(&self, scope: &MemoryExecutionScope) {
        let today = crate::modules::runtime::logical_day::get_today().date;
        let should_run = match self.state.lock() {
            Ok(g) => g.last_daily_job_date != Some(today) && !g.daily_running,
            Err(e) => {
                tracing::error!(error = %e, "maybe_run_daily: mutex poisoned");
                return;
            }
        };
        if !should_run {
            return;
        }

        let Some(memory_root) = Self::memory_root() else {
            tracing::warn!("maybe_run_daily: data_local_dir unavailable; skipping daily kick");
            return;
        };
        let paths = CompilePaths::from_scope_root(&memory_root);
        let scope_owned = scope.clone();
        let state = Arc::clone(&self.state);
        let compiler = self.compiler.clone();

        tokio::spawn(async move {
            if let Err(e) = run_daily_inline(&state, &compiler, &scope_owned, &paths).await {
                tracing::error!(error = %e, "maybe_run_daily: do_daily failed");
            }
        });
    }

    /// Start the ticker — runs once on app boot.
    ///
    /// Two responsibilities:
    /// 1. `recover_unsummarized(scope).await` — catches up rolling
    ///    summaries for any session sidecar that's been written more
    ///    recently than its last persisted summary (the
    ///    "process killed mid-roll" failure mode).  The result is
    ///    surfaced via the `memory_ticker_recovery` audit event; no
    ///    synthetic re-roll happens here because the message
    ///    transcript is unavailable at boot — the next user turn
    ///    naturally drives the rolling pipeline (mirrors openhanako).
    /// 2. Spawns a backup `tokio::time::interval` loop that fires
    ///    `maybe_run_daily(scope)` every
    ///    [`TickerConfig::daily_check_interval_secs`].  The
    ///    per-turn opportunistic kick (8B.8) handles most cases; the
    ///    timer covers truly idle agents (user away for hours).
    ///
    /// Cheap to call (recover scan is bounded by sidecar file count).
    /// Re-spawning the timer is allowed in principle but guarded
    /// against in practice by `maybe_run_daily`'s `daily_running`
    /// short-circuit.
    ///
    /// Recover errors are demoted to `tracing::warn!` and never
    /// propagate — a failed scan must not block the app from booting.
    pub async fn start(self: &Arc<Self>, scope: MemoryExecutionScope) {
        match self.recover_unsummarized(&scope).await {
            Ok(recovered) => {
                tracing::info!(
                    count = recovered.len(),
                    "MemoryTicker.start: recover_unsummarized completed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "MemoryTicker.start: recover_unsummarized failed (non-fatal)"
                );
            }
        }

        let me = Arc::clone(self);
        let interval_secs = self.config.daily_check_interval_secs.max(1);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            // Skip the immediate first tick (interval fires at t=0 by
            // default) — we don't want to drive a daily run the
            // instant the app boots; let the on_turn_complete
            // opportunistic kick handle that case.
            tick.tick().await;
            loop {
                tick.tick().await;
                me.maybe_run_daily(&scope);
            }
        });
    }

    /// Catch up rolling summaries for sessions whose on-disk sidecar
    /// has been modified more recently than its last persisted
    /// summary.  Scans
    /// `<data_local>/.if2ai/memory/summaries/*.json` (the layout
    /// established by [`crate::modules::memory::summary::store::SqliteSessionSummaryStore`]
    /// in 8A.5).
    ///
    /// Filtering rules:
    /// - Only `*.json` files are considered.
    /// - Files whose `mtime` is older than 24h are skipped (avoids
    ///   re-recovering very stale sessions on a long-uptime machine).
    /// - For files within the 24h window, the sidecar is "dirty"
    ///   when `mtime > summary_at + 5s` (the 5s slop guards against
    ///   filesystem clock drift) or when no summary exists at all.
    ///
    /// Returns the list of recovered (dirty) sessions; the caller is
    /// also notified asynchronously via the
    /// `memory_ticker_recovery` audit event when the list is
    /// non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryError::Generic`] only when the OS does not
    /// expose a local data directory or the summaries directory
    /// `read_dir` itself fails.  Per-file errors (missing metadata,
    /// unreadable filename) are silently skipped — recover is a
    /// best-effort scan.
    pub async fn recover_unsummarized(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<RecoveredSummary>, MemoryError> {
        let memory_root = match Self::memory_root() {
            Some(d) => d,
            None => {
                return Err(MemoryError::Generic(
                    "data_local_dir unavailable; cannot scan summaries dir".into(),
                ));
            }
        };
        let summaries_dir = memory_root.join("summaries");
        if !summaries_dir.exists() {
            tracing::debug!(
                ?summaries_dir,
                "recover_unsummarized: summaries dir absent, nothing to recover"
            );
            return Ok(Vec::new());
        }

        let cutoff_at = chrono::Utc::now() - chrono::Duration::hours(24);
        let mut candidates: Vec<(String, chrono::DateTime<chrono::Utc>)> = Vec::new();
        let entries = std::fs::read_dir(&summaries_dir).map_err(|e| {
            MemoryError::Generic(format!(
                "recover_unsummarized: read_dir {summaries_dir:?} failed: {e}"
            ))
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let session_id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let metadata = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = match metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|d| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(
                        d.as_secs() as i64,
                        d.subsec_nanos(),
                    )
                }) {
                Some(t) => t,
                None => continue,
            };
            if mtime < cutoff_at {
                continue;
            }
            candidates.push((session_id, mtime));
        }

        let mut recovered: Vec<RecoveredSummary> = Vec::new();
        for (sid, mtime) in candidates {
            let existing = self.summary_store.get(&sid).await.ok().flatten();
            let summary_at = existing.as_ref().map(|r| r.updated_at);
            let stale = match summary_at {
                Some(ts) => mtime > ts + chrono::Duration::seconds(5),
                None => true,
            };
            if !stale {
                continue;
            }
            recovered.push(RecoveredSummary {
                session_id: sid,
                mtime,
                summary_at,
            });
        }

        if !recovered.is_empty() {
            let audit_ctx = AuditContext::from_scope(scope);
            MemoryAuditEmitter::memory_ticker_recovery(&audit_ctx, &recovered);
            tracing::info!(
                count = recovered.len(),
                "recover_unsummarized: surfaced dirty sessions"
            );
        }
        Ok(recovered)
    }
}
