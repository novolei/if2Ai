//! Agent commands - thin IPC adapters over `application::TurnService`.
//!
//! After MIG-001-c, the inline orchestration body for both
//! `run_agent_turn` and `start_agent_stream` lives in
//! `crate::modules::application::turn_service::*`. This file is
//! intentionally lean: it parses Tauri command parameters,
//! constructs a per-call `TurnService`, and delegates.

use tauri::{AppHandle, Manager, State};

use crate::commands::AppState;
#[cfg(test)]
use crate::modules::application::ToolRegistryExecutor;
use crate::modules::application::{TurnService, TurnServiceDeps};
#[cfg(test)]
use crate::modules::control_plane::SessionExecutionContext;
#[cfg(test)]
use crate::modules::runtime::conversation::{
    ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError,
};
use crate::modules::runtime::pending_permission::PendingPermissionRecord;
#[cfg(test)]
use crate::modules::runtime::permissions::PermissionMode;
use crate::modules::runtime::resume_cursor::parse_resume_cursor;
#[cfg(test)]
use crate::modules::runtime::session::ContentBlock;
use crate::modules::runtime::session::MessageRole;

/// Phase M1.1 / MIG-001-a — construct a per-call [`TurnService`]
/// from the already-shared `AppState` handles. Held as a small
/// helper so the IPC adapter does not have to repeat the
/// dependency wiring at every call site.
///
/// MIG-001-a expanded the [`TurnServiceDeps`] surface with the
/// session / harness / learning / runtime-budget / memory-ticker /
/// trajectory / `AppHandle` dependencies the canonical turn
/// orchestrator will own in MIG-001-b/c/d. The IPC adapter is the
/// only place that knows how to map `AppState` into `TurnService`,
/// because `application::*` MUST NOT import `crate::commands::*`.
///
/// Note: [`TurnService`] is intentionally cheap to construct
/// (`Arc` clones only); it does not need to live on `AppState`
/// during the M1 transition.
fn make_turn_service(state: &AppState, app_handle: Option<AppHandle>) -> TurnService {
    TurnService::new(TurnServiceDeps {
        tool_registry: state.tool_registry.clone(),
        pinned_store: state.pinned_store.clone(),
        memory_provider: state.memory_provider.clone(),
        active_retrieval_manager: state.active_retrieval_manager.clone(),
        session_manager: state.session_manager.clone(),
        project_manager: state.project_manager.clone(),
        harness: state.harness.clone(),
        learning_module: state.learning_module.clone(),
        context_budget: state.context_budget.clone(),
        memory_ticker: state.memory_ticker.clone(),
        trajectory_manager: state.trajectory_manager.clone(),
        app_handle,
        learned_traits: state.learned_traits.clone(),
        rolling_summarizer: state.rolling_summarizer.clone(),
        utility_llm: state.utility_llm.clone(),
        trajectory_collector: state.trajectory_collector.clone(),
        // T11 — preserve the env-var-derived `IF2AI_AGENT_MAX_ITERATIONS`
        // cap (default 10) at the AppState construction seam.  Without
        // this override, `AgenticLoopConfig::default()` would let
        // `run_agentic_loop` use the Steward baseline of 50, silently
        // 5× the production cap once both delegates (T5c stream + T10
        // sync) consume `loop_config.max_iterations`.
        loop_config: crate::modules::application::turn_service::AgenticLoopConfig {
            max_iterations: crate::modules::application::turn_service::agent_max_iterations_cap(),
            ..crate::modules::application::turn_service::AgenticLoopConfig::default()
        },
    })
}

// MIG-001-b: `RunAgentTurnResponse` is now sourced from
// `application::turn_service::RunTurnResponse` so the IPC contract
// stays bit-identical while ownership of the type follows the
// canonical orchestrator.
pub use crate::modules::application::RunTurnResponse as RunAgentTurnResponse;

// Note: helpers that the legacy IPC body used (
// `resolve_session_execution_context`, `parse_permission_mode`,
// `build_permission_policy`, `app_session_to_runtime`,
// `log_context_fingerprint`, the `MAX_REQUEST_*` budget constants,
// and the runtime / stream / memory shimmed re-exports) all moved
// with the orchestration body into
// `crate::modules::application::turn_service::{run, stream}` in
// MIG-001-b/c. See those modules for the canonical owners.

