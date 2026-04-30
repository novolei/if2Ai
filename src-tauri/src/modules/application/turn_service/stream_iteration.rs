//! S5b-2 (steward-align Phase 2 T3) — per-iteration phase helpers
//! extracted from `stream_task::run_stream_task_body`.
//!
//! This module is a *pure structural* split of the long iteration
//! body — both helpers move existing code verbatim, returning an
//! outcome enum so the caller can `break` / `continue` / fall
//! through. There is no behaviour change vs the prior inline body;
//! the e2e tests `turn_service_stream_turn_e2e` and
//! `turn_service_run_turn_e2e` are the canonical regression guards.
//!
//! See the parent `stream_task` module for the surrounding context
//! (state bag, `StreamTaskInputs`, finalize phase).
//!
//! Helpers:
//!  * [`iteration_preflight`] — phases 1-8 of the loop body
//!    (cancellation poll, iteration cap, per-iter setup, DW-002
//!    digester, preflight request build + diagnostics emit, cost
//!    guard, stream-start with resilience).
//!  * [`handle_no_tool_calls`] — phase 11 (textual tool-call
//!    extraction, tool-required / tool-intent retries, terminal
//!    status derivation when the assistant produced no tool calls).
//!  * [`iteration_run_stream`] — phase 10 (inner SSE event loop:
//!    `run_stream_event_loop` invocation + state restore + outer
//!    timeout retry signalling).
//!  * [`iteration_execute_tools`] — phases 13-14 (repeated tool-batch
//!    detection + `execute_tool_batch` + post-batch finalization
//!    guards + post-mutation update message injection).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::modules::api::{InputMessage, MessageStream, ToolDefinition};
use crate::modules::application::tool_executor::ToolRegistryExecutor;
use crate::modules::control_plane::SessionExecutionContext;
use crate::modules::harness::{AgentEvent, EventBus};
use crate::modules::provider::resilience::LlmResilienceConfig;
use crate::modules::runtime::budget::MAX_STREAM_RETRY_ON_TIMEOUT;
use crate::modules::runtime::contracts::agent_loop::WorkLoopDecision;
use crate::modules::runtime::cost_guard::{CostGuard, CostGuardConfig};
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::permissions::{
    PermissionMode, PermissionPolicy, PermissionPromptDecision,
};
use crate::modules::runtime::resume_cursor::build_resume_cursor;
use crate::modules::runtime::stream_emitter::{AgentStreamEmitter, StreamTokenPayload};
use crate::modules::runtime::stream_error_reason::{
    format_stream_error_reason, is_network_timeout_reason,
};
use crate::modules::runtime::stream_outcome::{
    ConversationTruth, ExecutionTruth, TaskOutcomeResolver,
};
use crate::modules::session::Session as AppSession;
use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;

use super::stream_loop_state::StreamLoopState;
use super::stream_task::{
    append_stream_event, apply_memory_recall_success_finalization_guard,
    should_retry_announced_tool_intent_no_tool, should_retry_tool_required_no_tool,
    tool_batch_signature, REPEATED_TOOL_BATCH_LIMIT,
};

/// Result of one iteration's preflight phase (steps 1-8 of the
/// outer loop body).
//
// The `Continue` variant carries a `MessageStream` (~345B) which
// dwarfs the other variants — but this enum is constructed once
// per iteration, immediately matched, and never stored, so boxing
// would just add an allocation without benefit. The previous
// inline code held the same value as a local `let`, so this is
// behaviour-neutral. (steward-align Phase 2 T3)
#[allow(clippy::large_enum_variant)]
pub(super) enum PreflightOutcome {
    /// Successful stream-start. Caller must capture the resulting
    /// [`MessageStream`] (and the per-call `force_final_response`
    /// flag derived during phase 3) and continue with the inner SSE
    /// event loop.
    Continue {
        stream: MessageStream,
        force_final_response: bool,
    },
    /// Terminal exit. `state.terminal_status` /
    /// `state.last_stream_error_reason` /
    /// `state.completion_already_emitted` already populated by the
    /// helper. Caller must `break` the outer loop.
    BreakTerminal,
    /// Stream-start timeout retry. Caller must sleep and `continue`
    /// without consuming a real iteration slot — the iteration
    /// counter has not been bumped yet for this retry attempt
    /// (phase 3 increments it before phase 8).
    RetryAfterSleep { sleep: Duration },
}

/// Result of handling the case where the LLM returned no tool calls
/// (phase 11 of the outer loop body).
pub(super) enum NoToolOutcome {
    /// Outer loop should `break` — `state.terminal_status` already
    /// populated by the helper.
    Break,
    /// Outer loop should `continue` — the helper bumped a retry
    /// counter, mutated `state.session_messages`, and set
    /// `state.force_tool_choice_next` so the next iteration applies
    /// the appropriate nudge.
    Continue,
    /// Textual tool-call extraction retroactively populated
    /// `pending_tool_uses`; caller should fall through to the
    /// remainder of the iteration body (phases 12-14) so the
    /// recovered tool calls run.
    FallThrough,
}

