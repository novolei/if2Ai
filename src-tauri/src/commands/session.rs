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
