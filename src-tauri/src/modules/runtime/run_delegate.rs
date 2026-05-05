//! `RunDelegate` — bridges the synchronous (non-streaming) agent turn body
//! into the unified [`run_agentic_loop`] from
//! [`crate::modules::runtime::agent_loop`].
//!
//! Phase 2 T10 (5c-6) — see `docs/superpowers/plans/2026-04-30-steward-alignment.md`.
//!
//! ## Design
//!
//! [`LoopDelegate`] requires `&self`-only methods, so [`RunDelegate`] wraps
//! the runtime and per-turn scratch state in `tokio::sync::Mutex` slots.
//! The delegate is the **sole caller** during `run_agentic_loop`, so locking
//! is contention-free; the mutexes exist purely for interior mutability.
//!
//! ## Outcome map (`call_llm`)
//!
//! Mirrors the pre-T10 inline loop body line-for-line:
//!
//! | Provider state                                    | Translation                              |
//! |---------------------------------------------------|------------------------------------------|
//! | `assistant_message` has tool-use blocks           | `RespondResult::ToolCalls`               |
//! | `assistant_message` has only text                 | `RespondResult::Text`                    |
//! | `api_client.stream` returns `Err`                 | stash `RuntimeError`, return `Err(msg)`  |
//! | `build_assistant_message` returns `Err`           | stash `RuntimeError`, return `Err(msg)`  |
//! | context-budget overflow                           | stash `RuntimeError`, return `Err(msg)`  |
//!
//! ## `LoopOutcome` translation in [`crate::modules::runtime::conversation::ConversationRuntime::run_turn`]
//!
//! - `Response(_)` → fall through to post-loop hook + `Ok(TurnSummary)`
//! - `MaxIterations` → `Err(RuntimeError::MaxIterationsExceeded)`
//! - `Failure(reason)` → prefer the stashed `RuntimeError` (preserves the
//!   structured variant — `SessionError`, `ApiError`, `ToolError`, …);
//!   fall back to `RuntimeError::ApiError(reason)` if no stash present.
//! - `Stopped` → `RuntimeError::ApiError("agentic loop stopped unexpectedly")`.
//!   `check_signals` always yields `Continue` today, so this is unreachable.
//!
//! ## Layering note
//!
//! Phase 3 T1 relocated `agentic_loop` + `loop_config` into
//! `runtime/agent_loop/`, eliminating the prior runtime → application
//! import inversion. This file now depends only on its own layer.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::compact::estimate_session_tokens;
use super::conversation::{
    build_assistant_message, ApiClient, ApiRequest, ConversationRuntime, RunLoopState,
    RuntimeError, ToolExecutor,
};
use super::permissions::PermissionPrompter;
use super::session::{ContentBlock, ConversationMessage};
use super::working_checkpoint::{extract_checkpoint, CheckpointInjectionConfig, WorkingCheckpoint};

use crate::modules::runtime::agent_loop::{
    ContextCompressionLevel, LoopContext, LoopDelegate, LoopOutcome, LoopSignal, RespondResult,
    TextAction,
};

/// Maximum number of automatic retries for transient / rate-limit errors
/// inside [`RunDelegate::call_llm`].
const RUN_DELEGATE_MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff on transient errors.
const RUN_DELEGATE_BASE_BACKOFF: Duration = Duration::from_millis(500);

/// Maximum backoff cap.
const RUN_DELEGATE_MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Detect whether a [`RuntimeError`] message indicates a transient / retryable
/// failure (network error, timeout, 5xx, etc.).
///
/// The `ApiClient::stream` method converts `ApiError` to
/// `RuntimeError::ApiError(e.to_string())`, so we match on well-known
/// patterns produced by [`ApiError::Display`].
fn is_retryable_runtime_error(err: &RuntimeError) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("timeout")
        || msg.contains("http error")
        || msg.contains("connect")
        || msg.contains("api returned 500")
        || msg.contains("api returned 502")
        || msg.contains("api returned 503")
        || msg.contains("api returned 504")
        || msg.contains("api returned 408")
        || msg.contains("io error")
}

