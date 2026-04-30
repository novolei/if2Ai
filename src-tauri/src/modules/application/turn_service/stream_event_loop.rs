//! Inner SSE event-processing loop extracted from
//! [`crate::modules::application::turn_service::stream_task::run_stream_task`].
//!
//! Owns: per-iteration provider stream event consumption (ContentBlockDelta,
//! ContentBlockStart, ContentBlockStop, MessageStart/Delta/Stop), tool-call
//! tracking by block index, periodic session saves every 50 tokens, error
//! handling with timeout retry logic, and stream-error emission (including
//! harness StreamErrored events).
//!
//! Lifted out of `stream_task.rs` so the orchestrator file stays below
//! CHARTER §5's 800-LOC hard limit (GAP-005). Behaviour is bit-for-bit
//! preserved.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::modules::api::{InputContentBlock, InputMessage, StreamEvent as ApiStreamEvent};
use crate::modules::harness::{AgentEvent, EventBus};
use crate::modules::runtime::block_conversion::{
    parse_tool_input_json, summarize_tool_result_for_model,
};
use crate::modules::runtime::budget::MAX_STREAM_RETRY_ON_TIMEOUT;
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::resume_cursor::build_resume_cursor;
use crate::modules::runtime::session::{ContentBlock, ConversationMessage};
use crate::modules::runtime::stream_emitter::{AgentStreamEmitter, StreamTokenPayload};
use crate::modules::runtime::stream_error_reason::{
    format_stream_error_reason, is_network_timeout_reason,
};
use crate::modules::runtime::stream_outcome::{
    ConversationTruth, ExecutionTruth, TaskOutcomeResolver,
};
use crate::modules::session::{Session as AppSession, SessionManager};

use super::stream_task::append_stream_event;

const SAVE_INTERVAL: u32 = 50;

/// All owned/mutable state the inner event loop needs to read and modify.
/// Bundled so the call signature stays one struct instead of 20+ arguments.
pub(super) struct StreamEventLoopContext {
    pub stream: crate::modules::api::MessageStream,
    pub cancel_rx: tokio::sync::oneshot::Receiver<()>,
    pub stream_id: String,
    pub session_id: String,
    pub provider_request_id: String,
    pub accumulated_text: String,
    pub accumulated_thinking: String,
    pub token_count: u32,
    pub current_call_usage: crate::modules::runtime::usage::TokenUsage,
    pub accumulated_usage: crate::modules::runtime::usage::TokenUsage,
    pub stream_emitter: AgentStreamEmitter,
    pub run_event_logger: RunEventLogger,
    pub app_session: AppSession,
    pub session_manager: Arc<SessionManager>,
    pub harness_bus: Option<EventBus>,
    pub execution_context: crate::modules::control_plane::SessionExecutionContext,
    pub has_successful_tool: bool,
    pub has_successful_mutating_tool: bool,
    pub provider_id: String,
    pub model: String,
    pub stream_event_retry_count: usize,
}

/// Results produced by one invocation of the inner event loop.
pub(super) struct StreamEventLoopResult {
    pub pending_tool_uses: Vec<(String, String, String)>,
    pub accumulated_text: String,
    pub accumulated_thinking: String,
    pub token_count: u32,
    pub current_call_usage: crate::modules::runtime::usage::TokenUsage,
    pub accumulated_usage: crate::modules::runtime::usage::TokenUsage,
    pub stream_failed: bool,
    pub last_stream_error_reason: Option<String>,
    pub terminal_status: Option<&'static str>,
    pub retry_outer_after_timeout: bool,
    pub stream_event_retry_count: usize,
    pub emitted_stream_delta_in_iteration: bool,
    pub completion_already_emitted: bool,
    /// Returned to the orchestrator so it can poll for cancellation
    /// at the top of the next outer-loop iteration.
    pub cancel_rx: tokio::sync::oneshot::Receiver<()>,
    /// Provider-supplied finish reason from the final SSE event of this
    /// API call. Captured from `MessageDelta.delta.stop_reason`, which
    /// both Anthropic (native) and OpenAI-compat (translated from
    /// `finish_reason`) populate prior to `MessageStop`.
    ///
    /// `Some("max_tokens")` / `Some("length")` indicates the provider
    /// truncated the response; `Some("end_turn")` / `Some("tool_use")`
    /// are normal terminations. `None` when the stream ended without a
    /// finish_reason event (cancelled, errored before MessageDelta, or
    /// `Ok(None)` reached before any MessageDelta carried a stop_reason).
    ///
    /// Steward-Alignment Phase 2 T2: prerequisite for `force_text` and
    /// `is_length_truncation` to fire on the streaming agent loop path.
    pub finish_reason: Option<String>,
}