/// Bundled shared borrows consumed by [`iteration_preflight`].
///
/// Grouped into a struct because the helper would otherwise need
/// ~20 positional parameters (per the steward-align Phase 2 T3
/// guidance: prefer a context struct when a helper signature would
/// exceed 8 params).
pub(super) struct PreflightSharedRefs<'a> {
    pub stream_id: &'a str,
    pub session_id: &'a str,
    pub max_iterations: usize,
    pub utility_llm: &'a Arc<dyn crate::modules::memory::UtilityLlm>,
    pub tool_defs: &'a [ToolDefinition],
    pub system_prompt: &'a str,
    pub model: &'a str,
    pub context_window: u64,
    pub tool_pool_names: &'a [String],
    pub tool_pool_schema_hash: &'a str,
    pub tool_pool_policy: &'a str,
    pub work_loop_decision: &'a WorkLoopDecision,
    pub cost_guard_cfg: &'a CostGuardConfig,
    pub stream_resilience_cfg: &'a LlmResilienceConfig,
    pub provider_client: &'a crate::modules::api::ProviderClient,
    pub failover_provider_client: Option<&'a crate::modules::api::ProviderClient>,
    pub stream_emitter: &'a AgentStreamEmitter,
    pub run_event_logger: &'a RunEventLogger,
    pub harness_bus: Option<&'a EventBus>,
    pub tool_executor: &'a ToolRegistryExecutor,
}

/// Bundled shared borrows consumed by [`handle_no_tool_calls`].
pub(super) struct NoToolSharedRefs<'a> {
    pub stream_id: &'a str,
    pub session_id: &'a str,
    pub provider_id: &'a str,
    pub model: &'a str,
    pub tool_defs: &'a [ToolDefinition],
    pub work_loop_decision: &'a WorkLoopDecision,
    pub run_event_logger: &'a RunEventLogger,
    pub force_final_response: bool,
}

