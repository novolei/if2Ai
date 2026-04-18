//! Session commands - list_sessions, delete_session, create_session, list_project_sessions
//!
//! Provides session management commands for Tauri.

use tauri::State;

use crate::commands::AppState;
use crate::modules::session::{Session, SessionMeta};

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
) -> Result<Session, String> {
    if project_id.is_empty() {
        state
            .session_manager
            .create_session(title)
            .await
            .map_err(|e| e.to_string())
    } else {
        state
            .session_manager
            .create_session_for_project(&project_id, title)
            .await
            .map_err(|e| e.to_string())
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
