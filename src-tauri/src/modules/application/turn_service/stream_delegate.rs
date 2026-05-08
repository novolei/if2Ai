//! StreamDelegate adapter for `run_agentic_loop`.
//!
//! Bridges the streaming agent loop body (factored into per-phase helpers in
//! `stream_iteration.rs`) into the `LoopDelegate` trait so the unified loop
//! algorithm can drive iteration. State lives in `tokio::sync::Mutex`-wrapped
//! fields because `LoopDelegate` methods take `&self`.
//!
//! ## T5 sub-commit lineage
//!
//! - **T5a**: struct + `new()` + stub trait methods that `unimplemented!()`.
//! - **T5b (this file)**: real trait method bodies routing through
//!   `iteration_preflight` / `iteration_run_stream` / `handle_no_tool_calls`
//!   / `iteration_execute_tools`. Production `run_stream_task_body` is still
//!   untouched — this code is not invoked at runtime yet.
//! - **T5c (final commit)**: replace the inline loop in `run_stream_task_body`
//!   with `run_agentic_loop(&delegate, &inputs.loop_config)` + LoopOutcome
//!   translation match + delegate destructure-back ceremony.
//!
//! ## Outcome map (`call_llm`)
//!
//! `iteration_run_stream` returns either `Completed` or `RetryAfterSleep`. On
//! `Completed`, behaviour branches by whether `pending_tool_uses` is empty
//! and whether `force_final_response` was set:
//!
//! | State                                                                 | Translation                                          |
//! |-----------------------------------------------------------------------|------------------------------------------------------|
//! | `pending_tool_uses` non-empty AND `force_final_response = false`      | `RespondResult::ToolCalls`                           |
//! | `pending_tool_uses` non-empty AND `force_final_response = true`       | terminal `max_iterations_reached`, return final text |
//! | empty → `handle_no_tool_calls` returns `Break`                        | `RespondResult::Text(accumulated)` → `Return`        |
//! | empty → `handle_no_tool_calls` returns `Continue`                     | `RespondResult::Text("")` (sentinel) → `Continue`    |
//! | empty → `handle_no_tool_calls` returns `FallThrough`                  | `RespondResult::ToolCalls` (recovered)               |
//! | `RetryAfterSleep`                                                     | sleep + sentinel `Text("")` → `Continue`             |

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{oneshot, Mutex};

use crate::modules::api::MessageStream;
use crate::modules::application::tool_executor::ToolRegistryExecutor;
use crate::modules::application::turn_service::finalize_hooks::extract_turn_checkpoint;
use crate::modules::application::turn_service::preflight_hooks::maybe_inject_checkpoint;
use crate::modules::control_plane::SessionExecutionContext;
use crate::modules::provider::resilience::LlmResilienceConfig;
use crate::modules::runtime::cost_guard::CostGuardConfig;
use crate::modules::runtime::permissions::{PermissionMode, PermissionPolicy};
use crate::modules::runtime::working_checkpoint::{CheckpointInjectionConfig, WorkingCheckpoint};

use super::agentic_loop::{
    ContextCompressionLevel, LoopContext, LoopDelegate, LoopOutcome, LoopSignal, RespondResult,
    TextAction,
};
use super::stream_iteration::{
    handle_no_tool_calls, iteration_execute_tools, iteration_preflight, iteration_run_stream,
    ExecuteToolsSharedRefs, NoToolOutcome, NoToolSharedRefs, PreflightOutcome, PreflightSharedRefs,
    RunStreamOutcome, RunStreamSharedRefs,
};
use super::stream_loop_state::StreamLoopState;
use super::stream_task::StreamTaskInputs;

/// Per-turn locals computed once at the top of `run_stream_task_body` that
/// are not on `StreamTaskInputs` directly. T5c will populate these at the
/// call site so `StreamDelegate` can build the `*SharedRefs` structs the
/// per-phase helpers consume.
pub(super) struct StreamDelegateExtras {
    pub(super) max_iterations: usize,
    pub(super) stream_resilience_cfg: LlmResilienceConfig,
    pub(super) cost_guard_cfg: CostGuardConfig,
    pub(super) mode: PermissionMode,
    pub(super) permission_policy: Arc<PermissionPolicy>,
    pub(super) execution_context_for_policy: SessionExecutionContext,
    pub(super) run_id_for_ledger: String,
    pub(super) app_data_dir_for_ledger: PathBuf,
}

