//! Application service registry — GAP-004 (T-009) command boundary thinning.
//!
//! Bundles the core business services that Tauri commands depend on
//! into a single [`ServiceRegistry`] struct.  This lets [`AppState`]
//! hold a single `Arc<ServiceRegistry>` instead of a flat list of
//! ad-hoc fields, keeping the command layer thin.
//!
//! ## GAP-004 contract
//!
//! - New commands MUST obtain services through this registry, not
//!   through ad-hoc `state.field` access.
//! - Existing commands continue to work via the direct fields on
//!   [`AppState`] during the transition period (IPC contract
//!   preserved per GAP-004 §Contract).
//! - Command error mapping uses [`map_command_error`] for a
//!   consistent user-facing error shape.

use std::sync::Arc;

use crate::modules::projects::ProjectManager;
use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;

/// Central registry of application services consumed by the command layer.
///
/// Constructed once in `bootstrap::app::build_app_bootstrap` alongside
/// [`AppStateConfig`] and held as `Arc<Self>` on [`AppState`].
pub struct ServiceRegistry {
    /// Session manager for conversation persistence.
    pub session_manager: Arc<SessionManager>,
    /// Tool registry for available tools.
    pub tool_registry: Arc<ToolRegistry>,
    /// Project manager for multi-project support.
    pub project_manager: Arc<ProjectManager>,
}

impl ServiceRegistry {
    /// Build the registry from already-initialised service instances.
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

/// Unified error mapping for the IPC command layer.
///
/// Converts a domain-level error into a consistent user-facing
/// `String` while preserving the internal context for tracing.
/// All Tauri commands should use this function (or a wrapper that
/// delegates to it) when returning `Result<_, String>` so the
/// frontend sees a predictable error shape.
///
/// ## Example
///
/// ```ignore
/// #[tauri::command]
/// async fn my_command(state: State<'_, AppState>) -> Result<MyPayload, String> {
///     let session = state.service_registry
///         .session_manager
///         .restore_session(&id)
///         .await
///         .map_err(|e| map_command_error("restore_session", e))?;
///     Ok(MyPayload { ... })
/// }
/// ```
pub fn map_command_error<E: std::fmt::Display>(context: &str, error: E) -> String {
    tracing::warn!(%context, %error, "command error mapped");
    format!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn service_registry_constructs_with_all_fields() {
        let sessions_dir = PathBuf::from("/tmp/test-sessions");
        let projects_dir = PathBuf::from("/tmp/test-projects");
        let session_mgr = SessionManager::new(sessions_dir.clone(), projects_dir.clone());
        let tool_ctx = crate::modules::tools::ToolContext::default_for_workdir(PathBuf::from("."));
        let tool_reg = ToolRegistry::new(Arc::new(std::sync::Mutex::new(tool_ctx)));
        let project_mgr = ProjectManager::new(projects_dir);

        let registry = ServiceRegistry::new(session_mgr, tool_reg, project_mgr);
        assert!(Arc::strong_count(&registry.session_manager) >= 1);
        assert!(Arc::strong_count(&registry.tool_registry) >= 1);
        assert!(Arc::strong_count(&registry.project_manager) >= 1);
    }

    #[test]
    fn map_command_error_produces_consistent_format() {
        let err = map_command_error("restore_session", "session not found");
        assert!(err.contains("restore_session"));
        assert!(err.contains("session not found"));
        assert!(err.contains(": "));
    }
}
