//! MIG-001-c — streaming chat turn lifecycle owned by
//! [`crate::modules::application::turn_service::TurnService`].
//!
//! Hosts [`TurnService::stream_turn`], which used to live as the
//! body of `commands::agent::start_agent_stream`. The IPC command
//! is now a thin adapter that constructs a [`TurnService`] from
//! `AppState` and delegates streaming turn ownership here.
//!
//! Layering:
//!
//! - All cross-module imports go through `crate::modules::*` per
//!   CHARTER §3.1.
//! - The `TurnServiceDeps` struct continues to feed long-lived
//!   handles (session / tool / memory / harness / learning /
//!   ticker / trajectory / app_handle).
//! - Streaming-specific cross-stream coordination state
//!   (`permission_senders`, `permission_overrides`,
//!   `stream_cancel_senders`) lives on `AppState` and is passed
//!   per-call inside [`StreamTurnRequest`] rather than bound to
//!   the service, because each Tauri-process instance shares those
//!   maps across all sessions.
//!
//! File-size note: MIG-001-d extracted the spawned tool-loop
//! closure body into the sibling [`super::stream_task`] module,
//! so this file is now back under the CHARTER §5 800-LOC backend
//! hard limit. `stream_task.rs` is the new tier-3 watchlist
//! entry pending future per-iteration / post-loop helper splits.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::Manager;

use std::time::Duration;

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
use crate::modules::application::turn_service::{
    PrepareChatInputsRequest, RuntimeProviderResolution, TurnServiceError,
};
use crate::modules::application::MemoryItemProjection;
use crate::modules::control_plane::session_bridge::{
    app_session_to_runtime, log_context_fingerprint,
};
use crate::modules::control_plane::{
    AuditEmitter, SessionContextResolver, SessionExecutionContext,
};
use crate::modules::harness::AgentEvent;
use crate::modules::learning::reflection::ReflectionEngine;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::runtime::block_conversion::{
    parse_tool_input_json, runtime_block_to_input_block, summarize_tool_result_for_model,
};
use crate::modules::runtime::compact::{compact_session, should_compact, CompactionConfig};
use crate::modules::runtime::contracts::agent_loop::{SkillResolutionPlan, WorkLoopDecision};
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::permissions::PermissionPromptDecision;
use crate::modules::runtime::resume_cursor::{
    build_resume_cursor, extract_resume_cursor_marker, parse_resume_cursor,
    session_contains_resume_cursor, strip_resume_cursor_marker,
};
use crate::modules::runtime::session::{
    ContentBlock, ConversationMessage, MessageRole, Session as RuntimeSession,
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
use crate::modules::runtime::supervisor::SupervisorOps;
use crate::modules::runtime::timeline_flush::{
    flush_assistant_timeline_segment, PersistedTurnOutcome,
};

use super::TurnService;

async fn append_stream_terminal_error(
    run_event_logger: &RunEventLogger,
    stage: &'static str,
    error: impl Into<String>,
) {
    let error = error.into();
    let _ = run_event_logger
        .append(
            "stream_error",
            serde_json::json!({
                "stage": stage,
                "error": error,
            }),
        )
        .await;
}

fn sanitize_compacted_system_block_for_provider(
    block: InputContentBlock,
) -> Option<InputContentBlock> {
    match block {
        InputContentBlock::Text { text } => {
            let sanitized = sanitize_compacted_continuation_directive(&text);
            (!sanitized.trim().is_empty()).then_some(InputContentBlock::Text { text: sanitized })
        }
        other => Some(other),
    }
}

fn sanitize_compacted_continuation_directive(text: &str) -> String {
    let mut sanitized_lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Continue the conversation from where it left off")
            || trimmed.starts_with("Resume directly")
            || trimmed.starts_with("do not acknowledge the summary")
            || trimmed.starts_with("do not recap what was happening")
            || trimmed.starts_with("do not preface with continuation text")
            || trimmed.starts_with("- Current work:")
            || trimmed.starts_with("Current work:")
            || trimmed.starts_with("- 当前工作:")
            || trimmed.starts_with("当前工作:")
        {
            continue;
        }
        sanitized_lines.push(line);
    }
    sanitized_lines.join("\n").trim().to_string()
}