/// Detect whether a [`RuntimeError`] message indicates a 429 rate-limit.
fn is_rate_limited_runtime_error(err: &RuntimeError) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("api returned 429")
        || msg.contains("rate limit")
        || msg.contains("rate_limit")
        || msg.contains("too many requests")
}

/// Compute exponential backoff with jitter, capped at [`RUN_DELEGATE_MAX_BACKOFF`].
fn retry_backoff(attempt: u32) -> Duration {
    let exp =
        RUN_DELEGATE_BASE_BACKOFF.saturating_mul(1u32.checked_shl(attempt).unwrap_or(u32::MAX));
    let jitter_ms = rand::random::<u64>() % 300;
    let with_jitter = exp.saturating_add(Duration::from_millis(jitter_ms));
    with_jitter.min(RUN_DELEGATE_MAX_BACKOFF)
}

/// Adapter that exposes a [`ConversationRuntime`] turn through the
/// [`LoopDelegate`] interface. Lives only for the duration of one
/// `run_turn` invocation; reclaim ownership of the per-turn state via
/// [`Self::into_parts`] after [`crate::modules::runtime::agent_loop::run_agentic_loop`]
/// returns.
pub(super) struct RunDelegate<'r, 'p, C, T>
where
    C: ApiClient + Send,
    T: ToolExecutor + Send,
{
    runtime: Mutex<&'r mut ConversationRuntime<C, T>>,
    state: Mutex<RunLoopState>,
    prompter: Mutex<Option<&'p mut (dyn PermissionPrompter + Send)>>,
    /// Tool uses produced by `call_llm`, drained by `execute_tool_calls`.
    /// `Vec` (not `HashMap`) so per-tool insertion order is preserved —
    /// `process_single_tool_call` records `record_tool_outcome` per call,
    /// and the existing tests assert ordered `tool_results` in the summary.
    pending_calls: Mutex<Vec<(String, String, String)>>,
    /// Stashed structured error from `call_llm`. `LoopOutcome::Failure`
    /// only carries a `String`, so we store the typed `RuntimeError` here
    /// to preserve the variant when the caller maps it back.
    pending_error: Arc<Mutex<Option<RuntimeError>>>,
    /// DK-002 working checkpoint: persists across iterations within a turn.
    /// Populated by `after_iteration` (extraction); consumed by `call_llm`
    /// (injection into the message stream before the LLM request).
    working_checkpoint: Mutex<Option<WorkingCheckpoint>>,
}

impl<'r, 'p, C, T> RunDelegate<'r, 'p, C, T>
where
    C: ApiClient + Send,
    T: ToolExecutor + Send,
{
    pub(super) fn new(
        runtime: &'r mut ConversationRuntime<C, T>,
        state: RunLoopState,
        prompter: Option<&'p mut (dyn PermissionPrompter + Send)>,
    ) -> Self {
        Self {
            runtime: Mutex::new(runtime),
            state: Mutex::new(state),
            prompter: Mutex::new(prompter),
            pending_calls: Mutex::new(Vec::new()),
            pending_error: Arc::new(Mutex::new(None)),
            working_checkpoint: Mutex::new(None),
        }
    }

    /// Reclaim the per-turn scratch state and any stashed error after the
    /// agentic loop returns. The `&mut ConversationRuntime` borrow held in
    /// `self.runtime` is released when `self` drops, freeing the original
    /// `&mut self` reference held by `run_turn` for the post-loop work
    /// (turn-hook firing, building the `TurnSummary`).
    pub(super) fn into_parts(self) -> (RunLoopState, Option<RuntimeError>) {
        let state = self.state.into_inner();
        let pending_error = Arc::try_unwrap(self.pending_error)
            .ok()
            .and_then(|m| m.into_inner());
        (state, pending_error)
    }
}