pub(super) struct StreamDelegate<'a> {
    pub(super) inputs: &'a StreamTaskInputs,
    pub(super) extras: StreamDelegateExtras,
    pub(super) state: Mutex<StreamLoopState>,
    pub(super) cancel_rx: Mutex<Option<oneshot::Receiver<()>>>,
    pub(super) tool_executor_slot: Mutex<Option<ToolRegistryExecutor>>,

    /// DK-002 working checkpoint: persists across iterations within a turn.
    /// Populated by `after_iteration` (extraction); consumed by `before_llm_call`
    /// (injection into the message stream before the LLM request).
    pub(super) working_checkpoint: Mutex<Option<WorkingCheckpoint>>,

    /// Produced by `before_llm_call`, consumed by `call_llm`.
    pub(super) pending_stream: Mutex<Option<MessageStream>>,

    /// Produced by `before_llm_call` alongside `pending_stream`, consumed by
    /// `call_llm`. Carries the `force_final_response` decision from
    /// `iteration_preflight`'s `PreflightOutcome::Continue`.
    pub(super) pending_force_final: Mutex<Option<bool>>,

    /// Tool uses returned by `iteration_run_stream`, ordered as the provider
    /// emitted them (preserved so `tool_batch_signature` is stable). Drained
    /// by `execute_tool_calls` and handed to `iteration_execute_tools`.
    pub(super) pending_tool_uses_vec: Mutex<Vec<(String, String, String)>>,

    /// Lookup by call_id (= tool_use_id) for `execute_tool_calls`. Populated
    /// by `call_llm` in lockstep with `pending_tool_uses_vec`. Tuple is
    /// `(tool_use_id, tool_name, input_json)`.
    pub(super) pending_calls: Mutex<HashMap<String, (String, String, String)>>,

    /// Sentinel set by `call_llm` when it routes a `NoToolOutcome::Continue`
    /// (retry/nudge) or a `RunStreamOutcome::RetryAfterSleep` through
    /// `RespondResult::Text("")`. `handle_text_response` observes this and
    /// returns `TextAction::Continue` (after clearing the flag) instead of
    /// `TextAction::Return`.
    pub(super) next_text_continues: Mutex<bool>,
}

impl<'a> StreamDelegate<'a> {
    pub(super) fn new(
        inputs: &'a StreamTaskInputs,
        extras: StreamDelegateExtras,
        state: StreamLoopState,
        cancel_rx: oneshot::Receiver<()>,
        tool_executor: ToolRegistryExecutor,
    ) -> Self {
        Self {
            inputs,
            extras,
            state: Mutex::new(state),
            cancel_rx: Mutex::new(Some(cancel_rx)),
            tool_executor_slot: Mutex::new(Some(tool_executor)),
            pending_stream: Mutex::new(None),
            pending_force_final: Mutex::new(None),
            pending_tool_uses_vec: Mutex::new(Vec::new()),
            pending_calls: Mutex::new(HashMap::new()),
            next_text_continues: Mutex::new(false),
            working_checkpoint: Mutex::new(None),
        }
    }

    /// Reclaim owned values after `run_agentic_loop` returns. Used by T5c at
    /// the post-loop site so the orchestrator can keep using `state` /
    /// `tool_executor` for finalize.
    pub(super) fn into_parts(self) -> (StreamLoopState, ToolRegistryExecutor) {
        let state = self.state.into_inner();
        let executor = self
            .tool_executor_slot
            .into_inner()
            .expect("tool_executor must be restored by iteration_execute_tools");
        (state, executor)
    }
}

// ---- *SharedRefs builders -------------------------------------------------
//
// Each builder is a one-shot `fn` (not a method) so it borrows `inputs` /
// `extras` independently of `&self`. Per-method bodies hold these structs only
// for the duration of one helper await, then drop them.

