//! MIG-001-d — extracted spawn-task closure body for the
//! streaming chat turn lifecycle.
//!
//! [`run_stream_task`] is the canonical home of the long async
//! tool-loop / event-emit / retry / persistence / post-turn
//! dispatch body that used to live inline inside the
//! `tokio::spawn(async move { ... })` block of
//! [`crate::modules::application::turn_service::TurnService::stream_turn`].
//!
//! Splitting the closure into a free function lets the caller
//! (`stream::stream_turn`) keep its surface small and lets future
//! sub-slices (post-MIG-001) refactor the loop / finalize phases
//! independently without touching the IPC / sync-prep code path.
//!
//! All captured state is bundled into [`StreamTaskInputs`] so the
//! call site is a single struct construction + `tokio::spawn`
//! call, not 30+ ad-hoc `let xxx_for_stream = ...` lines living
//! next to the spawn.
//!
//! File-size note: the function body is currently ~1500 LOC,
//! exceeding CHARTER §5's 800-LOC hard limit. The file is on the
//! REGISTRY tier-3 watchlist with the rationale that further
//! intra-function splits (per-iteration helpers, post-loop
//! finalize) are deferred to a follow-up refactor pack.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::modules::api::ToolDefinition;
use crate::modules::api::{
    InputContentBlock, InputMessage, MessageRequest, StreamEvent as ApiStreamEvent,
};
use crate::modules::application::memory_candidate_extractor::{
    extract_memory_store_tool_candidates, lookup_existing_records_for_candidates,
};
use crate::modules::application::memory_injection_service::MemoryInjectionDeps;
use crate::modules::application::permission_service::{
    build_permission_policy, parse_permission_mode, TauriPermissionPrompter,
};
use crate::modules::application::prompt_planner::{
    extend_sample_ids, sanitize_messages_for_provider, ContextGovernor,
};
use crate::modules::application::stream_emitter_service::dispatch_after_turn;
use crate::modules::application::tool_executor::ToolRegistryExecutor;
use crate::modules::application::tool_heuristics::{
    contains_unverified_file_claim, is_mutating_tool_success,
};
use crate::modules::application::trajectory_service::record_trajectory_if_possible;
use crate::modules::application::MemoryItemProjection;
use crate::modules::control_plane::session_bridge::log_context_fingerprint;
use crate::modules::control_plane::{AuditEmitter, SessionExecutionContext};
use crate::modules::harness::{AgentEvent, EventBus};
use crate::modules::learning::reflection::ReflectionEngine;
use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::learning::LearningModule;
use crate::modules::memory::retrieval::ActiveRetrievalManager;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryTicker, PinnedStore, SharedMemoryProvider};
use crate::modules::runtime::block_conversion::{
    parse_tool_input_json, runtime_block_to_input_block, summarize_tool_result_for_model,
};
use crate::modules::runtime::budget::{
    MAX_REQUEST_CHAR_BUDGET, MAX_REQUEST_MESSAGE_COUNT, MAX_REQUEST_TOKEN_BUDGET_ESTIMATE,
    MAX_STREAM_RETRY_ON_TIMEOUT,
};
use crate::modules::runtime::compact::{compact_session, should_compact, CompactionConfig};
use crate::modules::runtime::permissions::PermissionPromptDecision;
use crate::modules::runtime::resume_cursor::build_resume_cursor;
use crate::modules::runtime::session::{
    ContentBlock, ConversationMessage, Session as RuntimeSession,
};
use crate::modules::runtime::stream_emitter::{
    AgentStreamEmitter, ContextBudgetUsagePayload, StreamTokenPayload,
};
use crate::modules::runtime::stream_error_reason::{
    format_stream_error_reason, is_network_timeout_reason,
};
use crate::modules::runtime::stream_outcome::{
    ConversationTruth, ExecutionTruth, TaskOutcomeResolver,
};
use crate::modules::runtime::timeline_flush::{
    flush_assistant_timeline_segment, PersistedTurnOutcome,
};
use crate::modules::session::Session as AppSession;
use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;

// MAX_REQUEST_* / MAX_STREAM_RETRY_ON_TIMEOUT moved to
// `crate::modules::runtime::budget` so the canonical preflight /
// streaming budget constants live next to ContextBudget.
// Imported above.

