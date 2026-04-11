//! Commands module - Tauri command handlers
//!
//! Provides IPC commands for the frontend to interact with the backend.

use std::sync::Arc;

use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;

/// Application state shared across all Tauri commands.
#[allow(dead_code)]
pub struct AppState {
    /// Session manager for conversation persistence.
    pub session_manager: Arc<SessionManager>,
    /// Tool registry for available tools.
    pub tool_registry: Arc<ToolRegistry>,
}

#[allow(dead_code)]
impl AppState {
    /// Create a new AppState with the given session manager and tool registry.
    #[must_use]
    pub fn new(session_manager: SessionManager, tool_registry: ToolRegistry) -> Self {
        Self {
            session_manager: Arc::new(session_manager),
            tool_registry: Arc::new(tool_registry),
        }
    }
}

pub mod agent;
pub mod session;

#[allow(unused_imports)]
pub use agent::{run_agent_turn, RunAgentTurnResponse};
#[allow(unused_imports)]
pub use session::{delete_session, list_sessions};
