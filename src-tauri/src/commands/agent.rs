//! Agent commands - run_agent_turn and start_agent_stream
//!
//! Provides the main agent execution commands for Tauri.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono;
use tauri::{AppHandle, Emitter, State};
use tokio::time::timeout;

use crate::commands::stream_outcome::{ConversationTruth, ExecutionTruth, TaskOutcomeResolver};
use crate::commands::AppState;
use crate::modules::api::{
    InputContentBlock, InputMessage, MessageRequest, ProviderClient, ToolDefinition,
};
use crate::modules::application::memory_candidate_extractor::{
    extract_memory_store_tool_candidates, lookup_existing_records_for_candidates,
};
use crate::modules::application::memory_injection_service::MemoryInjectionDeps;
use crate::modules::application::{
    AfterTurnInput, ExistingRecordRef, MemoryCoordinator, MemoryItemProjection,
    PrepareChatInputsRequest, RuntimeProviderResolution, TurnService, TurnServiceDeps,
    TurnServiceError,
};
use crate::modules::control_plane::{
    AuditEmitter, SessionContextResolver, SessionExecutionContext, ToolExecutionBroker,
};
use crate::modules::harness::{AgentEvent, EventBus};
use crate::modules::learning::reflection::ReflectionEngine;
use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::working_memory::WorkingMemory;
use crate::modules::runtime::compact::{
    compact_session, estimate_token_count_from_chars, should_compact, CompactionConfig,
};
use crate::modules::runtime::contracts::memory::MemoryWriteCandidate;
use crate::modules::runtime::conversation::{
    ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError, ToolExecutor,
};
use crate::modules::runtime::episodic_compaction::WeibullDecay;
use crate::modules::runtime::permissions::{
    PermissionMode, PermissionPolicy, PermissionPromptDecision, PermissionPrompter,
    PermissionRequest,
};
use crate::modules::runtime::session::ConversationMessage;
use crate::modules::runtime::session::{ContentBlock, Session as RuntimeSession};
use crate::modules::runtime::snapshot::FrozenSnapshot;
use crate::modules::runtime::stream_emitter::MEMORY_AFTER_TURN_EVENT;
use crate::modules::runtime::stream_emitter::{
    AgentStreamEmitter, ContextBudgetUsagePayload, StreamTokenPayload,
};
use crate::modules::session::Session as AppSession;

/// Phase M1.1 — construct a per-call [`TurnService`] from the
/// already-shared `AppState` handles. Held as a small helper so the
/// IPC adapter does not have to repeat the dependency wiring at
/// every call site.
///
/// Note: [`TurnService`] is intentionally cheap to construct
/// (`Arc` clones only); it does not need to live on `AppState`
/// during the M1 transition.
fn make_turn_service(state: &AppState) -> TurnService {
    TurnService::new(TurnServiceDeps {
        tool_registry: state.tool_registry.clone(),
        pinned_store: state.pinned_store.clone(),
        memory_provider: state.memory_provider.clone(),
        active_retrieval_manager: state.active_retrieval_manager.clone(),
    })
}

/// Phase M4-A — stable governance trace contract version pinned
/// onto every `memory_after_turn` envelope (Tauri event + harness
/// `AgentEvent::MemoryAfterTurn`).  Bumping this string is a
/// breaking governance contract change; future graders / replay
/// MUST honor it.
pub const MEMORY_AFTER_TURN_TRACE_VERSION: &str = "memory-after-turn-trace@m4.1";

/// Phase M3-C closeout (extended in M4.1) — run the
/// [`MemoryCoordinator::after_turn`] write-policy / quality-gate /
/// conflict-resolver pipeline at the end of a turn and emit the
/// **batch envelope** through both:
///
///   1. the frontend [`MEMORY_AFTER_TURN_EVENT`] Tauri channel
///      (drives the runtime-projection store), and
///   2. the harness [`EventBus`] as
///      [`AgentEvent::MemoryAfterTurn`] (drives M4 trace sinks /
///      future grader components).
///
/// M4.1 — `candidates` is now sourced from
/// [`extract_memory_store_tool_candidates`] for `memory_store`
/// tool calls observed during the turn; `existing_records` is now
/// sourced from
/// [`lookup_existing_records_for_candidates`] so the conflict
/// resolver flips from "always NoConflict" to producing real
/// outcomes for same-key writes.  The batch envelope still fires
/// even when both arrays are empty — the empty case is the
/// explicit "no candidates this turn" signal (M3-C contract).
///
/// Emit failure on either channel is logged at TRACE — never
/// blocks the turn.
fn dispatch_after_turn(
    app_handle: &AppHandle,
    harness_bus: Option<&EventBus>,
    injection_deps: MemoryInjectionDeps,
    session_id: Option<String>,
    project_id: Option<String>,
    candidates: Vec<MemoryWriteCandidate>,
    existing_records: Vec<Option<ExistingRecordRef>>,
    reflection_notes: Vec<crate::modules::learning::reflection_note::ReflectionNote>,
    caller: &'static str,
) {
    debug_assert_eq!(
        candidates.len(),
        existing_records.len(),
        "candidate / existing_record arrays must be parallel"
    );
    let coordinator = MemoryCoordinator::with_default_policy(injection_deps);
    let output = coordinator.after_turn(AfterTurnInput {
        session_id: session_id.clone(),
        project_id: project_id.clone(),
        candidates,
        existing_records,
        reflection_notes,
        caller,
    });
    // RFC3339 timestamp for the batch envelope; per-decision
    // `decidedAt` lives inside each `MemoryWriteDecision`.
    let decided_at_dt = chrono::Utc::now();
    let decided_at_rfc = decided_at_dt.to_rfc3339();

    // (1) Frontend transport channel.
    let payload = serde_json::json!({
        "traceVersion": MEMORY_AFTER_TURN_TRACE_VERSION,
        "caller": caller,
        "policyVersion": output.policy_version,
        "decidedAt": decided_at_rfc,
        "decisions": output.decisions,
        "quality": output.quality,
        "conflicts": output.conflicts,
    });
    if let Err(err) = app_handle.emit(MEMORY_AFTER_TURN_EVENT, payload) {
        tracing::trace!(
            event = MEMORY_AFTER_TURN_EVENT,
            error = %err,
            "[after_turn] memory_after_turn emit failed (non-fatal)"
        );
    }

    // (2) Harness EventBus — M4.2 ground-truth seam.  Zero
    // overhead when `harness_bus` is `None` (no bus subscribed).
    if let Some(bus) = harness_bus {
        let event = AgentEvent::MemoryAfterTurn {
            trace_version: MEMORY_AFTER_TURN_TRACE_VERSION,
            caller,
            session_id,
            project_id,
            policy_version: output.policy_version,
            decided_at: decided_at_dt,
            decisions: output.decisions,
            quality: output.quality,
            conflicts: output.conflicts,
        };
        if let Err(err) = bus.emit(event) {
            tracing::trace!(
                error = %err,
                "[after_turn] harness MemoryAfterTurn emit failed (non-fatal)"
            );
        }
    }
}

// `ContextBudgetUsagePayload` / `StreamTokenPayload` moved to
// [`crate::modules::runtime::stream_emitter`] in Phase M1.5.
// Memory item projection moved to
// [`crate::modules::application::memory_injection_service::MemoryItemProjection`]
// in Phase M1.4.

#[derive(Debug, Clone)]
struct PersistedTurnOutcome {
    task_outcome: String,
    degraded_reason: Option<String>,
    resume_available: bool,
    resume_cursor: Option<String>,
    request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResumeCursor {
    stream_id: String,
    tool_loop_iter: usize,
    token_count: u32,
}

const MAX_TOOL_RESULT_FOR_MODEL_CHARS: usize = 8_000;
const TOOL_RESULT_PREVIEW_CHARS: usize = 320;
const MAX_REQUEST_MESSAGE_COUNT: usize = 180;
const MAX_REQUEST_CHAR_BUDGET: usize = 120_000;
const MAX_REQUEST_TOKEN_BUDGET_ESTIMATE: usize = 30_000;
const MAX_STREAM_RETRY_ON_TIMEOUT: usize = 1;

/// Response from a run_agent_turn command.
#[derive(serde::Serialize)]
pub struct RunAgentTurnResponse {
    /// The generated message/response.
    pub message: String,
    /// The session ID.
    pub session_id: String,
    /// Thinking content from the model (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

// Provider runtime resolution moved to
// `crate::modules::application::provider_service` in Phase M1.2.
// Call sites below now go through `TurnService::prepare_chat_inputs`,
// which delegates to
// [`crate::modules::application::provider_service::resolve_chat_runtime_provider`].

fn flush_assistant_timeline_segment(
    timeline_messages: &mut Vec<crate::modules::runtime::session::ConversationMessage>,
    accumulated_text: &mut String,
    accumulated_thinking: &mut String,
    persisted_outcome: Option<&PersistedTurnOutcome>,
) -> bool {
    if accumulated_text.is_empty() && accumulated_thinking.is_empty() {
        return false;
    }

    let text = std::mem::take(accumulated_text);
    let thinking = std::mem::take(accumulated_thinking);

    timeline_messages.push(crate::modules::runtime::session::ConversationMessage {
        role: crate::modules::runtime::session::MessageRole::Assistant,
        blocks: vec![ContentBlock::Text { text }],
        usage: None,
        thinking: if thinking.is_empty() {
            None
        } else {
            Some(thinking)
        },
        task_outcome: persisted_outcome.map(|value| value.task_outcome.clone()),
        degraded_reason: persisted_outcome.and_then(|value| value.degraded_reason.clone()),
        resume_available: persisted_outcome.map(|value| value.resume_available),
        resume_cursor: persisted_outcome.and_then(|value| value.resume_cursor.clone()),
        request_id: persisted_outcome.map(|value| value.request_id.clone()),
    });

    true
}

/// Convert application session to runtime session.
///
/// The application session has extra metadata (id, title, etc.) that we don't need
/// for the runtime. We only need the messages.
fn app_session_to_runtime(app_session: &AppSession) -> RuntimeSession {
    RuntimeSession {
        version: 1,
        messages: app_session.messages.clone(),
    }
}

/// Resolve session execution context for this turn/session.
pub(crate) async fn resolve_session_execution_context(
    state: &AppState,
    app_session: &AppSession,
    permission_mode: PermissionMode,
    caller: &str,
) -> SessionExecutionContext {
    let resolver =
        SessionContextResolver::new(state.session_manager.clone(), state.project_manager.clone());
    resolver
        .resolve_from_session(app_session, permission_mode, caller)
        .await
}

fn log_context_fingerprint(caller: &str, context: &SessionExecutionContext) {
    let fingerprint =
        crate::modules::tools::context::context_fingerprint(&context.session_id, &context.workdir);
    tracing::info!(
        "[{}] context fingerprint='{}', session_id='{}', workdir='{}', permission_mode='{}'",
        caller,
        fingerprint,
        context.session_id,
        context.workdir.display(),
        context.permission_mode.as_str()
    );
}

#[derive(Debug, Clone)]
struct ControlPlaneRuntimeSwitches {
    control_plane_v2_enabled: bool,
    boundary_enforce_mode: crate::modules::runtime::config::BoundaryEnforceMode,
    sandbox_strict_mode: bool,
}

fn load_control_plane_switches(workdir: &std::path::Path) -> ControlPlaneRuntimeSwitches {
    let mut switches = crate::modules::runtime::config::ConfigLoader::default_for(workdir)
        .load()
        .map(|loaded| ControlPlaneRuntimeSwitches {
            control_plane_v2_enabled: loaded.control_plane().control_plane_v2_enabled(),
            boundary_enforce_mode: loaded.control_plane().boundary_enforce_mode(),
            sandbox_strict_mode: loaded.control_plane().sandbox_strict_mode(),
        })
        .unwrap_or(ControlPlaneRuntimeSwitches {
            control_plane_v2_enabled: true,
            boundary_enforce_mode: crate::modules::runtime::config::BoundaryEnforceMode::Enforce,
            sandbox_strict_mode: true,
        });
    if let Ok(value) = std::env::var("IF2AI_CONTROL_PLANE_V2_ENABLED") {
        switches.control_plane_v2_enabled = value != "0";
    }
    if let Ok(value) = std::env::var("IF2AI_BOUNDARY_ENFORCE_MODE") {
        switches.boundary_enforce_mode = if value.eq_ignore_ascii_case("shadow") {
            crate::modules::runtime::config::BoundaryEnforceMode::Shadow
        } else {
            crate::modules::runtime::config::BoundaryEnforceMode::Enforce
        };
    }
    if let Ok(value) = std::env::var("IF2AI_SANDBOX_STRICT_MODE") {
        switches.sandbox_strict_mode = value != "0";
    }
    switches
}

/// Real API client that calls the Claw API (Claude/MiniMax).
///
/// This implements the `ApiClient` trait and makes real LLM API calls.
struct RealApiClient {
    provider: ProviderClient,
    model: String,
    request_timeout: Duration,
    tool_registry: Arc<crate::modules::tools::ToolRegistry>,
}

impl RealApiClient {
    fn new(
        provider: ProviderClient,
        model: String,
        request_timeout: Duration,
        tool_registry: Arc<crate::modules::tools::ToolRegistry>,
    ) -> Self {
        Self {
            provider,
            model,
            request_timeout,
            tool_registry,
        }
    }
}

impl ApiClient for RealApiClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        // Use block_in_place to run async code in a blocking context
        // This allows us to call async functions from the sync stream method
        let result = tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(async move {
                let api_future = self.call_api(request);
                timeout(self.request_timeout, api_future).await
            })
        });

        let response = result
            .map_err(|_| RuntimeError::ApiError("API call timed out".to_string()))?
            .map_err(|e| RuntimeError::ApiError(e.to_string()))?;

        // Convert MessageResponse to Vec<AssistantEvent>
        let mut events = Vec::new();

        for block in &response.content {
            match block {
                crate::modules::api::OutputContentBlock::Text { text } => {
                    events.push(AssistantEvent::TextDelta(text.clone()));
                }
                crate::modules::api::OutputContentBlock::ToolUse { id, name, input } => {
                    events.push(AssistantEvent::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: serde_json::to_string(input).unwrap_or_default(),
                    });
                }
                crate::modules::api::OutputContentBlock::Thinking { thinking, .. } => {
                    events.push(AssistantEvent::Thinking(thinking.clone()));
                }
                crate::modules::api::OutputContentBlock::RedactedThinking { .. } => {
                    // Skip redacted thinking blocks - don't expose internal data
                }
            }
        }

        events.push(AssistantEvent::Usage(
            crate::modules::runtime::usage::TokenUsage {
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                cache_creation_input_tokens: response.usage.cache_creation_input_tokens,
                cache_read_input_tokens: response.usage.cache_read_input_tokens,
            },
        ));

        events.push(AssistantEvent::MessageStop);

        Ok(events)
    }
}