/// All state captured by the original
/// `tokio::spawn(async move { ... })` closure inside
/// `stream_turn`. Bundled into a struct so the spawn call site
/// stays one line and so individual fields can be added / removed
/// without churn at the spawn boundary.
pub(super) struct StreamTaskInputs {
    pub stream_id_for_task: String,
    pub session_id: String,
    pub user_message_clone: String,
    pub permission_mode_for_stream: Option<String>,
    pub inbound_resume_cursor: Option<String>,
    pub turn_number_for_stream: u64,
    pub baseline_message_count_stream: usize,
    pub messages_for_stream: Vec<InputMessage>,
    pub tool_defs_for_stream: Vec<ToolDefinition>,
    pub system_prompt_for_stream: String,
    pub provider_client_for_stream: crate::modules::api::ProviderClient,
    pub model_for_stream: String,
    pub execution_context_for_task: SessionExecutionContext,
    pub tool_registry_clone: Arc<ToolRegistry>,
    pub session_manager: Arc<SessionManager>,
    pub app_session_clone: AppSession,
    pub stream_emitter: AgentStreamEmitter,
    pub cancel_rx: oneshot::Receiver<()>,
    pub permission_senders:
        Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<PermissionPromptDecision>>>>,
    pub permission_overrides:
        Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
    pub trajectory_manager_for_stream: Option<Arc<TrajectoryManager>>,
    pub learning_module_for_stream: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
    pub memory_provider_for_stream: SharedMemoryProvider,
    pub memory_ticker_for_stream: Arc<MemoryTicker>,
    pub harness_event_bus_for_stream: Option<EventBus>,
    pub memory_context_items_for_task: Vec<MemoryItemProjection>,
    pub app_handle_for_after_turn: AppHandle,
    pub pinned_store_for_after_turn: Arc<dyn PinnedStore>,
    pub memory_provider_for_after_turn: SharedMemoryProvider,
    pub active_retrieval_manager_for_after_turn: Option<Arc<ActiveRetrievalManager>>,
    pub stream_session_id_for_after_turn: String,
    pub stream_project_id_for_after_turn: Option<String>,
    pub harness_bus_for_after_turn: Option<EventBus>,
}

