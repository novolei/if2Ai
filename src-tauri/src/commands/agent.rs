//! Agent commands - run_agent_turn
//!
//! Provides the main agent execution command for Tauri.

use tauri::State;

use super::AppState;

/// Response from a run_agent_turn command.
#[derive(serde::Serialize)]
pub struct RunAgentTurnResponse {
    /// The generated message/response.
    pub message: String,
    /// The session ID.
    pub session_id: String,
}

/// Run a single agent turn with the given user message.
///
/// This is the main entry point for the frontend to interact with the agent.
#[tauri::command]
#[allow(dead_code)]
pub async fn run_agent_turn(
    _state: State<'_, AppState>,
    session_id: String,
    user_message: String,
) -> Result<RunAgentTurnResponse, String> {
    // For now, return a placeholder response
    // The actual implementation would:
    // 1. Restore or create the session
    // 2. Run the agent loop
    // 3. Save the updated session
    Ok(RunAgentTurnResponse {
        message: format!("Echo: {}", user_message),
        session_id,
    })
}
