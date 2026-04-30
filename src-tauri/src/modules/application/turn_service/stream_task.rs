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
pub(super) const REPEATED_TOOL_BATCH_LIMIT: usize = 3;
const INVALID_TOOL_ARGS_LIMIT: usize = 2;

pub(crate) fn agent_max_iterations() -> usize {
    std::env::var("IF2AI_AGENT_MAX_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.max(MIN_MAX_TOOL_LOOP_ITERATIONS))
        .unwrap_or(DEFAULT_MAX_TOOL_LOOP_ITERATIONS)
}

pub(super) fn tool_batch_signature(pending_tool_uses: &[(String, String, String)]) -> String {
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

pub(super) fn apply_memory_recall_success_finalization_guard(
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

pub(super) fn should_retry_tool_required_no_tool(
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

pub(super) fn should_retry_announced_tool_intent_no_tool(
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

pub(super) async fn run_stream_task_body(mut inputs: StreamTaskInputs) -> AgentLoopDelegateOutput {
    // Resolve run_id and app_data_dir for attempt ledger (MIG-022 / T-013).
    let run_id_for_ledger = inputs.run_event_logger.run_id().to_string();
    let app_data_dir_for_ledger = inputs
        .app_handle_for_after_turn
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    tracing::info!(
        "[start_agent_stream] Spawned background task for stream_id: {}",
        inputs.stream_id_for_task
    );

    let max_iterations: usize = agent_max_iterations();

    // Phase 6E harness: emit TurnStarted at the top of the spawned task
    // so all timing measurements include API client setup time.
    crate::modules::harness::agent_loop_integration::emit_turn_started(
        inputs.harness_event_bus_for_stream.as_ref(),
        &inputs.session_id,
        inputs.turn_number_for_stream,
    );
    let stream_turn_started_at = std::time::Instant::now();

    // S5b-1: bag the ~38 mutable per-turn locals into `StreamLoopState`.
    // P1-7 / P2-11 note (carried over): `accumulated_usage` /
    // `current_call_usage` track provider-billable token usage across
    // events — Anthropic puts `input_tokens` on `message_start` and
    // `output_tokens` on `message_delta`; OpenAI compat puts both on
    // the synthesised `message_delta` in `finish()`. The per-call
    // snapshot is committed into the running total on `message_stop`
    // so multi-iteration tool loops sum correctly.
    let initial_force_tool_choice_next =
        (super::work_loop::requires_tool_execution_evidence(&inputs.work_loop_decision_for_stream)
            || super::work_loop::requires_single_shell_command_evidence(
                &inputs.work_loop_decision_for_stream,
            ))
            && !inputs.tool_defs_for_stream.is_empty();
    // Move messages_for_stream out of `inputs` (replaced with empty Vec) so
    // StreamLoopState owns them; `inputs` stays borrowable by StreamDelegate.
    let messages_for_stream = std::mem::take(&mut inputs.messages_for_stream);
    let state = stream_loop_state::StreamLoopState::new(
        messages_for_stream,
        format!("stream_{}", inputs.stream_id_for_task),
        initial_force_tool_choice_next,
    );

    let stream_resilience_cfg =
        crate::modules::provider::resilience::LlmResilienceConfig::from_env();
    let cost_guard_cfg = crate::modules::runtime::cost_guard::CostGuardConfig::from_env();
    let is_resume_turn = inputs.inbound_resume_cursor.is_some();
    let mode = parse_permission_mode(inputs.permission_mode_for_stream.as_deref());
    // Phase M4-C P2 — wrap in `Arc` so the harness
    // `prepare_step_execution` shadow trace can borrow the
    // same policy without re-constructing it.
    let permission_policy = std::sync::Arc::new(build_permission_policy(mode));
    // Preserve `tool_success_evidence` Arc from the parent stream so broker
    // bumps + per-iteration resets apply to the same counter (FEAT-AE-002).
    let execution_context = SessionExecutionContext {
        session_id: inputs.execution_context_for_task.session_id.clone(),
        project_id: inputs.execution_context_for_task.project_id.clone(),
        workdir: inputs.execution_context_for_task.workdir.clone(),
        permission_mode: mode,
        tool_success_evidence: inputs
            .execution_context_for_task
            .tool_success_evidence
            .clone(),
    };
    let execution_context_for_policy = execution_context.clone();
    log_context_fingerprint("start_agent_stream_task", &execution_context);
    let tool_executor = ToolRegistryExecutor::new_with_context(
        inputs.tool_registry_clone.clone(),
        execution_context,
    );

    // Move cancel_rx out of `inputs` (replaced with a never-firing dummy)
    // so the delegate can own it without a borrow conflict on `&inputs`.
    // The dummy sender is dropped immediately; nothing reads inputs.cancel_rx
    // after this point.
    let cancel_rx = {
        let (_dropped_tx, replacement) = tokio::sync::oneshot::channel::<()>();
        std::mem::replace(&mut inputs.cancel_rx, replacement)
    };

    // S5-T5c: drive the loop through the unified `run_agentic_loop` via the
    // StreamDelegate adapter. Replaces the previous inline 12-phase iteration
    // body. Latent semantic shifts vs the inline body (audited in T5b,
    // accepted as the price of the abstraction):
    //   1. RetryAfterSleep paths (preflight+run-stream) now consume an outer
    //      iteration slot via the synthetic `RespondResult::Text("")` →
    //      `TextAction::Continue` round-trip.
    //   2. `truncation_count` resets on that synthetic Text even though no
    //      real text response happened (run_agentic_loop resets on every
    //      `RespondResult::Text`).
    // Neither is exercised by the current e2e suite.
    let extras = super::stream_delegate::StreamDelegateExtras {
        max_iterations,
        stream_resilience_cfg,
        cost_guard_cfg,
        mode,
        permission_policy,
        execution_context_for_policy,
        run_id_for_ledger,
        app_data_dir_for_ledger,
    };
    let delegate = super::stream_delegate::StreamDelegate::new(
        &inputs,
        extras,
        state,
        cancel_rx,
        tool_executor,
    );
    let outcome = super::agentic_loop::run_agentic_loop(&delegate, &inputs.loop_config).await;
    let (mut state, _tool_executor_returned) = delegate.into_parts();

    match outcome {
        super::agentic_loop::LoopOutcome::Response(text) => {
            // accumulated_text is normally already populated by the helpers
            // that built the Text response. Backstop in case a future helper
            // returns text without mirroring it into state.
            if state.accumulated_text.is_empty() {
                state.accumulated_text = text;
            }
        }
        super::agentic_loop::LoopOutcome::Stopped => {
            // check_signals stamps terminal_status = "cancelled_by_user"
            // before returning LoopSignal::Stop.
            debug_assert!(state.terminal_status.is_some());
        }
        super::agentic_loop::LoopOutcome::MaxIterations => {
            // Outer (loop_config) cap. Inner per-iteration cap normally
            // trips first via PreflightOutcome::BreakTerminal → Failure.
            if state.terminal_status.is_none() {
                state.terminal_status = Some("max_iterations_reached");
            }
        }
        super::agentic_loop::LoopOutcome::Failure(_reason) => {
            // PreflightOutcome::BreakTerminal / call_llm Err: helpers have
            // already stamped terminal_status / stream_failed /
            // last_stream_error_reason before returning.
            debug_assert!(
                state.terminal_status.is_some() || state.stream_failed,
                "LoopOutcome::Failure should leave state with terminal_status or stream_failed set"
            );
        }
    }

    // Now destructure `inputs` for the post-loop / finalize block. Fields
    // already moved/taken above are bound with `_` so the destructure stays
    // exhaustive.
    let StreamTaskInputs {
        stream_id_for_task,
        run_event_logger,
        session_id,
        user_message_clone,
        permission_mode_for_stream: _,
        inbound_resume_cursor,
        turn_number_for_stream,
        baseline_message_count_stream,
        messages_for_stream: _,
        tool_defs_for_stream: _,
        tool_pool_names_for_stream: _,
        tool_pool_schema_hash_for_stream: _,
        tool_pool_policy_for_stream: _,
        system_prompt_for_stream,
        provider_client_for_stream: _,
        failover_provider_client: _,
        lifecycle_hooks,
        model_for_stream,
        provider_id_for_stream,
        context_window_for_stream,
        routing_info_for_stream,
        work_loop_decision_for_stream,
        skill_resolution_plan_for_stream,
        execution_context_for_task: _,
        tool_registry_clone: _,
        session_manager,
        rolling_summarizer_for_stream,
        app_session_clone,
        stream_emitter,
        cancel_rx: _,
        permission_senders: _,
        permission_overrides: _,
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
        utility_llm: _,
        loop_config: _,
    } = inputs;

    // S5b-1: destructure the bag back into locals so the post-loop
    // finalization block (lifecycle hook + delegate output + finalize
    // call) keeps its original variable names.
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
        last_finish_reason: _last_finish_reason,
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