/// Run phases 1-8 of the outer loop body.
///
/// Verbatim move of the prior inline code — see the per-phase
/// rationale at the original call site in `run_stream_task_body`.
pub(super) async fn iteration_preflight(
    state: &mut StreamLoopState,
    cancel_rx: &mut tokio::sync::oneshot::Receiver<()>,
    refs: &PreflightSharedRefs<'_>,
) -> PreflightOutcome {
    // Phase 1 — cancellation poll.
    if cancel_rx.try_recv().is_ok() {
        tracing::info!("[start_agent_stream] Stream cancelled at loop iteration");
        let cancelled_truth = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool: state.has_successful_tool,
                has_successful_mutating_tool: state.has_successful_mutating_tool,
            },
            &ConversationTruth {
                stream_failed: true,
                terminal_status: "cancelled_by_user",
                last_stream_error_reason: Some("cancelled_by_user".to_string()),
            },
        );
        let payload = StreamTokenPayload {
            stream_id: refs.stream_id.to_string(),
            correlation: None,
            text: None,
            thinking: None,
            event_type: "stream_complete".to_string(),
            tool_call_id: None,
            tool_name: None,
            tool_status: None,
            tool_args: None,
            tool_result: None,
            tool_duration_ms: None,
            effective_workdir: None,
            policy_decision: None,
            evidence_id: None,
            request_id: Some(state.provider_request_id.clone()),
            task_outcome: Some(cancelled_truth.task_outcome.to_string()),
            degraded_reason: cancelled_truth.degraded_reason,
            resume_available: Some(cancelled_truth.resume_available),
            resume_cursor: None,
            recoverability: None,
            context_budget_usage: None,
            memory_context: None,
            prompt_diagnostics: None,
            turn_cost: None,
            routing_info: None,
            session_totals: None,
        };
        refs.stream_emitter.emit_payload(payload.clone());
        append_stream_event(refs.run_event_logger, &payload).await;
        state.completion_already_emitted = true;
        state.terminal_status = Some("cancelled_by_user");
        return PreflightOutcome::BreakTerminal;
    }

    // Phase 2 — iteration cap.
    if state.tool_loop_iter >= refs.max_iterations {
        tracing::warn!(
            "[start_agent_stream] Tool loop exceeded max_iterations={}",
            refs.max_iterations
        );
        state.terminal_status = Some("max_iterations_reached");
        return PreflightOutcome::BreakTerminal;
    }

    // Phase 3 — per-iteration setup.
    state.tool_loop_iter += 1;
    refs.tool_executor
        .execution_context
        .tool_success_evidence
        .store(0, std::sync::atomic::Ordering::Relaxed);
    let force_final_response =
        state.force_final_response_next || state.tool_loop_iter >= refs.max_iterations;
    let force_tool_choice = state.force_tool_choice_next
        || (super::work_loop::requires_tool_execution_evidence(refs.work_loop_decision)
            && !state.has_successful_mutating_tool);
    state.force_tool_choice_next = false;
    let current_finalization_reason = if force_final_response {
        state
            .finalization_reason
            .clone()
            .unwrap_or_else(|| "max_iterations_finalization_pass".to_string())
    } else {
        String::new()
    };

    tracing::info!(
        "[start_agent_stream] === Outer loop iteration {} start. session_messages len={}, accumulated_text len={}, force_final_response={}, finalization_reason={}",
        state.tool_loop_iter,
        state.session_messages.len(),
        state.accumulated_text.len(),
        force_final_response,
        current_finalization_reason
    );

    // Phase 4 — DW-002 digester preflight (failure-isolated).
    let digested_owned: Option<Vec<crate::modules::api::InputMessage>> =
        super::preflight_hooks::digest_messages_for_preflight(
            &state.session_messages,
            refs.utility_llm.clone(),
        )
        .await;
    if let Some(ref kept) = digested_owned {
        let payload = serde_json::json!({
            "keptTokens": kept.len(),
            "dropped": state.session_messages.len().saturating_sub(kept.len()),
            "passthrough": kept.len() == state.session_messages.len(),
            "tier": "recent_messages",
        });
        let _ = crate::modules::runtime::evolution_emitter::emit_evolution_event(
            None,
            crate::modules::runtime::contracts::common::RuntimeEventType::CompressionEvent,
            "message_digest",
            crate::modules::runtime::contracts::common::CorrelationIds {
                session_id: Some(refs.session_id.to_string()),
                stream_id: Some(refs.stream_id.to_string()),
                ..Default::default()
            },
            &payload,
            None,
        );
    }

    // Phase 5 — preflight build_iteration_request + sanitize bookkeeping.
    let preflight_result = super::stream_preflight::build_iteration_request(
        super::stream_preflight::PreflightContext {
            session_messages: &state.session_messages,
            digested_messages: digested_owned.as_deref(),
            tool_defs: refs.tool_defs,
            system_prompt: refs.system_prompt,
            model: refs.model,
            context_window: refs.context_window,
            force_final_response,
            finalization_reason: &current_finalization_reason,
            force_tool_choice,
            tool_loop_iter: state.tool_loop_iter,
            max_iterations: refs.max_iterations,
            stream_id: refs.stream_id,
            session_id: refs.session_id,
        },
    );
    let iter_api_request = preflight_result.request;
    let request_tool_count = iter_api_request.tools.as_ref().map_or(0, Vec::len);
    let request_tool_names = iter_api_request
        .tools
        .as_ref()
        .map(|tools| {
            tools
                .iter()
                .take(48)
                .map(|tool| tool.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let request_tool_choice = iter_api_request
        .tool_choice
        .as_ref()
        .map(|choice| format!("{choice:?}"))
        .unwrap_or_else(|| "none".to_string());

    // Phase 6 — provider_tool_call_diagnostics emit.
    let _ = refs
        .run_event_logger
        .append(
            "provider_tool_call_diagnostics",
            serde_json::json!({
                "stream_id": refs.stream_id,
                "session_id": refs.session_id,
                "iteration": state.tool_loop_iter,
                "tool_count": request_tool_count,
                "tool_names": request_tool_names,
                "canonical_tool_names": refs.tool_pool_names.to_vec(),
                "tool_schema_hash": refs.tool_pool_schema_hash,
                "tool_pool_policy": refs.tool_pool_policy,
                "tool_choice": request_tool_choice,
                "force_final_response": force_final_response,
                "force_tool_choice": force_tool_choice,
                "requires_tool_execution_evidence": super::work_loop::requires_tool_execution_evidence(refs.work_loop_decision),
                "has_successful_tool": state.has_successful_tool,
                "has_successful_mutating_tool": state.has_successful_mutating_tool,
                "reason_codes": refs.work_loop_decision.reason_codes.clone(),
            }),
        )
        .await;
    state.session_messages = preflight_result.session_messages;
    state.preflight_trim_rounds += preflight_result.preflight_trim_rounds_added;
    state.preflight_dropped_messages_total += preflight_result.preflight_dropped_messages_added;
    state.preflight_trimmed_chars_total += preflight_result.preflight_trimmed_chars_added;
    state.sanitize_rounds += preflight_result.sanitize_rounds_added;
    state.sanitized_dropped_empty_messages +=
        preflight_result.sanitized_dropped_empty_messages_added;
    state.sanitized_dropped_orphan_tool_results +=
        preflight_result.sanitized_dropped_orphan_tool_results_added;
    state.sanitized_dropped_unmatched_tool_uses +=
        preflight_result.sanitized_dropped_unmatched_tool_uses_added;
    state.sanitized_dropped_invalid_tool_use_inputs +=
        preflight_result.sanitized_dropped_invalid_tool_use_inputs_added;
    state
        .sanitize_orphan_samples
        .extend(preflight_result.sanitize_orphan_samples_added);
    state
        .sanitize_unmatched_samples
        .extend(preflight_result.sanitize_unmatched_samples_added);
    state
        .sanitize_invalid_tool_use_samples
        .extend(preflight_result.sanitize_invalid_tool_use_samples_added);

    // Phase 7 — cost guard.
    if let Err(ce) = CostGuard::check_before_llm_call(refs.cost_guard_cfg, refs.session_id) {
        state.stream_failed = true;
        state.last_stream_error_reason = Some(ce.to_string());
        state.terminal_status = Some("failed_to_start_stream");
        let user_visible_truth = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool: state.has_successful_tool,
                has_successful_mutating_tool: state.has_successful_mutating_tool,
            },
            &ConversationTruth {
                stream_failed: true,
                terminal_status: "failed_to_start_stream",
                last_stream_error_reason: state.last_stream_error_reason.clone(),
            },
        );
        let payload = StreamTokenPayload {
            stream_id: refs.stream_id.to_string(),
            correlation: None,
            text: None,
            thinking: None,
            event_type: "stream_error".to_string(),
            tool_call_id: None,
            tool_name: None,
            tool_status: None,
            tool_args: None,
            tool_result: Some(ce.to_string()),
            tool_duration_ms: None,
            effective_workdir: None,
            policy_decision: None,
            evidence_id: None,
            request_id: Some(state.provider_request_id.clone()),
            task_outcome: Some(user_visible_truth.task_outcome.to_string()),
            degraded_reason: user_visible_truth.degraded_reason,
            resume_available: Some(user_visible_truth.resume_available),
            resume_cursor: None,
            recoverability: Some(user_visible_truth.recoverability.clone()),
            context_budget_usage: None,
            memory_context: None,
            prompt_diagnostics: None,
            turn_cost: None,
            routing_info: None,
            session_totals: None,
        };
        refs.stream_emitter.emit_payload(payload.clone());
        append_stream_event(refs.run_event_logger, &payload).await;
        if let Some(bus) = refs.harness_bus {
            let _ = bus.emit(AgentEvent::StreamErrored {
                session_id: refs.session_id.to_string(),
                reason: state
                    .last_stream_error_reason
                    .clone()
                    .unwrap_or_else(|| "cost_limit".to_string()),
                resume_available: user_visible_truth.resume_available,
                at: chrono::Utc::now(),
            });
        }
        return PreflightOutcome::BreakTerminal;
    }

    // Phase 8 — stream_message_with_resilience (timeout retry / hard fail / Ok).
    let stream = match crate::modules::provider::resilience::stream_message_with_resilience(
        refs.provider_client,
        refs.failover_provider_client,
        &iter_api_request,
        &mut state.stream_circuit,
        refs.stream_resilience_cfg,
    )
    .await
    {
        Ok(s) => {
            CostGuard::record_llm_call_charged(refs.cost_guard_cfg, refs.session_id);
            s
        }
        Err(e) => {
            let stream_error_reason = format_stream_error_reason(&e);
            if is_network_timeout_reason(&stream_error_reason)
                && state.stream_start_retry_count < MAX_STREAM_RETRY_ON_TIMEOUT
            {
                state.stream_start_retry_count += 1;
                tracing::warn!(
                    "[start_agent_stream] start-stream timeout, scheduling retry: stream_id={}, session_id={}, attempt={}/{}, reason={}",
                    refs.stream_id,
                    refs.session_id,
                    state.stream_start_retry_count,
                    MAX_STREAM_RETRY_ON_TIMEOUT,
                    stream_error_reason
                );
                return PreflightOutcome::RetryAfterSleep {
                    sleep: Duration::from_millis(350),
                };
            }
            state.stream_failed = true;
            state.last_stream_error_reason = Some(stream_error_reason.clone());
            state.terminal_status = Some("failed_to_start_stream");
            tracing::error!(
                "[start_agent_stream] Background task failed to start stream: {}",
                stream_error_reason
            );
            let user_visible_truth = TaskOutcomeResolver::resolve(
                ExecutionTruth {
                    has_successful_tool: state.has_successful_tool,
                    has_successful_mutating_tool: state.has_successful_mutating_tool,
                },
                &ConversationTruth {
                    stream_failed: true,
                    terminal_status: "failed_to_start_stream",
                    last_stream_error_reason: Some(stream_error_reason.clone()),
                },
            );
            let resume_cursor = user_visible_truth.resume_available.then(|| {
                build_resume_cursor(refs.stream_id, state.tool_loop_iter, state.token_count)
            });
            let degraded_reason = user_visible_truth.degraded_reason.clone();
            let payload = StreamTokenPayload {
                stream_id: refs.stream_id.to_string(),
                correlation: None,
                text: None,
                thinking: None,
                event_type: "stream_error".to_string(),
                tool_call_id: None,
                tool_name: None,
                tool_status: None,
                tool_args: None,
                tool_result: Some(stream_error_reason),
                tool_duration_ms: None,
                effective_workdir: None,
                policy_decision: None,
                evidence_id: None,
                request_id: Some(state.provider_request_id.clone()),
                task_outcome: Some(user_visible_truth.task_outcome.to_string()),
                degraded_reason,
                resume_available: Some(user_visible_truth.resume_available),
                resume_cursor,
                recoverability: Some(user_visible_truth.recoverability.clone()),
                context_budget_usage: None,
                memory_context: None,
                prompt_diagnostics: None,
                turn_cost: None,
                routing_info: None,
                session_totals: None,
            };
            refs.stream_emitter.emit_payload(payload.clone());
            append_stream_event(refs.run_event_logger, &payload).await;
            // Phase M4-C P5 — emit harness `StreamErrored` event so
            // the trace aggregator records the hard error against
            // the run report.
            if let Some(bus) = refs.harness_bus {
                let _ = bus.emit(AgentEvent::StreamErrored {
                    session_id: refs.session_id.to_string(),
                    reason: state
                        .last_stream_error_reason
                        .clone()
                        .unwrap_or_else(|| "unknown_stream_error".to_string()),
                    resume_available: user_visible_truth.resume_available,
                    at: chrono::Utc::now(),
                });
            }
            return PreflightOutcome::BreakTerminal;
        }
    };

    PreflightOutcome::Continue {
        stream,
        force_final_response,
    }
}

