//! Commands module - Tauri command handlers
//!
//! Provides IPC commands for the frontend to interact with the backend.

use std::sync::Arc;

use crate::modules::projects::ProjectManager;
use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;

/// Application state shared across all Tauri commands.
#[allow(dead_code)]
pub struct AppState {
    /// Session manager for conversation persistence.
    pub session_manager: Arc<SessionManager>,
    /// Tool registry for available tools.
    pub tool_registry: Arc<ToolRegistry>,
    /// Project manager for multi-project support.
    pub project_manager: Arc<ProjectManager>,
}

#[allow(dead_code)]
impl AppState {
    /// Create a new AppState with the given managers.
    #[must_use]
    pub fn new(
        session_manager: SessionManager,
        tool_registry: ToolRegistry,
        project_manager: ProjectManager,
    ) -> Self {
        Self {
            session_manager: Arc::new(session_manager),
            tool_registry: Arc::new(tool_registry),
            project_manager: Arc::new(project_manager),
        }
    }
}

pub mod agent;
pub mod project;
pub mod session;
pub mod tools;
pub mod window;

#[allow(unused_imports)]
pub use agent::{run_agent_turn, start_agent_stream, RunAgentTurnResponse};
#[allow(unused_imports)]
pub use project::{
    create_permanent_worktree, create_project, delete_project, get_project, list_projects,
    open_project_in_finder, rename_project,
};
#[allow(unused_imports)]
pub use session::{
    create_session, delete_session, get_session, list_project_sessions, list_sessions,
    set_session_pinned,
};
#[allow(unused_imports)]
pub use tools::{
    execute_tool, get_tool_definitions, list_tools, list_toolsets, ToolCallResult, ToolDefinition,
};
#[allow(unused_imports)]
pub use window::{close_settings_window, open_settings_window};
