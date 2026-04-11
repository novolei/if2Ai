//! Agent commands - run_agent_turn
//!
//! Provides the main agent execution command for Tauri.

use tauri::State;

use crate::commands::AppState;
use crate::modules::runtime::session::ConversationMessage;

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
/// It calls the ConversationRuntime with the session and returns the result.
#[tauri::command]
#[allow(dead_code)]
pub async fn run_agent_turn(
    state: State<'_, AppState>,
    session_id: String,
    user_message: String,
) -> Result<RunAgentTurnResponse, String> {
    // Restore the session
    let mut session = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    // Add user message to session
    session
        .messages
        .push(ConversationMessage::user_text(user_message.clone()));

    // For now, use a simple response since full ConversationRuntime integration
    // requires bridging async ProviderClient with sync ApiClient trait.
    // TODO: Implement full ConversationRuntime integration with proper async bridge
    let response_text = format!(
        "Received your message: '{}'. Session ID: {}. (Full agent integration pending)",
        user_message, session_id
    );

    // Add assistant response to session
    session.messages.push(ConversationMessage::assistant(vec![
        crate::modules::runtime::session::ContentBlock::Text {
            text: response_text.clone(),
        },
    ]));

    // Save the updated session
    state
        .session_manager
        .save_session(&session)
        .await
        .map_err(|e| e.to_string())?;

    Ok(RunAgentTurnResponse {
        message: response_text,
        session_id,
    })
}