/// Run phase 11 of the outer loop body.
///
/// Verbatim move of the prior inline code — see the original call
/// site in `run_stream_task_body` for surrounding context.
///
/// On entry, `pending_tool_uses` is empty. The helper may
/// retroactively populate it from textual tool-call extraction —
/// in that case the helper returns [`NoToolOutcome::FallThrough`]
/// and the caller continues the iteration body so the recovered
/// tool calls actually execute.
pub(super) async fn handle_no_tool_calls(
    state: &mut StreamLoopState,
    pending_tool_uses: &mut Vec<(String, String, String)>,
    refs: &NoToolSharedRefs<'_>,
) -> NoToolOutcome {
    tracing::info!(
        "[start_agent_stream] No pending tool uses, breaking outer loop. accumulated_text len={}",
        state.accumulated_text.len()
    );
    if !refs.force_final_response {
        let textual_tool_calls =
            super::work_loop::extract_textual_tool_calls(&state.accumulated_text);
        if !textual_tool_calls.is_empty() {
            state.provider_textual_tool_markup_seen = true;
            let markup_family =
                super::work_loop::detect_textual_tool_call_markup(&state.accumulated_text)
                    .unwrap_or_else(|| "textual_tool_calls".to_string());
            let warning = format!(
                "provider emitted {markup_family} text instead of structured tool_calls; converted to canonical tool calls"
            );
            if !state
                .diagnostic_warnings
                .iter()
                .any(|value| value == &warning)
            {
                state.diagnostic_warnings.push(warning);
            }
            let converted_count = textual_tool_calls.len();
            let _ = refs
                .run_event_logger
                .append(
                    "provider_tool_call_compat_warning",
                    serde_json::json!({
                        "stream_id": refs.stream_id,
                        "session_id": refs.session_id,
                        "provider_id": refs.provider_id,
                        "model": refs.model,
                        "markup_family": markup_family,
                        "text_len": state.accumulated_text.len(),
                        "converted_tool_call_count": converted_count,
                        "recovery_action": "execute_as_structured_tool_calls",
                        "sanitized": true,
                    }),
                )
                .await;
            *pending_tool_uses = textual_tool_calls;
            state.accumulated_text.clear();
            state.accumulated_thinking.clear();
        }
    }

    if !pending_tool_uses.is_empty() {
        return NoToolOutcome::FallThrough;
    }

    if let Some(markup_family) =
        super::work_loop::detect_textual_tool_call_markup(&state.accumulated_text)
    {
        state.provider_textual_tool_markup_seen = true;
        let warning =
            format!("provider emitted {markup_family} text instead of structured tool_calls");
        if !state
            .diagnostic_warnings
            .iter()
            .any(|value| value == &warning)
        {
            state.diagnostic_warnings.push(warning.clone());
        }
        let _ = refs
            .run_event_logger
            .append(
                "provider_tool_call_compat_warning",
                serde_json::json!({
                    "stream_id": refs.stream_id,
                    "session_id": refs.session_id,
                    "provider_id": refs.provider_id,
                    "model": refs.model,
                    "markup_family": markup_family,
                    "text_len": state.accumulated_text.len(),
                    "recovery_action": "nudge_with_required_tool_choice",
                    "sanitized": true,
                }),
            )
            .await;
    }
    if should_retry_tool_required_no_tool(
        refs.work_loop_decision,
        state.has_successful_mutating_tool,
        state.tool_required_no_tool_retry_count,
        refs.force_final_response,
        refs.tool_defs.len(),
    ) {
        state.tool_required_no_tool_retry_count += 1;
        tracing::warn!(
            "[start_agent_stream] tool-required task produced no mutating tool call; retrying once with tool-required loop control. stream_id={}, session_id={}",
            refs.stream_id,
            refs.session_id
        );
        let payload = serde_json::json!({
            "reason": "tool_required_no_tool",
            "retry_count": state.tool_required_no_tool_retry_count,
            "available_tool_count": refs.tool_defs.len(),
        });
        let _ = refs
            .run_event_logger
            .append("tool_required_no_tool_retry", payload)
            .await;
        state.accumulated_text.clear();
        state.accumulated_thinking.clear();
        state.session_messages.push(InputMessage::user_text(
            "[agent_loop_control] The previous assistant response did not call tools, but this user request requires concrete file/tool execution before completion. Call the available tools now to inspect, create or edit the artifact, and verify it. Use complete JSON arguments for every tool call. Do not answer only with prose. If tool execution is impossible, explain the blockage in the final report.",
        ));
        state.force_tool_choice_next = true;
        return NoToolOutcome::Continue;
    }
    if should_retry_announced_tool_intent_no_tool(
        &state.accumulated_text,
        state.tool_intent_nudge_retry_count,
        refs.force_final_response,
        refs.tool_defs.len(),
    ) {
        state.tool_intent_nudge_retry_count += 1;
        tracing::warn!(
            "[start_agent_stream] assistant announced tool intent without tool call; nudging once. stream_id={}, session_id={}",
            refs.stream_id,
            refs.session_id
        );
        let payload = serde_json::json!({
            "reason": "announced_tool_intent_no_tool",
            "retry_count": state.tool_intent_nudge_retry_count,
            "available_tool_count": refs.tool_defs.len(),
        });
        let _ = refs
            .run_event_logger
            .append("tool_intent_nudge_retry", payload)
            .await;
        state.accumulated_text.clear();
        state.accumulated_thinking.clear();
        state.session_messages.push(InputMessage::user_text(
            super::work_loop::tool_intent_nudge_message(),
        ));
        state.force_tool_choice_next = true;
        return NoToolOutcome::Continue;
    }
    if state.terminal_status.is_none() {
        state.terminal_status = match state.finalization_reason.as_deref() {
            Some("invalid_tool_args_repeated") => Some("invalid_tool_args_repeated"),
            Some("repeated_tool_batch_no_progress") => Some("repeated_tool_batch_no_progress"),
            Some("approval_required_for_mutation") => Some("approval_required_for_mutation"),
            _ if super::work_loop::requires_memory_recall_evidence(refs.work_loop_decision)
                && !state.has_successful_tool
                && super::work_loop::is_incomplete_memory_lookup_response(
                    &state.accumulated_text,
                ) =>
            {
                Some("memory_recall_required_no_tool")
            }
            _ if super::work_loop::requires_tool_execution_evidence(refs.work_loop_decision)
                && !state.has_successful_mutating_tool =>
            {
                if state.provider_textual_tool_markup_seen {
                    Some("provider_textual_tool_call_markup")
                } else {
                    Some("tool_required_no_tool")
                }
            }
            _ if super::work_loop::assistant_claims_tool_execution_without_tool(
                &state.accumulated_text,
            ) && !state.has_successful_mutating_tool =>
            {
                if state.provider_textual_tool_markup_seen {
                    Some("provider_textual_tool_call_markup")
                } else {
                    Some("tool_required_no_tool")
                }
            }
            _ if super::work_loop::detect_repetitive_model_output(&state.accumulated_text) => {
                Some("repetitive_model_output")
            }
            _ => Some("model_stop_no_tools"),
        };
    }
    NoToolOutcome::Break
}

