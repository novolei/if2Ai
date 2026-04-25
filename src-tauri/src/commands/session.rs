//! Session commands - list_sessions, delete_session, create_session, list_project_sessions
//!
//! Provides session management commands for Tauri.

use tauri::{AppHandle, Manager, State};

use crate::commands::AppState;
use crate::modules::identity::{
    apply_identity_customization_pack, read_identity_customization_pack, IdentityRegistry,
    SessionIdentityOverride,
};
use crate::modules::session::{ConversationUndoStatus, Session, SessionMeta};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistoryPageResponse {
    pub event_page: crate::modules::runtime::history::SessionHistoryEventPage,
    pub replay: crate::modules::runtime::history::SessionHistoryReplay,
    /// When the event log is empty and history falls back to session.json,
    /// this field records the reason so callers can distinguish canonical
    /// (event-log) reads from compatibility fallback reads.  GAP-001.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_session: Option<Session>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionIdentityInput {
    #[serde(default, alias = "soulId")]
    pub soul_id: Option<String>,
    #[serde(default, alias = "personaId")]
    pub persona_id: Option<String>,
}

fn normalized_identity_override(
    identity: Option<SessionIdentityInput>,
) -> Result<Option<SessionIdentityOverride>, String> {
    let Some(identity) = identity else {
        return Ok(None);
    };

    // Validate against the **effective** registry (built-in + custom
    // personas from identity-pack.json) so a session can pick a
    // user-created persona without hitting "unknown persona id".
    // Best-effort pack read: a malformed pack falls back to built-in
    // only — better to reject the override than to corrupt session state.
    let builtin = IdentityRegistry::builtin();
    let pack = read_identity_customization_pack().unwrap_or_default();
    let registry = apply_identity_customization_pack(&builtin, &pack);
    let mut soul_id = identity
        .soul_id
        .and_then(|value| (!value.trim().is_empty()).then_some(value));
    let persona_id = identity
        .persona_id
        .and_then(|value| (!value.trim().is_empty()).then_some(value));

    if let Some(ref soul) = soul_id {
        if registry.soul(soul).is_none() {
            return Err(format!("unknown soul id: {soul}"));
        }
    }

    if let Some(ref persona) = persona_id {
        let persona_def = registry
            .persona(persona)
            .ok_or_else(|| format!("unknown persona id: {persona}"))?;
        match soul_id.as_deref() {
            Some(soul) if soul != persona_def.soul_id => {
                return Err(format!(
                    "persona `{persona}` does not belong to soul `{soul}`"
                ));
            }
            None => {
                soul_id = Some(persona_def.soul_id.clone());
            }
            _ => {}
        }
    }

    Ok(Some(SessionIdentityOverride {
        soul_id,
        persona_id,
    }))
}

/// Create a new session.
///
/// If project_id is empty, creates a legacy session (no project).
/// If project_id is non-empty, creates a session within that project.
#[tauri::command]
#[allow(dead_code)]
pub async fn create_session(
    state: State<'_, AppState>,
    project_id: String,
    title: String,
    identity: Option<SessionIdentityInput>,
) -> Result<Session, String> {
    let normalized_identity = normalized_identity_override(identity)?;
    if project_id.is_empty() {
        match normalized_identity {
            Some(identity) => state
                .session_manager
                .create_session_with_identity(
                    title,
                    String::new(),
                    identity.soul_id,
                    identity.persona_id,
                )
                .await
                .map_err(|e| e.to_string()),
            None => state
                .session_manager
                .create_session(title)
                .await
                .map_err(|e| e.to_string()),
        }
    } else {
        match normalized_identity {
            Some(identity) => state
                .session_manager
                .create_session_with_identity(
                    title,
                    project_id,
                    identity.soul_id,
                    identity.persona_id,
                )
                .await
                .map_err(|e| e.to_string()),
            None => state
                .session_manager
                .create_session_for_project(&project_id, title)
                .await
                .map_err(|e| e.to_string()),
        }
    }
}

/// List all sessions (legacy path only).
#[tauri::command]
#[allow(dead_code)]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionMeta>, String> {
    state
        .session_manager
        .list_sessions()
        .await
        .map_err(|e| e.to_string())
}