fn build_preflight_refs<'r>(
    inputs: &'r StreamTaskInputs,
    extras: &'r StreamDelegateExtras,
    tool_executor: &'r ToolRegistryExecutor,
) -> PreflightSharedRefs<'r> {
    PreflightSharedRefs {
        stream_id: &inputs.stream_id_for_task,
        session_id: &inputs.session_id,
        max_iterations: extras.max_iterations,
        utility_llm: &inputs.utility_llm,
        tool_defs: &inputs.tool_defs_for_stream,
        system_prompt: &inputs.system_prompt_for_stream,
        model: &inputs.model_for_stream,
        context_window: inputs.context_window_for_stream,
        tool_pool_names: &inputs.tool_pool_names_for_stream,
        tool_pool_schema_hash: &inputs.tool_pool_schema_hash_for_stream,
        tool_pool_policy: &inputs.tool_pool_policy_for_stream,
        work_loop_decision: &inputs.work_loop_decision_for_stream,
        cost_guard_cfg: &extras.cost_guard_cfg,
        stream_resilience_cfg: &extras.stream_resilience_cfg,
        provider_client: &inputs.provider_client_for_stream,
        failover_provider_client: inputs.failover_provider_client.as_ref(),
        stream_emitter: &inputs.stream_emitter,
        run_event_logger: &inputs.run_event_logger,
        harness_bus: inputs.harness_event_bus_for_stream.as_ref(),
        tool_executor,
    }
}

fn build_run_stream_refs<'r>(
    inputs: &'r StreamTaskInputs,
    extras: &'r StreamDelegateExtras,
) -> RunStreamSharedRefs<'r> {
    RunStreamSharedRefs {
        stream_id: &inputs.stream_id_for_task,
        session_id: &inputs.session_id,
        provider_id: &inputs.provider_id_for_stream,
        model: &inputs.model_for_stream,
        stream_emitter: &inputs.stream_emitter,
        run_event_logger: &inputs.run_event_logger,
        app_session: &inputs.app_session_clone,
        session_manager: &inputs.session_manager,
        harness_bus: inputs.harness_event_bus_for_stream.as_ref(),
        execution_context: &extras.execution_context_for_policy,
    }
}

fn build_no_tool_refs<'r>(
    inputs: &'r StreamTaskInputs,
    force_final_response: bool,
) -> NoToolSharedRefs<'r> {
    NoToolSharedRefs {
        stream_id: &inputs.stream_id_for_task,
        session_id: &inputs.session_id,
        provider_id: &inputs.provider_id_for_stream,
        model: &inputs.model_for_stream,
        tool_defs: &inputs.tool_defs_for_stream,
        work_loop_decision: &inputs.work_loop_decision_for_stream,
        run_event_logger: &inputs.run_event_logger,
        force_final_response,
    }
}

fn build_execute_tools_refs<'r>(
    inputs: &'r StreamTaskInputs,
    extras: &'r StreamDelegateExtras,
) -> ExecuteToolsSharedRefs<'r> {
    ExecuteToolsSharedRefs {
        stream_id: &inputs.stream_id_for_task,
        session_id: &inputs.session_id,
        stream_emitter: &inputs.stream_emitter,
        run_event_logger: &inputs.run_event_logger,
        tool_registry: &inputs.tool_registry_clone,
        permission_senders: &inputs.permission_senders,
        permission_overrides: &inputs.permission_overrides,
        permission_policy: &extras.permission_policy,
        execution_context: &inputs.execution_context_for_task,
        work_loop_decision: &inputs.work_loop_decision_for_stream,
        harness_bus: inputs.harness_event_bus_for_stream.as_ref(),
        mode: extras.mode,
        run_id: &extras.run_id_for_ledger,
        app_data_dir: &extras.app_data_dir_for_ledger,
        trajectory_collector: &inputs.trajectory_collector,
    }
}

#[async_trait]
impl<'a> LoopDelegate for StreamDelegate<'a> {
    async fn check_signals(&self) -> LoopSignal {
        let mut cancel = self.cancel_rx.lock().await;
        let Some(rx) = cancel.as_mut() else {
            // Receiver currently held by `iteration_run_stream` (i.e. we're
            // mid-stream). That helper polls cancellation internally.
            return LoopSignal::Continue;
        };
        match rx.try_recv() {
            Ok(()) => {
                drop(cancel);
                let mut state = self.state.lock().await;
                if state.terminal_status.is_none() {
                    state.terminal_status = Some("cancelled_by_user");
                }
                // NOTE: Do NOT set completion_already_emitted here.
                // check_signals does not emit stream_complete itself,
                // so setting the flag would cause finalize to skip
                // emission — leaving the frontend stuck in "running"
                // state (race condition when cancel arrives as stream
                // is finishing). Let finalize_stream_task handle the
                // unified stream_complete emission.
                LoopSignal::Stop
            }
            Err(oneshot::error::TryRecvError::Closed) => {
                // Sender dropped without signalling — treat as no-cancel and
                // let the loop proceed; the helper's own poll will surface
                // any genuine error.
                LoopSignal::Continue
            }
            Err(oneshot::error::TryRecvError::Empty) => LoopSignal::Continue,
        }
    }