async fn emit_terminal_final_run_report(
    stream_emitter: &AgentStreamEmitter,
    run_event_logger: &RunEventLogger,
    stream_id: &str,
    request_id: &str,
    work_loop_decision: &WorkLoopDecision,
    skill_resolution_plan: Option<&SkillResolutionPlan>,
    terminal_status: &str,
) {
    let final_report = super::work_loop::build_final_run_report(
        work_loop_decision,
        "failed".to_string(),
        terminal_status.to_string(),
        request_id.to_string(),
        0,
        false,
        false,
        false,
        None,
        skill_resolution_plan,
        Vec::new(),
    );
    let mut final_report_payload =
        StreamTokenPayload::skeleton(stream_id.to_string(), "final_run_report");
    final_report_payload.request_id = Some(request_id.to_string());
    final_report_payload.task_outcome = Some("failed".to_string());
    final_report_payload.degraded_reason = Some(terminal_status.to_string());
    final_report_payload.resume_available = Some(false);
    final_report_payload.tool_args = Some(
        serde_json::to_value(final_report)
            .unwrap_or_else(|error| serde_json::json!({ "serializationError": error.to_string() })),
    );
    stream_emitter.emit_payload(final_report_payload.clone());
    let _ = run_event_logger
        .append("final_run_report", final_report_payload)
        .await;
}

fn fallback_work_loop_decision(
    user_message: &str,
    session_id: Option<String>,
    project_id: Option<String>,
    workdir: std::path::PathBuf,
) -> WorkLoopDecision {
    let intelligence = crate::modules::application::request_intelligence_service::classify(
        crate::modules::application::request_intelligence_service::RequestIntelligenceInput {
            user_message: user_message.to_string(),
            session_id,
            project_id,
            workdir: Some(workdir),
        },
    );
    super::work_loop::route_work_loop(&intelligence.decision, user_message)
}

// `MAX_REQUEST_*` budget constants now live with their consumers
// in `stream_task.rs`; they are no longer referenced from
// `stream_turn`.

/// Per-call input bundle for [`TurnService::stream_turn`].
///
/// The first three fields mirror the legacy IPC parameters. The
/// last three carry per-process cross-stream coordination state
/// from `AppState` because that state is shared across sessions
/// and does not belong on the long-lived `TurnServiceDeps` bundle.
pub struct StreamTurnRequest {
    pub session_id: String,
    pub user_message: String,
    pub permission_mode: Option<String>,
    /// Per-stream cancellation senders keyed by `stream_id`.
    /// Used by `stop_agent_stream` to interrupt an in-flight
    /// streaming response.
    pub stream_cancel_senders: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
    /// Pending permission-prompt senders keyed by session id.
    pub permission_senders:
        Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<PermissionPromptDecision>>>>,
    /// Per-session "remember in this session" tool overrides.
    pub permission_overrides:
        Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
}

impl TurnService {
    /// Drive one streaming chat turn end to end.
    ///
    /// MIG-001-c — owns the synchronous prepare + spawn-task setup
    /// for the streaming path that used to live inside
    /// `commands::agent::start_agent_stream`. Returns the
    /// `stream_id` immediately; events are emitted on the Tauri
    /// channel via [`AgentStreamEmitter`] from the spawned task.
    ///
    /// MIG-001-d extracted the spawned tool-loop body into the
    /// sibling [`super::stream_task::run_stream_task`] free
    /// function; this method is now a thin sync prep + spawn
    /// wrapper. The public signature is final.
    ///
    /// # Errors
    ///
    /// Returns the same user-facing error strings as the legacy
    /// IPC body — main-window lookup failure, session-restoration
    /// failure, resume-cursor validation failure, provider /
    /// prompt failures, and `stream_cancel_senders` mutex
    /// poisoning.
    pub async fn stream_turn(&self, request: StreamTurnRequest) -> Result<String, String> {
        let session_id = request.session_id.clone();
        let user_message = request.user_message.clone();
        let permission_mode = request.permission_mode.clone();
        let run_id = uuid::Uuid::new_v4().to_string();
        let app_handle = self
            .deps
            .app_handle
            .as_ref()
            .ok_or_else(|| {
                "TurnService::stream_turn requires an AppHandle; \
                 the IPC adapter must construct TurnService with Some(app_handle)"
                    .to_string()
            })?
            .clone();

        // MIG-020 (T-006) / DR-01: Record supervisor lifecycle event — idle → running.
        if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
            SupervisorOps {
                app_data_dir: &app_data_dir,
                session_id: &session_id,
                app_handle: Some(&app_handle),
                run_event_logger: None,
            }
            .start_run(&run_id);
        }