/// Run the spawned tool-loop body for one streaming chat turn.
///
/// This function takes ownership of every captured value via
/// [`StreamTaskInputs`] so the caller can `tokio::spawn(...)` it
/// directly without an intermediate `async move` block.
///
/// MIG-001-d preserves behaviour bit-for-bit; the only structural
/// change vs the previous inline closure is the destructuring at
/// the top of the function and the location.
pub(super) async fn run_stream_task(inputs: StreamTaskInputs) {
    let StreamTaskInputs {
        stream_id_for_task,
        session_id,
        user_message_clone,
        permission_mode_for_stream,
        inbound_resume_cursor,
        turn_number_for_stream,
        baseline_message_count_stream,
        messages_for_stream,
        tool_defs_for_stream,
        system_prompt_for_stream,
        provider_client_for_stream,
        model_for_stream,
        execution_context_for_task,
        tool_registry_clone,
        session_manager,
        app_session_clone,
        stream_emitter,
        mut cancel_rx,
        permission_senders,
        permission_overrides,
        trajectory_manager_for_stream,
        learning_module_for_stream,
        memory_provider_for_stream,
        memory_ticker_for_stream,
        harness_event_bus_for_stream,
        memory_context_items_for_task,
        app_handle_for_after_turn,
        pinned_store_for_after_turn,
        memory_provider_for_after_turn,
        active_retrieval_manager_for_after_turn,
        stream_session_id_for_after_turn,
        stream_project_id_for_after_turn,
        harness_bus_for_after_turn,
    } = inputs;

    tracing::info!(
        "[start_agent_stream] Spawned background task for stream_id: {}",
        stream_id_for_task
    );

    let max_iterations: usize = 10;
    let mut tool_loop_iter: usize = 0;
    let mut session_messages = messages_for_stream.clone();
    let mut accumulated_text = String::new();
    let mut accumulated_thinking = String::new();
    let mut token_count: u32 = 0;
    let mut stream_failed = false;
    let mut completion_already_emitted = false;
    let mut has_successful_tool = false;
    let mut has_successful_mutating_tool = false;
    let mut terminal_status: Option<&'static str> = None;

    // Phase 6E harness: emit TurnStarted at the top of the spawned task
    // so all timing measurements include API client setup time.
    crate::modules::harness::agent_loop_integration::emit_turn_started(
        harness_event_bus_for_stream.as_ref(),
        &session_id,
        turn_number_for_stream,
    );
    let stream_turn_started_at = std::time::Instant::now();
    let mut last_stream_error_reason: Option<String> = None;
    let mut sanitize_rounds = 0usize;
    let mut sanitized_dropped_empty_messages = 0usize;
    let mut sanitized_dropped_orphan_tool_results = 0usize;
    let mut sanitized_dropped_unmatched_tool_uses = 0usize;
    let mut sanitized_dropped_invalid_tool_use_inputs = 0usize;
    let mut sanitize_orphan_samples: Vec<String> = Vec::new();
    let mut sanitize_unmatched_samples: Vec<String> = Vec::new();
    let mut sanitize_invalid_tool_use_samples: Vec<String> = Vec::new();
    let mut preflight_trim_rounds = 0usize;
    let mut preflight_dropped_messages_total = 0usize;
    let mut preflight_trimmed_chars_total = 0usize;
    let mut stream_start_retry_count = 0usize;
    let mut stream_event_retry_count = 0usize;
    let mut provider_request_id = format!("stream_{}", stream_id_for_task);
    let is_resume_turn = inbound_resume_cursor.is_some();
    let mode = parse_permission_mode(permission_mode_for_stream.as_deref());
    // Phase M4-C P2 — wrap in `Arc` so the harness
    // `prepare_step_execution` shadow trace can borrow the
    // same policy without re-constructing it (re-construction
    // would lose any per-tool requirements set on the
    // original policy).
    let permission_policy = std::sync::Arc::new(build_permission_policy(mode));
    let execution_context = SessionExecutionContext::new(
        execution_context_for_task.session_id.clone(),
        execution_context_for_task.project_id.clone(),
        execution_context_for_task.workdir.clone(),
        mode,
    );
    let execution_context_for_policy = execution_context.clone();
    log_context_fingerprint("start_agent_stream_task", &execution_context);
    let mut tool_executor =
        ToolRegistryExecutor::new_with_context(tool_registry_clone.clone(), execution_context);
    const SAVE_INTERVAL: u32 = 50;
    // Session-format timeline messages (for persistence in chronological order)
    let mut timeline_session_messages: Vec<crate::modules::runtime::session::ConversationMessage> =
        Vec::new();

    loop {
        // Check for cancellation at the start of each iteration
        if cancel_rx.try_recv().is_ok() {
            tracing::info!("[start_agent_stream] Stream cancelled at loop iteration");
            let cancelled_truth = TaskOutcomeResolver::resolve(
                ExecutionTruth {
                    has_successful_tool,
                    has_successful_mutating_tool,
                },
                &ConversationTruth {
                    stream_failed: true,
                    terminal_status: "cancelled_by_user",
                    last_stream_error_reason: Some("cancelled_by_user".to_string()),
                },
            );
            let payload = StreamTokenPayload {
                stream_id: stream_id_for_task.clone(),
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
                request_id: Some(provider_request_id.clone()),
                task_outcome: Some(cancelled_truth.task_outcome.to_string()),
                degraded_reason: cancelled_truth.degraded_reason,
                resume_available: Some(cancelled_truth.resume_available),
                resume_cursor: None,
                context_budget_usage: None,
                memory_context: None,
            };
            stream_emitter.emit_payload(payload);
            completion_already_emitted = true;
            terminal_status = Some("cancelled_by_user");
            break;
        }

        if tool_loop_iter >= max_iterations {
            tracing::warn!(
                "[start_agent_stream] Tool loop exceeded max_iterations={}",
                max_iterations
            );
            terminal_status = Some("max_iterations_reached");
            break;
        }
        tool_loop_iter += 1;

        tracing::info!(
            "[start_agent_stream] === Outer loop iteration {} start. session_messages len={}, accumulated_text len={}",
            tool_loop_iter,
            session_messages.len(),
            accumulated_text.len()
        );

        // Build API request for this iteration
        let (trimmed_session_messages, preflight_stats) = ContextGovernor.admit(
            &session_messages,
            MAX_REQUEST_MESSAGE_COUNT,
            MAX_REQUEST_CHAR_BUDGET,
            MAX_REQUEST_TOKEN_BUDGET_ESTIMATE,
        );
        if preflight_stats.has_changes() {
            preflight_trim_rounds += 1;
            preflight_dropped_messages_total += preflight_stats.dropped_messages;
            preflight_trimmed_chars_total += preflight_stats.trimmed_chars;
            tracing::warn!(
                "[start_agent_stream] preflight request trim: stream_id={}, session_id={}, before_messages={}, after_messages={}, before_chars={}, after_chars={}, dropped_messages={}, trimmed_chars={}",
                stream_id_for_task,
                session_id,
                preflight_stats.before_messages,
                preflight_stats.after_messages,
                preflight_stats.before_chars,
                preflight_stats.after_chars,
                preflight_stats.dropped_messages,
                preflight_stats.trimmed_chars,
            );
        }
        let (sanitized_session_messages, sanitize_stats) =
            sanitize_messages_for_provider(&trimmed_session_messages);
        if sanitize_stats.has_changes() {
            sanitize_rounds += 1;
            sanitized_dropped_empty_messages += sanitize_stats.dropped_empty_messages;
            sanitized_dropped_orphan_tool_results += sanitize_stats.dropped_orphan_tool_results;
            sanitized_dropped_unmatched_tool_uses += sanitize_stats.dropped_unmatched_tool_uses;
            sanitized_dropped_invalid_tool_use_inputs +=
                sanitize_stats.dropped_invalid_tool_use_inputs;
            extend_sample_ids(
                &mut sanitize_orphan_samples,
                &sanitize_stats.orphan_tool_result_ids,
                12,
            );
            extend_sample_ids(
                &mut sanitize_unmatched_samples,
                &sanitize_stats.unmatched_tool_use_ids,
                12,
            );
            extend_sample_ids(
                &mut sanitize_invalid_tool_use_samples,
                &sanitize_stats.invalid_tool_use_input_ids,
                12,
            );
            tracing::warn!(
                "[start_agent_stream] sanitized malformed tool history before request: stream_id={}, session_id={}, before_messages={}, after_messages={}, dropped_empty_messages={}, dropped_orphan_tool_results={}, dropped_unmatched_tool_uses={}, dropped_invalid_tool_use_inputs={}, orphan_tool_result_ids={:?}, unmatched_tool_use_ids={:?}, invalid_tool_use_input_ids={:?}",
                stream_id_for_task,
                session_id,
                session_messages.len(),
                sanitized_session_messages.len(),
                sanitize_stats.dropped_empty_messages,
                sanitize_stats.dropped_orphan_tool_results,
                sanitize_stats.dropped_unmatched_tool_uses,
                sanitize_stats.dropped_invalid_tool_use_inputs,
                sanitize_stats.orphan_tool_result_ids,
                sanitize_stats.unmatched_tool_use_ids,
                sanitize_stats.invalid_tool_use_input_ids,
            );
        }
        let mut request_messages = sanitized_session_messages.clone();
        if preflight_stats.has_changes() || sanitize_stats.has_changes() {
            request_messages.insert(
                0,
                InputMessage::user_text(format!(
                    "[context_trim_notice] dropped_messages={}, dropped_empty_messages={}, dropped_orphan_tool_results={}, dropped_unmatched_tool_uses={}, dropped_invalid_tool_use_inputs={}",
                    preflight_stats.dropped_messages,
                    sanitize_stats.dropped_empty_messages,
                    sanitize_stats.dropped_orphan_tool_results,
                    sanitize_stats.dropped_unmatched_tool_uses,
                    sanitize_stats.dropped_invalid_tool_use_inputs
                )),
            );
        }
        let (final_request_messages, final_preflight_stats) = ContextGovernor.admit(
            &request_messages,
            MAX_REQUEST_MESSAGE_COUNT,
            MAX_REQUEST_CHAR_BUDGET,
            MAX_REQUEST_TOKEN_BUDGET_ESTIMATE,
        );
        if final_preflight_stats.has_changes() {
            preflight_trim_rounds += 1;
            preflight_dropped_messages_total += final_preflight_stats.dropped_messages;
            preflight_trimmed_chars_total += final_preflight_stats.trimmed_chars;
            tracing::warn!(
                "[start_agent_stream] final preflight trim: stream_id={}, session_id={}, before_messages={}, after_messages={}, before_chars={}, after_chars={}, dropped_messages={}, trimmed_chars={}",
                stream_id_for_task,
                session_id,
                final_preflight_stats.before_messages,
                final_preflight_stats.after_messages,
                final_preflight_stats.before_chars,
                final_preflight_stats.after_chars,
                final_preflight_stats.dropped_messages,
                final_preflight_stats.trimmed_chars,
            );
        }
        session_messages = final_request_messages;
        let iter_api_request = MessageRequest {
            model: model_for_stream.clone(),
            max_tokens: 4096,
            messages: session_messages.clone(),
            system: if system_prompt_for_stream.is_empty() {
                None
            } else {
                Some(system_prompt_for_stream.clone())
            },
            tools: if tool_defs_for_stream.is_empty() {
                None
            } else {
                Some(tool_defs_for_stream.clone())
            },
            tool_choice: None,
            stream: true,
        };

        let mut stream = match provider_client_for_stream
            .stream_message(&iter_api_request)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                let stream_error_reason = format_stream_error_reason(&e);
                if is_network_timeout_reason(&stream_error_reason)
                    && stream_start_retry_count < MAX_STREAM_RETRY_ON_TIMEOUT
                {
                    stream_start_retry_count += 1;
                    tracing::warn!(
                        "[start_agent_stream] start-stream timeout, scheduling retry: stream_id={}, session_id={}, attempt={}/{}, reason={}",
                        stream_id_for_task,
                        session_id,
                        stream_start_retry_count,
                        MAX_STREAM_RETRY_ON_TIMEOUT,
                        stream_error_reason
                    );
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    continue;
                }
                stream_failed = true;
                last_stream_error_reason = Some(stream_error_reason.clone());
                terminal_status = Some("failed_to_start_stream");
                tracing::error!(
                    "[start_agent_stream] Background task failed to start stream: {}",
                    stream_error_reason
                );
                let user_visible_truth = TaskOutcomeResolver::resolve(
                    ExecutionTruth {
                        has_successful_tool,
                        has_successful_mutating_tool,
                    },
                    &ConversationTruth {
                        stream_failed: true,
                        terminal_status: "failed_to_start_stream",
                        last_stream_error_reason: Some(stream_error_reason.clone()),
                    },
                );
                let resume_cursor = user_visible_truth
                    .resume_available
                    .then(|| build_resume_cursor(&stream_id_for_task, tool_loop_iter, token_count));
                let degraded_reason = user_visible_truth.degraded_reason.clone();
                let payload = StreamTokenPayload {
                    stream_id: stream_id_for_task.clone(),
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
                    context_budget_usage: None,
                    memory_context: None,
                };
                stream_emitter.emit_payload(payload);
                // Phase M4-C P5 — emit harness `StreamErrored`
                // event so the trace aggregator records the
                // hard error against the run report.
                if let Some(bus) = harness_event_bus_for_stream.as_ref() {
                    let _ = bus.emit(AgentEvent::StreamErrored {
                        session_id: session_id.clone(),
                        reason: last_stream_error_reason
                            .clone()
                            .unwrap_or_else(|| "unknown_stream_error".to_string()),
                        resume_available: user_visible_truth.resume_available,
                        at: chrono::Utc::now(),
                    });
                }
                // Save session and emit stream_complete even on error
                // (break from outer loop so cleanup code runs below)
                break;
            }
        };
        if let Some(request_id) = stream
            .request_id()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
        {
            provider_request_id = request_id.clone();
            tracing::info!(
                "[start_agent_stream] provider request_id captured: stream_id={}, session_id={}, request_id={}",
                stream_id_for_task,
                session_id,
                request_id
            );
        }

        // Tool call tracking for this iteration — uses block index to support
        // parallel tool calls (each tool_call has its own index in the stream).
        let mut tool_arguments: HashMap<String, String> = HashMap::new();
        let mut index_to_tool_id: HashMap<u32, String> = HashMap::new();
        let mut index_to_tool_name: HashMap<u32, String> = HashMap::new();
        let mut pending_tool_uses: Vec<(String, String, String)> = Vec::new();
        let mut retry_outer_after_timeout = false;
        let mut emitted_stream_delta_in_iteration = false;

        loop {
            match stream.next_event().await {
                Ok(Some(event)) => match event {
                    ApiStreamEvent::ContentBlockDelta(delta_event) => match delta_event.delta {
                        crate::modules::api::ContentBlockDelta::TextDelta { text } => {
                            accumulated_text.push_str(&text);
                            emitted_stream_delta_in_iteration = true;
                            token_count += 1;
                            // Log every 5 text deltas to track streaming progress
                            if token_count.is_multiple_of(5) {
                                tracing::info!(
                                    "[start_agent_stream] text_delta: +{} chars, accumulated {} total",
                                    text.len(),
                                    accumulated_text.len()
                                );
                            }
                            if token_count.is_multiple_of(SAVE_INTERVAL) {
                                let mut interim_session = app_session_clone.clone();
                                interim_session.messages.push(
                                    crate::modules::runtime::session::ConversationMessage {
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
                                    },
                                );
                                let _ = session_manager.save_session(&interim_session).await;
                                tracing::debug!(
                                    "[start_agent_stream] Periodic session save at token {}",
                                    token_count
                                );
                            }
                            let payload = StreamTokenPayload {
                                stream_id: stream_id_for_task.clone(),
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
                                context_budget_usage: None,
                                memory_context: None,
                            };
                            stream_emitter.emit_payload(payload);
                        }
                        crate::modules::api::ContentBlockDelta::ThinkingDelta { thinking } => {
                            accumulated_thinking.push_str(&thinking);
                            emitted_stream_delta_in_iteration = true;
                            let payload = StreamTokenPayload {
                                stream_id: stream_id_for_task.clone(),
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
                                context_budget_usage: None,
                                memory_context: None,
                            };
                            stream_emitter.emit_payload(payload);
                        }
                        crate::modules::api::ContentBlockDelta::SignatureDelta { .. } => {}
                        crate::modules::api::ContentBlockDelta::InputJsonDelta { partial_json } => {
                            // Route delta to the correct tool_call via block index.
                            if let Some(tool_id) = index_to_tool_id.get(&delta_event.index).cloned()
                            {
                                tool_arguments
                                    .entry(tool_id)
                                    .or_default()
                                    .push_str(&partial_json);
                            }
                        }
                    },
                    ApiStreamEvent::ContentBlockStop(stop_event) => {
                        // Extract completed tool_call as its block ends
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
                        // Extract any remaining tools (fallback — should already
                        // have been caught by ContentBlockStop above)
                        for (index, tool_id) in index_to_tool_id.drain() {
                            let tool_name = index_to_tool_name.remove(&index).unwrap_or_default();
                            if let Some(input_json) = tool_arguments.remove(&tool_id) {
                                pending_tool_uses.push((tool_id, tool_name, input_json));
                            }
                        }

                        tracing::info!(
                            "[start_agent_stream] MessageStop received, {} pending tool uses",
                            pending_tool_uses.len()
                        );
                        break;
                    }
                    ApiStreamEvent::ContentBlockStart(start_event) => {
                        match start_event.content_block {
                            crate::modules::api::OutputContentBlock::Thinking { .. } => {
                                let payload = StreamTokenPayload {
                                    stream_id: stream_id_for_task.clone(),
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
                                    context_budget_usage: None,
                                    memory_context: None,
                                };
                                stream_emitter.emit_payload(payload);
                            }
                            crate::modules::api::OutputContentBlock::ToolUse {
                                id, name, ..
                            } => {
                                // Track by block index to support parallel tool calls
                                index_to_tool_id.insert(start_event.index, id.clone());
                                index_to_tool_name.insert(start_event.index, name.clone());
                                let payload = StreamTokenPayload {
                                    stream_id: stream_id_for_task.clone(),
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
                                    context_budget_usage: None,
                                    memory_context: None,
                                };
                                stream_emitter.emit_payload(payload);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                },
                Ok(None) => {
                    // Stream ended without MessageStop - extract any remaining tools
                    for (index, tool_id) in index_to_tool_id.drain() {
                        let tool_name = index_to_tool_name.remove(&index).unwrap_or_default();
                        if let Some(input_json) = tool_arguments.remove(&tool_id) {
                            pending_tool_uses.push((tool_id, tool_name, input_json));
                        }
                    }
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
                            stream_id_for_task,
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
                    let resume_cursor = user_visible_truth.resume_available.then(|| {
                        build_resume_cursor(&stream_id_for_task, tool_loop_iter, token_count)
                    });
                    let degraded_reason = user_visible_truth.degraded_reason.clone();

                    // Force-settle any in-flight tool cards so frontend does not
                    // keep them in queued/running after stream failure.
                    for (index, tool_id) in index_to_tool_id.drain() {
                        let tool_name = index_to_tool_name
                            .remove(&index)
                            .unwrap_or_else(|| "unknown".to_string());
                        let payload = StreamTokenPayload {
                            stream_id: stream_id_for_task.clone(),
                            text: None,
                            thinking: None,
                            event_type: "tool_call_update".to_string(),
                            tool_call_id: Some(tool_id.clone()),
                            tool_name: Some(tool_name),
                            tool_status: Some("error".to_string()),
                            tool_args: None,
                            tool_result: Some(stream_error_reason.clone()),
                            tool_duration_ms: None,
                            effective_workdir: Some(
                                execution_context_for_policy.workdir.display().to_string(),
                            ),
                            policy_decision: None,
                            evidence_id: Some(tool_id),
                            request_id: Some(provider_request_id.clone()),
                            task_outcome: Some(user_visible_truth.task_outcome.to_string()),
                            degraded_reason: degraded_reason.clone(),
                            resume_available: Some(user_visible_truth.resume_available),
                            resume_cursor: resume_cursor.clone(),
                            context_budget_usage: None,
                            memory_context: None,
                        };
                        stream_emitter.emit_payload(payload);
                    }

                    let payload = StreamTokenPayload {
                        stream_id: stream_id_for_task.clone(),
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
                        context_budget_usage: None,
                        memory_context: None,
                    };
                    stream_emitter.emit_payload(payload);
                    // Phase M4-C P5 — emit harness `StreamErrored`
                    // event from the inner-loop error path too.
                    if let Some(bus) = harness_event_bus_for_stream.as_ref() {
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

        if retry_outer_after_timeout {
            tokio::time::sleep(Duration::from_millis(350)).await;
            continue;
        }

        // If no tool calls, exit the outer loop
        if pending_tool_uses.is_empty() {
            tracing::info!(
                "[start_agent_stream] No pending tool uses, breaking outer loop. accumulated_text len={}",
                accumulated_text.len()
            );
            if terminal_status.is_none() {
                terminal_status = Some("model_stop_no_tools");
            }
            break;
        }

        tracing::info!(
            "[start_agent_stream] Executing {} tools, accumulated_text so far: {} chars",
            pending_tool_uses.len(),
            accumulated_text.len()
        );

        // Persist the assistant segment that led to these tool calls before
        // the tool results so reload preserves chronological order.
        flush_assistant_timeline_segment(
            &mut timeline_session_messages,
            &mut accumulated_text,
            &mut accumulated_thinking,
            None,
        );

        // Execute each tool and append results to session_messages

        // Set up TauriPermissionPrompter for interactive permission requests
        let (perm_tx, perm_rx): (
            std::sync::mpsc::Sender<PermissionPromptDecision>,
            std::sync::mpsc::Receiver<PermissionPromptDecision>,
        ) = std::sync::mpsc::channel();
        match permission_senders.lock() {
            Ok(mut senders) => {
                senders.insert(session_id.clone(), perm_tx);
            }
            Err(e) => {
                tracing::error!(
                    "[start_agent_stream] failed to lock permission_senders: {}",
                    e
                );
            }
        }
        let mut prompter = TauriPermissionPrompter::new(
            stream_emitter.window().clone(),
            session_id.clone(),
            perm_rx,
        );

        for (tool_id, tool_name, input_json) in pending_tool_uses.drain(..) {
            let policy_trace_id = AuditEmitter::new_trace_id();
            let diag_key = format!(
                "stream_id={};trace_id={};request_id={}",
                stream_id_for_task, policy_trace_id, provider_request_id
            );
            tracing::info!(
                "[stream_audit_link] diag_key={}, stream_id={}, session_id={}, tool_call_id={}, trace_id={}, request_id={}, tool_name={}",
                diag_key,
                stream_id_for_task,
                session_id,
                tool_id,
                policy_trace_id,
                provider_request_id.as_str(),
                tool_name
            );
            // Emit running event
            stream_emitter.emit_payload(StreamTokenPayload {
                stream_id: stream_id_for_task.clone(),
                text: None,
                thinking: None,
                event_type: "tool_call_update".to_string(),
                tool_call_id: Some(tool_id.clone()),
                tool_name: Some(tool_name.clone()),
                tool_status: Some("running".to_string()),
                tool_args: None,
                tool_result: None,
                tool_duration_ms: None,
                effective_workdir: Some(execution_context_for_policy.workdir.display().to_string()),
                policy_decision: Some("prompt".to_string()),
                evidence_id: Some(policy_trace_id.clone()),
                request_id: Some(provider_request_id.clone()),
                task_outcome: None,
                degraded_reason: None,
                resume_available: None,
                resume_cursor: None,
                context_budget_usage: None,
                memory_context: None,
            });

            // Permission check: apply session-scoped remember decisions first.
            let remembered_decision = permission_overrides
                .lock()
                .ok()
                .and_then(|all| all.get(&session_id).cloned())
                .and_then(|tool_map| {
                    tool_map
                        .get(&tool_name)
                        .cloned()
                        .or_else(|| tool_map.get("*").cloned())
                });
            let permission_outcome = match remembered_decision {
                Some(PermissionPromptDecision::Allow) => {
                    crate::modules::runtime::permissions::PermissionOutcome::Allow
                }
                Some(PermissionPromptDecision::Deny { reason }) => {
                    crate::modules::runtime::permissions::PermissionOutcome::Deny { reason }
                }
                None => permission_policy.authorize(&tool_name, &input_json, Some(&mut prompter)),
            };
            match &permission_outcome {
                crate::modules::runtime::permissions::PermissionOutcome::Allow => {
                    AuditEmitter::policy_decision_made(
                        &policy_trace_id,
                        &execution_context_for_policy.session_id,
                        &tool_name,
                        &execution_context_for_policy.workdir,
                        mode,
                        "allow",
                        Some(provider_request_id.as_str()),
                    );
                }
                crate::modules::runtime::permissions::PermissionOutcome::Deny { reason } => {
                    AuditEmitter::policy_decision_made(
                        &policy_trace_id,
                        &execution_context_for_policy.session_id,
                        &tool_name,
                        &execution_context_for_policy.workdir,
                        mode,
                        &format!("deny:{reason}"),
                        Some(provider_request_id.as_str()),
                    );
                }
            }
            if let crate::modules::runtime::permissions::PermissionOutcome::Deny { reason } =
                &permission_outcome
            {
                tracing::warn!(
                    "[start_agent_stream] Permission denied for tool '{}' in mode {}: {}",
                    tool_name,
                    mode.as_str(),
                    reason
                );
            }

            let tool_input = parse_tool_input_json(&input_json);

            timeline_session_messages.push(
                crate::modules::runtime::session::ConversationMessage::tool_use(
                    tool_id.clone(),
                    tool_name.clone(),
                    input_json.clone(),
                ),
            );

            let start_time = std::time::Instant::now();
            let denied_by_policy = matches!(
                permission_outcome,
                crate::modules::runtime::permissions::PermissionOutcome::Deny { .. }
            );
            // Phase 6E harness: emit ToolCalled before invocation.
            crate::modules::harness::agent_loop_integration::emit_tool_called(
                harness_event_bus_for_stream.as_ref(),
                &session_id,
                &tool_name,
                &input_json.to_string(),
            );
            // Phase M4-C P2 — emit harness `PrepareStepExecuted`
            // shadow trace.  Calls the typed
            // `prepare_step_execution` seam with the actual
            // tool args + policy and records what the seam
            // would decide.  Production dispatch still goes
            // through the existing `permission_policy.authorize`
            // path above; this trace is for governance only
            // (no enforcement until M4.8 gate work).
            if let Some(bus) = harness_event_bus_for_stream.as_ref() {
                let parsed_args = parse_tool_input_json(&input_json);
                let prep_out = crate::modules::control_plane::prepare_step_execution::prepare_step_execution(
                    crate::modules::control_plane::prepare_step_execution::PrepareStepExecutionInput {
                        tool_name: &tool_name,
                        session_context: &execution_context_for_policy,
                        args: &parsed_args,
                        permission_policy: permission_policy.clone(),
                    },
                );
                let _ = bus.emit(AgentEvent::PrepareStepExecuted {
                    session_id: session_id.clone(),
                    tool_name: tool_name.clone(),
                    outcome: prep_out.outcome,
                    boundary: prep_out.boundary_decision,
                    permission: prep_out.permission_decision,
                    sandbox: prep_out.sandbox_policy,
                    policy_version: prep_out.policy_version,
                    at: chrono::Utc::now(),
                });
            }
            let (result_text, is_error) = match permission_outcome {
                crate::modules::runtime::permissions::PermissionOutcome::Allow => {
                    match tool_executor.execute_with_trace(
                        &tool_name,
                        &input_json,
                        &policy_trace_id,
                        Some(provider_request_id.as_str()),
                    ) {
                        Ok(output) => (output, false),
                        Err(e) => (e.to_string(), true),
                    }
                }
                crate::modules::runtime::permissions::PermissionOutcome::Deny { reason } => {
                    (reason, true)
                }
            };
            let policy_decision = if denied_by_policy { "deny" } else { "allow" };
            let duration_ms = start_time.elapsed().as_millis() as u64;
            if !is_error {
                has_successful_tool = true;
            }
            // Phase 6E harness: emit ToolResult after invocation.
            crate::modules::harness::agent_loop_integration::emit_tool_result(
                harness_event_bus_for_stream.as_ref(),
                &session_id,
                &tool_name,
                !is_error,
                duration_ms,
            );
            if is_mutating_tool_success(&tool_name, &input_json, is_error) {
                has_successful_mutating_tool = true;
            }

            // Emit completed/error event
            stream_emitter.emit_payload(StreamTokenPayload {
                stream_id: stream_id_for_task.clone(),
                text: None,
                thinking: None,
                event_type: "tool_call_update".to_string(),
                tool_call_id: Some(tool_id.clone()),
                tool_name: Some(tool_name.clone()),
                tool_status: Some(if is_error { "error" } else { "completed" }.to_string()),
                tool_args: None,
                tool_result: Some(result_text.clone()),
                tool_duration_ms: Some(duration_ms),
                effective_workdir: Some(execution_context_for_policy.workdir.display().to_string()),
                policy_decision: Some(policy_decision.to_string()),
                evidence_id: Some(policy_trace_id.clone()),
                request_id: Some(provider_request_id.clone()),
                task_outcome: None,
                degraded_reason: None,
                resume_available: None,
                resume_cursor: None,
                context_budget_usage: None,
                memory_context: None,
            });

            // Append tool_use as assistant message, then tool_result as user message.
            // MiniMax requires this pairing: assistant tool_use + user tool_result.
            session_messages.push(crate::modules::api::InputMessage {
                role: "assistant".to_string(),
                content: vec![crate::modules::api::InputContentBlock::ToolUse {
                    id: tool_id.clone(),
                    name: tool_name.clone(),
                    input: tool_input,
                }],
            });
            session_messages.push(crate::modules::api::InputMessage {
                role: "user".to_string(),
                content: vec![crate::modules::api::InputContentBlock::ToolResult {
                    tool_use_id: tool_id.clone(),
                    content: vec![crate::modules::api::ToolResultContentBlock::Text {
                        text: summarize_tool_result_for_model(
                            &tool_name,
                            &tool_id,
                            &result_text,
                            is_error,
                        ),
                    }],
                    is_error,
                }],
            });

            // Also collect session-format message for persistence in order.
            timeline_session_messages.push(
                crate::modules::runtime::session::ConversationMessage::tool_result(
                    tool_id,
                    tool_name,
                    result_text,
                    is_error,
                ),
            );
            if let Some(last_message) = timeline_session_messages.last_mut() {
                last_message.request_id = Some(provider_request_id.clone());
            }
        }
        // Continue outer loop → send next LLM request with tool results
        tracing::info!(
            "[start_agent_stream] Tool execution done, continuing outer loop. session_messages len={}",
            session_messages.len()
        );
    }

    super::stream_finalize::finalize_stream_task(super::stream_finalize::FinalizeStreamInputs {
        session_id,
        stream_id_for_task,
        provider_request_id,
        turn_number_for_stream,
        stream_turn_started_at,
        user_message_clone,
        inbound_resume_cursor,
        is_resume_turn,
        accumulated_text,
        accumulated_thinking,
        has_successful_tool,
        has_successful_mutating_tool,
        stream_failed,
        completion_already_emitted,
        terminal_status,
        last_stream_error_reason,
        tool_loop_iter,
        token_count,
        timeline_session_messages,
        session_messages,
        system_prompt_for_stream,
        preflight_trim_rounds,
        preflight_dropped_messages_total,
        preflight_trimmed_chars_total,
        sanitize_rounds,
        sanitized_dropped_empty_messages,
        sanitized_dropped_orphan_tool_results,
        sanitized_dropped_unmatched_tool_uses,
        sanitized_dropped_invalid_tool_use_inputs,
        stream_start_retry_count,
        stream_event_retry_count,
        sanitize_orphan_samples,
        sanitize_unmatched_samples,
        sanitize_invalid_tool_use_samples,
        stream_emitter,
        session_manager,
        app_session_clone,
        trajectory_manager_for_stream,
        memory_provider_for_stream,
        memory_ticker_for_stream,
        harness_event_bus_for_stream,
        learning_module_for_stream,
        memory_context_items_for_task,
        baseline_message_count_stream,
        app_handle_for_after_turn,
        harness_bus_for_after_turn,
        pinned_store_for_after_turn,
        memory_provider_for_after_turn,
        active_retrieval_manager_for_after_turn,
        stream_session_id_for_after_turn,
        stream_project_id_for_after_turn,
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MIG-001-d structural smoke test: the extracted task helper
    /// is a `Send + Sync` async function whose returned future is
    /// `Send`, which is the contract `tokio::spawn` consumes.
    /// This guards future regressions where adding a `!Send`
    /// type to `StreamTaskInputs` would silently break the
    /// streaming spawn.
    #[test]
    fn run_stream_task_is_spawnable() {
        // The `StreamTaskInputs` struct itself MUST be `Send` for
        // `tokio::spawn` to consume the future returned by
        // `run_stream_task(inputs)`. This compile-time guard
        // catches future regressions where a `!Send` field would
        // silently break the streaming spawn.
        fn assert_inputs_send<T: Send>() {}
        assert_inputs_send::<StreamTaskInputs>();
        // Symbol-existence check: `run_stream_task` is a fn item
        // with the bundled-inputs signature. If a future change
        // renames or re-splits the helper, this fails at compile
        // time so callers update in lockstep.
        let _: fn(StreamTaskInputs) -> _ = run_stream_task;
    }
}