    async fn before_llm_call(&self, ctx: &mut LoopContext, _iter: usize) -> Option<LoopOutcome> {
        // DK-002: inject working checkpoint into session_messages before preflight
        // builds the LLM request. We track whether we injected so we can remove it
        // afterward (checkpoint is ephemeral per-request, not persisted in state).
        let checkpoint_injected = {
            let cp_guard = self.working_checkpoint.lock().await;
            if cp_guard.is_some() {
                let mut state = self.state.lock().await;
                let config = CheckpointInjectionConfig::default();
                let injected = maybe_inject_checkpoint(
                    cp_guard.as_ref(),
                    &mut state.session_messages,
                    &config,
                );
                injected
            } else {
                false
            }
        };

        // Take the executor out of the slot so we can borrow it for refs and
        // restore it when done. (`PreflightSharedRefs::tool_executor` is &-borrow
        // only, but the slot still needs to give up exclusive access for the
        // duration of the await.)
        let executor = {
            let mut slot = self.tool_executor_slot.lock().await;
            slot.take()
                .expect("tool_executor must be present at before_llm_call")
        };

        let outcome = {
            let mut state = self.state.lock().await;
            let mut cancel_guard = self.cancel_rx.lock().await;
            let cancel_rx = cancel_guard
                .as_mut()
                .expect("cancel_rx must be present at before_llm_call");
            let refs = build_preflight_refs(self.inputs, &self.extras, &executor);
            iteration_preflight(&mut state, cancel_rx, &refs, ctx.force_text).await
        };

        // Restore executor unconditionally.
        *self.tool_executor_slot.lock().await = Some(executor);

        // DK-002: remove the injected checkpoint message from session_messages
        // so it doesn't accumulate across iterations. The message was already
        // baked into the HTTP request body by iteration_preflight.
        if checkpoint_injected {
            let mut state = self.state.lock().await;
            // The checkpoint is injected before the last user message or at
            // position 0 (system). Find and remove it by content marker.
            if let Some(pos) = state.session_messages.iter().position(|m| {
                m.content.iter().any(|b| match b {
                    crate::modules::api::InputContentBlock::Text { text } => {
                        text.starts_with("[checkpoint] ")
                    }
                    _ => false,
                })
            }) {
                state.session_messages.remove(pos);
            }
        }

        match outcome {
            PreflightOutcome::Continue {
                stream,
                force_final_response,
            } => {
                *self.pending_stream.lock().await = Some(stream);
                *self.pending_force_final.lock().await = Some(force_final_response);
                None
            }
            PreflightOutcome::BreakTerminal => {
                let state = self.state.lock().await;
                let reason = state
                    .last_stream_error_reason
                    .clone()
                    .or_else(|| state.terminal_status.map(str::to_string))
                    .unwrap_or_else(|| "preflight_terminal".to_string());
                Some(LoopOutcome::Failure(reason))
            }
            PreflightOutcome::RetryAfterSleep { sleep } => {
                tokio::time::sleep(sleep).await;
                None
            }
        }
    }