impl RealApiClient {
    /// Call the API asynchronously
    async fn call_api(
        &self,
        request: ApiRequest,
    ) -> Result<crate::modules::api::MessageResponse, crate::modules::api::ApiError> {
        // Convert ApiRequest to MessageRequest
        let messages: Vec<InputMessage> = request
            .messages
            .iter()
            .map(|msg| {
                let content: Vec<InputContentBlock> = msg
                    .blocks
                    .iter()
                    .map(runtime_block_to_input_block)
                    .collect();

                let role = match msg.role {
                    crate::modules::runtime::session::MessageRole::System => "user".to_string(),
                    crate::modules::runtime::session::MessageRole::User => "user".to_string(),
                    crate::modules::runtime::session::MessageRole::Assistant => {
                        "assistant".to_string()
                    }
                    crate::modules::runtime::session::MessageRole::Tool => "user".to_string(),
                };

                InputMessage { role, content }
            })
            .collect();

        let system_prompt = if request.system_prompt.is_empty() {
            None
        } else {
            Some(request.system_prompt.join("\n"))
        };

        // Prefer tool definitions from request.tools; fall back to registry
        let tool_defs: Option<Vec<ToolDefinition>> = request.tools.clone().or_else(|| {
            let definitions = self.tool_registry.get_definitions(None);
            Some(
                definitions
                    .into_iter()
                    .filter_map(|def| {
                        let obj = def.as_object()?;
                        let func = obj.get("function")?.as_object()?;
                        Some(ToolDefinition {
                            name: func.get("name")?.as_str()?.to_string(),
                            description: func
                                .get("description")
                                .and_then(|d| d.as_str())
                                .map(String::from),
                            input_schema: func.get("parameters")?.clone(),
                        })
                    })
                    .collect(),
            )
        });
        let tools = tool_defs.filter(|t| !t.is_empty());

        let api_request = MessageRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages,
            system: system_prompt,
            tools,
            tool_choice: None,
            stream: false,
        };

        self.provider.send_message(&api_request).await
    }
}

/// Bridge from async ToolRegistry to sync ToolExecutor trait.
///
/// This allows ConversationRuntime to use the ToolRegistry for tool calls.
struct ToolRegistryExecutor {
    tool_registry: Arc<crate::modules::tools::ToolRegistry>,
    broker: ToolExecutionBroker,
    execution_context: SessionExecutionContext,
}

impl ToolRegistryExecutor {
    fn new_with_context(
        tool_registry: Arc<crate::modules::tools::ToolRegistry>,
        execution_context: SessionExecutionContext,
    ) -> Self {
        Self {
            tool_registry: tool_registry.clone(),
            broker: ToolExecutionBroker::new(tool_registry),
            execution_context,
        }
    }

    fn execute_with_trace(
        &mut self,
        tool_name: &str,
        input: &str,
        trace_id: &str,
        request_id: Option<&str>,
    ) -> Result<String, crate::modules::runtime::conversation::ToolError> {
        let args = parse_tool_input_json(input);
        let switches = load_control_plane_switches(&self.execution_context.workdir);
        tracing::info!(
            "[tool_executor] control_plane_v2_enabled={}, boundary_enforce_mode={}, sandbox_strict_mode={}",
            switches.control_plane_v2_enabled,
            switches.boundary_enforce_mode.as_str(),
            switches.sandbox_strict_mode
        );
        let result = tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::current();
            if switches.control_plane_v2_enabled {
                handle.block_on(self.broker.execute_with_trace(
                    &self.execution_context,
                    tool_name,
                    args,
                    trace_id,
                    request_id,
                ))
            } else {
                tracing::warn!(
                    "[tool_executor] controlPlaneV2Enabled=false, falling back to direct dispatch_with_context"
                );
                // Phase 7C, slice 7C.2 — registry now returns ToolOutput;
                // collapse to legacy String here so the existing executor
                // contract (Result<String, ToolError>) stays intact.  Slice
                // 7C.3+ will lift the broker + executor to ToolOutput.
                handle.block_on(self.tool_registry.dispatch_with_context_legacy(
                    tool_name,
                    args,
                    self.broker.to_tool_context(&self.execution_context),
                ))
            }
        })
        .map_err(|e: crate::modules::tools::ToolError| {
            crate::modules::runtime::conversation::ToolError::new(e.to_string())
        })?;
        Ok(result)
    }
}

impl ToolExecutor for ToolRegistryExecutor {
    fn execute(
        &mut self,
        tool_name: &str,
        input: &str,
    ) -> Result<String, crate::modules::runtime::conversation::ToolError> {
        let trace_id = AuditEmitter::new_trace_id();
        self.execute_with_trace(tool_name, input, &trace_id, None)
    }

    fn get_definitions(&self) -> Vec<crate::modules::api::ToolDefinition> {
        let definitions = self.tool_registry.get_definitions(None);
        definitions
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
            .collect()
    }
}

/// Parse a permission_mode string into PermissionMode enum.
pub(crate) fn parse_permission_mode(mode: Option<&str>) -> PermissionMode {
    match mode {
        Some("readOnly") | Some("read-only") | Some("read_only") => PermissionMode::ReadOnly,
        Some("workspaceWrite") | Some("workspace-write") | Some("workspace_write") => {
            PermissionMode::WorkspaceWrite
        }
        Some("prompt") => PermissionMode::Prompt,
        Some("dangerFullAccess")
        | Some("danger-full-access")
        | Some("danger_full_access")
        | None => PermissionMode::DangerFullAccess,
        _ => PermissionMode::DangerFullAccess,
    }
}