        let run_event_logger =
            RunEventLogger::for_app_handle(&app_handle, session_id.clone(), run_id.clone());
        let stream_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(
            "[start_agent_stream] Starting - stream_id: {}, run_id: {}, session_id: {}, requested_permission_mode: {}",
            stream_id,
            run_id,
            session_id,
            permission_mode
                .as_deref()
                .unwrap_or("dangerFullAccess(default)")
        );
        let _ = run_event_logger
            .append(
                "run_started",
                serde_json::json!({
                    "caller": "start_agent_stream",
                    "stream_id": stream_id.clone(),
                    "permission_mode": permission_mode.clone(),
                    "message_preview": user_message.chars().take(160).collect::<String>(),
                }),
            )
            .await;

        // Get the main window for emitting events
        let window = match app_handle.get_webview_window("main") {
            Some(window) => window,
            None => {
                let message = "Failed to get main window".to_string();
                append_stream_terminal_error(&run_event_logger, "get_main_window", &message).await;
                return Err(message);
            }
        };
        let stream_emitter = AgentStreamEmitter::new(window);

        // Restore the session
        let app_session = match self.deps.session_manager.restore_session(&session_id).await {
            Ok(session) => session,
            Err(error) => {
                let message = error.to_string();
                append_stream_terminal_error(&run_event_logger, "restore_session", &message).await;
                return Err(message);
            }
        };

        tracing::info!(
            "[start_agent_stream] Session restored, {} messages",
            app_session.messages.len()
        );

        let mode = parse_permission_mode(permission_mode.as_deref());
        let execution_context = SessionContextResolver::new(
            self.deps.session_manager.clone(),
            self.deps.project_manager.clone(),
        )
        .resolve_from_session(&app_session, mode, "start_agent_stream")
        .await;
        log_context_fingerprint("start_agent_stream", &execution_context);

        let inbound_resume_cursor = extract_resume_cursor_marker(&user_message);
        if let Some(cursor_value) = inbound_resume_cursor.as_deref() {
            let parsed_cursor = match parse_resume_cursor(cursor_value) {
                Some(parsed_cursor) => parsed_cursor,
                None => {
                    let message = format!("invalid resume cursor: {cursor_value}");
                    append_stream_terminal_error(
                        &run_event_logger,
                        "resume_cursor_parse",
                        &message,
                    )
                    .await;
                    return Err(message);
                }
            };
            if !session_contains_resume_cursor(&app_session, &parsed_cursor) {
                let message = format!("resume cursor not found or expired: {cursor_value}");
                append_stream_terminal_error(&run_event_logger, "resume_cursor_lookup", &message)
                    .await;
                return Err(message);
            }
        }

        let normalized_user_message = if inbound_resume_cursor.is_some() {
            strip_resume_cursor_marker(&user_message)
        } else {
            user_message.trim().to_string()
        };

        if let Some(warn) = crate::modules::security::safety::shared_safety_layer()
            .scan_inbound_for_secrets(&normalized_user_message)
        {
            let _ = run_event_logger
                .append(
                    "stream_error",
                    serde_json::json!({
                        "stage": "inbound_secret_scan",
                        "error": warn,
                    }),
                )
                .await;
            return Err(warn);
        }

        let lifecycle_hooks =
            crate::modules::runtime::lifecycle_hooks::build_default_registry().await;
        let normalized_user_message = match lifecycle_hooks
            .run_point(
                crate::modules::runtime::lifecycle_hooks::HookPoint::BeforeInbound,
                Some(session_id.as_str()),
                normalized_user_message,
            )
            .await
        {
            Ok(t) => t,
            Err(reason) => {
                let _ = run_event_logger
                    .append(
                        "stream_error",
                        serde_json::json!({
                            "stage": "before_inbound_hook",
                            "error": reason,
                        }),
                    )
                    .await;
                return Err(reason);
            }
        };

