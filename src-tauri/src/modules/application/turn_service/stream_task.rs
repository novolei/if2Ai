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

use tauri::{AppHandle, Manager};
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
use crate::modules::runtime::contracts::agent_loop::PendingOperationMetadata;
use crate::modules::runtime::contracts::agent_loop::{SkillResolutionPlan, WorkLoopDecision};
use crate::modules::runtime::contracts::prompt::PromptDiagnosticsSummary;
use crate::modules::runtime::event_log::RunEventLogger;
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

#[allow(deprecated)]
use super::agent_loop_delegate::{
    AgentLoopDelegate, AgentLoopDelegateInput, AgentLoopDelegateOutput, AgentLoopTerminalState,
    StreamingAgentLoopDelegate,
};
use super::stream_loop_state;

// MAX_REQUEST_* / MAX_STREAM_RETRY_ON_TIMEOUT moved to
// `crate::modules::runtime::budget` so the canonical preflight /
// streaming budget constants live next to ContextBudget.
// Imported above.

const DEFAULT_MAX_TOOL_LOOP_ITERATIONS: usize = 10;
const MIN_MAX_TOOL_LOOP_ITERATIONS: usize = 3;
const REPEATED_TOOL_BATCH_LIMIT: usize = 3;
const INVALID_TOOL_ARGS_LIMIT: usize = 2;