/// Result of one iteration's inner SSE event-loop phase (step 10 of
/// the outer loop body).
///
/// `cancel_rx` is threaded through so the orchestrator can carry it
/// to the next preflight call without keeping the helper's borrow
/// alive across the retry/sleep boundary.
pub(super) enum RunStreamOutcome {
    /// Inner SSE loop completed.  `state.accumulated_text`,
    /// `state.accumulated_thinking`, `state.token_count`,
    /// `state.current_call_usage`, `state.accumulated_usage`,
    /// `state.stream_failed`, `state.last_stream_error_reason`,
    /// `state.terminal_status`, `state.stream_event_retry_count`,
    /// `state.completion_already_emitted`, and
    /// `state.last_finish_reason` are all populated.  `pending_tool_uses`
    /// is returned to the caller so phases 11-14 can consume it without
    /// growing the state bag.
    Completed {
        cancel_rx: tokio::sync::oneshot::Receiver<()>,
        pending_tool_uses: Vec<(String, String, String)>,
    },
    /// Inner SSE loop returned `retry_outer_after_timeout`.  Caller
    /// must sleep `sleep` and `continue` the outer loop without
    /// consuming a real iteration slot.
    RetryAfterSleep {
        cancel_rx: tokio::sync::oneshot::Receiver<()>,
        sleep: Duration,
    },
}