/// Run the inner SSE event-processing loop for one provider API call
/// within a streaming chat turn.
///
/// Consumes the [`MessageStream`], processes every event (text/thinking deltas,
/// tool-call tracking, usage accumulation), and returns all mutated state via
/// [`StreamEventLoopResult`]. The caller (the outer tool-loop orchestrator)
/// destructures the result and continues.
pub(super) async fn run_stream_event_loop(ctx: StreamEventLoopContext) -> StreamEventLoopResult {
    let StreamEventLoopContext {
        mut stream,
        mut cancel_rx,
        stream_id,
        session_id,
        provider_request_id,
        mut accumulated_text,
        mut accumulated_thinking,
        mut token_count,
        mut current_call_usage,
        mut accumulated_usage,
        stream_emitter,
        run_event_logger,
        app_session,
        session_manager,
        harness_bus,
        execution_context,
        has_successful_tool,
        has_successful_mutating_tool,
        provider_id,
        model: _model,
        mut stream_event_retry_count,
    } = ctx;

    let mut index_to_tool_id: HashMap<u32, String> = HashMap::new();
    let mut index_to_tool_name: HashMap<u32, String> = HashMap::new();
    let mut tool_arguments: HashMap<String, String> = HashMap::new();
    let mut pending_tool_uses: Vec<(String, String, String)> = Vec::new();
    let mut stream_failed = false;
    let mut last_stream_error_reason: Option<String> = None;
    let mut terminal_status: Option<&'static str> = None;
    let mut retry_outer_after_timeout = false;
    let mut emitted_stream_delta_in_iteration = false;
    let mut completion_already_emitted = false;
    let mut finish_reason: Option<String> = None;

    loop {
        let next_event = tokio::select! {
            _ = &mut cancel_rx => {
                tracing::info!("[start_agent_stream] Stream cancelled while waiting for provider event");
                stream_failed = true;
                last_stream_error_reason = Some("cancelled_by_user".to_string());
                terminal_status = Some("cancelled_by_user");
                completion_already_emitted = true;
                break;
            }
            event = stream.next_event() => event,
        };
        match next_event {
            Ok(Some(event)) => match event {
                ApiStreamEvent::ProviderRawChunkDiagnostic(diagnostic) => {
                    let _ = run_event_logger
                        .append(
                            "provider_raw_chunk_ledger",
                            serde_json::json!({
                                "stream_id": stream_id.clone(),
                                "session_id": session_id.clone(),
                                "provider_id": provider_id.clone(),
                                "model": _model.clone(),
                                "request_id": provider_request_id.clone(),
                                "sanitized": true,
                                "diagnostic": diagnostic,
                            }),
                        )
                        .await;
                }
                ApiStreamEvent::ContentBlockDelta(delta_event) => match delta_event.delta {
                    crate::modules::api::ContentBlockDelta::TextDelta { text } => {
                        accumulated_text.push_str(&text);
                        emitted_stream_delta_in_iteration = true;
                        token_count += 1;
                        if token_count % 5 == 0 {
                            tracing::info!(
                                "[start_agent_stream] text_delta: +{} chars, accumulated {} total",
                                text.len(),
                                accumulated_text.len()
                            );
                        }
                        if token_count % SAVE_INTERVAL == 0 {
                            let mut interim_session = app_session.clone();
                            interim_session.messages.push(ConversationMessage {
                                role: crate::modules::runtime::session::MessageRole::Assistant,
                                blocks: vec![ContentBlock::Text {
                                    text: accumulated_text.clone(),
                                }],
                                usage: None,
                                thinking: if accumulated_thinking.is_empty() {
                                    None
                                } else {
                                    Some(accumulated_thinking.clone())
                                },
                                task_outcome: None,
                                degraded_reason: None,
                                resume_available: None,
                                resume_cursor: None,
                                request_id: Some(provider_request_id.clone()),
                            });
                            let _ = session_manager.save_session(&interim_session).await;
                            tracing::debug!(
                                "[start_agent_stream] Periodic session save at token {}",
                                token_count
                            );
                        }
                        let payload = StreamTokenPayload {
                            stream_id: stream_id.clone(),
                            correlation: None,
                            text: Some(text),
                            thinking: None,
                            event_type: "text_delta".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            tool_status: None,
                            tool_args: None,
                            tool_result: None,
                            tool_duration_ms: None,
                            effective_workdir: None,
                            policy_decision: None,
                            evidence_id: None,
                            request_id: Some(provider_request_id.clone()),
                            task_outcome: None,
                            degraded_reason: None,
                            resume_available: None,
                            resume_cursor: None,
                            recoverability: None,
                            context_budget_usage: None,
                            memory_context: None,
                            prompt_diagnostics: None,
                            turn_cost: None,
                            routing_info: None,
                            session_totals: None,
                        };
                        stream_emitter.emit_payload(payload.clone());
                        append_stream_event(&run_event_logger, &payload).await;
                    }
                    crate::modules::api::ContentBlockDelta::ThinkingDelta { thinking } => {
                        accumulated_thinking.push_str(&thinking);
                        emitted_stream_delta_in_iteration = true;
                        let payload = StreamTokenPayload {
                            stream_id: stream_id.clone(),
                            correlation: None,
                            text: None,
                            thinking: Some(thinking),
                            event_type: "thinking_delta".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            tool_status: None,
                            tool_args: None,
                            tool_result: None,
                            tool_duration_ms: None,
                            effective_workdir: None,
                            policy_decision: None,
                            evidence_id: None,
                            request_id: Some(provider_request_id.clone()),
                            task_outcome: None,
                            degraded_reason: None,
                            resume_available: None,
                            resume_cursor: None,
                            recoverability: None,
                            context_budget_usage: None,
                            memory_context: None,
                            prompt_diagnostics: None,
                            turn_cost: None,
                            routing_info: None,
                            session_totals: None,
                        };
                        stream_emitter.emit_payload(payload.clone());
                        append_stream_event(&run_event_logger, &payload).await;
                    }
                    crate::modules::api::ContentBlockDelta::SignatureDelta { .. } => {}
                    crate::modules::api::ContentBlockDelta::InputJsonDelta { partial_json } => {
                        if let Some(tool_id) = index_to_tool_id.get(&delta_event.index).cloned() {
                            tool_arguments
                                .entry(tool_id)
                                .or_default()
                                .push_str(&partial_json);
                        }
                    }
                },
                ApiStreamEvent::ContentBlockStop(stop_event) => {
                    if let Some(tool_id) = index_to_tool_id.remove(&stop_event.index) {
                        let tool_name = index_to_tool_name
                            .remove(&stop_event.index)
                            .unwrap_or_default();
                        if let Some(input_json) = tool_arguments.remove(&tool_id) {
                            pending_tool_uses.push((tool_id, tool_name, input_json));
                        }
                    }
                }
                ApiStreamEvent::MessageStop(_) => {
                    for (index, tool_id) in index_to_tool_id.drain() {
                        let tool_name = index_to_tool_name.remove(&index).unwrap_or_default();
                        if let Some(input_json) = tool_arguments.remove(&tool_id) {
                            pending_tool_uses.push((tool_id, tool_name, input_json));
                        }
                    }
                    accumulated_usage.input_tokens = accumulated_usage
                        .input_tokens
                        .saturating_add(current_call_usage.input_tokens);
                    accumulated_usage.output_tokens = accumulated_usage
                        .output_tokens
                        .saturating_add(current_call_usage.output_tokens);
                    accumulated_usage.cache_creation_input_tokens = accumulated_usage
                        .cache_creation_input_tokens
                        .saturating_add(current_call_usage.cache_creation_input_tokens);
                    accumulated_usage.cache_read_input_tokens = accumulated_usage
                        .cache_read_input_tokens
                        .saturating_add(current_call_usage.cache_read_input_tokens);
                    tracing::info!(
                        "[start_agent_stream] MessageStop: this_call_usage in={} out={} cache_w={} cache_r={} | turn_total in={} out={}",
                        current_call_usage.input_tokens,
                        current_call_usage.output_tokens,
                        current_call_usage.cache_creation_input_tokens,
                        current_call_usage.cache_read_input_tokens,
                        accumulated_usage.input_tokens,
                        accumulated_usage.output_tokens,
                    );
                    current_call_usage = crate::modules::runtime::usage::TokenUsage::default();
                    tracing::info!(
                        "[start_agent_stream] MessageStop received, {} pending tool uses",
                        pending_tool_uses.len()
                    );
                    break;
                }
                ApiStreamEvent::ContentBlockStart(start_event) => match start_event.content_block {
                    crate::modules::api::OutputContentBlock::Thinking { .. } => {
                        let payload = StreamTokenPayload {
                            stream_id: stream_id.clone(),
                            correlation: None,
                            text: None,
                            thinking: None,
                            event_type: "thinking_start".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            tool_status: None,
                            tool_args: None,
                            tool_result: None,
                            tool_duration_ms: None,
                            effective_workdir: None,
                            policy_decision: None,
                            evidence_id: None,
                            request_id: Some(provider_request_id.clone()),
                            task_outcome: None,
                            degraded_reason: None,
                            resume_available: None,
                            resume_cursor: None,
                            recoverability: None,
                            context_budget_usage: None,
                            memory_context: None,
                            prompt_diagnostics: None,
                            turn_cost: None,
                            routing_info: None,
                            session_totals: None,
                        };
                        stream_emitter.emit_payload(payload.clone());
                        append_stream_event(&run_event_logger, &payload).await;
                    }
                    crate::modules::api::OutputContentBlock::ToolUse { id, name, .. } => {
                        index_to_tool_id.insert(start_event.index, id.clone());
                        index_to_tool_name.insert(start_event.index, name.clone());
                        let payload = StreamTokenPayload {
                            stream_id: stream_id.clone(),
                            correlation: None,
                            text: None,
                            thinking: None,
                            event_type: "tool_call_update".to_string(),
                            tool_call_id: Some(id.clone()),
                            tool_name: Some(name.clone()),
                            tool_status: Some("queued".to_string()),
                            tool_args: None,
                            tool_result: None,
                            tool_duration_ms: None,
                            effective_workdir: None,
                            policy_decision: None,
                            evidence_id: Some(id.clone()),
                            request_id: Some(provider_request_id.clone()),
                            task_outcome: None,
                            degraded_reason: None,
                            resume_available: None,
                            resume_cursor: None,
                            recoverability: None,
                            context_budget_usage: None,
                            memory_context: None,
                            prompt_diagnostics: None,
                            turn_cost: None,
                            routing_info: None,
                            session_totals: None,
                        };
                        stream_emitter.emit_payload(payload.clone());
                        append_stream_event(&run_event_logger, &payload).await;
                    }
                    _ => {}
                },
                ApiStreamEvent::MessageStart(ev) => {
                    let u = &ev.message.usage;
                    current_call_usage.input_tokens =
                        current_call_usage.input_tokens.max(u.input_tokens);
                    current_call_usage.output_tokens =
                        current_call_usage.output_tokens.max(u.output_tokens);
                    current_call_usage.cache_creation_input_tokens = current_call_usage
                        .cache_creation_input_tokens
                        .max(u.cache_creation_input_tokens);
                    current_call_usage.cache_read_input_tokens = current_call_usage
                        .cache_read_input_tokens
                        .max(u.cache_read_input_tokens);
                }
                ApiStreamEvent::MessageDelta(ev) => {
                    record_finish_reason_from_delta(&mut finish_reason, &ev);
                    let u = &ev.usage;
                    current_call_usage.input_tokens =
                        current_call_usage.input_tokens.max(u.input_tokens);
                    current_call_usage.output_tokens =
                        current_call_usage.output_tokens.max(u.output_tokens);
                    current_call_usage.cache_creation_input_tokens = current_call_usage
                        .cache_creation_input_tokens
                        .max(u.cache_creation_input_tokens);
                    current_call_usage.cache_read_input_tokens = current_call_usage
                        .cache_read_input_tokens
                        .max(u.cache_read_input_tokens);
                }
            },
            Ok(None) => {
                for (index, tool_id) in index_to_tool_id.drain() {
                    let tool_name = index_to_tool_name.remove(&index).unwrap_or_default();
                    if let Some(input_json) = tool_arguments.remove(&tool_id) {
                        pending_tool_uses.push((tool_id, tool_name, input_json));
                    }
                }
                accumulated_usage.input_tokens = accumulated_usage
                    .input_tokens
                    .saturating_add(current_call_usage.input_tokens);
                accumulated_usage.output_tokens = accumulated_usage
                    .output_tokens
                    .saturating_add(current_call_usage.output_tokens);
                accumulated_usage.cache_creation_input_tokens = accumulated_usage
                    .cache_creation_input_tokens
                    .saturating_add(current_call_usage.cache_creation_input_tokens);
                accumulated_usage.cache_read_input_tokens = accumulated_usage
                    .cache_read_input_tokens
                    .saturating_add(current_call_usage.cache_read_input_tokens);
                current_call_usage = crate::modules::runtime::usage::TokenUsage::default();
                tracing::info!(
                    "[start_agent_stream] Stream ended (Ok(None)), {} pending tool uses",
                    pending_tool_uses.len()
                );
                break;
            }
            Err(e) => {
                let stream_error_reason = format_stream_error_reason(&e);
                if is_network_timeout_reason(&stream_error_reason)
                    && index_to_tool_id.is_empty()
                    && tool_arguments.is_empty()
                    && pending_tool_uses.is_empty()
                    && !emitted_stream_delta_in_iteration
                    && stream_event_retry_count < MAX_STREAM_RETRY_ON_TIMEOUT
                {
                    stream_event_retry_count += 1;
                    retry_outer_after_timeout = true;
                    tracing::warn!(
                        "[start_agent_stream] stream-event timeout, scheduling retry: stream_id={}, session_id={}, attempt={}/{}, reason={}",
                        stream_id,
                        session_id,
                        stream_event_retry_count,
                        MAX_STREAM_RETRY_ON_TIMEOUT,
                        stream_error_reason
                    );
                    break;
                }
                last_stream_error_reason = Some(stream_error_reason.clone());
                if terminal_status.is_none() {
                    terminal_status = Some("stream_error");
                }
                tracing::error!(
                    "[start_agent_stream] Background task stream error: {}",
                    stream_error_reason
                );
                let user_visible_truth = TaskOutcomeResolver::resolve(
                    ExecutionTruth {
                        has_successful_tool,
                        has_successful_mutating_tool,
                    },
                    &ConversationTruth {
                        stream_failed: true,
                        terminal_status: terminal_status.unwrap_or("stream_error"),
                        last_stream_error_reason: Some(stream_error_reason.clone()),
                    },
                );
                let resume_cursor = user_visible_truth
                    .resume_available
                    .then(|| build_resume_cursor(&stream_id, 1, token_count));
                let degraded_reason = user_visible_truth.degraded_reason.clone();

                for (index, tool_id) in index_to_tool_id.drain() {
                    let tool_name = index_to_tool_name
                        .remove(&index)
                        .unwrap_or_else(|| "unknown".to_string());
                    let payload = StreamTokenPayload {
                        stream_id: stream_id.clone(),
                        correlation: None,
                        text: None,
                        thinking: None,
                        event_type: "tool_call_update".to_string(),
                        tool_call_id: Some(tool_id.clone()),
                        tool_name: Some(tool_name),
                        tool_status: Some("error".to_string()),
                        tool_args: None,
                        tool_result: Some(stream_error_reason.clone()),
                        tool_duration_ms: None,
                        effective_workdir: Some(execution_context.workdir.display().to_string()),
                        policy_decision: None,
                        evidence_id: Some(tool_id),
                        request_id: Some(provider_request_id.clone()),
                        task_outcome: Some(user_visible_truth.task_outcome.to_string()),
                        degraded_reason: degraded_reason.clone(),
                        resume_available: Some(user_visible_truth.resume_available),
                        resume_cursor: resume_cursor.clone(),
                        recoverability: None,
                        context_budget_usage: None,
                        memory_context: None,
                        prompt_diagnostics: None,
                        turn_cost: None,
                        routing_info: None,
                        session_totals: None,
                    };
                    stream_emitter.emit_payload(payload.clone());
                    append_stream_event(&run_event_logger, &payload).await;
                }

                let payload = StreamTokenPayload {
                    stream_id: stream_id.clone(),
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
                    request_id: Some(provider_request_id.clone()),
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
                stream_emitter.emit_payload(payload.clone());
                append_stream_event(&run_event_logger, &payload).await;
                if let Some(bus) = harness_bus.as_ref() {
                    let _ = bus.emit(AgentEvent::StreamErrored {
                        session_id: session_id.clone(),
                        reason: last_stream_error_reason
                            .clone()
                            .unwrap_or_else(|| "unknown_stream_error".to_string()),
                        resume_available: user_visible_truth.resume_available,
                        at: chrono::Utc::now(),
                    });
                }
                stream_failed = true;
                break;
            }
        }
    }

    StreamEventLoopResult {
        pending_tool_uses,
        accumulated_text,
        accumulated_thinking,
        token_count,
        current_call_usage,
        accumulated_usage,
        stream_failed,
        last_stream_error_reason,
        terminal_status,
        retry_outer_after_timeout,
        stream_event_retry_count,
        emitted_stream_delta_in_iteration,
        completion_already_emitted,
        cancel_rx,
        finish_reason,
    }
}

/// When a `MessageDelta` SSE event carries a non-`None` `stop_reason`,
/// record it as the running finish reason for the current API call.
///
/// Both providers feed this field:
/// - Anthropic (Claw): native `message_delta.delta.stop_reason`
///   (`"end_turn"`, `"tool_use"`, `"max_tokens"`, etc.).
/// - OpenAI-compat (Xai/OpenAI): translated from chat-completion
///   `choice.finish_reason` (`"stop"` → `"end_turn"`, `"tool_calls"` →
///   `"tool_use"`, `"length"` → `"length"`/`"max_tokens"`).
///
/// Later events overwrite earlier values: in practice only the final
/// `MessageDelta` of an API call carries a non-`None` stop_reason, but
/// "last write wins" is the safe and conventional rule.
fn record_finish_reason_from_delta(
    current: &mut Option<String>,
    ev: &crate::modules::api::MessageDeltaEvent,
) {
    if let Some(reason) = ev.delta.stop_reason.as_ref() {
        *current = Some(reason.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::record_finish_reason_from_delta;
    use crate::modules::api::{MessageDelta, MessageDeltaEvent, Usage};

    fn delta_event(stop_reason: Option<&str>) -> MessageDeltaEvent {
        MessageDeltaEvent {
            delta: MessageDelta {
                stop_reason: stop_reason.map(|s| s.to_string()),
                stop_sequence: None,
            },
            usage: Usage::default(),
        }
    }

    #[test]
    fn records_stop_reason_when_present() {
        let mut current = None;
        record_finish_reason_from_delta(&mut current, &delta_event(Some("max_tokens")));
        assert_eq!(current.as_deref(), Some("max_tokens"));
    }

    #[test]
    fn leaves_existing_value_untouched_when_stop_reason_none() {
        let mut current = Some("end_turn".to_string());
        record_finish_reason_from_delta(&mut current, &delta_event(None));
        assert_eq!(current.as_deref(), Some("end_turn"));
    }

    #[test]
    fn last_non_none_wins() {
        let mut current = None;
        record_finish_reason_from_delta(&mut current, &delta_event(Some("tool_use")));
        record_finish_reason_from_delta(&mut current, &delta_event(None));
        record_finish_reason_from_delta(&mut current, &delta_event(Some("end_turn")));
        assert_eq!(current.as_deref(), Some("end_turn"));
    }

    #[test]
    fn finish_reason_remains_none_when_no_delta_carries_it() {
        let mut current: Option<String> = None;
        record_finish_reason_from_delta(&mut current, &delta_event(None));
        record_finish_reason_from_delta(&mut current, &delta_event(None));
        assert!(current.is_none());
    }

    #[test]
    fn captures_length_truncation_signal() {
        let mut current = None;
        record_finish_reason_from_delta(&mut current, &delta_event(Some("length")));
        assert_eq!(current.as_deref(), Some("length"));
    }
}