/// Build tool-level permission policy for the active mode.
///
/// Read-only tools are allowed in all modes.
/// Workspace-write tools require at least WorkspaceWrite.
/// Dangerous/system tools require DangerFullAccess (or prompt escalation).
pub(crate) fn build_permission_policy(mode: PermissionMode) -> PermissionPolicy {
    PermissionPolicy::new(mode)
        // Read-only tools
        .with_tool_requirement("read_file", PermissionMode::ReadOnly)
        .with_tool_requirement("glob_search", PermissionMode::ReadOnly)
        .with_tool_requirement("grep_search", PermissionMode::ReadOnly)
        .with_tool_requirement("content_search", PermissionMode::ReadOnly)
        .with_tool_requirement("web_fetch", PermissionMode::ReadOnly)
        .with_tool_requirement("web_search", PermissionMode::ReadOnly)
        .with_tool_requirement("WebFetch", PermissionMode::ReadOnly)
        .with_tool_requirement("WebSearch", PermissionMode::ReadOnly)
        .with_tool_requirement("tool_search", PermissionMode::ReadOnly)
        .with_tool_requirement("ToolSearch", PermissionMode::ReadOnly)
        .with_tool_requirement("json_parse", PermissionMode::ReadOnly)
        .with_tool_requirement("skill", PermissionMode::ReadOnly)
        .with_tool_requirement("skill_search", PermissionMode::ReadOnly)
        .with_tool_requirement("skill_find", PermissionMode::ReadOnly)
        .with_tool_requirement("skill_view", PermissionMode::ReadOnly)
        .with_tool_requirement("skills_categories", PermissionMode::ReadOnly)
        .with_tool_requirement("skill_manage", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("memory_recall", PermissionMode::ReadOnly)
        .with_tool_requirement("memory_export", PermissionMode::ReadOnly)
        .with_tool_requirement("cron_list", PermissionMode::ReadOnly)
        .with_tool_requirement("sleep", PermissionMode::ReadOnly)
        .with_tool_requirement("Sleep", PermissionMode::ReadOnly)
        .with_tool_requirement("SendUserMessage", PermissionMode::ReadOnly)
        .with_tool_requirement("structured_output", PermissionMode::ReadOnly)
        .with_tool_requirement("StructuredOutput", PermissionMode::ReadOnly)
        // Workspace-write tools
        .with_tool_requirement("file_write", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("file_edit", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("edit_file", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("NotebookEdit", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("memory_store", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("memory_forget", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("memory_purge", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("TodoWrite", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("Config", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("cron_add", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("cron_remove", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("cron_run", PermissionMode::WorkspaceWrite)
        // Dangerous/system tools
        .with_tool_requirement("bash", PermissionMode::DangerFullAccess)
        .with_tool_requirement("PowerShell", PermissionMode::DangerFullAccess)
        .with_tool_requirement("REPL", PermissionMode::DangerFullAccess)
        .with_tool_requirement("http_request", PermissionMode::DangerFullAccess)
        .with_tool_requirement("agent", PermissionMode::DangerFullAccess)
}

fn contains_unverified_file_claim(text: &str) -> bool {
    let lower = text.to_lowercase();
    let patterns = [
        "已创建",
        "已写入",
        "已删除",
        "已修改",
        "created",
        "written",
        "deleted",
        "successfully created",
        "successfully wrote",
        "successfully deleted",
    ];
    patterns
        .iter()
        .any(|p| text.contains(p) || lower.contains(p))
}

fn is_mutating_tool_success(tool_name: &str, input_json: &str, is_error: bool) -> bool {
    if is_error {
        return false;
    }

    let write_tools = [
        "file_write",
        "write_file",
        "file_edit",
        "edit_file",
        "NotebookEdit",
        "TodoWrite",
        "memory_store",
        "memory_forget",
        "memory_purge",
        "cron_add",
        "cron_remove",
        "cron_run",
    ];
    if write_tools.contains(&tool_name) {
        return true;
    }

    // Shell-like tools can mutate files; inspect command heuristically.
    if ["bash", "PowerShell", "REPL"].contains(&tool_name) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(input_json) {
            let command = value.get("command").and_then(|v| v.as_str()).unwrap_or("");
            return shell_command_likely_mutates_files(command);
        }
    }

    false
}

fn shell_command_likely_mutates_files(command: &str) -> bool {
    let normalized = command.to_lowercase();
    let mutation_markers = [
        "rm ",
        "mv ",
        "cp ",
        "touch ",
        "mkdir ",
        "rmdir ",
        "chmod ",
        "chown ",
        "sed -i",
        "perl -0pi",
        "python -c",
        "python - <<",
        "node -e",
        "tee ",
        "printf ",
        "cat >",
        "cat <<",
        "echo >",
        "echo >>",
        ">>",
        " > ",
        "| tee",
        "git add",
        "git mv",
        "git rm",
        "install -d",
    ];

    mutation_markers
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn extract_skill_proposal_name(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("skill_proposal:"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
// harness symbol marker: skill_proposal|draft|approval

// `RetrievedMemoryContext` / `map_scored_memory_to_payload` /
// `retrieve_memory_context` moved to
// [`crate::modules::application::memory_injection_service`] in
// Phase M1.4. Call sites below now resolve memory through the
// `TurnService` seam.

/// Record the conversation as a trajectory for future RL training.
///
/// Uses the `AppState`-level `TrajectoryManager` when available to avoid
/// re-creating the manager (and re-scanning the directory) on every turn.
/// Falls back to constructing a one-off manager if the state-level one is
/// absent (e.g. during tests or early startup).
///
/// Errors are logged as warnings and never block the main flow.
/// Short sessions (<3 turns) are silently skipped per privacy defaults.
async fn record_trajectory_if_possible(
    session: &RuntimeSession,
    system_prompt: &[String],
    tm: Option<&std::sync::Arc<TrajectoryManager>>,
) {
    let system_text = system_prompt.join("\n");

    // Prefer the shared AppState manager.
    if let Some(manager) = tm {
        match manager.record(session, &system_text, "if2ai-default").await {
            Ok(id) => tracing::info!("[record_trajectory] Recorded trajectory {id}"),
            Err(e) => tracing::debug!("[record_trajectory] Skipping trajectory record: {e}"),
        }
        return;
    }

    // Fallback: create a temporary manager.
    let trajectories_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("trajectories");

    let manager = match TrajectoryManager::new(trajectories_dir) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("[record_trajectory] Failed to create TrajectoryManager: {e}");
            return;
        }
    };

    match manager.record(session, &system_text, "if2ai-default").await {
        Ok(id) => tracing::info!("[record_trajectory] Recorded trajectory {id}"),
        Err(e) => tracing::debug!("[record_trajectory] Skipping trajectory record: {e}"),
    }
}

/// Run a single agent turn with the given user message.
///
/// This is the main entry point for the frontend to interact with the agent.
/// It calls the ConversationRuntime with the session and returns the result.
#[tauri::command]
#[allow(dead_code)]
pub async fn run_agent_turn(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    session_id: String,
    user_message: String,
    permission_mode: Option<String>,
) -> Result<RunAgentTurnResponse, String> {
    eprintln!(
        "[DEBUG] run_agent_turn called with session_id: {}, message: {}",
        session_id, user_message
    );
    tracing::info!(
        "[run_agent_turn] Starting - session_id: {}, message: {}",
        session_id,
        user_message
    );

    // Restore the session
    let app_session = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "[run_agent_turn] Session restored, {} messages",
        app_session.messages.len()
    );

    // Phase 6E harness: emit TurnStarted onto the EventBus.  The bus is
    // `None` in production unless `IF2AI_HARNESS_ENABLED=1`, so this is a
    // zero-cost no-op for typical users.  Turn number is derived from the
    // current message count to keep the API stateless.
    let harness_event_bus_run = state.harness.as_ref().map(|h| h.event_bus.clone());
    let turn_number_run = (app_session.messages.len() as u64) + 1;
    // Phase M4.1 — capture the message-vector baseline so the
    // turn-end candidate extractor can slice "messages added
    // during this turn" without ambiguity.
    let baseline_message_count_run = app_session.messages.len();
    crate::modules::harness::agent_loop_integration::emit_turn_started(
        harness_event_bus_run.as_ref(),
        &session_id,
        turn_number_run,
    );
    let turn_started_at_run = std::time::Instant::now();

    let mode = parse_permission_mode(permission_mode.as_deref());

    // Create a per-turn execution context to avoid cross-session context leakage.
    let execution_context =
        resolve_session_execution_context(&state, &app_session, mode, "run_agent_turn").await;
    let proposal_workdir = execution_context.workdir.clone();
    log_context_fingerprint("run_agent_turn", &execution_context);

    // Convert application session to runtime session
    let runtime_session = app_session_to_runtime(&app_session);

    // Phase M1.4 — memory retrieval + injection now flow through
    // `TurnService::prepare_chat_inputs` (which delegates to
    // `application::memory_injection_service`). This single seam
    // produces the provider, the prompt plan, and the per-turn
    // memory items in one await.
    let turn_service = make_turn_service(&state);
    let project_id_opt: Option<String> = if execution_context.project_id.is_empty() {
        None
    } else {
        Some(execution_context.project_id.clone())
    };
    let prepared = turn_service
        .prepare_chat_inputs(PrepareChatInputsRequest {
            workdir: execution_context.workdir.clone(),
            current_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            os_name: std::env::consts::OS.to_string(),
            os_family: std::env::consts::FAMILY.to_string(),
            session_id: Some(execution_context.session_id.clone()),
            project_id: project_id_opt.clone(),
            workdir_str: execution_context.workdir.to_str().map(str::to_string),
            user_message: user_message.clone(),
            caller: "run_agent_turn",
        })
        .await
        .map_err(|err| match err {
            TurnServiceError::Provider(msg) => {
                tracing::error!("[run_agent_turn] Failed to create API client: {}", msg);
                format!("Failed to connect to AI service: {msg}")
            }
            TurnServiceError::Prompt(p) => {
                tracing::error!("[run_agent_turn] Prompt planning failed: {}", p);
                p.to_string()
            }
        })?;
    // Phase M1.6 — log the request-intelligence decision so harness
    // / operators can observe routing today even though the existing
    // single execution path keeps running.
    tracing::info!(
        execution_mode = ?prepared.execution_mode_decision.execution_mode,
        risk_level = ?prepared.execution_mode_decision.risk_level,
        complexity_level = ?prepared.execution_mode_decision.complexity_level,
        policy_version = %prepared.execution_mode_decision.classifier_policy_version,
        rules = ?prepared.execution_mode_decision.classifier_matched_rule_ids,
        "[run_agent_turn] request_intelligence decision (advisory)"
    );
    let RuntimeProviderResolution {
        provider_client,
        model,
        request_timeout,
    } = prepared.provider;
    tracing::info!("[run_agent_turn] API client created, model: {}", model);
    let api_client = RealApiClient::new(
        provider_client,
        model,
        request_timeout,
        state.tool_registry.clone(),
    );

    // Create permission policy from parameter (defaults to DangerFullAccess)
    let permission_policy = build_permission_policy(mode);
    tracing::info!(
        "[run_agent_turn] effective permission mode: {}",
        mode.as_str()
    );

    // Create tool executor bridge
    let tool_executor =
        ToolRegistryExecutor::new_with_context(state.tool_registry.clone(), execution_context);

    // Phase M1.3 — prompt vec + joined text both come from the
    // structured plan. The `Vec<String>` shape is preserved for
    // backwards compatibility with the existing
    // `ConversationRuntime::with_system_prompt(...)` call below.
    let system_prompt: Vec<String> = prepared
        .prompt
        .plan
        .blocks
        .iter()
        .map(|b| b.content.clone())
        .collect();
    let system_prompt_text = prepared.prompt.text;
    let frozen_snapshot = FrozenSnapshot::capture(&system_prompt_text);
    tracing::info!(
        "[run_agent_turn] Frozen snapshot captured, prompt hash={}, estimate={} tokens",
        frozen_snapshot.prompt_hash,
        frozen_snapshot.token_estimate()
    );

    // Create runtime with working-memory sliding window (C1 integration).
    // Each LLM call will only see the most recent turns within the token budget,
    // while full history is preserved in session for compaction / trajectory.
    // Phase 8B.11 fix — wire MemoryTicker as TurnHook + supply
    // session/project context so RollingSummarizer + compile_today
    // actually fire on each turn (the 8A.7 default fired with
    // session_id="-" which the ticker silently skipped).
    let session_ctx_id = tool_executor.execution_context.session_id.clone();
    let session_ctx_project = if tool_executor.execution_context.project_id.is_empty() {
        None
    } else {
        Some(tool_executor.execution_context.project_id.clone())
    };

    let mut runtime = ConversationRuntime::new(
        runtime_session,
        api_client,
        tool_executor,
        permission_policy,
        system_prompt,
    )
    .with_context_budget(state.context_budget.clone())
    .with_working_memory(WorkingMemory::default())
    .with_turn_hook(state.memory_ticker.clone())
    .with_session_context(session_ctx_id.clone(), session_ctx_project.clone());

    tracing::info!(
        "[run_agent_turn] Runtime created, calling run_turn with message: {}",
        user_message
    );

    // Run the conversation turn
    let result = runtime.run_turn(user_message.clone(), None);

    tracing::info!(
        "[run_agent_turn] run_turn completed, result: {:?}",
        result.is_ok()
    );

    match result {
        Ok(summary) => {
            // Extract text from assistant messages
            let response_text = summary
                .assistant_messages
                .iter()
                .filter_map(|msg| {
                    msg.blocks.iter().find_map(|block| {
                        if let ContentBlock::Text { text } = block {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                })
                .collect::<Vec<_>>()
                .join("\n");

            // Extract thinking content from assistant messages
            let thinking_content: Option<String> = {
                let collected: Vec<String> = summary
                    .assistant_messages
                    .iter()
                    .filter_map(|msg| msg.thinking.clone())
                    .collect();
                let joined = collected.join("\n\n");
                if joined.is_empty() {
                    None
                } else {
                    Some(joined)
                }
            };

            let final_text = if response_text.is_empty() {
                "Agent completed the request.".to_string()
            } else {
                response_text
            };
            let final_text = if let Some(proposal_name) = extract_skill_proposal_name(&final_text) {
                match crate::modules::tools::builtin::skill::create_agent_skill_proposal_draft(
                    &proposal_workdir,
                    &proposal_name,
                    &final_text,
                ) {
                    Ok(path) => format!(
                        "{final_text}\n\n[skill_proposal] draft created at {} (requires approval)",
                        path.display()
                    ),
                    Err(err) => {
                        format!("{final_text}\n\n[skill_proposal] draft create failed: {err}")
                    }
                }
            } else {
                final_text
            };

            // Get the updated session from the runtime
            let updated_runtime_session = runtime.into_session();
            let next_message_count = app_session.logical_message_count()
                + updated_runtime_session
                    .messages
                    .len()
                    .saturating_sub(app_session.messages.len());

            // Keep a clone for trajectory recording (before compaction may consume it)
            let trajectory_session = updated_runtime_session.clone();

            // Context compaction — compact if session exceeds token threshold
            let compaction_config = CompactionConfig::default();
            let pre_compact_message_count = updated_runtime_session.messages.len();
            let final_runtime_session =
                if should_compact(&updated_runtime_session, compaction_config) {
                    let compact_result =
                        compact_session(&updated_runtime_session, compaction_config);
                    compact_result.compacted_session
                } else {
                    updated_runtime_session
                };
            let post_compact_message_count = final_runtime_session.messages.len();

            // Update the application session with the new messages
            let mut updated_app_session = app_session;
            updated_app_session.messages = final_runtime_session.messages;
            updated_app_session.message_count = next_message_count;

            // Save the updated session
            state
                .session_manager
                .save_session(&updated_app_session)
                .await
                .map_err(|e| e.to_string())?;

            // Record trajectory after session save (non-blocking, warn-only).
            // Pass the AppState-level TrajectoryManager to avoid per-turn re-init.
            record_trajectory_if_possible(
                &trajectory_session,
                std::slice::from_ref(&system_prompt_text),
                state.trajectory_manager.as_ref(),
            )
            .await;

            // Phase M4.1 — extract real `MemoryWriteCandidate`s
            // from the assistant `memory_store` tool calls
            // produced during THIS turn (slice from the captured
            // baseline) and look up existing records so the
            // conflict resolver renders real outcomes.
            let new_messages_run: Vec<ConversationMessage> = updated_app_session
                .messages
                .iter()
                .skip(baseline_message_count_run)
                .cloned()
                .collect();
            let after_turn_scope_run = MemoryExecutionScope {
                session_id: Some(session_ctx_id.clone()),
                project_id: session_ctx_project.clone(),
                workdir: None,
            };
            let candidates_run =
                extract_memory_store_tool_candidates(&new_messages_run, &after_turn_scope_run);
            let existing_run = lookup_existing_records_for_candidates(
                &state.memory_provider,
                &after_turn_scope_run,
                &candidates_run,
            )
            .await;
            dispatch_after_turn(
                &app_handle,
                harness_event_bus_run.as_ref(),
                MemoryInjectionDeps {
                    pinned_store: state.pinned_store.clone(),
                    memory_provider: state.memory_provider.clone(),
                    active_retrieval_manager: state.active_retrieval_manager.clone(),
                },
                Some(session_ctx_id.clone()),
                session_ctx_project.clone(),
                candidates_run,
                existing_run,
                Vec::new(),
                "run_agent_turn",
            );

            // LearningModule: record turn outcome using shared AppState instance.
            // Using AppState-level module avoids per-turn re-init and lets SelfModel
            // accumulate knowledge across turns.
            if let Some(lm_arc) = &state.learning_module {
                let mut lm = lm_arc.lock().await;
                lm.self_model_mut()
                    .record_turn(/* success= */ true, /* response_time_ms= */ 0.0);

                let turn_count = lm.self_model().performance.total_turns;
                tracing::info!(
                    "[run_agent_turn] LearningModule: turn {} recorded, {} patterns tracked",
                    turn_count,
                    lm.self_model().learned_patterns.len(),
                );

                // Reflection trigger — every REFLECT_INTERVAL turns, analyze the session
                // and update the self-model with new learned patterns.
                // This is async and non-blocking: errors are warn-logged, not propagated.
                const REFLECT_INTERVAL: u64 = 5;
                if turn_count > 0 && turn_count % REFLECT_INTERVAL == 0 {
                    match lm
                        .reflection_engine
                        .analyze_session(&trajectory_session)
                        .await
                    {
                        Ok(reflections) => {
                            let count: usize = reflections.len();
                            lm.self_model.update_from_reflections(&reflections);
                            tracing::info!(
                                "[run_agent_turn] Reflection: {} insights at turn {}",
                                count,
                                turn_count
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[run_agent_turn] Reflection failed at turn {turn_count}: {e}"
                            );
                        }
                    }
                }
            } else {
                tracing::debug!(
                    "[run_agent_turn] LearningModule: not initialised, skipping self-model update"
                );
            }

            // Verify system prompt integrity: detect if prompt was modified during session
            // The prompt captured at start is compared against the current text.
            // Since system_prompt is moved into ConversationRuntime, we compare
            // the captured snapshot against the original text (which includes memory context).
            // M6: use verify_detailed so failures expose actionable diagnostics
            // (length delta or first differing byte) instead of an opaque
            // "hash mismatch" log line.
            let verify = frozen_snapshot.verify_detailed(&system_prompt_text);
            if !verify.valid {
                tracing::warn!(
                    expected_hash = %verify.expected_hash,
                    actual_hash = %verify.actual_hash,
                    details = %verify.details.as_deref().unwrap_or("-"),
                    "[run_agent_turn] System prompt integrity check FAILED",
                );
            } else {
                tracing::info!(
                    "[run_agent_turn] System prompt integrity verified: snapshot hash={}",
                    verify.expected_hash
                );
            }

            // WorkingMemory: check if the post-turn session fits within working memory budget
            let working_memory = WorkingMemory::default();
            let working_tokens: usize = trajectory_session
                .messages
                .iter()
                .map(crate::modules::memory::working_memory::message_token_count)
                .sum();
            if working_tokens > working_memory.max_tokens {
                tracing::warn!(
                    "[run_agent_turn] WorkingMemory budget exceeded: {} tokens > {} max ({} messages)",
                    working_tokens,
                    working_memory.max_tokens,
                    trajectory_session.messages.len()
                );
            } else {
                tracing::info!(
                    "[run_agent_turn] WorkingMemory within budget: {} tokens / {} max",
                    working_tokens,
                    working_memory.max_tokens
                );
            }

            // WeibullDecay: apply importance decay to memory entries post-turn (C5).
            // Uses the default 7-day scale (lambda=168h, k=1.2) so that entries
            // that haven't been accessed recently gradually fade in importance.
            let decay_default = WeibullDecay::default();
            match state
                .memory_provider
                .apply_importance_decay(decay_default.lambda, decay_default.k)
                .await
            {
                Ok(updated) if updated > 0 => {
                    tracing::info!(
                        "[run_agent_turn] WeibullDecay: applied to {updated} memory entries \
                         (lambda={:.0}h, k={:.2})",
                        decay_default.lambda,
                        decay_default.k
                    );
                }
                Ok(_) => {
                    tracing::debug!(
                        "[run_agent_turn] WeibullDecay: no entries updated (no-op or empty store)"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "[run_agent_turn] WeibullDecay: apply_importance_decay failed: {e}"
                    );
                }
            }

            // Background memory promotion scan (throttled).  See
            // `start_agent_stream` for full rationale; same hook here so
            // non-streaming turns also surface candidates.
            {
                use crate::modules::memory::promotion::{
                    MemoryPromotionEngine, PromotionThresholds,
                };
                let thresholds = PromotionThresholds::load_from_disk();
                let engine = MemoryPromotionEngine::with_thresholds(
                    state.memory_provider.as_ref(),
                    thresholds,
                );
                match engine.evaluate_and_audit().await {
                    Ok(Some(n)) if n > 0 => tracing::info!(
                        "[run_agent_turn] PromotionEngine: surfaced {n} candidate(s)"
                    ),
                    Ok(Some(_)) => {
                        tracing::debug!("[run_agent_turn] PromotionEngine: scan ran, no candidates")
                    }
                    Ok(None) => {
                        tracing::debug!("[run_agent_turn] PromotionEngine: throttled, scan skipped")
                    }
                    Err(e) => tracing::warn!("[run_agent_turn] PromotionEngine: scan failed: {e}"),
                }
            }

            // Log compaction-related decay factor for observability.
            let removed_count =
                pre_compact_message_count.saturating_sub(post_compact_message_count);
            if removed_count > 0 {
                let decay_factor = decay_default.decay_factor(24.0); // 1-day decay factor
                tracing::info!(
                    "[run_agent_turn] WeibullDecay: {removed_count} msgs compacted, 1-day factor={:.3}",
                    decay_factor
                );
            }

            tracing::info!("[run_agent_turn] Returning response with message length: {}, thinking length: {:?}, session_id: {}", final_text.len(), thinking_content.as_ref().map(|s| s.len()), session_id);
            // Phase 6E harness: emit TurnFinished on success.
            crate::modules::harness::agent_loop_integration::emit_turn_finished(
                harness_event_bus_run.as_ref(),
                &session_id,
                turn_number_run,
                true,
                0,
                turn_started_at_run.elapsed().as_millis() as u64,
            );
            Ok(RunAgentTurnResponse {
                message: final_text,
                session_id,
                thinking: thinking_content,
            })
        }
        Err(e) => {
            // Return friendly error message
            let error_message = match e {
                RuntimeError::MaxIterationsExceeded => {
                    "Maximum conversation iterations reached. Please try simplifying your question."
                        .to_string()
                }
                RuntimeError::ApiError(msg) => {
                    // Check for common network errors and provide friendly messages
                    if msg.contains("connection refused") {
                        "Failed to connect to AI service server. Please check your network connection.".to_string()
                    } else if msg.contains("timeout") || msg.contains("timed out") {
                        "AI service response timed out. Please try again later.".to_string()
                    } else if msg.contains("dns") || msg.contains("Name or service not known") {
                        "Failed to resolve AI service address. Please check network configuration."
                            .to_string()
                    } else if msg.contains("401")
                        || msg.contains("403")
                        || msg.contains("invalid signature")
                    {
                        "AI service authentication failed. Please check API configuration."
                            .to_string()
                    } else if msg.contains("429") {
                        "Too many AI service requests. Please try again later.".to_string()
                    } else if msg.contains("500") || msg.contains("502") || msg.contains("503") {
                        "AI service temporarily unavailable. Please try again later.".to_string()
                    } else {
                        format!("AI service call failed: {}. Please try again later.", msg)
                    }
                }
                RuntimeError::ToolError(msg) => {
                    format!("Tool execution failed: {}. Please try again later.", msg)
                }
                RuntimeError::PermissionDenied(msg) => {
                    format!("Permission denied: {}. Please check your settings.", msg)
                }
                RuntimeError::SessionError(msg) => {
                    format!(
                        "Session error: {}. Please refresh the page and try again.",
                        msg
                    )
                }
                RuntimeError::ConfigError(msg) => {
                    format!("Configuration error: {}. Please check your settings.", msg)
                }
            };
            // Phase 6E harness: emit TurnFinished on error.
            crate::modules::harness::agent_loop_integration::emit_turn_finished(
                harness_event_bus_run.as_ref(),
                &session_id,
                turn_number_run,
                false,
                0,
                turn_started_at_run.elapsed().as_millis() as u64,
            );
            Err(error_message)
        }
    }
}

/// Start a streaming agent turn.
///
/// This command initiates a streaming response from the AI. It emits token events
/// via Tauri's event system which the frontend can listen to for progressive updates.
///
/// # Arguments
/// * `state` - Application state with session manager and tool registry
/// * `app_handle` - Tauri app handle for emitting events
/// * `session_id` - The session ID to continue
/// * `user_message` - The user's message
///
/// # Returns
/// A stream ID that the frontend uses to correlate events
#[tauri::command]
pub async fn start_agent_stream(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    session_id: String,
    user_message: String,
    permission_mode: Option<String>,
) -> Result<String, String> {
    use crate::modules::api::StreamEvent as ApiStreamEvent;
    use tauri::Manager;

    let stream_id = uuid::Uuid::new_v4().to_string();
    eprintln!(
        "[DEBUG] start_agent_stream called - stream_id: {}, session_id: {}, message: {}",
        stream_id, session_id, user_message
    );
    tracing::info!(
        "[start_agent_stream] Starting - stream_id: {}, session_id: {}, requested_permission_mode: {}",
        stream_id,
        session_id,
        permission_mode
            .as_deref()
            .unwrap_or("dangerFullAccess(default)")
    );

    // Get the main window for emitting events
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "Failed to get main window".to_string())?;

    // Restore the session
    let app_session = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "[start_agent_stream] Session restored, {} messages",
        app_session.messages.len()
    );

    let mode = parse_permission_mode(permission_mode.as_deref());
    let execution_context =
        resolve_session_execution_context(&state, &app_session, mode, "start_agent_stream").await;
    log_context_fingerprint("start_agent_stream", &execution_context);

    let inbound_resume_cursor = extract_resume_cursor_marker(&user_message);
    if let Some(cursor_value) = inbound_resume_cursor.as_deref() {
        let parsed_cursor = parse_resume_cursor(cursor_value)
            .ok_or_else(|| format!("invalid resume cursor: {cursor_value}"))?;
        if !session_contains_resume_cursor(&app_session, &parsed_cursor) {
            return Err(format!(
                "resume cursor not found or expired: {cursor_value}"
            ));
        }
    }

    let normalized_user_message = if inbound_resume_cursor.is_some() {
        strip_resume_cursor_marker(&user_message)
    } else {
        user_message.trim().to_string()
    };

    // Convert session messages to API format
    let runtime_session = app_session_to_runtime(&app_session);
    let messages: Vec<InputMessage> = runtime_session
        .messages
        .iter()
        .map(|msg| {
            let content: Vec<InputContentBlock> = msg
                .blocks
                .iter()
                .map(runtime_block_to_input_block)
                .collect();

            let role = match msg.role {
                crate::modules::runtime::session::MessageRole::System => "user".to_string(),
                crate::modules::runtime::session::MessageRole::User => "user".to_string(),
                crate::modules::runtime::session::MessageRole::Assistant => "assistant".to_string(),
                crate::modules::runtime::session::MessageRole::Tool => "user".to_string(),
            };

            InputMessage { role, content }
        })
        .collect();

    // Add the user's new message
    let mut all_messages = messages;
    all_messages.push(InputMessage::user_text(&normalized_user_message));

    // Get tool definitions from registry and convert to ToolDefinition format
    let definitions = state.tool_registry.get_definitions(None);
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

    // Phase M1.4 — single application service seam composes
    // provider + memory injection (static + retrieved) + prompt
    // plan in one await.  `memory_items` is moved into the spawned
    // task and emitted on `stream_complete` so the frontend
    // `MemoryChip` / `MemoryEvidencePanel` can render them.
    let turn_service = make_turn_service(&state);
    let stream_project_id_opt: Option<String> = if execution_context.project_id.is_empty() {
        None
    } else {
        Some(execution_context.project_id.clone())
    };
    let prepared_stream = turn_service
        .prepare_chat_inputs(PrepareChatInputsRequest {
            workdir: execution_context.workdir.clone(),
            current_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            os_name: std::env::consts::OS.to_string(),
            os_family: std::env::consts::FAMILY.to_string(),
            session_id: Some(execution_context.session_id.clone()),
            project_id: stream_project_id_opt.clone(),
            workdir_str: execution_context.workdir.to_str().map(str::to_string),
            user_message: normalized_user_message.clone(),
            caller: "start_agent_stream",
        })
        .await
        .map_err(|err| match err {
            TurnServiceError::Provider(msg) => {
                tracing::error!("[start_agent_stream] Failed to create API client: {}", msg);
                msg
            }
            TurnServiceError::Prompt(p) => {
                tracing::error!("[start_agent_stream] Prompt planning failed: {}", p);
                p.to_string()
            }
        })?;
    // Phase M1.6 — same advisory log as `run_agent_turn`.
    tracing::info!(
        execution_mode = ?prepared_stream.execution_mode_decision.execution_mode,
        risk_level = ?prepared_stream.execution_mode_decision.risk_level,
        complexity_level = ?prepared_stream.execution_mode_decision.complexity_level,
        policy_version = %prepared_stream.execution_mode_decision.classifier_policy_version,
        rules = ?prepared_stream.execution_mode_decision.classifier_matched_rule_ids,
        "[start_agent_stream] request_intelligence decision (advisory)"
    );
    if !prepared_stream.memory_items.is_empty() {
        tracing::info!(
            "[start_agent_stream] Injected {} memory items into prompt",
            prepared_stream.memory_items.len(),
        );
    }
    let memory_context_items_for_task: Vec<MemoryItemProjection> =
        prepared_stream.memory_items.clone();
    let RuntimeProviderResolution {
        provider_client,
        model,
        request_timeout: _request_timeout,
    } = prepared_stream.provider;
    let system_prompt_with_memory = prepared_stream.prompt.text;

    // Clone everything needed for the background task
    let session_manager = state.session_manager.clone();
    let app_session_clone = app_session.clone();
    let user_message_clone = normalized_user_message.clone();
    let tool_registry_clone = state.tool_registry.clone();
    let model_for_stream = model.clone();
    let provider_client_for_stream = provider_client.clone();
    let messages_for_stream = all_messages.clone();
    let tool_defs_for_stream = tool_defs.clone();
    let system_prompt_for_stream = system_prompt_with_memory;
    let permission_mode_for_stream = permission_mode.clone();
    let execution_context_for_task = execution_context.clone();
    let permission_senders = state.permission_senders.clone();
    let permission_overrides = state.permission_overrides.clone();
    // Clones for post-turn learning (trajectory + self-model)
    let trajectory_manager_for_stream = state.trajectory_manager.clone();
    let learning_module_for_stream = state.learning_module.clone();
    let memory_provider_for_stream = state.memory_provider.clone();
    // Phase 8B.11 fix — clone the ticker so the spawned stream task can
    // fire on_turn_complete after the LLM loop terminates (the streaming
    // path doesn't go through ConversationRuntime where the runtime
    // auto-fires the hook).
    let memory_ticker_for_stream = state.memory_ticker.clone();
    // Phase 6E harness EventBus clone (zero-cost when harness disabled).
    let harness_event_bus_for_stream = state.harness.as_ref().map(|h| h.event_bus.clone());
    let turn_number_for_stream = (app_session.messages.len() as u64) + 1;

    // Phase M3-B audit fix (extended in M4.1) — clone deps for
    // the spawned-task `dispatch_after_turn` call.  All `Arc`
    // clones; no perf cost.  Also capture the message-vector
    // baseline so the candidate extractor can slice "messages
    // added during this turn" inside the spawned task.
    let app_handle_for_after_turn = app_handle.clone();
    let pinned_store_for_after_turn = state.pinned_store.clone();
    let memory_provider_for_after_turn = state.memory_provider.clone();
    let active_retrieval_manager_for_after_turn = state.active_retrieval_manager.clone();
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
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut senders = state.stream_cancel_senders.lock().map_err(|e| {
            tracing::error!("[start_agent_stream] Failed to lock cancel senders: {}", e);
            e.to_string()
        })?;
        senders.insert(stream_id.clone(), cancel_tx);
    }

    // Phase M1.5 — single boundary at which agent-loop runtime
    // events leave the backend. Replaces the ~13 ad-hoc
    // `window.emit("agent-token", ...)` call sites that used to be
    // scattered through this spawned task.
    let stream_emitter = AgentStreamEmitter::new(window);
    tokio::spawn(async move {
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
        let mut tool_executor = crate::commands::agent::ToolRegistryExecutor::new_with_context(
            tool_registry_clone.clone(),
            execution_context,
        );
        const SAVE_INTERVAL: u32 = 50;
        // Session-format timeline messages (for persistence in chronological order)
        let mut timeline_session_messages: Vec<
            crate::modules::runtime::session::ConversationMessage,
        > = Vec::new();

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
                    let resume_cursor = user_visible_truth.resume_available.then(|| {
                        build_resume_cursor(&stream_id_for_task, tool_loop_iter, token_count)
                    });
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
                            crate::modules::api::ContentBlockDelta::InputJsonDelta {
                                partial_json,
                            } => {
                                // Route delta to the correct tool_call via block index.
                                if let Some(tool_id) =
                                    index_to_tool_id.get(&delta_event.index).cloned()
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
                                let tool_name =
                                    index_to_tool_name.remove(&index).unwrap_or_default();
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
                                    id,
                                    name,
                                    ..
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
                    effective_workdir: Some(
                        execution_context_for_policy.workdir.display().to_string(),
                    ),
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
                    None => {
                        permission_policy.authorize(&tool_name, &input_json, Some(&mut prompter))
                    }
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
                    effective_workdir: Some(
                        execution_context_for_policy.workdir.display().to_string(),
                    ),
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

        // Guardrail: do not allow "operation completed" claims without a successful
        // mutating tool evidence in this request.
        if contains_unverified_file_claim(&accumulated_text) && !has_successful_mutating_tool {
            let guarded = "未执行工具，无法确认完成。".to_string();
            tracing::warn!(
                "[start_agent_stream] Rewriting unverified completion claim to guarded message"
            );
            accumulated_text = guarded.clone();
            stream_emitter.emit_payload(StreamTokenPayload {
                stream_id: stream_id_for_task.clone(),
                text: Some(guarded),
                thinking: None,
                event_type: "final_text_override".to_string(),
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
            });
        }

        let user_visible_truth = TaskOutcomeResolver::resolve(
            ExecutionTruth {
                has_successful_tool,
                has_successful_mutating_tool,
            },
            &ConversationTruth {
                stream_failed,
                terminal_status: terminal_status.unwrap_or("unknown"),
                last_stream_error_reason: last_stream_error_reason.clone(),
            },
        );
        let resume_cursor = user_visible_truth
            .resume_available
            .then(|| build_resume_cursor(&stream_id_for_task, tool_loop_iter, token_count));
        let degraded_reason = user_visible_truth.degraded_reason.clone();
        let persisted_turn_outcome = PersistedTurnOutcome {
            task_outcome: user_visible_truth.task_outcome.to_string(),
            degraded_reason: degraded_reason.clone(),
            resume_available: user_visible_truth.resume_available,
            resume_cursor: resume_cursor.clone(),
            request_id: provider_request_id.clone(),
        };

        // Save session with all accumulated messages
        let mut updated_app_session = app_session_clone;
        let user_msg = crate::modules::runtime::session::ConversationMessage {
            role: crate::modules::runtime::session::MessageRole::User,
            blocks: vec![ContentBlock::Text {
                text: user_message_clone.clone(),
            }],
            usage: None,
            thinking: None,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            request_id: Some(provider_request_id.clone()),
        };
        flush_assistant_timeline_segment(
            &mut timeline_session_messages,
            &mut accumulated_text,
            &mut accumulated_thinking,
            if stream_failed || user_visible_truth.task_outcome == "partial_success" {
                Some(&persisted_turn_outcome)
            } else {
                None
            },
        );
        if (stream_failed || user_visible_truth.task_outcome == "partial_success")
            && timeline_session_messages
                .last()
                .is_none_or(|message| message.resume_cursor.as_deref() != resume_cursor.as_deref())
        {
            timeline_session_messages.push(crate::modules::runtime::session::ConversationMessage {
                role: crate::modules::runtime::session::MessageRole::Assistant,
                blocks: vec![ContentBlock::Text {
                    text: String::new(),
                }],
                usage: None,
                thinking: None,
                task_outcome: Some(persisted_turn_outcome.task_outcome.clone()),
                degraded_reason: persisted_turn_outcome.degraded_reason.clone(),
                resume_available: Some(persisted_turn_outcome.resume_available),
                resume_cursor: persisted_turn_outcome.resume_cursor.clone(),
                request_id: Some(persisted_turn_outcome.request_id.clone()),
            });
        }
        let appended_message_count = 1 + timeline_session_messages.len();
        updated_app_session.messages.push(user_msg);
        updated_app_session
            .messages
            .extend(timeline_session_messages);
        updated_app_session.message_count =
            updated_app_session.logical_message_count() + appended_message_count;

        // Context compaction — compact if session exceeds token threshold
        let compaction_config = CompactionConfig::default();
        if should_compact(
            &RuntimeSession {
                version: 1,
                messages: updated_app_session.messages.clone(),
            },
            compaction_config,
        ) {
            let compact_result = compact_session(
                &RuntimeSession {
                    version: 1,
                    messages: updated_app_session.messages.clone(),
                },
                compaction_config,
            );
            updated_app_session.messages = compact_result.compacted_session.messages;
        }

        if let Err(e) = session_manager.save_session(&updated_app_session).await {
            tracing::error!("[start_agent_stream] Failed to save session: {}", e);
        }

        // ── Post-turn streaming parity (mirrors run_agent_turn) ──

        // Build a runtime session snapshot for trajectory recording.
        let trajectory_runtime_session = RuntimeSession {
            version: 1,
            messages: updated_app_session.messages.clone(),
        };

        // Trajectory recording.
        record_trajectory_if_possible(
            &trajectory_runtime_session,
            std::slice::from_ref(&system_prompt_for_stream),
            trajectory_manager_for_stream.as_ref(),
        )
        .await;

        // Phase M4.1 — extract real `MemoryWriteCandidate`s from
        // the assistant `memory_store` tool calls produced during
        // THIS streaming turn (slice from the captured baseline)
        // and look up existing records so the conflict resolver
        // renders real outcomes.  Mirrors the `run_agent_turn`
        // site exactly.
        let new_messages_stream: Vec<ConversationMessage> = updated_app_session
            .messages
            .iter()
            .skip(baseline_message_count_stream)
            .cloned()
            .collect();
        let after_turn_scope_stream = MemoryExecutionScope {
            session_id: Some(stream_session_id_for_after_turn.clone()),
            project_id: stream_project_id_for_after_turn.clone(),
            workdir: None,
        };
        let candidates_stream =
            extract_memory_store_tool_candidates(&new_messages_stream, &after_turn_scope_stream);
        let existing_stream = lookup_existing_records_for_candidates(
            &memory_provider_for_after_turn,
            &after_turn_scope_stream,
            &candidates_stream,
        )
        .await;
        dispatch_after_turn(
            &app_handle_for_after_turn,
            harness_bus_for_after_turn.as_ref(),
            MemoryInjectionDeps {
                pinned_store: pinned_store_for_after_turn.clone(),
                memory_provider: memory_provider_for_after_turn.clone(),
                active_retrieval_manager: active_retrieval_manager_for_after_turn.clone(),
            },
            Some(stream_session_id_for_after_turn.clone()),
            stream_project_id_for_after_turn.clone(),
            candidates_stream,
            existing_stream,
            Vec::new(),
            "start_agent_stream",
        );

        // LearningModule: record turn + reflection trigger.
        if let Some(lm_arc) = &learning_module_for_stream {
            let mut lm = lm_arc.lock().await;
            lm.self_model_mut().record_turn(
                /* success= */ !stream_failed,
                /* response_time_ms= */ 0.0,
            );

            let turn_count = lm.self_model().performance.total_turns;
            tracing::info!(
                "[start_agent_stream] LearningModule: turn {} recorded",
                turn_count
            );

            const STREAM_REFLECT_INTERVAL: u64 = 5;
            if turn_count > 0 && turn_count % STREAM_REFLECT_INTERVAL == 0 {
                match lm
                    .reflection_engine
                    .analyze_session(&trajectory_runtime_session)
                    .await
                {
                    Ok(reflections) => {
                        let count: usize = reflections.len();
                        lm.self_model.update_from_reflections(&reflections);
                        tracing::info!(
                            "[start_agent_stream] Reflection: {} insights at turn {}",
                            count,
                            turn_count
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[start_agent_stream] Reflection failed at turn {turn_count}: {e}"
                        );
                    }
                }
            }
        }

        // WeibullDecay importance decay (non-blocking, warn-only on error).
        {
            use crate::modules::runtime::episodic_compaction::WeibullDecay;
            let decay_default = WeibullDecay::default();
            if let Err(e) = memory_provider_for_stream
                .apply_importance_decay(decay_default.lambda, decay_default.k)
                .await
            {
                tracing::warn!(
                    "[start_agent_stream] WeibullDecay: apply_importance_decay failed: {e}"
                );
            }
        }

        // Background memory promotion scan — throttled to once per minute
        // (process-wide) so the cost is amortised across turns.  Surfaces
        // candidates as `memory_promotion_candidate` audit events; never
        // mutates the store on its own.
        {
            use crate::modules::memory::promotion::{MemoryPromotionEngine, PromotionThresholds};
            let thresholds = PromotionThresholds::load_from_disk();
            let engine = MemoryPromotionEngine::with_thresholds(
                memory_provider_for_stream.as_ref(),
                thresholds,
            );
            match engine.evaluate_and_audit().await {
                Ok(Some(n)) if n > 0 => tracing::info!(
                    "[start_agent_stream] PromotionEngine: surfaced {n} candidate(s)"
                ),
                Ok(Some(_)) => {
                    tracing::debug!("[start_agent_stream] PromotionEngine: scan ran, no candidates")
                }
                Ok(None) => {
                    tracing::debug!("[start_agent_stream] PromotionEngine: throttled, scan skipped")
                }
                Err(e) => tracing::warn!("[start_agent_stream] PromotionEngine: scan failed: {e}"),
            }
        }

        // Emit stream_complete exactly once, and only after the full
        // tool/LLM loop has finished for this request.
        if !stream_failed && !completion_already_emitted {
            // Compute live token-budget breakdown for the frontend ContextBar.
            // Counts are estimates derived from tiktoken cl100k_base; the
            // total budget mirrors the runtime ContextGovernor.
            let system_tokens =
                crate::modules::runtime::budget::estimate_tokens(&system_prompt_for_stream);
            let history_tokens: usize = session_messages
                .iter()
                .map(|m| {
                    m.content
                        .iter()
                        .map(|c| match c {
                            crate::modules::api::InputContentBlock::Text { text } => {
                                crate::modules::runtime::budget::estimate_tokens(text)
                            }
                            crate::modules::api::InputContentBlock::ToolResult {
                                content, ..
                            } => content
                                .iter()
                                .map(|b| match b {
                                    crate::modules::api::ToolResultContentBlock::Text { text } => {
                                        crate::modules::runtime::budget::estimate_tokens(text)
                                    }
                                    // JSON tool results are estimated from their serialised
                                    // representation so structured outputs still count toward
                                    // the per-turn history budget surfaced in `ContextBar`.
                                    crate::modules::api::ToolResultContentBlock::Json { value } => {
                                        crate::modules::runtime::budget::estimate_tokens(
                                            &value.to_string(),
                                        )
                                    }
                                    // Image content blocks (Phase 7C, slice 7C.2):
                                    // base64 payload doesn't go through the text
                                    // tokeniser (vision providers count it on
                                    // their own); contribute the alt text only.
                                    crate::modules::api::ToolResultContentBlock::Image {
                                        alt,
                                        ..
                                    } => alt.as_deref().map_or(0, |a| {
                                        crate::modules::runtime::budget::estimate_tokens(a)
                                    }),
                                })
                                .sum::<usize>(),
                            _ => 0,
                        })
                        .sum::<usize>()
                })
                .sum();
            let memory_tokens: usize = memory_context_items_for_task
                .iter()
                .map(|i| crate::modules::runtime::budget::estimate_tokens(&i.content))
                .sum();
            const OUTPUT_RESERVE: usize = 4_096;
            let total_budget = MAX_REQUEST_TOKEN_BUDGET_ESTIMATE
                .max(system_tokens + history_tokens + memory_tokens + OUTPUT_RESERVE + 1024);
            let used = system_tokens + history_tokens + memory_tokens + OUTPUT_RESERVE;
            let remaining = total_budget.saturating_sub(used);
            let usage = ContextBudgetUsagePayload {
                total_budget,
                system_tokens,
                history_tokens,
                memory_tokens,
                output_reserve: OUTPUT_RESERVE,
                remaining,
            };
            let memory_payload = if memory_context_items_for_task.is_empty() {
                None
            } else {
                Some(memory_context_items_for_task.clone())
            };

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
                task_outcome: Some(user_visible_truth.task_outcome.to_string()),
                degraded_reason: degraded_reason.clone(),
                resume_available: Some(user_visible_truth.resume_available),
                resume_cursor: resume_cursor.clone(),
                context_budget_usage: Some(usage),
                memory_context: memory_payload,
            };
            stream_emitter.emit_payload(payload);
            if terminal_status.is_none() {
                terminal_status = Some("completed");
            }
        }

        // Phase 6E harness: emit TurnFinished for the streaming path.
        // We treat any non-failed stream as success here; downstream consumers
        // can refine via `task_outcome` if needed.
        crate::modules::harness::agent_loop_integration::emit_turn_finished(
            harness_event_bus_for_stream.as_ref(),
            &session_id,
            turn_number_for_stream,
            !stream_failed,
            token_count,
            stream_turn_started_at.elapsed().as_millis() as u64,
        );

        // Phase 8B.11 fix — fire MemoryTicker.on_turn_complete for the
        // streaming path.  The non-streaming run_agent_turn path goes
        // through ConversationRuntime.with_turn_hook, but
        // start_agent_stream streams directly so the hook needs an
        // explicit invocation here.  Using the persisted message list
        // ensures RollingSummarizer's incremental slice (`message_count`)
        // matches what the user actually saw.
        if !stream_failed {
            use crate::modules::memory::scope::MemoryExecutionScope;
            use crate::modules::runtime::conversation::TurnHook;
            let messages_for_hook = updated_app_session.messages.clone();
            let scope_for_hook = MemoryExecutionScope {
                session_id: Some(session_id.clone()),
                project_id: if updated_app_session.project_id.is_empty() {
                    None
                } else {
                    Some(updated_app_session.project_id.clone())
                },
                workdir: None,
            };
            // Phase 8B.11 fix-debug — info log so it's visible in dev console.
            tracing::info!(
                session_id = %session_id,
                project_id = updated_app_session.project_id.as_str(),
                messages = messages_for_hook.len(),
                "[stream] firing memory_ticker.on_turn_complete"
            );
            memory_ticker_for_stream.on_turn_complete(
                &scope_for_hook,
                &session_id,
                &messages_for_hook,
            );
        } else {
            tracing::info!(
                session_id = %session_id,
                "[stream] SKIP memory_ticker.on_turn_complete (stream_failed=true)"
            );
        }

        if is_resume_turn {
            tracing::info!(
                "[resume_outcome] session_id='{}', stream_id='{}', request_id='{}', inbound_resume_cursor='{}', task_outcome='{}', success={}",
                session_id,
                stream_id_for_task,
                provider_request_id.as_str(),
                inbound_resume_cursor.as_deref().unwrap_or("none"),
                user_visible_truth.task_outcome,
                user_visible_truth.task_outcome == "completed"
            );
        }

        tracing::info!(
            "[stream_diag_summary] stream_id='{}', session_id='{}', request_id='{}', status='{}', task_outcome='{}', degraded_reason='{}', resume_available={}, resume_cursor='{}', is_resume_turn={}, inbound_resume_cursor='{}', tool_loop_iter={}, token_count={}, stream_failed={}, completion_already_emitted={}, has_successful_tool={}, has_successful_mutating_tool={}, preflight_trim_rounds={}, preflight_dropped_messages_total={}, preflight_trimmed_chars_total={}, sanitize_rounds={}, dropped_empty_messages_total={}, dropped_orphan_tool_results_total={}, dropped_unmatched_tool_uses_total={}, dropped_invalid_tool_use_inputs_total={}, start_retry_count={}, event_retry_count={}, orphan_tool_result_samples={:?}, unmatched_tool_use_samples={:?}, invalid_tool_use_input_samples={:?}, last_stream_error={}",
            stream_id_for_task,
            session_id,
            provider_request_id.as_str(),
            terminal_status.unwrap_or("unknown"),
            user_visible_truth.task_outcome,
            degraded_reason.as_deref().unwrap_or("none"),
            user_visible_truth.resume_available,
            resume_cursor.as_deref().unwrap_or("none"),
            is_resume_turn,
            inbound_resume_cursor.as_deref().unwrap_or("none"),
            tool_loop_iter,
            token_count,
            stream_failed,
            completion_already_emitted,
            has_successful_tool,
            has_successful_mutating_tool,
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
            last_stream_error_reason.as_deref().unwrap_or("none"),
        );
    });

    // Return immediately with stream_id
    tracing::info!(
        "[start_agent_stream] Returning stream_id: {}",
        stream_id_return
    );
    Ok(stream_id_return)
}

fn format_stream_error_reason(error: &impl std::fmt::Display) -> String {
    let raw = error.to_string();
    let lower = raw.to_ascii_lowercase();
    let kind = if lower.contains("timed out") || lower.contains("timeout") {
        "network_timeout"
    } else if lower.contains("invalid_request_error")
        || lower.contains("invalid params")
        || lower.contains("bad request")
    {
        "request_validation_error"
    } else if lower.contains("permission")
        || lower.contains("forbidden")
        || lower.contains("denied")
    {
        "permission_error"
    } else if lower.contains("connection") || lower.contains("broken pipe") || lower.contains("eof")
    {
        "network_transport_error"
    } else {
        "model_stream_error"
    };
    format!("{kind}: {raw}")
}

fn truncate_tool_result_for_model(result: &str) -> String {
    let total_chars = result.chars().count();
    if total_chars <= MAX_TOOL_RESULT_FOR_MODEL_CHARS {
        return result.to_string();
    }
    let kept: String = result
        .chars()
        .take(MAX_TOOL_RESULT_FOR_MODEL_CHARS)
        .collect();
    format!(
        "{kept}\n\n[tool_result_truncated_for_context: omitted {} chars]",
        total_chars - MAX_TOOL_RESULT_FOR_MODEL_CHARS
    )
}

fn summarize_tool_result_for_model(
    tool_name: &str,
    tool_use_id: &str,
    result: &str,
    is_error: bool,
) -> String {
    let preview: String = result.chars().take(TOOL_RESULT_PREVIEW_CHARS).collect();
    let digest = short_text_digest(result);
    let total_chars = result.chars().count();
    let compact = format!(
        "[tool_result_handle] tool={tool_name} id={tool_use_id} status={} chars={total_chars} digest={digest}\npreview:\n{}",
        if is_error { "error" } else { "ok" },
        preview
    );
    truncate_tool_result_for_model(&compact)
}

fn short_text_digest(text: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn build_resume_cursor(stream_id: &str, tool_loop_iter: usize, token_count: u32) -> String {
    // harness symbol marker: resume_cursor\|degraded
    format!("resume_cursor:v1:{stream_id}:{tool_loop_iter}:{token_count}")
}

fn parse_resume_cursor(value: &str) -> Option<ResumeCursor> {
    let mut parts = value.split(':');
    if parts.next()? != "resume_cursor" || parts.next()? != "v1" {
        return None;
    }
    let stream_id = parts.next()?.to_string();
    let tool_loop_iter = parts.next()?.parse().ok()?;
    let token_count = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(ResumeCursor {
        stream_id,
        tool_loop_iter,
        token_count,
    })
}

fn session_contains_resume_cursor(app_session: &AppSession, resume_cursor: &ResumeCursor) -> bool {
    app_session.messages.iter().any(|message| {
        message.resume_available == Some(true)
            && message.resume_cursor.as_deref()
                == Some(&build_resume_cursor(
                    &resume_cursor.stream_id,
                    resume_cursor.tool_loop_iter,
                    resume_cursor.token_count,
                ))
    })
}

fn strip_resume_cursor_marker(message: &str) -> String {
    let marker = "[resume_cursor]";
    if let Some(start) = message.find(marker) {
        let before = &message[..start];
        let tail = &message[start + marker.len()..];
        let remainder = tail
            .split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or_default()
            .trim();
        let merged = format!("{} {}", before.trim(), remainder)
            .trim()
            .to_string();
        if merged.is_empty() {
            "请从上一次中断处继续完成未完成部分，禁止重复已确认的副作用操作。".to_string()
        } else {
            merged
        }
    } else {
        message.trim().to_string()
    }
}

fn extract_resume_cursor_marker(message: &str) -> Option<String> {
    let marker = "[resume_cursor]";
    let start = message.find(marker)?;
    let tail = &message[start + marker.len()..];
    let cursor = tail
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(';');
    if cursor.is_empty() {
        None
    } else {
        Some(cursor.to_string())
    }
}

fn runtime_block_to_input_block(block: &ContentBlock) -> InputContentBlock {
    match block {
        ContentBlock::Text { text } => InputContentBlock::Text { text: text.clone() },
        ContentBlock::ToolUse { id, name, input } => {
            let input_value = parse_tool_input_json(input);
            InputContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input_value,
            }
        }
        ContentBlock::ToolResult {
            tool_use_id,
            output,
            is_error,
            ..
        } => InputContentBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: vec![crate::modules::api::ToolResultContentBlock::Text {
                text: summarize_tool_result_for_model("history", tool_use_id, output, *is_error),
            }],
            is_error: *is_error,
        },
    }
}

#[derive(Debug, Default)]
struct RequestPreflightStats {
    before_messages: usize,
    after_messages: usize,
    before_chars: usize,
    after_chars: usize,
    dropped_messages: usize,
    trimmed_chars: usize,
}

impl RequestPreflightStats {
    fn has_changes(&self) -> bool {
        self.dropped_messages > 0 || self.trimmed_chars > 0
    }
}

#[derive(Debug, Default)]
// harness symbol marker: ContextGovernor\|token_budget_gate\|message_char_budget_gate\|artifact_gate
struct ContextGovernor;

impl ContextGovernor {
    fn admit(
        &self,
        messages: &[InputMessage],
        max_messages: usize,
        max_chars: usize,
        max_tokens: usize,
    ) -> (Vec<InputMessage>, RequestPreflightStats) {
        let (artifact_trimmed, artifact_stats) = self.artifact_gate(messages);
        let (token_trimmed, token_stats) = self.token_budget_gate(&artifact_trimmed, max_tokens);
        let (char_trimmed, mut preflight_stats) =
            self.message_char_budget_gate(&token_trimmed, max_messages, max_chars);
        preflight_stats.dropped_messages +=
            artifact_stats.dropped_messages + token_stats.dropped_messages;
        preflight_stats.trimmed_chars += artifact_stats.trimmed_chars + token_stats.trimmed_chars;
        (char_trimmed, preflight_stats)
    }

    fn token_budget_gate(
        &self,
        messages: &[InputMessage],
        max_tokens: usize,
    ) -> (Vec<InputMessage>, RequestPreflightStats) {
        let before_tokens = estimate_messages_token_count(messages);
        let mut trimmed = messages.to_vec();
        while trimmed.len() > 1 && estimate_messages_token_count(&trimmed) > max_tokens {
            trimmed.remove(0);
        }
        if trimmed.len() == 1 && estimate_messages_token_count(&trimmed) > max_tokens {
            trimmed[0] = summarize_message_for_budget(&trimmed[0], max_tokens.saturating_mul(4));
        }
        let after_tokens = estimate_messages_token_count(&trimmed);
        let after_messages = trimmed.len();
        (
            trimmed,
            RequestPreflightStats {
                before_messages: messages.len(),
                after_messages,
                before_chars: before_tokens.saturating_mul(4),
                after_chars: after_tokens.saturating_mul(4),
                dropped_messages: messages.len().saturating_sub(after_messages),
                trimmed_chars: before_tokens.saturating_sub(after_tokens).saturating_mul(4),
            },
        )
    }

    fn message_char_budget_gate(
        &self,
        messages: &[InputMessage],
        max_messages: usize,
        max_chars: usize,
    ) -> (Vec<InputMessage>, RequestPreflightStats) {
        apply_request_preflight_limits(messages, max_messages, max_chars)
    }

    fn artifact_gate(
        &self,
        messages: &[InputMessage],
    ) -> (Vec<InputMessage>, RequestPreflightStats) {
        let before_chars = estimate_messages_char_count(messages);
        let transformed = messages
            .iter()
            .map(|message| InputMessage {
                role: message.role.clone(),
                content: message
                    .content
                    .iter()
                    .map(|block| match block {
                        InputContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            let summarized = content
                                .iter()
                                .map(|part| match part {
                                    crate::modules::api::ToolResultContentBlock::Text { text } => {
                                        if text
                                            .trim_start()
                                            .starts_with("[tool_result_handle] tool=")
                                        {
                                            text.clone()
                                        } else {
                                            summarize_tool_result_for_model(
                                                "artifact_gate",
                                                tool_use_id,
                                                text,
                                                *is_error,
                                            )
                                        }
                                    }
                                    crate::modules::api::ToolResultContentBlock::Json { value } => {
                                        summarize_tool_result_for_model(
                                            "artifact_gate",
                                            tool_use_id,
                                            &value.to_string(),
                                            *is_error,
                                        )
                                    }
                                    crate::modules::api::ToolResultContentBlock::Image {
                                        source,
                                        alt,
                                    } => {
                                        let bytes = source.data.len();
                                        let caption = alt.as_deref().unwrap_or("image");
                                        format!(
                                            "[image: {} {bytes}B — {caption}]",
                                            source.media_type
                                        )
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            InputContentBlock::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                content: vec![crate::modules::api::ToolResultContentBlock::Text {
                                    text: summarized,
                                }],
                                is_error: *is_error,
                            }
                        }
                        _ => block.clone(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let after_chars = estimate_messages_char_count(&transformed);
        (
            transformed,
            RequestPreflightStats {
                before_messages: messages.len(),
                after_messages: messages.len(),
                before_chars,
                after_chars,
                dropped_messages: 0,
                trimmed_chars: before_chars.saturating_sub(after_chars),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::modules::memory;
    use crate::modules::runtime::permissions::PermissionPolicy;
    use crate::modules::runtime::session::{MessageRole, Session as RuntimeSession};
    use crate::modules::scheduler;
    use crate::modules::tools::{register_builtin_tools, ToolContext, ToolRegistry};

    struct TempDirGuard {
        path: std::path::PathBuf,
    }

    impl TempDirGuard {
        fn new(path: std::path::PathBuf) -> Self {
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct ScriptedSkillApiClient {
        call_count: usize,
    }

    impl ApiClient for ScriptedSkillApiClient {
        fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
            self.call_count += 1;
            match self.call_count {
                1 => {
                    let has_skill_definition = request
                        .tools
                        .as_ref()
                        .is_some_and(|tools| tools.iter().any(|tool| tool.name == "skill"));
                    assert!(
                        has_skill_definition,
                        "request should include skill tool definition"
                    );
                    Ok(vec![
                        AssistantEvent::ToolUse {
                            id: "tool-skill-1".to_string(),
                            name: "skill".to_string(),
                            input: r#"{"skill":"demo-skill"}"#.to_string(),
                        },
                        AssistantEvent::MessageStop,
                    ])
                }
                2 => {
                    let last_message = request
                        .messages
                        .last()
                        .ok_or_else(|| RuntimeError::api_error("missing tool result message"))?;
                    assert_eq!(last_message.role, MessageRole::Tool);
                    let has_expected_skill_content = last_message.blocks.iter().any(|block| {
                        matches!(
                            block,
                            ContentBlock::ToolResult { output, .. }
                                if output.contains("# Demo Skill")
                        )
                    });
                    assert!(
                        has_expected_skill_content,
                        "tool result should contain loaded SKILL.md content"
                    );
                    Ok(vec![
                        AssistantEvent::TextDelta("技能已执行".to_string()),
                        AssistantEvent::MessageStop,
                    ])
                }
                _ => Err(RuntimeError::api_error("unexpected extra API call")),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_loop_executes_skill_tool_end_to_end() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let workdir = std::env::temp_dir().join(format!("if2ai-agent-skill-e2e-{nanos}"));
        let _workdir_guard = TempDirGuard::new(workdir.clone());
        let skill_dir = workdir.join(".if2ai/skills/demo-skill");
        fs::create_dir_all(&skill_dir).expect("create skill directory");
        fs::write(
            skill_dir.join("SKILL.md"),
            "# Demo Skill\n\nUse demo skill.",
        )
        .expect("write skill markdown");
        fs::write(
            skill_dir.join("skill.json"),
            r#"{
  "id": "demo-skill",
  "version": "1.0.0",
  "apiVersion": "v1",
  "minAppVersion": "0.1.0",
  "capabilities": ["custom"],
  "review": {
    "status": "active",
    "riskLevel": "low",
    "lastReviewedAt": "2026-04-14T00:00:00Z"
  }
}"#,
        )
        .expect("write skill manifest");

        let context = std::sync::Arc::new(Mutex::new(ToolContext::default_for_workdir(
            workdir.clone(),
        )));
        let registry = Arc::new(ToolRegistry::new(context));
        let test_browser_registry = crate::modules::browser::BrowserRegistry::for_test(
            std::path::PathBuf::from("/tmp/browser-cold-state-test.json"),
        );
        register_builtin_tools(
            &registry,
            memory::default_memory_provider().await,
            scheduler::default_scheduler(),
            test_browser_registry,
            std::sync::Arc::new(crate::modules::memory::NullPinnedStore::new()),
        );

        let execution_context =
            SessionExecutionContext::stateless(workdir.clone(), PermissionMode::DangerFullAccess);
        let tool_executor = ToolRegistryExecutor::new_with_context(registry, execution_context);
        let mut runtime = ConversationRuntime::new(
            RuntimeSession::new(),
            ScriptedSkillApiClient { call_count: 0 },
            tool_executor,
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            vec!["system".to_string()],
        );

        let summary = runtime
            .run_turn("请运行 demo-skill", None)
            .expect("runtime should complete skill loop");
        assert_eq!(summary.iterations, 2);
        assert_eq!(summary.tool_results.len(), 1);
    }
}

fn apply_request_preflight_limits(
    messages: &[InputMessage],
    max_messages: usize,
    max_chars: usize,
) -> (Vec<InputMessage>, RequestPreflightStats) {
    let before_chars = estimate_messages_char_count(messages);
    let mut trimmed: Vec<InputMessage> = if messages.len() > max_messages {
        messages[messages.len() - max_messages..].to_vec()
    } else {
        messages.to_vec()
    };

    while trimmed.len() > 1 && estimate_messages_char_count(&trimmed) > max_chars {
        trimmed.remove(0);
    }
    if trimmed.len() == 1 && estimate_messages_char_count(&trimmed) > max_chars {
        trimmed[0] = summarize_message_for_budget(&trimmed[0], max_chars);
    }

    let after_chars = estimate_messages_char_count(&trimmed);
    let stats = RequestPreflightStats {
        before_messages: messages.len(),
        after_messages: trimmed.len(),
        before_chars,
        after_chars,
        dropped_messages: messages.len().saturating_sub(trimmed.len()),
        trimmed_chars: before_chars.saturating_sub(after_chars),
    };
    (trimmed, stats)
}

fn estimate_messages_char_count(messages: &[InputMessage]) -> usize {
    messages
        .iter()
        .map(|message| {
            let role_chars = message.role.chars().count();
            let content_chars: usize = message
                .content
                .iter()
                .map(|block| match block {
                    InputContentBlock::Text { text } => text.chars().count(),
                    InputContentBlock::ToolUse { id, name, input } => {
                        id.chars().count()
                            + name.chars().count()
                            + input.to_string().chars().count()
                    }
                    InputContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => {
                        tool_use_id.chars().count()
                            + content
                                .iter()
                                .map(|part| match part {
                                    crate::modules::api::ToolResultContentBlock::Text { text } => {
                                        text.chars().count()
                                    }
                                    crate::modules::api::ToolResultContentBlock::Json { value } => {
                                        value.to_string().chars().count()
                                    }
                                    // Phase 7C, slice 7C.2 — base64 image bytes
                                    // do not contribute to char-budgets used by
                                    // the textual context summariser.
                                    crate::modules::api::ToolResultContentBlock::Image {
                                        alt,
                                        ..
                                    } => alt.as_deref().map(str::len).unwrap_or(0),
                                })
                                .sum::<usize>()
                    }
                })
                .sum();
            role_chars + content_chars
        })
        .sum()
}

fn estimate_messages_token_count(messages: &[InputMessage]) -> usize {
    estimate_token_count_from_chars(estimate_messages_char_count(messages)) + messages.len()
}

fn summarize_message_for_budget(message: &InputMessage, max_chars: usize) -> InputMessage {
    let mut parts: Vec<String> = Vec::new();
    for block in &message.content {
        match block {
            InputContentBlock::Text { text } => {
                parts.push(format!(
                    "text:{}",
                    truncate_middle_chars(text, TOOL_RESULT_PREVIEW_CHARS)
                ));
            }
            InputContentBlock::ToolUse { id, name, input } => {
                parts.push(format!(
                    "tool_use id={id} name={name} input={}",
                    truncate_middle_chars(&input.to_string(), TOOL_RESULT_PREVIEW_CHARS)
                ));
            }
            InputContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let content_preview = content
                    .iter()
                    .map(|part| match part {
                        crate::modules::api::ToolResultContentBlock::Text { text } => {
                            truncate_middle_chars(text, TOOL_RESULT_PREVIEW_CHARS)
                        }
                        crate::modules::api::ToolResultContentBlock::Json { value } => {
                            truncate_middle_chars(&value.to_string(), TOOL_RESULT_PREVIEW_CHARS)
                        }
                        // Phase 7C, slice 7C.2 — image parts collapse to a
                        // short placeholder so the budget summariser stays
                        // text-only.
                        crate::modules::api::ToolResultContentBlock::Image { source, alt } => {
                            let alt_part = alt.as_deref().unwrap_or("image");
                            format!(
                                "[image:{} {}B {}]",
                                source.media_type,
                                source.data.len(),
                                truncate_middle_chars(alt_part, 32)
                            )
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                parts.push(format!(
                    "tool_result id={tool_use_id} is_error={is_error} content={content_preview}"
                ));
            }
        }
    }
    let joined = parts.join("\n");
    let summary = format!(
        "[context_trim_notice] latest message compacted for request budget. role={} compacted_content=\n{}",
        message.role,
        truncate_middle_chars(&joined, max_chars.saturating_sub(96))
    );
    InputMessage::user_text(summary)
}

fn truncate_middle_chars(input: &str, max_chars: usize) -> String {
    let total = input.chars().count();
    if total <= max_chars || max_chars < 16 {
        return input.to_string();
    }
    let keep_head = max_chars / 2;
    let keep_tail = max_chars.saturating_sub(keep_head + 14);
    let head: String = input.chars().take(keep_head).collect();
    let tail: String = input
        .chars()
        .rev()
        .take(keep_tail)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}[...omitted...]{tail}")
}

fn is_network_timeout_reason(reason: &str) -> bool {
    reason.to_ascii_lowercase().contains("network_timeout:")
}

#[derive(Debug, Default)]
struct SanitizationStats {
    dropped_empty_messages: usize,
    dropped_orphan_tool_results: usize,
    dropped_unmatched_tool_uses: usize,
    dropped_invalid_tool_use_inputs: usize,
    orphan_tool_result_ids: Vec<String>,
    unmatched_tool_use_ids: Vec<String>,
    invalid_tool_use_input_ids: Vec<String>,
}

impl SanitizationStats {
    fn has_changes(&self) -> bool {
        self.dropped_empty_messages > 0
            || self.dropped_orphan_tool_results > 0
            || self.dropped_unmatched_tool_uses > 0
            || self.dropped_invalid_tool_use_inputs > 0
    }

    fn push_orphan_tool_result_id(&mut self, tool_use_id: &str) {
        if self.orphan_tool_result_ids.len() < 8 {
            self.orphan_tool_result_ids.push(tool_use_id.to_string());
        }
    }

    fn push_unmatched_tool_use_id(&mut self, tool_use_id: &str) {
        if self.unmatched_tool_use_ids.len() < 8 {
            self.unmatched_tool_use_ids.push(tool_use_id.to_string());
        }
    }

    fn push_invalid_tool_use_input_id(&mut self, tool_use_id: &str) {
        if self.invalid_tool_use_input_ids.len() < 8 {
            self.invalid_tool_use_input_ids
                .push(tool_use_id.to_string());
        }
    }
}

fn sanitize_messages_for_provider(
    messages: &[InputMessage],
) -> (Vec<InputMessage>, SanitizationStats) {
    let mut sanitized: Vec<InputMessage> = Vec::with_capacity(messages.len());
    let mut expected_tool_results: HashSet<String> = HashSet::new();
    let mut pending_assistant_index: Option<usize> = None;
    let mut stats = SanitizationStats::default();

    for message in messages {
        if let Some(idx) = pending_assistant_index {
            let matched = message.role == "user"
                && message.content.iter().any(|block| {
                    matches!(
                        block,
                        InputContentBlock::ToolResult { tool_use_id, .. } if expected_tool_results.contains(tool_use_id)
                    )
                });
            if !matched {
                let removed_ids = remove_tool_use_blocks(&mut sanitized[idx]);
                stats.dropped_unmatched_tool_uses += removed_ids.len();
                for tool_use_id in removed_ids {
                    stats.push_unmatched_tool_use_id(&tool_use_id);
                }
                pending_assistant_index = None;
                expected_tool_results.clear();
            }
        }

        let mut next_content: Vec<InputContentBlock> = Vec::new();
        for block in &message.content {
            match block {
                InputContentBlock::ToolResult { tool_use_id, .. } => {
                    if message.role == "user" && expected_tool_results.contains(tool_use_id) {
                        next_content.push(block.clone());
                        expected_tool_results.remove(tool_use_id);
                        if expected_tool_results.is_empty() {
                            pending_assistant_index = None;
                        }
                    } else {
                        stats.dropped_orphan_tool_results += 1;
                        stats.push_orphan_tool_result_id(tool_use_id);
                    }
                }
                InputContentBlock::ToolUse { id, input, .. } => {
                    if input.is_object() {
                        next_content.push(block.clone());
                    } else {
                        stats.dropped_invalid_tool_use_inputs += 1;
                        stats.push_invalid_tool_use_input_id(id);
                    }
                }
                _ => next_content.push(block.clone()),
            }
        }

        if next_content.is_empty() {
            stats.dropped_empty_messages += 1;
            continue;
        }

        let tool_use_ids: HashSet<String> = next_content
            .iter()
            .filter_map(|block| match block {
                InputContentBlock::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();

        let sanitized_message = InputMessage {
            role: match message.role.as_str() {
                "assistant" => "assistant".to_string(),
                _ => "user".to_string(),
            },
            content: next_content,
        };
        if !tool_use_ids.is_empty() && sanitized_message.role == "assistant" {
            expected_tool_results = tool_use_ids;
            pending_assistant_index = Some(sanitized.len());
        }

        if sanitized_message.content.is_empty() {
            continue;
        }
        sanitized.push(sanitized_message);
    }

    if let Some(idx) = pending_assistant_index {
        let removed_ids = remove_tool_use_blocks(&mut sanitized[idx]);
        stats.dropped_unmatched_tool_uses += removed_ids.len();
        for tool_use_id in removed_ids {
            stats.push_unmatched_tool_use_id(&tool_use_id);
        }
    }

    let result: Vec<InputMessage> = sanitized
        .into_iter()
        .filter(|message| !message.content.is_empty())
        .collect();
    (result, stats)
}

fn remove_tool_use_blocks(message: &mut InputMessage) -> Vec<String> {
    let mut removed_ids: Vec<String> = Vec::new();
    message.content.retain(|block| {
        if let InputContentBlock::ToolUse { id, .. } = block {
            removed_ids.push(id.clone());
            return false;
        }
        true
    });
    removed_ids
}

fn extend_sample_ids(target: &mut Vec<String>, incoming: &[String], max_samples: usize) {
    for sample in incoming {
        if target.len() >= max_samples {
            break;
        }
        if !target.iter().any(|existing| existing == sample) {
            target.push(sample.clone());
        }
    }
}

fn parse_tool_input_json(raw_input: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(raw_input) {
        Ok(value) if value.is_object() => value,
        _ => serde_json::json!({}),
    }
}

/// Stop an in-flight streaming agent response.
///
/// Sends a cancellation signal to the background task associated with
/// the given stream_id, causing it to terminate early and emit a
/// `stream_complete` event.
#[tauri::command]
pub fn stop_agent_stream(state: State<'_, AppState>, stream_id: String) -> Result<(), String> {
    let mut senders = state
        .stream_cancel_senders
        .lock()
        .map_err(|e| format!("Failed to lock cancel senders: {e}"))?;

    let sender = senders
        .remove(&stream_id)
        .ok_or_else(|| format!("No active stream found for stream_id: {stream_id}"))?;

    // Sending the cancel signal (if the receiver is already dropped, the task completed)
    let _ = sender.send(());
    tracing::info!(
        "[stop_agent_stream] Cancel signal sent for stream_id: {}",
        stream_id
    );

    Ok(())
}

/// TauriPermissionPrompter — bridges the sync PermissionPrompter trait
/// with async Tauri IPC. Emits a `permission-request` event to the
/// frontend and blocks on an mpsc channel until the user responds.
///
/// Usage: register `respond_permission` on the frontend side and have it
/// invoke with `{ sessionId, decision: "allow" | "deny", scope?: "once" | "session" }`.
pub struct TauriPermissionPrompter {
    window: tauri::WebviewWindow,
    session_id: String,
    receiver: std::sync::mpsc::Receiver<PermissionPromptDecision>,
}

#[allow(dead_code)]
impl TauriPermissionPrompter {
    /// Create a new TauriPermissionPrompter.
    pub fn new(
        window: tauri::WebviewWindow,
        session_id: String,
        receiver: std::sync::mpsc::Receiver<PermissionPromptDecision>,
    ) -> Self {
        Self {
            window,
            session_id,
            receiver,
        }
    }
}

impl PermissionPrompter for TauriPermissionPrompter {
    fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision {
        tracing::info!(
            "[permission] request session_id={}, tool={}, current={}, required={}",
            self.session_id,
            request.tool_name,
            request.current_mode.as_str(),
            request.required_mode.as_str()
        );
        // 1. emit confirmation event to the frontend
        let _ = self.window.emit(
            "permission-request",
            serde_json::json!({
                "session_id": self.session_id,
                "tool_name": request.tool_name,
                "permission_mode": request.required_mode.as_str(),
                "current_mode": request.current_mode.as_str(),
                "message": format!(
                    "Tool '{}' requires {} permission (current: {})",
                    request.tool_name,
                    request.required_mode.as_str(),
                    request.current_mode.as_str()
                ),
            }),
        );

        // 2. block waiting for frontend response (mpsc blocks — acceptable in sync context)
        match self
            .receiver
            .recv_timeout(std::time::Duration::from_secs(60))
        {
            Ok(decision) => {
                tracing::info!(
                    "[permission] decision received for session_id={}",
                    self.session_id
                );
                decision
            }
            Err(_) => PermissionPromptDecision::Deny {
                reason: "Permission request timed out".to_string(),
            },
        }
    }
}

/// Respond to a permission request from the frontend.
/// The decision is sent to the waiting TauriPermissionPrompter via mpsc channel.
#[tauri::command]
#[allow(dead_code)]
pub fn respond_permission(
    state: State<'_, AppState>,
    session_id: String,
    decision: String,
    tool_name: Option<String>,
    scope: Option<String>,
) -> Result<(), String> {
    tracing::info!(
        "[permission] respond session_id={}, decision={}, scope={}",
        session_id,
        decision,
        scope.as_deref().unwrap_or("once")
    );
    let decision_enum = match decision.as_str() {
        "allow" => PermissionPromptDecision::Allow,
        _ => PermissionPromptDecision::Deny {
            reason: "User denied permission".to_string(),
        },
    };

    let senders = state
        .permission_senders
        .lock()
        .map_err(|e| format!("Failed to lock permission senders: {e}"))?;

    let sender = senders
        .get(&session_id)
        .ok_or_else(|| "No pending permission request for this session".to_string())?;

    sender
        .send(decision_enum)
        .map_err(|_| "Failed to send permission decision".to_string())?;

    // Phase M4-C P4 — emit harness `PermissionResolved` event so
    // the trace aggregator pairs this resolution with the
    // earlier `PermissionPrompted` event.  Zero-cost no-op when
    // harness is not initialised.
    if let Some(harness) = state.harness.as_ref() {
        let _ = harness.event_bus.emit(AgentEvent::PermissionResolved {
            session_id: session_id.clone(),
            tool_name: tool_name.clone(),
            decision: decision.clone(),
            scope: scope.clone().unwrap_or_else(|| "once".to_string()),
            at: chrono::Utc::now(),
        });
    }

    // Optional session-scoped remember decision
    if scope.as_deref() == Some("session") {
        // Store by latest requested tool in this session if known from channel context.
        // We cannot extract tool_name from the mpsc payload here, so we keep a coarse
        // fallback decision bucket under "*" to be read by the prompter side.
        let mut overrides = state
            .permission_overrides
            .lock()
            .map_err(|e| format!("Failed to lock permission overrides: {e}"))?;
        let session_map = overrides.entry(session_id).or_default();
        let key = tool_name.unwrap_or_else(|| "*".to_string());
        session_map.insert(
            key,
            match decision.as_str() {
                "allow" => PermissionPromptDecision::Allow,
                _ => PermissionPromptDecision::Deny {
                    reason: "User denied permission (session policy)".to_string(),
                },
            },
        );
    }

    Ok(())
}