    async fn call_llm(&self, _ctx: &mut LoopContext) -> Result<RespondResult, String> {
        let stream =
            self.pending_stream.lock().await.take().ok_or_else(|| {
                "call_llm: missing pending_stream from before_llm_call".to_string()
            })?;
        let force_final_response = self
            .pending_force_final
            .lock()
            .await
            .take()
            .unwrap_or(false);
        let cancel_rx = self
            .cancel_rx
            .lock()
            .await
            .take()
            .ok_or_else(|| "call_llm: missing cancel_rx".to_string())?;

        // Capture provider request_id before iteration_run_stream consumes the
        // stream — mirrors the inline production code (stream_task.rs L501-514).
        if let Some(request_id) = stream
            .request_id()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
        {
            self.state.lock().await.provider_request_id = request_id;
        }

        // Run the inner SSE event loop.
        let outcome = {
            let mut state = self.state.lock().await;
            let refs = build_run_stream_refs(self.inputs, &self.extras);
            iteration_run_stream(&mut state, stream, cancel_rx, &refs).await
        };

        match outcome {
            RunStreamOutcome::RetryAfterSleep {
                cancel_rx: returned,
                sleep,
            } => {
                *self.cancel_rx.lock().await = Some(returned);
                tokio::time::sleep(sleep).await;
                // Sentinel: tell handle_text_response to Continue, not Return.
                *self.next_text_continues.lock().await = true;
                Ok(RespondResult::Text(String::new()))
            }
            RunStreamOutcome::Completed {
                cancel_rx: returned,
                pending_tool_uses,
            } => {
                *self.cancel_rx.lock().await = Some(returned);

                if !pending_tool_uses.is_empty() {
                    if force_final_response {
                        // Provider emitted tool calls during finalization pass —
                        // mirror inline behaviour: stamp terminal status and
                        // return the accumulated text.
                        let mut state = self.state.lock().await;
                        state.terminal_status = Some("max_iterations_reached");
                        let text = state.accumulated_text.clone();
                        *self.next_text_continues.lock().await = false;
                        return Ok(RespondResult::Text(text));
                    }
                    return Ok(self.stash_pending_tool_calls(pending_tool_uses).await);
                }

                // Empty path — dispatch through handle_no_tool_calls.
                let mut pending = pending_tool_uses;
                let no_tool_outcome = {
                    let mut state = self.state.lock().await;
                    let refs = build_no_tool_refs(self.inputs, force_final_response);
                    handle_no_tool_calls(&mut state, &mut pending, &refs).await
                };
                match no_tool_outcome {
                    NoToolOutcome::Break => {
                        *self.next_text_continues.lock().await = false;
                        let text = self.state.lock().await.accumulated_text.clone();
                        Ok(RespondResult::Text(text))
                    }
                    NoToolOutcome::Continue => {
                        *self.next_text_continues.lock().await = true;
                        Ok(RespondResult::Text(String::new()))
                    }
                    NoToolOutcome::FallThrough => Ok(self.stash_pending_tool_calls(pending).await),
                }
            }
        }
    }

    async fn execute_tool_calls(&self, _ids: Vec<String>, _ctx: &mut LoopContext) -> Vec<String> {
        // Drain the cached tool uses (preserves provider-emitted ordering, which
        // tool_batch_signature relies on). The loop-level `_ids` are derived
        // from this same Vec so we don't re-translate.
        let pending = std::mem::take(&mut *self.pending_tool_uses_vec.lock().await);
        let executed_ids: Vec<String> = pending.iter().map(|tu| tu.0.clone()).collect();

        let executor = self
            .tool_executor_slot
            .lock()
            .await
            .take()
            .expect("tool_executor must be present at execute_tool_calls");

        let returned = {
            let mut state = self.state.lock().await;
            let refs = build_execute_tools_refs(self.inputs, &self.extras);
            iteration_execute_tools(&mut state, pending, executor, &refs).await
        };

        *self.tool_executor_slot.lock().await = Some(returned);
        self.pending_calls.lock().await.clear();
        executed_ids
    }

    async fn handle_text_response(&self, text: String, _ctx: &mut LoopContext) -> TextAction {
        let should_continue = {
            let mut next = self.next_text_continues.lock().await;
            let prev = *next;
            *next = false;
            prev
        };
        if should_continue {
            return TextAction::Continue;
        }
        // Note: iteration_run_stream has already populated state.accumulated_text,
        // so we don't need to overwrite it here. We pass `text` straight through
        // as the LoopOutcome::Response payload.
        TextAction::Return(LoopOutcome::Response(text))
    }

    async fn after_iteration(&self, _ctx: &mut LoopContext, iter: usize) {
        // DK-002: extract checkpoint from the assistant's accumulated text.
        // This is a cheap synchronous tag scan that runs after every iteration.
        let state = self.state.lock().await;
        let accumulated = state.accumulated_text.clone();
        drop(state);

        let extraction = extract_turn_checkpoint(&accumulated);

        if extraction.should_clear {
            // Agent signalled <task_complete/> — drop the checkpoint.
            *self.working_checkpoint.lock().await = None;
            tracing::debug!(
                "[StreamDelegate::after_iteration] checkpoint cleared (task_complete)"
            );
        } else if let Some(ref key_info) = extraction.key_info {
            let mut cp_guard = self.working_checkpoint.lock().await;
            let turn = iter as u64;
            if let Some(ref mut existing) = *cp_guard {
                existing.key_info = key_info.clone();
                existing.related_sop = extraction.related_sop.clone();
                existing.turn_updated = turn;
            } else {
                *cp_guard = Some(WorkingCheckpoint {
                    session_id: self.inputs.session_id.clone(),
                    key_info: key_info.clone(),
                    related_sop: extraction.related_sop.clone(),
                    turn_created: turn,
                    turn_updated: turn,
                });
            }
            tracing::debug!(
                "[StreamDelegate::after_iteration] checkpoint updated at iteration {iter}"
            );
        }
    }