#[async_trait]
impl<'r, 'p, C, T> LoopDelegate for RunDelegate<'r, 'p, C, T>
where
    C: ApiClient + Send,
    T: ToolExecutor + Send,
{
    async fn check_signals(&self) -> LoopSignal {
        // Sync path has no cancellation channel today. Future work may
        // route a oneshot::Receiver through here; for now the loop runs
        // to natural completion / max-iterations.
        LoopSignal::Continue
    }

    async fn before_llm_call(&self, _ctx: &mut LoopContext, iter: usize) -> Option<LoopOutcome> {
        let runtime_guard = self.runtime.lock().await;
        let runtime: &ConversationRuntime<C, T> = &runtime_guard;

        // Pre-flight token estimation + logging (mirrors streaming path's
        // preflight diagnostics).  Gives operators visibility into context
        // growth before the LLM call.
        let estimated_tokens = estimate_session_tokens(&runtime.session);
        tracing::debug!(
            "[RunDelegate::before_llm_call] iteration={iter}, \
             estimated_session_tokens={estimated_tokens}, \
             message_count={}",
            runtime.session.messages.len(),
        );

        // Early context-budget guard — fail fast before building the full
        // request inside `call_llm`.  This duplicates the check in
        // `call_llm` intentionally so `before_llm_call` can surface a
        // terminal `LoopOutcome::Failure` (the loop won't proceed to
        // `call_llm` when we return `Some`).
        if let Some(ref budget) = runtime.context_budget {
            if estimated_tokens > budget.total {
                let reason = format!(
                    "context budget exceeded in preflight: \
                     estimated {estimated_tokens} tokens > budget {}",
                    budget.total,
                );
                tracing::warn!("[RunDelegate::before_llm_call] {reason}");
                // Don't return Failure here — let call_llm handle it with
                // the structured RuntimeError so the caller can inspect the
                // variant.  We only log the warning.
            }
        }

        None
    }

    async fn call_llm(&self, ctx: &mut LoopContext) -> Result<RespondResult, String> {
        let mut runtime_guard = self.runtime.lock().await;
        let mut state = self.state.lock().await;

        state.iterations += 1;

        // ContextBudget check — validates total token usage against
        // configured budget. System 10%, Episodic 20%, Semantic 30%, Working 40%.
        if let Some(ref budget) = runtime_guard.context_budget {
            let estimated_tokens = estimate_session_tokens(&runtime_guard.session);
            if estimated_tokens > budget.total {
                let err = RuntimeError::SessionError(format!(
                    "context budget exceeded: estimated {estimated_tokens} tokens exceeds total budget of {} (system={}, episodic={}, semantic={}, working={})",
                    budget.total,
                    budget.system_tokens(),
                    budget.episodic_tokens(),
                    budget.semantic_tokens(),
                    budget.working_tokens(),
                ));
                let msg = err.to_string();
                *self.pending_error.lock().await = Some(err);
                return Err(msg);
            }
        }

        // Build the message list for this LLM request.
        // When a WorkingMemory is configured, rebuild it from the current
        // session so that only the most recent turns (within token budget)
        // are sent — full history stays in `runtime_guard.session.messages`.
        // Uses `sync_working_memory` which also runs the volatile-content
        // audit (verification_gate). We clone the messages first to avoid
        // overlapping mutable/immutable borrows on `runtime_guard`.
        let messages_for_request = if runtime_guard.working_memory.is_some() {
            let msgs = runtime_guard.session.messages.clone();
            let wm = runtime_guard
                .working_memory
                .as_mut()
                .expect("working_memory checked above");
            ConversationRuntime::<C, T>::sync_working_memory(&msgs, wm);
            wm.messages().to_vec()
        } else {
            runtime_guard.session.messages.clone()
        };

        // DK-002: inject working checkpoint into the request messages.
        // This is ephemeral — the checkpoint message is only in the
        // messages sent to the LLM, not persisted in session state.
        // Note: messages_for_request is Vec<ConversationMessage>, so we
        // construct the checkpoint as a ConversationMessage directly.
        let mut messages_for_request = messages_for_request;
        {
            let cp_guard = self.working_checkpoint.lock().await;
            if let Some(ref checkpoint) = *cp_guard {
                if !checkpoint.key_info.trim().is_empty() {
                    let config = CheckpointInjectionConfig::default();
                    let mut payload = format!("[checkpoint] {}", checkpoint.key_info.trim());
                    if let Some(ref sop) = checkpoint.related_sop {
                        if !sop.is_empty() {
                            payload = format!("{payload}\n[related_sop] {sop}");
                        }
                    }
                    super::working_checkpoint::truncate_to_token_budget_pub(
                        &mut payload,
                        config.max_tokens,
                    );
                    let block = ConversationMessage {
                        role: super::session::MessageRole::User,
                        blocks: vec![ContentBlock::Text { text: payload }],
                        usage: None,
                        thinking: None,
                        task_outcome: None,
                        degraded_reason: None,
                        finish_reason: None,
                        request_id: None,
                        resume_available: None,
                        resume_cursor: None,
                    };
                    // Insert before the last user message (mirrors
                    // InjectionPosition::BeforeLastUserMessage).
                    let insert_at = messages_for_request
                        .iter()
                        .rposition(|m| matches!(m.role, super::session::MessageRole::User))
                        .unwrap_or(messages_for_request.len());
                    messages_for_request.insert(insert_at, block);
                }
            }
        }

        // Phase 3 T3 (N2-β): when the loop has detected `force_text_after_truncations`
        // consecutive `length` finishes, drop tool definitions so the provider
        // is forced to emit a text reply instead of yet another truncated tool call.
        let request = ApiRequest {
            system_prompt: runtime_guard.system_prompt.clone(),
            messages: messages_for_request,
            tools: if ctx.force_text {
                None
            } else {
                Some(runtime_guard.tool_executor.get_definitions())
            },
        };

        // --- LLM call with retry for transient / rate-limit errors ----------
        // Mirrors the streaming path’s `stream_message_with_resilience()`:
        // retries on network errors, timeouts, 5xx, and 429 with exponential
        // backoff + jitter.  Non-retryable errors (auth, config, etc.) are
        // surfaced immediately.
        let events = {
            let mut attempt: u32 = 0;
            loop {
                if attempt > 0 {
                    tracing::info!(
                        "[RunDelegate::call_llm] retry attempt {attempt}/{RUN_DELEGATE_MAX_RETRIES}"
                    );
                }
                match runtime_guard.api_client.stream(request.clone()).await {
                    Ok(events) => break events,
                    Err(err) => {
                        let retryable =
                            is_retryable_runtime_error(&err) || is_rate_limited_runtime_error(&err);
                        if !retryable || attempt >= RUN_DELEGATE_MAX_RETRIES {
                            let msg = err.to_string();
                            *self.pending_error.lock().await = Some(err);
                            return Err(msg);
                        }
                        let delay = if is_rate_limited_runtime_error(&err) {
                            // Longer backoff for rate-limit errors.
                            retry_backoff(attempt).max(Duration::from_secs(2))
                        } else {
                            retry_backoff(attempt)
                        };
                        tracing::warn!(
                            "[RunDelegate::call_llm] transient error (attempt {attempt}): \
                             {err}; retrying after {delay:?}"
                        );
                        // Release locks during sleep to avoid holding mutexes.
                        drop(state);
                        drop(runtime_guard);
                        tokio::time::sleep(delay).await;
                        // Re-acquire after sleep.
                        runtime_guard = self.runtime.lock().await;
                        state = self.state.lock().await;
                        attempt += 1;
                    }
                }
            }
        };
        let (assistant_message, usage) = match build_assistant_message(events) {
            Ok(parsed) => parsed,
            Err(err) => {
                let msg = err.to_string();
                *self.pending_error.lock().await = Some(err);
                return Err(msg);
            }
        };
        if let Some(usage) = usage {
            runtime_guard.usage_tracker.record(usage);
        }

        let pending_tool_uses: Vec<(String, String, String)> = assistant_message
            .blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.clone(), name.clone(), input.clone()))
                }
                _ => None,
            })
            .collect();

        let assistant_text = assistant_message
            .blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        let finish_reason = assistant_message.finish_reason.clone().unwrap_or_default();

        runtime_guard
            .session
            .messages
            .push(assistant_message.clone());
        state.assistant_messages.push(assistant_message);

        if pending_tool_uses.is_empty() {
            return Ok(RespondResult::Text(assistant_text));
        }

        let ids: Vec<String> = pending_tool_uses.iter().map(|tu| tu.0.clone()).collect();
        *self.pending_calls.lock().await = pending_tool_uses;

        Ok(RespondResult::ToolCalls {
            calls: ids,
            finish_reason,
        })
    }

    async fn execute_tool_calls(&self, ids: Vec<String>, _ctx: &mut LoopContext) -> Vec<String> {
        let pending = std::mem::take(&mut *self.pending_calls.lock().await);
        debug_assert_eq!(
            pending.iter().map(|tu| &tu.0).collect::<Vec<_>>(),
            ids.iter().collect::<Vec<_>>(),
            "loop ids must match the order populated by call_llm"
        );

        let mut runtime_guard = self.runtime.lock().await;
        let runtime: &mut ConversationRuntime<C, T> = &mut runtime_guard;
        let mut prompter_guard = self.prompter.lock().await;
        let mut state = self.state.lock().await;

        for (tool_use_id, tool_name, input) in pending {
            let result_message = runtime
                .process_single_tool_call(tool_use_id, tool_name, input, &mut prompter_guard)
                .await;
            runtime.session.messages.push(result_message.clone());
            state.tool_results.push(result_message);
        }

        ids
    }

    async fn handle_text_response(&self, text: String, _ctx: &mut LoopContext) -> TextAction {
        // Sync path has no retry-via-empty-text sentinel (the streaming path
        // uses one for `RetryAfterSleep`); every text response terminates
        // the loop with the assistant's reply.
        TextAction::Return(LoopOutcome::Response(text))
    }

    async fn after_iteration(&self, _ctx: &mut LoopContext, iter: usize) {
        // DK-002: extract checkpoint from the assistant's text output.
        // Scans for <key_info>...</key_info> and <task_complete/> markers.
        let assistant_text = {
            let state = self.state.lock().await;
            state
                .assistant_messages
                .last()
                .map(|msg| {
                    msg.blocks
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<String>()
                })
                .unwrap_or_default()
        };

        if assistant_text.is_empty() {
            return;
        }

        let extraction = extract_checkpoint(&assistant_text);

        if extraction.should_clear {
            *self.working_checkpoint.lock().await = None;
            tracing::debug!("[RunDelegate::after_iteration] checkpoint cleared (task_complete)");
        } else if let Some(ref key_info) = extraction.key_info {
            let mut cp_guard = self.working_checkpoint.lock().await;
            let turn = iter as u64;
            if let Some(ref mut existing) = *cp_guard {
                existing.key_info = key_info.clone();
                existing.related_sop = extraction.related_sop.clone();
                existing.turn_updated = turn;
            } else {
                *cp_guard = Some(WorkingCheckpoint {
                    session_id: String::from("run_delegate"),
                    key_info: key_info.clone(),
                    related_sop: extraction.related_sop.clone(),
                    turn_created: turn,
                    turn_updated: turn,
                });
            }
            tracing::debug!(
                "[RunDelegate::after_iteration] checkpoint updated at iteration {iter}"
            );
        }
    }

    async fn compress_context(&self, level: ContextCompressionLevel) -> bool {
        let mut runtime_guard = self.runtime.lock().await;
        let runtime: &mut ConversationRuntime<C, T> = &mut runtime_guard;

        match level {
            ContextCompressionLevel::StandardCompression => {
                // Ideally we would call compress_for_request() with the
                // tier-budget allocation (as StreamDelegate does), but
                // RunDelegate operates on Vec<ConversationMessage> whereas
                // compress_for_request expects &[InputMessage]. Without a
                // ConversationMessage → InputMessage conversion we fall
                // back to a gentler message-drop heuristic: drop the
                // oldest quarter (not half) to preserve more context while
                // still relieving pressure.
                //
                // TODO: once From<&ConversationMessage> for InputMessage is
                // implemented, route through compress_for_request here to
                // match StreamDelegate behaviour.
                let len = runtime.session.messages.len();
                let drop_count = len / 4;
                if drop_count > 0 {
                    runtime.session.messages.drain(..drop_count);
                    tracing::info!(
                        "[compress_context] StandardCompression: \
                         dropped {drop_count}/{len} oldest messages"
                    );
                    true
                } else {
                    false
                }
            }
            ContextCompressionLevel::ReduceWindowSize => {
                // Halve the WorkingMemory window turns.
                if let Some(ref mut wm) = runtime.working_memory {
                    let current = wm.max_turns;
                    let halved = current / 2;
                    if halved >= 2 {
                        wm.max_turns = halved;
                        tracing::info!(
                            "[compress_context] ReduceWindowSize: {current} -> {halved} turns"
                        );
                        return true;
                    }
                }
                // Fallback: drop the oldest quarter of session messages.
                let len = runtime.session.messages.len();
                let drop = len / 4;
                if drop > 0 {
                    runtime.session.messages.drain(..drop);
                    tracing::info!(
                        "[compress_context] ReduceWindowSize fallback: dropped {drop}/{len} messages"
                    );
                    true
                } else {
                    false
                }
            }
            ContextCompressionLevel::StripNonCriticalSystemPrompt => {
                // Truncate system prompt to the first 2000 characters,
                // keeping only the most critical identity / constitution.
                let sp = &runtime.system_prompt;
                let combined: String = sp.join("\n");
                if combined.len() > 2000 {
                    let mut end = 2000.min(combined.len());
                    while end > 0 && !combined.is_char_boundary(end) {
                        end -= 1;
                    }
                    let truncated = combined[..end].to_string();
                    runtime.system_prompt = vec![truncated];
                    tracing::info!(
                        "[compress_context] StripNonCriticalSystemPrompt: \
                         truncated system prompt from {} to 2000 chars",
                        combined.len()
                    );
                    true
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Compile-time witnesses. Runtime regression coverage lives in
    //! [`crate::modules::runtime::conversation::tests`] (9 unit tests) and
    //! the `turn_service_run_turn_e2e` integration suite — both exercise
    //! `ConversationRuntime::run_turn`, which now routes through this
    //! delegate, so any divergence from the pre-T10 inline loop fails
    //! there rather than here.

    use super::*;
    use crate::modules::runtime::conversation::{ConversationRuntime, StaticToolExecutor};
    use crate::modules::runtime::permissions::{PermissionMode, PermissionPolicy};
    use crate::modules::runtime::session::Session;

    #[allow(dead_code)]
    fn assert_send<T: Send>() {}
    #[allow(dead_code)]
    fn assert_sync<T: Sync>() {}

    #[test]
    fn delegate_is_send_and_sync() {
        // `LoopDelegate: Send + Sync` — without these bounds the
        // `&dyn LoopDelegate` coercion at the `run_agentic_loop` call site
        // would be rejected.
        type ScriptedRuntime = ConversationRuntime<DummyApi, StaticToolExecutor>;
        assert_send::<RunDelegate<'_, '_, DummyApi, StaticToolExecutor>>();
        assert_sync::<RunDelegate<'_, '_, DummyApi, StaticToolExecutor>>();
        // Compile-time witness only — never instantiated.
        let _: fn(&mut ScriptedRuntime) = |_| ();
    }

    struct DummyApi;
    #[async_trait]
    impl ApiClient for DummyApi {
        async fn stream(
            &mut self,
            _request: ApiRequest,
        ) -> Result<Vec<crate::modules::runtime::conversation::AssistantEvent>, RuntimeError>
        {
            Ok(Vec::new())
        }
    }

    #[test]
    fn into_parts_returns_state_and_pending_error() {
        let mut session = Session::new();
        session.messages.clear();
        let mut runtime = ConversationRuntime::new(
            session,
            DummyApi,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            vec!["s".to_string()],
        );
        let delegate = RunDelegate::new(&mut runtime, RunLoopState::default(), None);
        let (state, err) = delegate.into_parts();
        assert_eq!(state.iterations, 0);
        assert!(state.assistant_messages.is_empty());
        assert!(state.tool_results.is_empty());
        assert!(err.is_none());
    }
}
