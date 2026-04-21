//! Daily pipeline runner: idempotent step naming + the `run_daily_inline`
//! orchestration that drives compile_today/week/longterm/facts/assemble.
//!
//! Extracted from `ticker/mod.rs` in GFR-T1-F-1 (pure structural move;
//! function bodies byte-identical).

use std::sync::{Arc, Mutex};

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compiler::{CompilePaths, MemoryCompiler};
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::MemoryError;

use super::types::{DailyStep, TickerState};

pub(super) fn daily_step_name(step: DailyStep) -> &'static str {
    match step {
        DailyStep::Today => "compile_today",
        DailyStep::Week => "compile_week",
        DailyStep::Longterm => "compile_longterm",
        DailyStep::Facts => "compile_facts",
        DailyStep::Assemble => "assemble",
        DailyStep::DeepMemory => "deep_memory",
    }
}

/// Shared implementation of the daily compile cycle.
///
/// Lives outside [`MemoryTicker`] so it can be invoked both from
/// [`MemoryTicker::do_daily`] (synchronously, with `&self`) and from
/// [`MemoryTicker::maybe_run_daily`]'s `tokio::spawn` closure
/// (without lifetime juggling on the ticker reference).
///
/// Implements: reentrancy guard via `daily_running` + scopeguard,
/// day-rollover detection (clears `daily_steps_completed`),
/// topological step walk with per-step idempotence, and
/// `Longterm`-depends-on-`Week` short-circuit.
pub(super) async fn run_daily_inline(
    state: &Arc<Mutex<TickerState>>,
    compiler: &Arc<MemoryCompiler>,
    scope: &MemoryExecutionScope,
    paths: &CompilePaths,
) -> Result<(), MemoryError> {
    use scopeguard::guard;

    let today = crate::modules::runtime::logical_day::get_today().date;

    // 1. Reentrancy guard + day-rollover detection.
    {
        let mut g = state
            .lock()
            .map_err(|e| MemoryError::Generic(format!("ticker mutex poisoned: {e}")))?;
        if g.daily_running {
            tracing::debug!(date = %today, "do_daily: already running, skipping");
            return Ok(());
        }
        g.daily_running = true;
        if g.daily_steps_date != Some(today) {
            tracing::info!(
                date = %today,
                prev = ?g.daily_steps_date,
                "do_daily: new logical day, clearing completed steps"
            );
            g.daily_steps_completed.clear();
            g.daily_steps_date = Some(today);
        }
    }

    // 2. Scopeguard releases daily_running even on panic / early
    //    return.  Holds an Arc clone so the closure outlives the
    //    enclosing borrow.
    let release_state = Arc::clone(state);
    let _release_guard = guard((), move |()| {
        if let Ok(mut g) = release_state.lock() {
            g.daily_running = false;
        } else {
            tracing::error!("do_daily release: mutex poisoned, daily_running flag stuck");
        }
    });

    // 3. Walk the topological step list.
    let is_done = |s: DailyStep| -> bool {
        state
            .lock()
            .map(|g| g.daily_steps_completed.contains(&s))
            .unwrap_or(false)
    };
    let mark_done = |s: DailyStep| {
        if let Ok(mut g) = state.lock() {
            g.daily_steps_completed.insert(s);
        }
    };

    let max_retries = compiler.config().max_retries;
    let mut had_failure = false;
    for step in [
        DailyStep::Today,
        DailyStep::Week,
        DailyStep::Longterm,
        DailyStep::Facts,
        DailyStep::Assemble,
        DailyStep::DeepMemory,
    ] {
        if is_done(step) {
            continue;
        }
        // Longterm depends on Week — silently skip if Week not done.
        if step == DailyStep::Longterm && !is_done(DailyStep::Week) {
            tracing::debug!("do_daily: Longterm skipped (Week not yet completed)");
            continue;
        }

        let res: Result<(), MemoryError> = match step {
            DailyStep::Today => compiler.compile_today(scope, paths).await.map(drop),
            DailyStep::Week => compiler.compile_week(scope, paths).await.map(drop),
            DailyStep::Longterm => compiler.compile_longterm(scope, paths).await.map(drop),
            DailyStep::Facts => compiler.compile_facts(scope, paths).await.map(drop),
            DailyStep::Assemble => compiler.assemble(scope, paths),
            DailyStep::DeepMemory => {
                tracing::trace!("DeepMemory step is a no-op until 8D Phase E");
                Ok(())
            }
        };
        match res {
            Ok(()) => mark_done(step),
            Err(e) => {
                had_failure = true;
                tracing::error!(step = ?step, error = %e, "do_daily step failed");
                let audit_ctx = AuditContext::from_scope(scope);
                MemoryAuditEmitter::memory_job_failed(
                    &audit_ctx,
                    daily_step_name(step),
                    1, // attempt — JobRunner already tracks real per-job retry inside compile_*
                    max_retries,
                    &e.to_string(),
                );
            }
        }
    }

    // 4. Update last_daily_job_date only if every critical step
    //    succeeded this cycle.  Failures keep the previous date so
    //    the next opportunistic kick still tries to make progress.
    if !had_failure {
        if let Ok(mut g) = state.lock() {
            g.last_daily_job_date = Some(today);
        } else {
            tracing::error!("do_daily completion: mutex poisoned, last_daily_job_date not updated");
        }
    }
    Ok(())
}

/// Drop the `session_id` from `summary_in_progress`.  Used by the
/// background spawn closures (after the awaited work completes) and by
/// the [`InProgressGuard`] RAII helper that flush_session relies on.
pub(super) fn finish_in_progress(state: &Arc<Mutex<TickerState>>, session_id: &str) {
    if let Ok(mut g) = state.lock() {
        g.summary_in_progress.remove(session_id);
    }
}

/// RAII guard: ensures `session_id` is removed from `summary_in_progress`
/// even if the awaited future is cancelled / panics part-way through
/// `flush_session`.
pub(super) struct InProgressGuard<'a> {
    pub(super) state: &'a Arc<Mutex<TickerState>>,
    pub(super) session_id: &'a str,
}

impl<'a> Drop for InProgressGuard<'a> {
    fn drop(&mut self) {
        finish_in_progress(self.state, self.session_id);
    }
}