        // Convert session messages to API format
        let runtime_session = app_session_to_runtime(&app_session);

        // Phase 5 (F1): apply WorkingMemory recency window before further
        // processing. Pure recency slice cut at user-message boundaries;
        // orphan tool_use/tool_result pairs are cleaned downstream by
        // sanitize_messages_for_provider during preflight (defense-in-depth).
        let windowed = crate::modules::memory::working_memory::window_recent_turns(
            &runtime_session.messages,
            crate::modules::runtime::budget::streaming_window_turns(),
        );

        // Phase 6 T3: compress old tool transcripts within the windowed slice.
        // Recent turns stay full-fidelity; older turns get summarized while
        // preserving tool_use_id ↔ tool_result.tool_use_id pairing so the
        // downstream sanitize pass doesn't drop them as orphans.
        let windowed =
            crate::modules::memory::tool_transcript_compression::compress_old_tool_transcripts(
                windowed,
                crate::modules::runtime::budget::stream_tool_keep_recent(),
            );

        let messages: Vec<InputMessage> = windowed
            .iter()
            .map(|msg| {
                let mut content: Vec<InputContentBlock> = msg
                    .blocks
                    .iter()
                    .map(runtime_block_to_input_block)
                    .collect();
                if msg.role == MessageRole::System {
                    content = content
                        .into_iter()
                        .filter_map(sanitize_compacted_system_block_for_provider)
                        .collect();
                }

                let role = match msg.role {
                    MessageRole::System => "user".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Tool => "user".to_string(),
                };

                // Phase 7 / reasoning-replay fix: ALWAYS preserve message.thinking
                // on InputMessage, including for tool_use rows. Providers with
                // thinking-mode quirks (DeepSeek-V4-Flash, Kimi-k2.5, …) require
                // the original `reasoning_content` echoed back; stripping it here
                // caused 400 errors (DeepSeek) and silent early-stops (Kimi).
                // The downstream `translate_message` in `openai_compat.rs` decides
                // whether to actually emit `reasoning_content` based on provider
                // capabilities — that's the right layer for the policy decision.
                InputMessage {
                    role,
                    content,
                    thinking: msg.thinking.clone(),
                }
            })
            .collect();

        // Add the user's new message
        let mut all_messages = messages;
        all_messages.push(InputMessage::user_text(&normalized_user_message));

        // Phase M1.4 — single application service seam composes
        // provider + memory injection (static + retrieved) + prompt
        // plan in one await.  `memory_items` is moved into the spawned
        // task and emitted on `stream_complete` so the frontend
        // `MemoryChip` / `MemoryEvidencePanel` can render them.
        // MIG-001-c — stream_turn is itself a TurnService method; reuse self.
        let stream_project_id_opt: Option<String> = if execution_context.project_id.is_empty() {
            None
        } else {
            Some(execution_context.project_id.clone())
        };
        let prepared_stream = match self
            .prepare_chat_inputs(PrepareChatInputsRequest {
                workdir: execution_context.workdir.clone(),
                current_date: crate::modules::runtime::logical_day::get_today().display,
                os_name: std::env::consts::OS.to_string(),
                os_family: std::env::consts::FAMILY.to_string(),
                session_id: Some(execution_context.session_id.clone()),
                project_id: stream_project_id_opt.clone(),
                workdir_str: execution_context.workdir.to_str().map(str::to_string),
                user_message: normalized_user_message.clone(),
                caller: "start_agent_stream",
                active_skill_ids: app_session.active_skill_ids.clone(),
            })
            .await
        {
            Ok(prepared) => prepared,
            Err(err) => {
                let (message, terminal_status) = match err {
                    TurnServiceError::Provider(msg) => {
                        tracing::error!(
                            "[start_agent_stream] Failed to create API client: {}",
                            msg
                        );
                        (msg, "provider_prepare_failed")
                    }
                    TurnServiceError::Prompt(p) => {
                        tracing::error!("[start_agent_stream] Prompt planning failed: {}", p);
                        (p.to_string(), "prompt_prepare_failed")
                    }
                };
                let fallback_work_loop = fallback_work_loop_decision(
                    &normalized_user_message,
                    Some(execution_context.session_id.clone()),
                    stream_project_id_opt.clone(),
                    execution_context.workdir.clone(),
                );
                emit_terminal_final_run_report(
                    &stream_emitter,
                    &run_event_logger,
                    &stream_id,
                    &run_id,
                    &fallback_work_loop,
                    None,
                    terminal_status,
                )
                .await;
                let _ = run_event_logger
                    .append(
                        "stream_error",
                        serde_json::json!({
                            "stage": "prepare_chat_inputs",
                            "error": message,
                        }),
                    )
                    .await;
                return Err(message);
            }
        };

