//! `RunDelegate` — bridges the synchronous (non-streaming) agent turn body
//! into the unified [`run_agentic_loop`] from
//! [`crate::modules::application::turn_service::agentic_loop`].
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
//! This module imports from `application::turn_service` (the higher layer).
//! That inverse dependency is pragmatic for the bridge step: the alternative
//! is moving `agentic_loop` + `loop_config` down into `runtime`, which is
//! out of scope for T10.
//!
//! ## Layering note (Phase 2 closing)
//!
//! This file imports from `application::turn_service::agentic_loop`
//! (runtime → application), an inversion of the canonical CHARTER §2.1
//! direction. Acceptable as a transitional step; the clean fix is moving
//! `agentic_loop` + `loop_config` down into a shared `runtime::loop`
//! module (or extracting a thin adapter trait into `runtime` that
//! `application` plugs into). **FOLLOW-UP**: track in Phase 3 backlog.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::compact::estimate_session_tokens;
use super::conversation::{
    build_assistant_message, ApiClient, ApiRequest, ConversationRuntime, RunLoopState,
    RuntimeError, ToolExecutor,
};
use super::permissions::PermissionPrompter;
use super::session::ContentBlock;

use crate::modules::application::turn_service::agentic_loop::{
    LoopContext, LoopDelegate, LoopOutcome, LoopSignal, RespondResult, TextAction,
};

/// Adapter that exposes a [`ConversationRuntime`] turn through the
/// [`LoopDelegate`] interface. Lives only for the duration of one
/// `run_turn` invocation; reclaim ownership of the per-turn state via
/// [`Self::into_parts`] after [`crate::modules::application::turn_service::agentic_loop::run_agentic_loop`]
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

    async fn before_llm_call(&self, _ctx: &mut LoopContext, _iter: usize) -> Option<LoopOutcome> {
        // No preflight phase in the sync path (the streaming path uses this
        // for cost-guard / resilience pacing). Returning `None` lets the
        // loop proceed straight to `call_llm`.
        None
    }

    async fn call_llm(&self, _ctx: &mut LoopContext) -> Result<RespondResult, String> {
        let mut runtime_guard = self.runtime.lock().await;
        let runtime: &mut ConversationRuntime<C, T> = &mut runtime_guard;
        let mut state = self.state.lock().await;

        state.iterations += 1;

        // ContextBudget check — validates total token usage against
        // configured budget. System 10%, Episodic 20%, Semantic 30%, Working 40%.
        // Mirrors the pre-T10 inline check; preserved verbatim (incl. the
        // exact error text) so callers that match on the message keep working.
        if let Some(ref budget) = runtime.context_budget {
            let estimated_tokens = estimate_session_tokens(&runtime.session);
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
        // When a WorkingMemory is configured, populate it from the current
        // session so that only the most recent turns (within token budget)
        // are sent — full history stays in `runtime.session.messages`.
        let messages_for_request = if let Some(ref mut wm) = runtime.working_memory {
            wm.clear();
            wm.extend(runtime.session.messages.iter().cloned());
            wm.messages().to_vec()
        } else {
            runtime.session.messages.clone()
        };

        let request = ApiRequest {
            system_prompt: runtime.system_prompt.clone(),
            messages: messages_for_request,
            tools: Some(runtime.tool_executor.get_definitions()),
        };

        let events = match runtime.api_client.stream(request).await {
            Ok(events) => events,
            Err(err) => {
                let msg = err.to_string();
                *self.pending_error.lock().await = Some(err);
                return Err(msg);
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
            runtime.usage_tracker.record(usage);
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

        runtime.session.messages.push(assistant_message.clone());
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

    async fn after_iteration(&self, _ctx: &mut LoopContext, _iter: usize) {
        // No per-iteration hook today. `process_single_tool_call` already
        // records `record_tool_outcome`, merges hook feedback, and applies
        // the safety layer, so post-batch bookkeeping has nothing to add.
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