// `RetrievedMemoryContext` / `map_scored_memory_to_payload` /
// `retrieve_memory_context` moved to
// [`crate::modules::application::memory_injection_service`] in
// Phase M1.4. Call sites below now resolve memory through the
// `TurnService` seam.

// record_trajectory_if_possible moved to crate::modules::application::trajectory_service (GFR-006c).

/// Run a single agent turn with the given user message.
///
/// MIG-001-b — this IPC command is now a thin adapter over
/// [`crate::modules::application::TurnService::run_turn`]. It
/// constructs a per-call `TurnService` from the shared `AppState`
/// and delegates the entire `prepare -> execute -> finalize`
/// lifecycle. The previous 558-line inline orchestration body now
/// lives in `application/turn_service/run.rs` so the canonical
/// turn ownership sits behind a single seam (CHARTER §2.1).
#[tauri::command]
#[allow(dead_code)]
pub async fn run_agent_turn(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    session_id: String,
    user_message: String,
    permission_mode: Option<String>,
) -> Result<RunAgentTurnResponse, String> {
    state.daydream_coordinator.note_activity().await;
    let service = make_turn_service(&state, Some(app_handle));
    service
        .run_turn(crate::modules::application::RunTurnRequest {
            session_id,
            user_message,
            permission_mode,
        })
        .await
}

/// Start a streaming agent turn.
///
/// MIG-001-c — this IPC command is now a thin adapter over
/// [`crate::modules::application::TurnService::stream_turn`]. It
/// constructs a per-call `TurnService` from the shared `AppState`,
/// wires the per-process cross-stream coordination state
/// (`stream_cancel_senders`, `permission_senders`,
/// `permission_overrides`) into the request, and delegates the
/// streaming-turn lifecycle. The previous ~1700-line inline
/// orchestration body now lives in
/// `application/turn_service/stream.rs` (MIG-001-d will further
/// split that file internally).
#[tauri::command]
pub async fn start_agent_stream(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    session_id: String,
    user_message: String,
    permission_mode: Option<String>,
    provider_id: Option<String>,
    model_id: Option<String>,
) -> Result<String, String> {
    state.daydream_coordinator.note_activity().await;
    let stream_cancel_senders = state.stream_cancel_senders.clone();
    let permission_senders = state.permission_senders.clone();
    let permission_overrides = state.permission_overrides.clone();
    if let (Some(provider_id), Some(model_id)) = (provider_id.as_deref(), model_id.as_deref()) {
        crate::modules::config::model_resolver::ModelResolver::set_role_config(
            "chat",
            &format!("{provider_id}/{model_id}"),
        )
        .await?;
    }
    let service = make_turn_service(&state, Some(app_handle));
    service
        .stream_turn(crate::modules::application::StreamTurnRequest {
            session_id,
            user_message,
            permission_mode,
            stream_cancel_senders,
            permission_senders,
            permission_overrides,
        })
        .await
}

// stream-error-reason cluster moved to runtime::stream_error_reason (GFR-005b).

// truncate_tool_result_for_model / summarize_tool_result_for_model /
// short_text_digest moved to
// crate::modules::runtime::block_conversion (GFR-001). The latter two
// are imported above for residual call sites in this file;
// truncate_tool_result_for_model has no residual caller in this file.

// resume-cursor cluster moved to crate::modules::runtime::resume_cursor (GFR-005a).

// runtime_block_to_input_block moved to
// crate::modules::runtime::block_conversion (GFR-001).

// Governor cluster (RequestPreflightStats + ContextGovernor +
// apply_request_preflight_limits) moved to
// crate::modules::application::prompt_planner::governor (GFR-002b).

// Preflight estimators (estimate_messages_char_count + token_count +
// summarize_message_for_budget + truncate_middle_chars) moved to
// crate::modules::application::prompt_planner::preflight (GFR-002c).
// apply_request_preflight_limits moved to governor (GFR-002b).

// is_network_timeout_reason moved to runtime::stream_error_reason (GFR-005b).

// Sanitize cluster (SanitizationStats + sanitize_messages_for_provider +
// remove_tool_use_blocks + extend_sample_ids) moved to
// crate::modules::application::prompt_planner::sanitize (GFR-002a).

// parse_tool_input_json moved to
// crate::modules::runtime::block_conversion (GFR-001).