        let definitions = self.deps.tool_registry.get_definitions(None);
        let tool_defs: Vec<crate::modules::api::ToolDefinition> = definitions
            .into_iter()
            .filter_map(|def| {
                let obj = def.as_object()?;
                let func = obj.get("function")?.as_object()?;
                Some(crate::modules::api::ToolDefinition {
                    name: func.get("name")?.as_str()?.to_string(),
                    description: func
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(String::from),
                    input_schema: func.get("parameters")?.clone(),
                })
            })
            .collect();
        let tool_pool = super::work_loop::build_canonical_tool_pool(
            tool_defs,
            &prepared_stream.active_skill_ids,
            &prepared_stream.work_loop_decision,
        );
        let tool_defs = tool_pool.definitions.clone();

        let execution_decision_value = serde_json::json!({
            "executionModeDecision": prepared_stream.execution_mode_decision,
            "workLoopDecision": prepared_stream.work_loop_decision,
        });
        let mut execution_decision_payload =
            StreamTokenPayload::skeleton(stream_id.clone(), "execution_mode_decision");
        execution_decision_payload.request_id = Some(run_id.clone());
        execution_decision_payload.tool_args = Some(execution_decision_value);
        stream_emitter.emit_payload(execution_decision_payload.clone());
        let _ = run_event_logger
            .append("execution_mode_decision", execution_decision_payload)
            .await;

        let skill_resolution_value = serde_json::to_value(&prepared_stream.skill_resolution_plan)
            .unwrap_or_else(|error| serde_json::json!({ "serializationError": error.to_string() }));
        let mut skill_resolution_payload =
            StreamTokenPayload::skeleton(stream_id.clone(), "skill_resolution_snapshot");
        skill_resolution_payload.request_id = Some(run_id.clone());
        skill_resolution_payload.tool_args = Some(skill_resolution_value);
        stream_emitter.emit_payload(skill_resolution_payload.clone());
        let _ = run_event_logger
            .append("skill_resolution_snapshot", skill_resolution_payload)
            .await;