/// Bundled shared borrows consumed by [`iteration_run_stream`].
pub(super) struct RunStreamSharedRefs<'a> {
    pub stream_id: &'a str,
    pub session_id: &'a str,
    pub provider_id: &'a str,
    pub model: &'a str,
    pub stream_emitter: &'a AgentStreamEmitter,
    pub run_event_logger: &'a RunEventLogger,
    pub app_session: &'a AppSession,
    pub session_manager: &'a Arc<SessionManager>,
    pub harness_bus: Option<&'a EventBus>,
    pub execution_context: &'a SessionExecutionContext,
}

/// Run phase 10 of the outer loop body — the inner SSE event-processing
/// loop for one provider API call.
///
/// Verbatim move of the prior inline code in `run_stream_task_body`:
/// builds the [`StreamEventLoopContext`](super::stream_event_loop::StreamEventLoopContext),
/// invokes [`run_stream_event_loop`](super::stream_event_loop::run_stream_event_loop),
/// destructures the result back into `state`, and sleeps + signals
/// `RetryAfterSleep` if the inner loop reported `retry_outer_after_timeout`.
pub(super) async fn iteration_run_stream(
    state: &mut StreamLoopState,
    stream: MessageStream,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
    refs: &RunStreamSharedRefs<'_>,
) -> RunStreamOutcome {
    let event_loop_ctx = super::stream_event_loop::StreamEventLoopContext {
        stream,
        cancel_rx,
        stream_id: refs.stream_id.to_string(),
        session_id: refs.session_id.to_string(),
        provider_request_id: state.provider_request_id.clone(),
        accumulated_text: std::mem::take(&mut state.accumulated_text),
        accumulated_thinking: std::mem::take(&mut state.accumulated_thinking),
        token_count: state.token_count,
        current_call_usage: std::mem::take(&mut state.current_call_usage),
        accumulated_usage: std::mem::take(&mut state.accumulated_usage),
        stream_emitter: refs.stream_emitter.clone(),
        run_event_logger: refs.run_event_logger.clone(),
        app_session: refs.app_session.clone(),
        session_manager: refs.session_manager.clone(),
        harness_bus: refs.harness_bus.cloned(),
        execution_context: refs.execution_context.clone(),
        has_successful_tool: state.has_successful_tool,
        has_successful_mutating_tool: state.has_successful_mutating_tool,
        provider_id: refs.provider_id.to_string(),
        model: refs.model.to_string(),
        stream_event_retry_count: state.stream_event_retry_count,
    };
    let loop_result = super::stream_event_loop::run_stream_event_loop(event_loop_ctx).await;
    let pending_tool_uses = loop_result.pending_tool_uses;
    state.accumulated_text = loop_result.accumulated_text;
    state.accumulated_thinking = loop_result.accumulated_thinking;
    state.token_count = loop_result.token_count;
    state.current_call_usage = loop_result.current_call_usage;
    state.accumulated_usage = loop_result.accumulated_usage;
    state.stream_failed = loop_result.stream_failed;
    state.last_stream_error_reason = loop_result.last_stream_error_reason;
    state.terminal_status = loop_result.terminal_status;
    let retry_outer_after_timeout = loop_result.retry_outer_after_timeout;
    state.stream_event_retry_count = loop_result.stream_event_retry_count;
    let _emitted_stream_delta_in_iteration = loop_result.emitted_stream_delta_in_iteration;
    state.completion_already_emitted = loop_result.completion_already_emitted;
    state.last_finish_reason = loop_result.finish_reason;
    let cancel_rx = loop_result.cancel_rx;

    if retry_outer_after_timeout {
        return RunStreamOutcome::RetryAfterSleep {
            cancel_rx,
            sleep: Duration::from_millis(350),
        };
    }
    RunStreamOutcome::Completed {
        cancel_rx,
        pending_tool_uses,
    }
}

