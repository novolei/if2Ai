//! Session commands - list_sessions, delete_session
//!
//! Provides session management commands for Tauri.

use tauri::State;

use crate::commands::AppState;
use crate::modules::session::SessionMeta;

/// List all sessions.
#[tauri::command]
#[allow(dead_code)]
pub async fn list_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<SessionMeta>, String> {
    state
        .session_manager
        .list_sessions()
        .await
        .map_err(|e| e.to_string())
}

/// Delete a session by ID.
#[tauri::command]
#[allow(dead_code)]
pub async fn delete_session(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state
        .session_manager
        .delete_session(&id)
        .await
        .map_err(|e| e.to_string())
}