        // MIG-002-a — request intelligence now acts as a real route gate.
        // Short-circuit for SpecializedSurface mode.
        use crate::modules::runtime::contracts::execution_mode::ExecutionMode;
        match prepared_stream.execution_mode_decision.execution_mode {
            ExecutionMode::SpecializedSurface => {
                tracing::info!(
                    execution_mode = ?prepared_stream.execution_mode_decision.execution_mode,
                    route_hint = ?prepared_stream.execution_mode_decision.route_hint,
                    "[start_agent_stream] Short-circuiting: SpecializedSurface mode"
                );
                let routed_message = format!(
                    "Request routed to specialized surface: {:?}",
                    prepared_stream.execution_mode_decision.route_hint
                );
                emit_terminal_final_run_report(
                    &stream_emitter,
                    &run_event_logger,
                    &stream_id,
                    &run_id,
                    &prepared_stream.work_loop_decision,
                    Some(&prepared_stream.skill_resolution_plan),
                    "specialized_surface_routed",
                )
                .await;
                append_stream_terminal_error(
                    &run_event_logger,
                    "execution_mode_gate",
                    &routed_message,
                )
                .await;
                return Err(routed_message);
            }
            ExecutionMode::DirectExecute
            | ExecutionMode::AutoPlanExecute
            | ExecutionMode::PlanThenConfirm => {
                // Continue with normal execution path
                tracing::info!(
                    execution_mode = ?prepared_stream.execution_mode_decision.execution_mode,
                    risk_level = ?prepared_stream.execution_mode_decision.risk_level,
                    complexity_level = ?prepared_stream.execution_mode_decision.complexity_level,
                    policy_version = %prepared_stream.execution_mode_decision.classifier_policy_version,
                    rules = ?prepared_stream.execution_mode_decision.classifier_matched_rule_ids,
                    "[start_agent_stream] request_intelligence decision (enforced route gate)"
                );
            }
        }
        if !prepared_stream.memory_items.is_empty() {
            tracing::info!(
                "[start_agent_stream] Injected {} memory items into prompt",
                prepared_stream.memory_items.len(),
            );
        }
        let memory_context_items_for_task: Vec<MemoryItemProjection> =
            prepared_stream.memory_items.clone();
        let prompt_diagnostics_for_task = prepared_stream.prompt.plan.diagnostics.to_summary();
        let prompt_diagnostics_enabled_for_task = prepared_stream.prompt_diagnostics_enabled;
        let RuntimeProviderResolution {
            provider_client,
            provider_id,
            model,
            context_window,
            request_timeout: _request_timeout,
        } = prepared_stream.provider;
        tracing::info!(
            "[start_agent_stream] resolved chat model provider_id='{}', model='{}', context_window={}",
            provider_id,
            model,
            context_window
        );
        let system_prompt_with_memory = prepared_stream.prompt.text;