/// Bundled shared borrows consumed by [`iteration_execute_tools`].
pub(super) struct ExecuteToolsSharedRefs<'a> {
    pub stream_id: &'a str,
    pub session_id: &'a str,
    pub stream_emitter: &'a AgentStreamEmitter,
    pub run_event_logger: &'a RunEventLogger,
    pub tool_registry: &'a Arc<ToolRegistry>,
    pub permission_senders:
        &'a Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<PermissionPromptDecision>>>>,
    pub permission_overrides:
        &'a Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
    pub permission_policy: &'a Arc<PermissionPolicy>,
    pub execution_context: &'a SessionExecutionContext,
    pub work_loop_decision: &'a WorkLoopDecision,
    pub harness_bus: Option<&'a EventBus>,
    pub mode: PermissionMode,
    pub run_id: &'a str,
    pub app_data_dir: &'a PathBuf,
}

/// Run phases 13-14 (and the immediate post-batch state mutations) of
/// the outer loop body.
///
/// Verbatim move of the prior inline code in `run_stream_task_body`:
/// repeated-tool-batch detection (phase 13) which sets
/// `force_final_response_next` then falls through to
/// [`execute_tool_batch`](super::stream_tool_execution::execute_tool_batch)
/// (phase 14), followed by post-batch finalization-reason guards
/// (memory-recall success, single-shell evidence) and the
/// post-mutation update message injection.
///
/// `tool_executor` is consumed and the (possibly-mutated) executor is
/// returned so the orchestrator can reuse it on the next iteration.
pub(super) async fn iteration_execute_tools(
    state: &mut StreamLoopState,
    pending_tool_uses: Vec<(String, String, String)>,
    tool_executor: ToolRegistryExecutor,
    refs: &ExecuteToolsSharedRefs<'_>,
) -> ToolRegistryExecutor {
    // Phase 13 — repeated tool-batch detection.
    let current_tool_batch_signature = tool_batch_signature(&pending_tool_uses);
    if state.last_tool_batch_signature.as_deref() == Some(current_tool_batch_signature.as_str()) {
        state.repeated_tool_batch_count += 1;
    } else {
        state.repeated_tool_batch_count = 1;
        state.last_tool_batch_signature = Some(current_tool_batch_signature);
    }
    if state.repeated_tool_batch_count >= REPEATED_TOOL_BATCH_LIMIT {
        tracing::warn!(
            "[start_agent_stream] repeated tool batch detected; next iteration will force final summary. stream_id={}, session_id={}, repeated_count={}",
            refs.stream_id,
            refs.session_id,
            state.repeated_tool_batch_count
        );
        state.force_final_response_next = true;
        state.finalization_reason = Some("repeated_tool_batch_no_progress".to_string());
    }

    // Phase 14 — execute the tool batch.
    let tool_ctx = super::stream_tool_execution::ToolExecutionContext {
        pending_tool_uses,
        stream_id: refs.stream_id.to_string(),
        session_id: refs.session_id.to_string(),
        provider_request_id: state.provider_request_id.clone(),
        accumulated_text: std::mem::take(&mut state.accumulated_text),
        accumulated_thinking: std::mem::take(&mut state.accumulated_thinking),
        session_messages: std::mem::take(&mut state.session_messages),
        timeline_session_messages: std::mem::take(&mut state.timeline_session_messages),
        stream_emitter: refs.stream_emitter.clone(),
        run_event_logger: refs.run_event_logger.clone(),
        tool_registry: refs.tool_registry.clone(),
        tool_executor,
        permission_senders: refs.permission_senders.clone(),
        permission_overrides: refs.permission_overrides.clone(),
        permission_policy: refs.permission_policy.clone(),
        execution_context: refs.execution_context.clone(),
        work_loop_decision: refs.work_loop_decision.clone(),
        harness_bus: refs.harness_bus.cloned(),
        mode: refs.mode,
        invalid_tool_args_streak: state.invalid_tool_args_streak,
        has_successful_tool: state.has_successful_tool,
        has_successful_mutating_tool: state.has_successful_mutating_tool,
        sanitized_dropped_invalid_tool_use_inputs: state.sanitized_dropped_invalid_tool_use_inputs,
        sanitize_invalid_tool_use_samples: std::mem::take(
            &mut state.sanitize_invalid_tool_use_samples,
        ),
        run_id: refs.run_id.to_string(),
        app_data_dir: refs.app_data_dir.clone(),
    };
    let had_successful_mutating_tool_before = state.has_successful_mutating_tool;
    let tool_result = super::stream_tool_execution::execute_tool_batch(tool_ctx).await;
    state.accumulated_text = tool_result.accumulated_text;
    state.accumulated_thinking = tool_result.accumulated_thinking;
    state.session_messages = tool_result.session_messages;
    state.timeline_session_messages = tool_result.timeline_session_messages;
    state.has_successful_tool = tool_result.has_successful_tool;
    state.has_successful_mutating_tool = tool_result.has_successful_mutating_tool;
    if !had_successful_mutating_tool_before
        && state.has_successful_mutating_tool
        && super::work_loop::requires_tool_execution_evidence(refs.work_loop_decision)
    {
        if let Some(message) = super::todo_ledger::post_mutation_update_message(refs.session_id) {
            state
                .session_messages
                .push(InputMessage::user_text(message));
        }
    }
    state.force_final_response_next =
        state.force_final_response_next || tool_result.force_final_response_next;
    apply_memory_recall_success_finalization_guard(
        refs.work_loop_decision,
        tool_result.has_successful_tool,
        &mut state.force_final_response_next,
        &mut state.finalization_reason,
    );
    if super::work_loop::requires_single_shell_command_evidence(refs.work_loop_decision)
        && tool_result.has_successful_tool
    {
        state.force_final_response_next = true;
        state.finalization_reason = Some("single_shell_command_executed".to_string());
    }
    if state.finalization_reason.is_none() {
        state.finalization_reason = tool_result.finalization_reason;
    }
    state.invalid_tool_args_streak = tool_result.invalid_tool_args_streak;
    state.sanitized_dropped_invalid_tool_use_inputs =
        tool_result.sanitized_dropped_invalid_tool_use_inputs;
    state.sanitize_invalid_tool_use_samples = tool_result.sanitize_invalid_tool_use_samples;
    if state.pending_operation_for_delegate.is_none() {
        state.pending_operation_for_delegate = tool_result.pending_operation;
    }
    tracing::info!(
        "[start_agent_stream] Tool execution done, continuing outer loop. session_messages len={}",
        state.session_messages.len()
    );
    tool_result.tool_executor
}