/// List sessions within a specific project.
#[tauri::command]
#[allow(dead_code)]
pub async fn list_project_sessions(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<SessionMeta>, String> {
    state
        .session_manager
        .list_project_sessions(&project_id)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a session by ID.
#[tauri::command]
#[allow(dead_code)]
pub async fn delete_session(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .session_manager
        .delete_session(&id)
        .await
        .map_err(|e| e.to_string())
}

/// MEM-MOD-WIRE-FIX-2 — fire the `MemoryTicker::on_session_end` hook
/// for the given session. Called by the frontend whenever a session
/// loses focus (user clicks "+" to start a new session, switches to a
/// different session in the sidebar, or closes the tab/window).
///
/// Without this IPC the post-Pack feature loop is broken end-to-end:
/// `on_session_end` fires the rolling-summary flush + compile_today +
/// (P5) reflection extraction + (P7) `learned_traits` distillation.
/// All of those depend on a session "ending" — which historically had
/// **no caller** in the codebase, so memory.md stayed empty and the
/// learned-traits panel always read 0.
///
/// Idempotent + best-effort: a missing session is a 200, not a 500
/// (the user may have just deleted it). Errors loading the session are
/// converted to a structured warn so the UI doesn't surface a popup
/// for a hook that's intentionally fire-and-forget.
#[tauri::command]
#[allow(dead_code)]
pub async fn close_session(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    use crate::modules::memory::scope::MemoryExecutionScope;
    use crate::modules::runtime::conversation::TurnHook;

    let session = match state.session_manager.restore_session(&id).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                session_id = %id,
                error = %e,
                "[session_close] cannot restore session for end-hook; skipping"
            );
            return Ok(());
        }
    };

    let scope = MemoryExecutionScope {
        session_id: Some(session.id.clone()),
        project_id: if session.project_id.is_empty() {
            None
        } else {
            Some(session.project_id.clone())
        },
        // workdir is request-scoped (resolved per turn from
        // ToolContext); for the session-end hook we have no
        // active turn, so leave it None.
        workdir: None,
    };

    tracing::info!(
        session_id = %session.id,
        project_id = scope.project_id.as_deref().unwrap_or("-"),
        message_count = session.messages.len(),
        "[session_close] firing memory_ticker.on_session_end"
    );

    state
        .memory_ticker
        .on_session_end(&scope, &session.id, &session.messages);

    // MIG-020 (T-006): Update supervisor state on session close.
    if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
        if let Ok(mut snap) = crate::modules::runtime::supervisor::SessionSupervisor::load_or_create(
            &app_data_dir,
            &id,
        ) {
            crate::modules::runtime::supervisor::SessionSupervisor::close_session(&mut snap);
            if let Err(e) =
                crate::modules::runtime::supervisor::write_supervisor_snapshot(&app_data_dir, &snap)
            {
                tracing::warn!(
                    session_id = %id,
                    error = %e,
                    "[supervisor] failed to persist close_session"
                );
            }
        }
    }

    Ok(())
}