        // Clone everything needed for the background task
        let session_manager = self.deps.session_manager.clone();
        let app_session_clone = app_session.clone();
        let user_message_clone = normalized_user_message.clone();
        let tool_registry_clone = self.deps.tool_registry.clone();
        let model_for_stream = model.clone();
        // P1-8 — build the routing chip payload from the classifier
        // decision + the resolved (post smart-routing) model name. We
        // detect the cheap-model swap by comparing against
        // `IF2AI_CHEAP_MODEL_ID`; absence means "no smart routing".
        let routing_info_for_stream = {
            let decision = &prepared_stream.execution_mode_decision;
            let cheap_match = std::env::var("IF2AI_CHEAP_MODEL_ID")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .map(|cheap| cheap == model_for_stream)
                .unwrap_or(false);
            Some(
                crate::modules::runtime::stream_emitter::RoutingInfoPayload {
                    complexity_score: decision.complexity_score,
                    complexity_level: format!("{:?}", decision.complexity_level).to_lowercase(),
                    execution_mode: format!("{:?}", decision.execution_mode).to_lowercase(),
                    used_cheap_model: cheap_match,
                    effective_model: model_for_stream.clone(),
                },
            )
        };
        let provider_client_for_stream = provider_client.clone();
        let failover_provider_client =
            crate::modules::application::provider_service::resolve_optional_failover_openai_client(
                &execution_context.workdir,
            )
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "[start_agent_stream] optional failover provider not available: {}",
                    e
                );
                None
            });
        let messages_for_stream = all_messages.clone();
        let tool_defs_for_stream = tool_defs.clone();
        let tool_pool_names_for_stream = tool_pool.tool_names.clone();
        let tool_pool_schema_hash_for_stream = tool_pool.schema_hash.clone();
        let tool_pool_policy_for_stream = tool_pool.policy.clone();
        let system_prompt_for_stream = system_prompt_with_memory;
        let permission_mode_for_stream = permission_mode.clone();
        let execution_context_for_task = execution_context.clone();
        // permission_senders / permission_overrides are passed to
        // the spawned task via `StreamTaskInputs` directly (see
        // construction below); no local bindings needed here.
        // Clones for post-turn learning (trajectory + self-model)
        let trajectory_manager_for_stream = self.deps.trajectory_manager.clone();
        let learning_module_for_stream = self.deps.learning_module.clone();
        let memory_provider_for_stream = self.deps.memory_provider.clone();
        // Phase 8B.11 fix — clone the ticker so the spawned stream task can
        // fire on_turn_complete after the LLM loop terminates (the streaming
        // path doesn't go through ConversationRuntime where the runtime
        // auto-fires the hook).
        let memory_ticker_for_stream = self.deps.memory_ticker.clone();
        // Phase 6E harness EventBus clone (zero-cost when harness disabled).
        let harness_event_bus_for_stream = self.deps.harness.as_ref().map(|h| h.event_bus.clone());
        let turn_number_for_stream = (app_session.messages.len() as u64) + 1;

        // Phase M3-B audit fix (extended in M4.1) — clone deps for
        // the spawned-task `dispatch_after_turn` call.  All `Arc`
        // clones; no perf cost.  Also capture the message-vector
        // baseline so the candidate extractor can slice "messages
        // added during this turn" inside the spawned task.
        let app_handle_for_after_turn = app_handle.clone();
        let pinned_store_for_after_turn = self.deps.pinned_store.clone();
        let memory_provider_for_after_turn = self.deps.memory_provider.clone();
        let active_retrieval_manager_for_after_turn = self.deps.active_retrieval_manager.clone();
        let stream_session_id_for_after_turn = execution_context.session_id.clone();
        let stream_project_id_for_after_turn = if execution_context.project_id.is_empty() {
            None
        } else {
            Some(execution_context.project_id.clone())
        };
        let baseline_message_count_stream = app_session.messages.len();
        let harness_bus_for_after_turn = harness_event_bus_for_stream.clone();

        // Spawn a background task to process the stream
        let stream_id_for_task = stream_id.clone();
        let stream_id_return = stream_id.clone();

        // Create a oneshot channel for cancellation
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        {
            let mut senders = request.stream_cancel_senders.lock().map_err(|e| {
                tracing::error!("[start_agent_stream] Failed to lock cancel senders: {}", e);
                e.to_string()
            })?;
            senders.insert(stream_id.clone(), cancel_tx);
        }

        // MIG-001-d — closure body lives in
        // [`super::stream_task::run_stream_task`]; here we just
        // bundle every captured value into [`StreamTaskInputs`]
        // and hand it to `tokio::spawn`. Behaviour is preserved
        // bit-for-bit; only the location of the spawned body
        // changed.
        let stream_task_inputs = super::stream_task::StreamTaskInputs {
            stream_id_for_task,
            run_event_logger,
            session_id: session_id.clone(),
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
            provider_id_for_stream: provider_id,
            context_window_for_stream: context_window,
            routing_info_for_stream,
            work_loop_decision_for_stream: prepared_stream.work_loop_decision,
            skill_resolution_plan_for_stream: prepared_stream.skill_resolution_plan,
            execution_context_for_task,
            tool_registry_clone,
            session_manager,
            rolling_summarizer_for_stream: self.deps.rolling_summarizer.clone(),
            app_session_clone,
            stream_emitter,
            cancel_rx,
            permission_senders: request.permission_senders.clone(),
            permission_overrides: request.permission_overrides.clone(),
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
            utility_llm: self.deps.utility_llm.clone(),
            loop_config: self.deps.loop_config.clone(),
            trajectory_collector: self.deps.trajectory_collector.clone(),
        };
        tokio::spawn(super::stream_task::run_stream_task(stream_task_inputs));

        // Return immediately with stream_id
        tracing::info!(
            "[start_agent_stream] Returning stream_id: {}",
            stream_id_return
        );
        Ok(stream_id_return)
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_compacted_continuation_directive;

    #[test]
    fn compacted_summary_drops_stale_resume_directive() {
        let sanitized = sanitize_compacted_continuation_directive(
            "Summary:\n- prior work\nContinue the conversation from where it left off without asking the user any further questions. Resume directly — do not acknowledge the summary, do not recap what was happening, and do not preface with continuation text.",
        );

        assert!(sanitized.contains("Summary:"));
        assert!(sanitized.contains("prior work"));
        assert!(!sanitized.contains("Resume directly"));
        assert!(!sanitized.contains("Continue the conversation from where it left off"));
    }

    #[test]
    fn compacted_summary_drops_stale_current_work_directive() {
        let sanitized = sanitize_compacted_continuation_directive(
            "Summary:\n- Current work: keep inspecting index.html\n- durable fact",
        );

        assert!(sanitized.contains("durable fact"));
        assert!(!sanitized.contains("Current work:"));
    }
}
