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

#[allow(unused_imports)]
pub use agent::{run_agent_turn, RunAgentTurnResponse};
#[allow(unused_imports)]
pub use project::{create_project, delete_project, get_project, list_projects, rename_project};
#[allow(unused_imports)]
pub use session::{create_session, delete_session, list_project_sessions, list_sessions};
