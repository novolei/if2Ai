//! `TurnHook` trait implementation for `MemoryTicker`.
//!
//! Extracted from `ticker/mod.rs` in GFR-T1-F-1 (pure structural move;
//! function bodies byte-identical).

use std::sync::Arc;

use crate::modules::memory::compiler::CompilePaths;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::runtime::conversation::TurnHook;
use crate::modules::runtime::session::ConversationMessage;

use super::daily::finish_in_progress;
use super::MemoryTicker;

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
        // Phase 8B.11 fix-debug — info-level so it's visible without RUST_LOG.
        tracing::info!(
            session_id,
            project_id = scope.project_id.as_deref().unwrap_or("-"),
            messages = messages.len(),
            "[ticker] on_turn_complete invoked"
        );
        if session_id == "-" || messages.is_empty() {
            tracing::info!(
                session_id,
                msg_count = messages.len(),
                "[ticker] on_turn_complete SKIPPED (sentinel session_id or empty messages)"
            );
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
        tracing::info!(
            session_id,
            count,
            threshold,
            will_fire = (threshold > 0 && count > 0 && count % threshold == 0),
            "[ticker] turn_count snapshot"
        );
        if threshold > 0 && count > 0 && count % threshold == 0 {
            tracing::info!(
                session_id,
                count,
                "[ticker] threshold reached — spawning rolling_summary + compile_today"
            );
            self.spawn_rolling_then_compile_today(
                scope.clone(),
                session_id.to_string(),
                messages.to_vec(),
            );
        }

        // Phase 8B.8 — opportunistic daily kick.  Cheap when the
        // logical day hasn't rolled over since the last completed
        // daily run.
        self.maybe_run_daily(scope);

        // MEM-MOD-P5 — reflection pulse.  Fires only when (a) the
        // ticker was built with `with_reflection_runtime(...)`,
        // (b) `reflection_threshold` is non-zero, and (c) the current
        // turn count is a positive multiple of the threshold.  All
        // three conditions are off in tests / legacy configs so this
        // is a true zero-cost addition for those paths.
        let reflection_threshold = self.config.reflection_threshold;
        if reflection_threshold > 0 && count > 0 && count % reflection_threshold == 0 {
            if let Some((llm, memory)) = self.reflection_runtime() {
                let scope_owned = scope.clone();
                let session_owned = session_id.to_string();
                let messages_owned = messages.to_vec();
                tracing::info!(
                    session_id,
                    count,
                    reflection_threshold,
                    "[ticker] reflection pulse fired — spawning synthesis task"
                );
                tokio::spawn(async move {
                    match crate::modules::memory::reflection_loop::synthesize_and_persist(
                        llm.as_ref(),
                        &memory,
                        &scope_owned,
                        &session_owned,
                        &messages_owned,
                    )
                    .await
                    {
                        Ok(key) => tracing::info!(
                            session_id = %session_owned,
                            key = %key,
                            "[ticker] reflection persisted"
                        ),
                        Err(err) => tracing::warn!(
                            session_id = %session_owned,
                            error = %err,
                            "[ticker] reflection synthesis failed (non-fatal)"
                        ),
                    }
                });
            }
        }
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

        // MEM-MOD-P7 — distill this session's reflection memories into
        // durable cross-session traits.  Independent task so it never
        // blocks the rolling-summary / compile_today pipeline above.
        if let Some((llm, store, memory)) = self.learned_traits_runtime() {
            let scope_owned = scope.clone();
            let session_owned = session_id.to_string();
            tokio::spawn(async move {
                // Pull at most 20 reflections from this session.  We
                // ask for a generous limit (50) and filter client-side
                // because the recall API has no session predicate
                // beyond scope here; the LLM extractor caps the
                // output at 1-3 traits regardless of input size.
                let reflections: Vec<String> = match memory
                    .recall_scoped("", Some("reflection"), 50, &scope_owned)
                    .await
                {
                    Ok(entries) => entries
                        .into_iter()
                        .filter(|e| {
                            e.session_id
                                .as_deref()
                                .map(|s| s == session_owned)
                                .unwrap_or(false)
                        })
                        .take(20)
                        .map(|e| e.content)
                        .collect(),
                    Err(err) => {
                        tracing::warn!(
                            session_id = %session_owned,
                            error = %err,
                            "[ticker.p7] failed to recall reflections; skipping trait extraction"
                        );
                        return;
                    }
                };

                if reflections.is_empty() {
                    tracing::debug!(
                        session_id = %session_owned,
                        "[ticker.p7] no reflections in this session — nothing to distil"
                    );
                    return;
                }

                match crate::modules::memory::learned_traits::extract_and_persist(
                    llm.as_ref(),
                    &store,
                    &session_owned,
                    &reflections,
                )
                .await
                {
                    Ok(n) => tracing::info!(
                        session_id = %session_owned,
                        traits_persisted = n,
                        "[ticker.p7] learned_traits extracted"
                    ),
                    Err(err) => tracing::warn!(
                        session_id = %session_owned,
                        error = %err,
                        "[ticker.p7] trait extraction failed (non-fatal)"
                    ),
                }
            });
        }
    }
}