    async fn compress_context(&self, level: ContextCompressionLevel) -> bool {
        let mut state = self.state.lock().await;

        match level {
            ContextCompressionLevel::StandardCompression => {
                // Apply tier-budget compression via compress_for_request on
                // the streaming session messages.
                use crate::modules::runtime::context_compression::{
                    compress_for_request, TierBudgetAllocation,
                };
                let allocation = TierBudgetAllocation::default();
                let outcome = compress_for_request(&state.session_messages, &allocation);
                if outcome.passthrough {
                    return false;
                }
                let dropped = outcome.dropped;
                state.session_messages = outcome.kept;
                tracing::info!(
                    "[compress_context] StandardCompression: dropped {dropped} oldest messages"
                );
                true
            }
            ContextCompressionLevel::ReduceWindowSize => {
                // Drop the oldest quarter of session messages.
                let len = state.session_messages.len();
                let drop = len / 4;
                if drop > 0 {
                    state.session_messages.drain(..drop);
                    tracing::info!(
                        "[compress_context] ReduceWindowSize: dropped {drop}/{len} messages"
                    );
                    true
                } else {
                    false
                }
            }
            ContextCompressionLevel::StripNonCriticalSystemPrompt => {
                // Cannot mutate system_prompt_for_stream (immutable on inputs).
                // Instead, drop the oldest half of session messages as a
                // last-resort aggressive compression.
                let len = state.session_messages.len();
                let drop = len / 2;
                if drop > 0 {
                    state.session_messages.drain(..drop);
                    tracing::info!(
                        "[compress_context] StripNonCriticalSystemPrompt: \
                         aggressively dropped {drop}/{len} messages"
                    );
                    true
                } else {
                    false
                }
            }
        }
    }
}

impl<'a> StreamDelegate<'a> {
    /// Populate `pending_tool_uses_vec` + `pending_calls` from the helper's
    /// returned tool uses and emit a `RespondResult::ToolCalls`. Called from
    /// the two `call_llm` branches (non-empty + recovered FallThrough) that
    /// dispatch tool execution.
    async fn stash_pending_tool_calls(
        &self,
        pending: Vec<(String, String, String)>,
    ) -> RespondResult {
        let ids: Vec<String> = pending.iter().map(|tu| tu.0.clone()).collect();

        debug_assert!(
            !ids.is_empty(),
            "StreamDelegate::call_llm returned empty ToolCalls — would double-fire \
             the loop-level tool_intent_nudge"
        );

        {
            let mut map = self.pending_calls.lock().await;
            map.clear();
            for tu in &pending {
                map.insert(tu.0.clone(), tu.clone());
            }
        }
        *self.pending_tool_uses_vec.lock().await = pending;

        let finish_reason = self
            .state
            .lock()
            .await
            .last_finish_reason
            .clone()
            .unwrap_or_default();

        RespondResult::ToolCalls {
            calls: ids,
            finish_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time witness: the constructor signature matches what T5c will
    /// need at the call site, and the trait impl is wired into the unified
    /// loop. Runtime invocation is intentionally avoided (StreamTaskInputs is
    /// heavy to fake); the e2e tests in `turn_service_stream_turn_e2e` and
    /// `turn_service_run_turn_e2e` are the canonical regression guard once
    /// T5c wires this delegate into production.
    #[test]
    fn struct_signatures_compile() {
        fn _witness_construct<'a>(
            inputs: &'a StreamTaskInputs,
            extras: StreamDelegateExtras,
            state: StreamLoopState,
            cancel_rx: oneshot::Receiver<()>,
            tool_executor: ToolRegistryExecutor,
        ) -> StreamDelegate<'a> {
            StreamDelegate::new(inputs, extras, state, cancel_rx, tool_executor)
        }

        fn _witness_into_parts(d: StreamDelegate<'_>) -> (StreamLoopState, ToolRegistryExecutor) {
            d.into_parts()
        }

        fn _witness_trait_object<'a>(d: &'a StreamDelegate<'a>) -> &'a dyn LoopDelegate {
            d
        }
    }
}