fn agent_max_iterations() -> usize {
    std::env::var("IF2AI_AGENT_MAX_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.max(MIN_MAX_TOOL_LOOP_ITERATIONS))
        .unwrap_or(DEFAULT_MAX_TOOL_LOOP_ITERATIONS)
}

fn tool_batch_signature(pending_tool_uses: &[(String, String, String)]) -> String {
    let mut parts = pending_tool_uses
        .iter()
        .map(|(_, tool_name, input_json)| {
            let compact_input = input_json.split_whitespace().collect::<Vec<_>>().join(" ");
            let compact_input = if compact_input.chars().count() > 240 {
                format!("{}...", compact_input.chars().take(240).collect::<String>())
            } else {
                compact_input
            };
            format!("{tool_name}:{compact_input}")
        })
        .collect::<Vec<_>>();
    parts.sort();
    parts.join("|")
}

/// All state captured by the original
/// `tokio::spawn(async move { ... })` closure inside
/// `stream_turn`. Bundled into a struct so the spawn call site
/// stays one line and so individual fields can be added / removed
/// without churn at the spawn boundary.
pub struct StreamTaskInputs {
    pub stream_id_for_task: String,
    pub run_event_logger: RunEventLogger,
    pub session_id: String,
    pub user_message_clone: String,
    pub permission_mode_for_stream: Option<String>,
    pub inbound_resume_cursor: Option<String>,
    pub turn_number_for_stream: u64,
    pub baseline_message_count_stream: usize,
    pub messages_for_stream: Vec<InputMessage>,
    pub tool_defs_for_stream: Vec<ToolDefinition>,
    pub tool_pool_names_for_stream: Vec<String>,
    pub tool_pool_schema_hash_for_stream: String,
    pub tool_pool_policy_for_stream: String,
    pub system_prompt_for_stream: String,
    pub provider_client_for_stream: crate::modules::api::ProviderClient,
    /// Optional OpenAI-compatible failover when `IF2AI_FAILOVER_BASE_URL` is set.
    pub failover_provider_client: Option<crate::modules::api::ProviderClient>,
    pub lifecycle_hooks: std::sync::Arc<crate::modules::runtime::lifecycle_hooks::HookRegistry>,
    pub model_for_stream: String,
    /// Provider id paired with [`model_for_stream`].  Forwarded into
    /// the usage store so the per-role chart can break chat usage down
    /// per vendor.
    pub provider_id_for_stream: String,
    /// Provider-advertised context window in tokens (post smart-routing).
    /// Forwarded into preflight admission + final ContextBudget so the
    /// UI bar reflects the actual model limit instead of a hardcoded
    /// 30k baseline.
    pub context_window_for_stream: u64,
    /// P1-8: smart-routing decision summary already computed in
    /// `turn_service` (see `apply_complexity_model_routing`). When `Some`
    /// it is forwarded into `stream_complete` so the chat UI can render a
    /// per-message routing chip.
    pub routing_info_for_stream:
        Option<crate::modules::runtime::stream_emitter::RoutingInfoPayload>,
    /// Internal work-loop decision emitted for run visibility and final report.
    pub work_loop_decision_for_stream: WorkLoopDecision,
    /// Skill-resolution snapshot emitted for run visibility.
    pub skill_resolution_plan_for_stream: SkillResolutionPlan,
    pub execution_context_for_task: SessionExecutionContext,
    pub tool_registry_clone: Arc<ToolRegistry>,
    pub session_manager: Arc<SessionManager>,
    pub rolling_summarizer_for_stream: Arc<crate::modules::memory::summary::RollingSummarizer>,
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
    pub prompt_diagnostics_for_task: PromptDiagnosticsSummary,
    pub prompt_diagnostics_enabled_for_task: bool,
    pub app_handle_for_after_turn: AppHandle,
    pub pinned_store_for_after_turn: Arc<dyn PinnedStore>,
    pub memory_provider_for_after_turn: SharedMemoryProvider,
    pub active_retrieval_manager_for_after_turn: Option<Arc<ActiveRetrievalManager>>,
    pub stream_session_id_for_after_turn: String,
    pub stream_project_id_for_after_turn: Option<String>,
    pub harness_bus_for_after_turn: Option<EventBus>,
    /// DW-002 (truth-loop iter-7) — shared utility LLM reused by the
    /// preflight digester at each outer-loop iteration.  Plumbed from
    /// `TurnServiceDeps::utility_llm` (which in production is the same
    /// `ChatProviderUtilityLlm` instance bootstrapped in
    /// `bootstrap/memory.rs`).  Removing this field would force the
    /// digester to allocate a fresh provider shim per iteration —
    /// behaviourally equivalent today but masks ownership and makes
    /// future per-turn `workdir` overrides harder.
    pub utility_llm: Arc<dyn crate::modules::memory::UtilityLlm>,
    /// Steward-aligned safety-valve configuration for the agent loop
    /// (S2-S1b). Plumbed from `TurnServiceDeps::loop_config`. Today
    /// the streaming body still uses `agent_max_iterations()` and the
    /// pre-Steward truncation handling; S5 Task 5.1's `run_agentic_loop`
    /// is the canonical home for `force_text_after_truncations`,
    /// `enable_tool_intent_nudge`, and `max_iterations` consumption.
    pub loop_config: crate::modules::application::turn_service::AgenticLoopConfig,
}

pub(super) async fn append_stream_event(
    run_event_logger: &RunEventLogger,
    payload: &StreamTokenPayload,
) {
    let event_type = match payload.event_type.as_str() {
        "thinking_start" => "thinking_started",
        "tool_call_update" => match payload.tool_status.as_deref() {
            Some("queued") => "tool_call_queued",
            Some("running") => "tool_call_running",
            Some("completed") => "tool_call_completed",
            Some("error") => "tool_call_failed",
            _ => "tool_call_update",
        },
        other => other,
    };
    let _ = run_event_logger.append(event_type, payload.clone()).await;
}

pub(super) async fn append_remembered_permission_events(
    run_event_logger: &RunEventLogger,
    tool_name: &str,
    decision: &PermissionPromptDecision,
) {
    let (decision_label, reason) = match decision {
        PermissionPromptDecision::Allow => ("allow", None),
        PermissionPromptDecision::Deny { reason } => ("deny", Some(reason.clone())),
    };
    let payload = serde_json::json!({
        "tool_name": tool_name,
        "source": "session_override",
        "decision": decision_label,
        "reason": reason,
    });
    let _ = run_event_logger
        .append("permission_requested", payload.clone())
        .await;
    let _ = run_event_logger
        .append("permission_resolved", payload)
        .await;
}

fn apply_memory_recall_success_finalization_guard(
    work_loop_decision: &WorkLoopDecision,
    has_successful_tool: bool,
    force_final_response_next: &mut bool,
    finalization_reason: &mut Option<String>,
) {
    if super::work_loop::should_force_final_after_memory_recall(
        work_loop_decision,
        has_successful_tool,
    ) {
        *force_final_response_next = true;
        *finalization_reason = Some("memory_recall_evidence_observed".to_string());
    }
}

fn should_retry_tool_required_no_tool(
    work_loop_decision: &WorkLoopDecision,
    has_successful_mutating_tool: bool,
    retry_count: usize,
    force_final_response: bool,
    available_tool_count: usize,
) -> bool {
    super::work_loop::requires_tool_execution_evidence(work_loop_decision)
        && !has_successful_mutating_tool
        && retry_count == 0
        && !force_final_response
        && available_tool_count > 0
}

fn should_retry_announced_tool_intent_no_tool(
    accumulated_text: &str,
    retry_count: usize,
    force_final_response: bool,
    available_tool_count: usize,
) -> bool {
    retry_count == 0
        && !force_final_response
        && available_tool_count > 0
        && super::work_loop::assistant_signals_tool_intent(accumulated_text)
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
#[allow(deprecated)]
pub(super) async fn run_stream_task(inputs: StreamTaskInputs) {
    let delegate = StreamingAgentLoopDelegate::new();
    let _ = delegate
        .execute(AgentLoopDelegateInput {
            stream_task_inputs: inputs,
        })
        .await;
}

pub(super) async fn run_stream_task_body(inputs: StreamTaskInputs) -> AgentLoopDelegateOutput {
    let StreamTaskInputs {
        stream_id_for_task,
        run_event_logger,
        session_id,
        user_message_clone,
        permission_mode_for_stream,
        inbound_resume_cursor,
        turn_number_for_stream,
        baseline_message_count_stream,
        messages_for_stream,
        tool_defs_for_stream,
        tool_pool_names_for_stream,
        tool_pool_schema_hash_for_stream,
        tool_pool_policy_for_stream,
        system_prompt_for_stream,
        provider_client_for_stream,
        failover_provider_client,
        lifecycle_hooks,
        model_for_stream,
        provider_id_for_stream,
        context_window_for_stream,
        routing_info_for_stream,
        work_loop_decision_for_stream,
        skill_resolution_plan_for_stream,
        execution_context_for_task,
        tool_registry_clone,
        session_manager,
        rolling_summarizer_for_stream,
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
        prompt_diagnostics_for_task,
        prompt_diagnostics_enabled_for_task,
        app_handle_for_after_turn,
        pinned_store_for_after_turn,
        memory_provider_for_after_turn,
        active_retrieval_manager_for_after_turn,
        stream_session_id_for_after_turn,
        stream_project_id_for_after_turn,
        harness_bus_for_after_turn,
        utility_llm,
        // S2-S1b: plumbed through but not yet consumed; S5 Task 5.1
        // `run_agentic_loop` becomes the consumer.
        loop_config: _loop_config,
    } = inputs;

    // Resolve run_id and app_data_dir for attempt ledger (MIG-022 / T-013).
    let run_id_for_ledger = run_event_logger.run_id().to_string();
    let app_data_dir_for_ledger = app_handle_for_after_turn
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    tracing::info!(
        "[start_agent_stream] Spawned background task for stream_id: {}",
        stream_id_for_task
    );

    let max_iterations: usize = agent_max_iterations();

    // Phase 6E harness: emit TurnStarted at the top of the spawned task
    // so all timing measurements include API client setup time.
    crate::modules::harness::agent_loop_integration::emit_turn_started(
        harness_event_bus_for_stream.as_ref(),
        &session_id,
        turn_number_for_stream,
    );
    let stream_turn_started_at = std::time::Instant::now();

    // S5b-1: bag the ~38 mutable per-turn locals into `StreamLoopState`.
    // Pure structural prep — no behaviour change vs the prior inline
    // `let mut name = ...` declarations. See `stream_loop_state.rs` for
    // the per-field rationale; future 5b-2..5b-5 substeps will shrink
    // this bag as helpers are extracted.
    //
    // P1-7 / P2-11 note (carried over): `accumulated_usage` /
    // `current_call_usage` track provider-billable token usage across
    // events — Anthropic puts `input_tokens` on `message_start` and
    // `output_tokens` on `message_delta`; OpenAI compat puts both on
    // the synthesised `message_delta` in `finish()`. The per-call
    // snapshot is committed into the running total on `message_stop`
    // so multi-iteration tool loops sum correctly.
    let initial_force_tool_choice_next =
        (super::work_loop::requires_tool_execution_evidence(&work_loop_decision_for_stream)
            || super::work_loop::requires_single_shell_command_evidence(
                &work_loop_decision_for_stream,
            ))
            && !tool_defs_for_stream.is_empty();
    let mut state = stream_loop_state::StreamLoopState::new(
        messages_for_stream,
        format!("stream_{}", stream_id_for_task),
        initial_force_tool_choice_next,
    );

    let stream_resilience_cfg =
        crate::modules::provider::resilience::LlmResilienceConfig::from_env();
    let cost_guard_cfg = crate::modules::runtime::cost_guard::CostGuardConfig::from_env();
    let is_resume_turn = inbound_resume_cursor.is_some();
    let mode = parse_permission_mode(permission_mode_for_stream.as_deref());
    // Phase M4-C P2 — wrap in `Arc` so the harness
    // `prepare_step_execution` shadow trace can borrow the
    // same policy without re-constructing it (re-construction
    // would lose any per-tool requirements set on the
    // original policy).
    let permission_policy = std::sync::Arc::new(build_permission_policy(mode));
    // Preserve `tool_success_evidence` Arc from the parent stream so broker
    // bumps + per-iteration resets apply to the same counter (FEAT-AE-002).
    let execution_context = SessionExecutionContext {
        session_id: execution_context_for_task.session_id.clone(),
        project_id: execution_context_for_task.project_id.clone(),
        workdir: execution_context_for_task.workdir.clone(),
        permission_mode: mode,
        tool_success_evidence: execution_context_for_task.tool_success_evidence.clone(),
    };
    let execution_context_for_policy = execution_context.clone();
    log_context_fingerprint("start_agent_stream_task", &execution_context);
    let mut tool_executor =
        ToolRegistryExecutor::new_with_context(tool_registry_clone.clone(), execution_context);
    const SAVE_INTERVAL: u32 = 50;

    loop {
        // Check for cancellation at the start of each iteration
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
                stream_id: stream_id_for_task.clone(),
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
            stream_emitter.emit_payload(payload.clone());
            append_stream_event(&run_event_logger, &payload).await;
            state.completion_already_emitted = true;
            state.terminal_status = Some("cancelled_by_user");
            break;
        }

        if state.tool_loop_iter >= max_iterations {
            tracing::warn!(
                "[start_agent_stream] Tool loop exceeded max_iterations={}",
                max_iterations
            );
            state.terminal_status = Some("max_iterations_reached");
            break;
        }
        state.tool_loop_iter += 1;
        tool_executor
            .execution_context
            .tool_success_evidence
            .store(0, std::sync::atomic::Ordering::Relaxed);
        let force_final_response =
            state.force_final_response_next || state.tool_loop_iter >= max_iterations;
        let force_tool_choice = state.force_tool_choice_next
            || (super::work_loop::requires_tool_execution_evidence(&work_loop_decision_for_stream)
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

        // DW-002 — call the WU-004 digester before preflight so that
        // long histories get LLM-summarized into a tighter token
        // footprint. **Truth-loop iter-7 (2026-04-30)**: replaced the
        // per-iteration `Arc::new(ChatProviderUtilityLlm::new(...))`
        // with `utility_llm.clone()`, threaded from
        // `TurnServiceDeps::utility_llm` — the same shared shim
        // `bootstrap/memory.rs` constructs once at startup.  Avoids
        // re-allocating a provider wrapper every outer loop and
        // proves the `AppState::utility_llm` field has a live
        // streaming-turn consumer (the `#[allow(dead_code)]` was
        // dropped in the same commit).
        // Failure-isolated: any error → `digested_messages = None`.
        let digested_owned: Option<Vec<crate::modules::api::InputMessage>> =
            super::preflight_hooks::digest_messages_for_preflight(
                &state.session_messages,
                utility_llm.clone(),
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
                    session_id: Some(session_id.clone()),
                    stream_id: Some(stream_id_for_task.clone()),
                    ..Default::default()
                },
                &payload,
                None,
            );
        }

        // Build the iteration request via the extracted preflight module (GAP-005).
        let preflight_result = super::stream_preflight::build_iteration_request(
            super::stream_preflight::PreflightContext {
                session_messages: &state.session_messages,
                digested_messages: digested_owned.as_deref(),
                tool_defs: &tool_defs_for_stream,
                system_prompt: &system_prompt_for_stream,
                model: &model_for_stream,
                context_window: context_window_for_stream,
                force_final_response,
                finalization_reason: &current_finalization_reason,
                force_tool_choice,
                tool_loop_iter: state.tool_loop_iter,
                max_iterations,
                stream_id: &stream_id_for_task,
                session_id: &session_id,
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
        let _ = run_event_logger
            .append(
                "provider_tool_call_diagnostics",
                serde_json::json!({
                    "stream_id": stream_id_for_task.clone(),
                    "session_id": session_id.clone(),
                    "iteration": state.tool_loop_iter,
                    "tool_count": request_tool_count,
                    "tool_names": request_tool_names,
                    "canonical_tool_names": tool_pool_names_for_stream.clone(),
                    "tool_schema_hash": tool_pool_schema_hash_for_stream.clone(),
                    "tool_pool_policy": tool_pool_policy_for_stream.clone(),
                    "tool_choice": request_tool_choice,
                    "force_final_response": force_final_response,
                    "force_tool_choice": force_tool_choice,
                    "requires_tool_execution_evidence": super::work_loop::requires_tool_execution_evidence(&work_loop_decision_for_stream),
                    "has_successful_tool": state.has_successful_tool,
                    "has_successful_mutating_tool": state.has_successful_mutating_tool,
                    "reason_codes": work_loop_decision_for_stream.reason_codes.clone(),
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

        if let Err(ce) = crate::modules::runtime::cost_guard::CostGuard::check_before_llm_call(
            &cost_guard_cfg,
            &session_id,
        ) {
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
                stream_id: stream_id_for_task.clone(),
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
            stream_emitter.emit_payload(payload.clone());
            append_stream_event(&run_event_logger, &payload).await;
            if let Some(bus) = harness_event_bus_for_stream.as_ref() {
                let _ = bus.emit(AgentEvent::StreamErrored {
                    session_id: session_id.clone(),
                    reason: state
                        .last_stream_error_reason
                        .clone()
                        .unwrap_or_else(|| "cost_limit".to_string()),
                    resume_available: user_visible_truth.resume_available,
                    at: chrono::Utc::now(),
                });
            }
            break;
        }

        let stream = match crate::modules::provider::resilience::stream_message_with_resilience(
            &provider_client_for_stream,
            failover_provider_client.as_ref(),
            &iter_api_request,
            &mut state.stream_circuit,
            &stream_resilience_cfg,
        )
        .await
        {
            Ok(s) => {
                crate::modules::runtime::cost_guard::CostGuard::record_llm_call_charged(
                    &cost_guard_cfg,
                    &session_id,
                );
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
                        stream_id_for_task,
                        session_id,
                        state.stream_start_retry_count,
                        MAX_STREAM_RETRY_ON_TIMEOUT,
                        stream_error_reason
                    );
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    continue;
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
                    build_resume_cursor(
                        &stream_id_for_task,
                        state.tool_loop_iter,
                        state.token_count,
                    )
                });
                let degraded_reason = user_visible_truth.degraded_reason.clone();
                let payload = StreamTokenPayload {
                    stream_id: stream_id_for_task.clone(),
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
                stream_emitter.emit_payload(payload.clone());
                append_stream_event(&run_event_logger, &payload).await;
                // Phase M4-C P5 — emit harness `StreamErrored`
                // event so the trace aggregator records the
                // hard error against the run report.
                if let Some(bus) = harness_event_bus_for_stream.as_ref() {
                    let _ = bus.emit(AgentEvent::StreamErrored {
                        session_id: session_id.clone(),
                        reason: state
                            .last_stream_error_reason
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
            state.provider_request_id = request_id.clone();
            tracing::info!(
                "[start_agent_stream] provider request_id captured: stream_id={}, session_id={}, request_id={}",
                stream_id_for_task,
                session_id,
                request_id
            );
        }

        // Run the extracted inner SSE event-processing loop (GAP-005).
        let event_loop_ctx = super::stream_event_loop::StreamEventLoopContext {
            stream,
            cancel_rx,
            stream_id: stream_id_for_task.clone(),
            session_id: session_id.clone(),
            provider_request_id: state.provider_request_id.clone(),
            accumulated_text: std::mem::take(&mut state.accumulated_text),
            accumulated_thinking: std::mem::take(&mut state.accumulated_thinking),
            token_count: state.token_count,
            current_call_usage: std::mem::take(&mut state.current_call_usage),
            accumulated_usage: std::mem::take(&mut state.accumulated_usage),
            stream_emitter: stream_emitter.clone(),
            run_event_logger: run_event_logger.clone(),
            app_session: app_session_clone.clone(),
            session_manager: session_manager.clone(),
            harness_bus: harness_event_bus_for_stream.clone(),
            execution_context: execution_context_for_policy.clone(),
            has_successful_tool: state.has_successful_tool,
            has_successful_mutating_tool: state.has_successful_mutating_tool,
            provider_id: provider_id_for_stream.clone(),
            model: model_for_stream.clone(),
            stream_event_retry_count: state.stream_event_retry_count,
        };
        let loop_result = super::stream_event_loop::run_stream_event_loop(event_loop_ctx).await;
        let mut pending_tool_uses = loop_result.pending_tool_uses;
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
        cancel_rx = loop_result.cancel_rx;

        if retry_outer_after_timeout {
            tokio::time::sleep(Duration::from_millis(350)).await;
            continue;
        }

        // Some providers stream textual pseudo tool calls instead of
        // structured tool-call deltas. Convert supported formats before
        // falling back to nudge/finalization.
        if pending_tool_uses.is_empty() {
            tracing::info!(
                "[start_agent_stream] No pending tool uses, breaking outer loop. accumulated_text len={}",
                state.accumulated_text.len()
            );
            if !force_final_response {
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
                    let _ = run_event_logger
                        .append(
                            "provider_tool_call_compat_warning",
                            serde_json::json!({
                                "stream_id": stream_id_for_task.clone(),
                                "session_id": session_id.clone(),
                                "provider_id": provider_id_for_stream.clone(),
                                "model": model_for_stream.clone(),
                                "markup_family": markup_family,
                                "text_len": state.accumulated_text.len(),
                                "converted_tool_call_count": converted_count,
                                "recovery_action": "execute_as_structured_tool_calls",
                                "sanitized": true,
                            }),
                        )
                        .await;
                    pending_tool_uses = textual_tool_calls;
                    state.accumulated_text.clear();
                    state.accumulated_thinking.clear();
                }
            }
        }
        if pending_tool_uses.is_empty() {
            if let Some(markup_family) =
                super::work_loop::detect_textual_tool_call_markup(&state.accumulated_text)
            {
                state.provider_textual_tool_markup_seen = true;
                let warning = format!(
                    "provider emitted {markup_family} text instead of structured tool_calls"
                );
                if !state
                    .diagnostic_warnings
                    .iter()
                    .any(|value| value == &warning)
                {
                    state.diagnostic_warnings.push(warning.clone());
                }
                let _ = run_event_logger
                    .append(
                        "provider_tool_call_compat_warning",
                        serde_json::json!({
                            "stream_id": stream_id_for_task.clone(),
                            "session_id": session_id.clone(),
                            "provider_id": provider_id_for_stream.clone(),
                            "model": model_for_stream.clone(),
                            "markup_family": markup_family,
                            "text_len": state.accumulated_text.len(),
                            "recovery_action": "nudge_with_required_tool_choice",
                            "sanitized": true,
                        }),
                    )
                    .await;
            }
            if should_retry_tool_required_no_tool(
                &work_loop_decision_for_stream,
                state.has_successful_mutating_tool,
                state.tool_required_no_tool_retry_count,
                force_final_response,
                tool_defs_for_stream.len(),
            ) {
                state.tool_required_no_tool_retry_count += 1;
                tracing::warn!(
                    "[start_agent_stream] tool-required task produced no mutating tool call; retrying once with tool-required loop control. stream_id={}, session_id={}",
                    stream_id_for_task,
                    session_id
                );
                let payload = serde_json::json!({
                    "reason": "tool_required_no_tool",
                    "retry_count": state.tool_required_no_tool_retry_count,
                    "available_tool_count": tool_defs_for_stream.len(),
                });
                let _ = run_event_logger
                    .append("tool_required_no_tool_retry", payload)
                    .await;
                state.accumulated_text.clear();
                state.accumulated_thinking.clear();
                state.session_messages.push(InputMessage::user_text(
                    "[agent_loop_control] The previous assistant response did not call tools, but this user request requires concrete file/tool execution before completion. Call the available tools now to inspect, create or edit the artifact, and verify it. Use complete JSON arguments for every tool call. Do not answer only with prose. If tool execution is impossible, explain the blockage in the final report.",
                ));
                state.force_tool_choice_next = true;
                continue;
            }
            if should_retry_announced_tool_intent_no_tool(
                &state.accumulated_text,
                state.tool_intent_nudge_retry_count,
                force_final_response,
                tool_defs_for_stream.len(),
            ) {
                state.tool_intent_nudge_retry_count += 1;
                tracing::warn!(
                    "[start_agent_stream] assistant announced tool intent without tool call; nudging once. stream_id={}, session_id={}",
                    stream_id_for_task,
                    session_id
                );
                let payload = serde_json::json!({
                    "reason": "announced_tool_intent_no_tool",
                    "retry_count": state.tool_intent_nudge_retry_count,
                    "available_tool_count": tool_defs_for_stream.len(),
                });
                let _ = run_event_logger
                    .append("tool_intent_nudge_retry", payload)
                    .await;
                state.accumulated_text.clear();
                state.accumulated_thinking.clear();
                state.session_messages.push(InputMessage::user_text(
                    super::work_loop::tool_intent_nudge_message(),
                ));
                state.force_tool_choice_next = true;
                continue;
            }
            if state.terminal_status.is_none() {
                state.terminal_status = match state.finalization_reason.as_deref() {
                    Some("invalid_tool_args_repeated") => Some("invalid_tool_args_repeated"),
                    Some("repeated_tool_batch_no_progress") => {
                        Some("repeated_tool_batch_no_progress")
                    }
                    Some("approval_required_for_mutation") => {
                        Some("approval_required_for_mutation")
                    }
                    _ if super::work_loop::requires_memory_recall_evidence(
                        &work_loop_decision_for_stream,
                    ) && !state.has_successful_tool
                        && super::work_loop::is_incomplete_memory_lookup_response(
                            &state.accumulated_text,
                        ) =>
                    {
                        Some("memory_recall_required_no_tool")
                    }
                    _ if super::work_loop::requires_tool_execution_evidence(
                        &work_loop_decision_for_stream,
                    ) && !state.has_successful_mutating_tool =>
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
                    _ if super::work_loop::detect_repetitive_model_output(
                        &state.accumulated_text,
                    ) =>
                    {
                        Some("repetitive_model_output")
                    }
                    _ => Some("model_stop_no_tools"),
                };
            }
            break;
        }
        if force_final_response {
            tracing::warn!(
                "[start_agent_stream] Provider emitted tool calls during finalization pass; ending as max_iterations_reached. pending_tool_uses={}",
                pending_tool_uses.len()
            );
            state.terminal_status = Some("max_iterations_reached");
            break;
        }

        let current_tool_batch_signature = tool_batch_signature(&pending_tool_uses);
        if state.last_tool_batch_signature.as_deref() == Some(current_tool_batch_signature.as_str())
        {
            state.repeated_tool_batch_count += 1;
        } else {
            state.repeated_tool_batch_count = 1;
            state.last_tool_batch_signature = Some(current_tool_batch_signature);
        }
        if state.repeated_tool_batch_count >= REPEATED_TOOL_BATCH_LIMIT {
            tracing::warn!(
                "[start_agent_stream] repeated tool batch detected; next iteration will force final summary. stream_id={}, session_id={}, repeated_count={}",
                stream_id_for_task,
                session_id,
                state.repeated_tool_batch_count
            );
            state.force_final_response_next = true;
            state.finalization_reason = Some("repeated_tool_batch_no_progress".to_string());
        }

        // Execute the extracted tool batch (GAP-005).
        let tool_ctx = super::stream_tool_execution::ToolExecutionContext {
            pending_tool_uses: std::mem::take(&mut pending_tool_uses),
            stream_id: stream_id_for_task.clone(),
            session_id: session_id.clone(),
            provider_request_id: state.provider_request_id.clone(),
            accumulated_text: std::mem::take(&mut state.accumulated_text),
            accumulated_thinking: std::mem::take(&mut state.accumulated_thinking),
            session_messages: std::mem::take(&mut state.session_messages),
            timeline_session_messages: std::mem::take(&mut state.timeline_session_messages),
            stream_emitter: stream_emitter.clone(),
            run_event_logger: run_event_logger.clone(),
            tool_registry: tool_registry_clone.clone(),
            tool_executor,
            permission_senders: permission_senders.clone(),
            permission_overrides: permission_overrides.clone(),
            permission_policy: permission_policy.clone(),
            execution_context: execution_context_for_task.clone(),
            work_loop_decision: work_loop_decision_for_stream.clone(),
            harness_bus: harness_event_bus_for_stream.clone(),
            mode,
            invalid_tool_args_streak: state.invalid_tool_args_streak,
            has_successful_tool: state.has_successful_tool,
            has_successful_mutating_tool: state.has_successful_mutating_tool,
            sanitized_dropped_invalid_tool_use_inputs: state
                .sanitized_dropped_invalid_tool_use_inputs,
            sanitize_invalid_tool_use_samples: std::mem::take(
                &mut state.sanitize_invalid_tool_use_samples,
            ),
            run_id: run_id_for_ledger.clone(),
            app_data_dir: app_data_dir_for_ledger.clone(),
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
            && super::work_loop::requires_tool_execution_evidence(&work_loop_decision_for_stream)
        {
            if let Some(message) = super::todo_ledger::post_mutation_update_message(&session_id) {
                state
                    .session_messages
                    .push(InputMessage::user_text(message));
            }
        }
        state.force_final_response_next =
            state.force_final_response_next || tool_result.force_final_response_next;
        apply_memory_recall_success_finalization_guard(
            &work_loop_decision_for_stream,
            tool_result.has_successful_tool,
            &mut state.force_final_response_next,
            &mut state.finalization_reason,
        );
        if super::work_loop::requires_single_shell_command_evidence(&work_loop_decision_for_stream)
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
        tool_executor = tool_result.tool_executor;
        // Continue outer loop → send next LLM request with tool results
        tracing::info!(
            "[start_agent_stream] Tool execution done, continuing outer loop. session_messages len={}",
            state.session_messages.len()
        );
    }

    // S5b-1: destructure the bag back into locals so the post-loop
    // finalization block (lifecycle hook + delegate output + finalize
    // call) keeps its original variable names. This avoids touching
    // any post-loop logic in this pure-refactor commit.
    let stream_loop_state::StreamLoopState {
        tool_loop_iter,
        stream_event_retry_count,
        stream_start_retry_count,
        tool_required_no_tool_retry_count: _tool_required_no_tool_retry_count,
        tool_intent_nudge_retry_count: _tool_intent_nudge_retry_count,
        repeated_tool_batch_count: _repeated_tool_batch_count,
        invalid_tool_args_streak: _invalid_tool_args_streak,
        session_messages,
        accumulated_text,
        accumulated_thinking,
        token_count,
        accumulated_usage,
        current_call_usage: _current_call_usage,
        timeline_session_messages,
        diagnostic_warnings,
        sanitize_orphan_samples,
        sanitize_unmatched_samples,
        sanitize_invalid_tool_use_samples,
        sanitize_rounds,
        sanitized_dropped_empty_messages,
        sanitized_dropped_orphan_tool_results,
        sanitized_dropped_unmatched_tool_uses,
        sanitized_dropped_invalid_tool_use_inputs,
        preflight_trim_rounds,
        preflight_dropped_messages_total,
        preflight_trimmed_chars_total,
        force_final_response_next: _force_final_response_next,
        force_tool_choice_next: _force_tool_choice_next,
        stream_failed,
        completion_already_emitted,
        has_successful_tool,
        has_successful_mutating_tool,
        provider_textual_tool_markup_seen: _provider_textual_tool_markup_seen,
        terminal_status,
        last_stream_error_reason,
        finalization_reason: _finalization_reason,
        last_tool_batch_signature: _last_tool_batch_signature,
        pending_operation_for_delegate,
        provider_request_id,
        stream_circuit: _stream_circuit,
    } = state;

    let accumulated_text = match lifecycle_hooks
        .run_point(
            crate::modules::runtime::lifecycle_hooks::HookPoint::BeforeOutbound,
            Some(session_id.as_str()),
            accumulated_text.clone(),
        )
        .await
    {
        Ok(t) => t,
        Err(e) => format!("{accumulated_text}\n\n[outbound lifecycle hook: {e}]"),
    };

    let delegate_output =
        StreamingAgentLoopDelegate::output_from_terminal_state(AgentLoopTerminalState {
            work_loop_decision: work_loop_decision_for_stream.clone(),
            skill_resolution_plan: skill_resolution_plan_for_stream.clone(),
            stream_id: stream_id_for_task.clone(),
            provider_request_id: provider_request_id.clone(),
            stream_failed,
            terminal_status,
            last_stream_error_reason: last_stream_error_reason.clone(),
            tool_loop_iterations: tool_loop_iter,
            token_count,
            has_successful_tool,
            has_successful_mutating_tool,
            pending_operation: pending_operation_for_delegate,
        });

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
        accumulated_usage,
        effective_model: model_for_stream.clone(),
        effective_provider_id: provider_id_for_stream.clone(),
        effective_context_window: context_window_for_stream,
        routing_info: routing_info_for_stream.clone(),
        work_loop_decision: work_loop_decision_for_stream,
        skill_resolution_plan: skill_resolution_plan_for_stream,
        diagnostic_warnings,
        stream_emitter,
        run_event_logger,
        session_manager,
        rolling_summarizer_for_finalize: rolling_summarizer_for_stream,
        app_session_clone,
        trajectory_manager_for_stream,
        memory_provider_for_stream,
        memory_ticker_for_stream,
        harness_event_bus_for_stream,
        learning_module_for_stream,
        memory_context_items_for_task,
        prompt_diagnostics_for_task,
        prompt_diagnostics_enabled_for_task,
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

    delegate_output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::api::InputContentBlock;
    use crate::modules::runtime::contracts::execution_mode::{
        ComplexityLevel, ExecutionMode, ExecutionModeDecision, ReasonCode, RiskLevel,
    };
    use crate::modules::runtime::event_log::{RunEventLogger, RunLogEntry};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn memory_intent_decision() -> WorkLoopDecision {
        let decision = ExecutionModeDecision {
            execution_mode: ExecutionMode::DirectExecute,
            risk_level: RiskLevel::Low,
            complexity_level: ComplexityLevel::Trivial,
            complexity_score: 0.1,
            reason_codes: vec![ReasonCode::new("test")],
            route_hint: None,
            requires_plan: false,
            scenario_profile_hint: None,
            classifier_policy_version: "test".to_string(),
            classifier_matched_rule_ids: Vec::new(),
            classifier_slot_summary: serde_json::json!({}),
            classifier_ambiguous_escalated: false,
            classifier_escalation_source: None,
        };
        super::super::work_loop::route_work_loop(&decision, "你记得关于我的什么事情")
    }

    fn tool_def(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: None,
            input_schema: serde_json::json!({ "type": "object" }),
        }
    }

    fn unique_temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("if2ai-{label}-{nanos}"))
    }

    fn read_entries(path: &Path) -> Vec<RunLogEntry> {
        std::fs::read_to_string(path)
            .expect("read run log")
            .lines()
            .map(|line| serde_json::from_str::<RunLogEntry>(line).expect("parse run log entry"))
            .collect()
    }

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

    #[test]
    fn memory_recall_success_forces_next_preflight_without_tools() {
        let routed = memory_intent_decision();
        let mut force_final_response_next = false;
        let mut finalization_reason = None;
        apply_memory_recall_success_finalization_guard(
            &routed,
            true,
            &mut force_final_response_next,
            &mut finalization_reason,
        );
        assert!(force_final_response_next);
        assert_eq!(
            finalization_reason.as_deref(),
            Some("memory_recall_evidence_observed")
        );

        let session_messages = vec![InputMessage::user_tool_result(
            "memory_recall:1",
            "[conversation] helen_husky: Ryan Liu 家有一只哈士奇叫 Helen",
            false,
        )];
        let tool_defs = vec![tool_def("memory_recall"), tool_def("memory_export")];
        let result = super::super::stream_preflight::build_iteration_request(
            super::super::stream_preflight::PreflightContext {
                session_messages: &session_messages,
                digested_messages: None,
                tool_defs: &tool_defs,
                system_prompt: "system",
                model: "test-model",
                context_window: 32_000,
                force_final_response: force_final_response_next,
                finalization_reason: finalization_reason.as_deref().unwrap_or(""),
                force_tool_choice: false,
                tool_loop_iter: 1,
                max_iterations: 10,
                stream_id: "stream-memory",
                session_id: "session-memory",
            },
        );
        assert!(result.request.tools.is_none());
        let has_loop_control = result.request.messages.iter().any(|message| {
            message.content.iter().any(|block| match block {
                InputContentBlock::Text { text } => {
                    text.contains("[agent_loop_control]")
                        && text.contains("memory_recall_evidence_observed")
                }
                _ => false,
            })
        });
        assert!(has_loop_control);
    }

    #[test]
    fn tool_required_no_tool_gets_one_retry_before_terminal_failure() {
        let routed = crate::modules::application::turn_service::work_loop::route_work_loop(
            &crate::modules::runtime::contracts::execution_mode::ExecutionModeDecision {
                execution_mode:
                    crate::modules::runtime::contracts::execution_mode::ExecutionMode::DirectExecute,
                risk_level: crate::modules::runtime::contracts::execution_mode::RiskLevel::Low,
                complexity_level:
                    crate::modules::runtime::contracts::execution_mode::ComplexityLevel::Trivial,
                complexity_score: 0.1,
                reason_codes: vec![],
                route_hint: None,
                requires_plan: false,
                scenario_profile_hint: None,
                classifier_policy_version: "test".to_string(),
                classifier_matched_rule_ids: Vec::new(),
                classifier_slot_summary: serde_json::json!({}),
                classifier_ambiguous_escalated: false,
                classifier_escalation_source: None,
            },
            "帮我创建一个泡泡龙网页游戏 需要有声效 界面美观大方",
        );

        assert!(should_retry_tool_required_no_tool(
            &routed, false, 0, false, 3
        ));
        assert!(!should_retry_tool_required_no_tool(
            &routed, false, 1, false, 3
        ));
        assert!(!should_retry_tool_required_no_tool(
            &routed, true, 0, false, 3
        ));
        assert!(!should_retry_tool_required_no_tool(
            &routed, false, 0, true, 3
        ));
        assert!(!should_retry_tool_required_no_tool(
            &routed, false, 0, false, 0
        ));
    }

    #[test]
    fn announced_tool_intent_gets_one_nudge_retry() {
        assert!(should_retry_announced_tool_intent_no_tool(
            "我来用 bash 直接写入文件。",
            0,
            false,
            4
        ));
        assert!(!should_retry_announced_tool_intent_no_tool(
            "我来用 bash 直接写入文件。",
            1,
            false,
            4
        ));
        assert!(!should_retry_announced_tool_intent_no_tool(
            "我来用 bash 直接写入文件。",
            0,
            true,
            4
        ));
        assert!(!should_retry_announced_tool_intent_no_tool(
            "我来用 bash 直接写入文件。",
            0,
            false,
            0
        ));
        assert!(!should_retry_announced_tool_intent_no_tool(
            "我可以解释一下这个概念。",
            0,
            false,
            4
        ));
    }

    #[test]
    fn preflight_sets_required_tool_choice_when_forced() {
        let session_messages = vec![InputMessage::user_text("创建一个网页游戏")];
        let tool_defs = vec![tool_def("file_write"), tool_def("REPL")];
        let result = super::super::stream_preflight::build_iteration_request(
            super::super::stream_preflight::PreflightContext {
                session_messages: &session_messages,
                digested_messages: None,
                tool_defs: &tool_defs,
                system_prompt: "system",
                model: "test-model",
                context_window: 32_000,
                force_final_response: false,
                finalization_reason: "",
                force_tool_choice: true,
                tool_loop_iter: 1,
                max_iterations: 10,
                stream_id: "stream-tool-required",
                session_id: "session-tool-required",
            },
        );

        assert_eq!(
            result.request.tool_choice,
            Some(crate::modules::api::ToolChoice::Any)
        );
        assert_eq!(result.request.tools.as_ref().map(Vec::len), Some(2));
    }

    #[tokio::test]
    async fn remembered_permission_events_are_logged() {
        let root = unique_temp_root("remembered-permission-events");
        let logger = RunEventLogger::for_base_dir(&root, "session-1", "run-1");

        append_remembered_permission_events(
            &logger,
            "write_file",
            &PermissionPromptDecision::Deny {
                reason: "session override deny".to_string(),
            },
        )
        .await;

        let entries = read_entries(logger.file_path().expect("run log path"));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].event_type, "permission_requested");
        assert_eq!(entries[1].event_type, "permission_resolved");
        assert_eq!(entries[1].payload["decision"], "deny");
        assert_eq!(entries[1].payload["reason"], "session override deny");
        assert_eq!(entries[1].payload["source"], "session_override");
    }
}