/// Rename a session in place.
#[tauri::command]
#[allow(dead_code)]
pub async fn rename_session(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<SessionMeta, String> {
    state
        .session_manager
        .rename_session(&id, title)
        .await
        .map(|session| SessionMeta::from_session(&session))
        .map_err(|e| e.to_string())
}

/// Set the pinned state of a session.
#[tauri::command]
#[allow(dead_code)]
pub async fn set_session_pinned(
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> Result<Session, String> {
    state
        .session_manager
        .set_session_pinned(&id, pinned)
        .await
        .map_err(|e| e.to_string())
}

/// Toggle per-session memory on or off.
///
/// `enabled = false` silences the rolling summary / compile / fact-extract
/// pipelines for this session even when the master memory switch is on,
/// and stamps `memory_disabled_since = now`.  `enabled = true` re-enables
/// memory and stamps `memory_reenabled_at = now`.  Both timestamps are
/// later read by the compile pipeline (lands in 8B) to skip the silenced
/// window when aggregating summaries.  Phase 8A.4 / v2 §Sprint 1 / T-A4.
#[tauri::command]
#[allow(dead_code)]
pub async fn memory_session_set_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<Session, String> {
    state
        .session_manager
        .set_session_memory_enabled(&id, enabled)
        .await
        .map_err(|e| e.to_string())
}

/// Update session-level identity override.
#[tauri::command]
#[allow(dead_code)]
pub async fn set_session_identity(
    state: State<'_, AppState>,
    id: String,
    identity: SessionIdentityInput,
) -> Result<SessionMeta, String> {
    let normalized =
        normalized_identity_override(Some(identity))?.unwrap_or(SessionIdentityOverride {
            soul_id: None,
            persona_id: None,
        });
    state
        .session_manager
        .set_session_identity(&id, normalized.soul_id, normalized.persona_id)
        .await
        .map(|session| SessionMeta::from_session(&session))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::SessionIdentityInput;

    #[test]
    fn session_identity_input_accepts_frontend_snake_case() {
        let input: SessionIdentityInput = serde_json::from_value(serde_json::json!({
            "soul_id": "if2ai-core",
            "persona_id": "execution-partner"
        }))
        .expect("deserialize snake_case identity");

        assert_eq!(input.soul_id.as_deref(), Some("if2ai-core"));
        assert_eq!(input.persona_id.as_deref(), Some("execution-partner"));
    }

    #[test]
    fn session_identity_input_accepts_tauri_camel_case() {
        let input: SessionIdentityInput = serde_json::from_value(serde_json::json!({
            "soulId": "if2ai-core",
            "personaId": "execution-partner"
        }))
        .expect("deserialize camelCase identity");

        assert_eq!(input.soul_id.as_deref(), Some("if2ai-core"));
        assert_eq!(input.persona_id.as_deref(), Some("execution-partner"));
    }
}

/// Replace session-scoped active skill ids (prompt + low-trust tool attenuation).
#[tauri::command]
#[allow(dead_code)]
pub async fn set_session_active_skill_ids(
    state: State<'_, AppState>,
    id: String,
    active_skill_ids: Vec<String>,
) -> Result<Session, String> {
    state
        .session_manager
        .set_session_active_skill_ids(&id, active_skill_ids)
        .await
        .map_err(|e| e.to_string())
}

/// Get a session with its messages (for restoring chat history).
#[tauri::command]
#[allow(dead_code)]
pub async fn get_session(state: State<'_, AppState>, id: String) -> Result<Session, String> {
    state
        .session_manager
        .restore_session(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Whether transcript undo/redo is available for this session (in-memory stacks).
#[tauri::command]
#[allow(dead_code)]
pub async fn session_undo_status(
    state: State<'_, AppState>,
    id: String,
) -> Result<ConversationUndoStatus, String> {
    Ok(state.session_manager.conversation_undo_status(&id))
}

/// Restore the previous transcript snapshot (one level).
#[tauri::command]
#[allow(dead_code)]
pub async fn session_undo(state: State<'_, AppState>, id: String) -> Result<Session, String> {
    state
        .session_manager
        .apply_conversation_undo(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Re-apply the last undone transcript snapshot.
#[tauri::command]
#[allow(dead_code)]
pub async fn session_redo(state: State<'_, AppState>, id: String) -> Result<Session, String> {
    state
        .session_manager
        .apply_conversation_redo(&id)
        .await
        .map_err(|e| e.to_string())
}

/// Drain buffered job-monitor lines for a session (P2-12 diagnostics).
#[tauri::command]
#[allow(dead_code)]
pub async fn drain_job_monitor_lines(id: String) -> Result<Vec<String>, String> {
    Ok(crate::modules::application::job_monitor::drain_lines(&id))
}

/// Page through canonical run-log events and replay them into history projection.
///
/// When no event log exists for the first page, returns the legacy full
/// session in `fallback_session` so existing callers can stay compatible
/// while new history consumers migrate to event-log replay.
#[tauri::command]
#[allow(dead_code)]
pub async fn get_session_history_page(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    id: String,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<SessionHistoryPageResponse, String> {
    let base_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let event_page = crate::modules::runtime::history::read_session_history_event_page(
        &base_dir,
        &id,
        limit,
        cursor.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    let replay = crate::modules::runtime::history::replay_session_history(&id, &event_page.entries);

    // GAP-001: When the event log is empty on the first page, fall back to
    // session.json as a compatibility source.  Record the fallback reason
    // so callers and diagnostics can distinguish canonical event-log reads
    // from legacy compatibility reads.
    let mut fallback_reason = None;
    let fallback_session = if cursor.is_none() && event_page.entries.is_empty() {
        fallback_reason = Some("session_history_fallback: event log is empty".to_string());
        tracing::info!(
            session_id = %id,
            "[GAP-001] history fallback to session.json: event log has no entries"
        );
        Some(
            state
                .session_manager
                .restore_session(&id)
                .await
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };

    Ok(SessionHistoryPageResponse {
        event_page,
        replay,
        fallback_reason,
        fallback_session,
    })
}

/// Read the canonical supervisor snapshot for a session (MIG-020 / T-006).
///
/// Returns the supervisor lifecycle state — active run, run status,
/// blocked/recoverable, pending permission count, retry budget, and
/// disconnect grace window.  The frontend uses this to render
/// session-level status labels (active / blocked / recoverable failed)
/// without assembling state from scattered sources.
#[tauri::command]
#[allow(dead_code)]
pub async fn get_supervisor_snapshot(
    app_handle: AppHandle,
    id: String,
) -> Result<crate::modules::runtime::supervisor::SupervisorSnapshot, String> {
    let base_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let snapshot =
        crate::modules::runtime::supervisor::SessionSupervisor::load_or_create(&base_dir, &id)
            .map_err(|e| e.to_string())?;
    Ok(snapshot)
}