/// Stop an in-flight streaming agent response.
///
/// Thin IPC adapter — delegates to
/// [`crate::modules::application::stream_cancel_service::cancel_stream`]
/// which owns the lookup-and-fire behaviour.
#[tauri::command]
pub fn stop_agent_stream(state: State<'_, AppState>, stream_id: String) -> Result<(), String> {
    crate::modules::application::stream_cancel_service::cancel_stream(
        &state.stream_cancel_senders,
        stream_id,
    )
}

/// Resume a previously-stopped agent run from its resume cursor.
///
/// Looks up the latest resume cursor from the session's assistant
/// messages, wraps it in the `[resume_cursor]` marker protocol, and
/// delegates to [`crate::modules::application::TurnService::stream_turn`].
///
/// When `user_message` is `None` or empty, a default continuation
/// prompt is used.
#[tauri::command]
pub async fn resume_run(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    session_id: String,
    user_message: Option<String>,
    permission_mode: Option<String>,
    provider_id: Option<String>,
    model_id: Option<String>,
) -> Result<String, String> {
    state.daydream_coordinator.note_activity().await;
    let session = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| format!("Failed to load session: {e}"))?;

    // Find the latest assistant message with a resume cursor.
    let resume_cursor = session
        .messages
        .iter()
        .rev()
        .find(|m| m.role == MessageRole::Assistant && m.resume_cursor.is_some())
        .and_then(|m| m.resume_cursor.clone())
        .ok_or_else(|| "No resume cursor found in session".to_string())?;

    // Validate the cursor is well-formed.
    let _ = parse_resume_cursor(&resume_cursor)
        .ok_or_else(|| format!("Invalid resume cursor: {resume_cursor}"))?;

    let user_text = user_message
        .as_deref()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or("请继续完成未完成的任务。");
    let resume_message = format!("[resume_cursor] {resume_cursor}; {user_text}");

    let stream_cancel_senders = state.stream_cancel_senders.clone();
    let permission_senders = state.permission_senders.clone();
    let permission_overrides = state.permission_overrides.clone();
    if let (Some(provider_id), Some(model_id)) = (provider_id.as_deref(), model_id.as_deref()) {
        crate::modules::config::model_resolver::ModelResolver::set_role_config(
            "chat",
            &format!("{provider_id}/{model_id}"),
        )
        .await?;
    }

    let service = make_turn_service(&state, Some(app_handle));
    service
        .stream_turn(crate::modules::application::StreamTurnRequest {
            session_id,
            user_message: resume_message,
            permission_mode,
            stream_cancel_senders,
            permission_senders,
            permission_overrides,
        })
        .await
}

// TauriPermissionPrompter moved to
// crate::modules::application::permission_service (GFR-003).

/// Respond to a permission request from the frontend.
///
/// Thin IPC adapter — delegates to
/// [`crate::modules::application::permission_service::respond_to_permission_prompt`]
/// which owns the decision decoding, mpsc forwarding, harness
/// event emission, and session-scope override bookkeeping.
#[tauri::command]
#[allow(dead_code)]
pub fn respond_permission(
    state: State<'_, AppState>,
    session_id: String,
    decision: String,
    tool_name: Option<String>,
    scope: Option<String>,
) -> Result<(), String> {
    crate::modules::application::permission_service::respond_to_permission_prompt(
        &state.permission_senders,
        &state.permission_overrides,
        state.harness.as_ref(),
        session_id,
        decision,
        tool_name,
        scope,
    )
}

/// Return the current pending permission for a session, if one is recoverable.
#[tauri::command]
pub fn get_pending_permission(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    session_id: String,
) -> Result<Option<PendingPermissionRecord>, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir: {err}"))?;
    let pending = crate::modules::runtime::pending_permission::read_pending_permission(
        &app_data_dir,
        &session_id,
    )
    .map_err(|err| format!("Failed to read pending permission: {err}"))?;
    if pending.is_none() {
        return Ok(None);
    }

    let has_live_waiter = state
        .permission_senders
        .lock()
        .map_err(|err| format!("Failed to lock permission senders: {err}"))?
        .contains_key(&session_id);
    if !has_live_waiter {
        crate::modules::runtime::pending_permission::clear_pending_permission(
            &app_data_dir,
            &session_id,
        )
        .map_err(|err| format!("Failed to clear stale pending permission: {err}"))?;
        return Ok(None);
    }

    Ok(pending)
}

#[cfg(test)]
mod tests;
