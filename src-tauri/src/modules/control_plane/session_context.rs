//! Session context resolver for control-plane execution.

use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use crate::modules::projects::ProjectManager;
use crate::modules::runtime::permissions::PermissionMode;
use crate::modules::session::{Session as AppSession, SessionManager};

/// Immutable execution context snapshot for a single session-scoped tool flow.
#[derive(Debug, Clone)]
pub struct SessionExecutionContext {
    /// Session ID bound to this execution.
    pub session_id: String,
    /// Owning project ID (empty for legacy sessions).
    pub project_id: String,
    /// Effective workdir used for tool execution.
    pub workdir: PathBuf,
    /// Effective permission mode for this execution.
    pub permission_mode: PermissionMode,
    /// Successful non-memory tool completions this outer stream iteration.
    /// Reset at each `stream_task` loop step; bumped by
    /// [`crate::modules::control_plane::ToolExecutionBroker`] (FEAT-AE-002).
    pub tool_success_evidence: Arc<AtomicU32>,
}

impl SessionExecutionContext {
    /// Build a session execution context from explicit fields.
    #[must_use]
    pub fn new(
        session_id: String,
        project_id: String,
        workdir: PathBuf,
        permission_mode: PermissionMode,
    ) -> Self {
        Self {
            session_id,
            project_id,
            workdir,
            permission_mode,
            tool_success_evidence: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Build a context for non-session direct executions.
    #[must_use]
    pub fn stateless(workdir: PathBuf, permission_mode: PermissionMode) -> Self {
        Self {
            session_id: String::new(),
            project_id: String::new(),
            workdir,
            permission_mode,
            tool_success_evidence: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Count a successful tool toward FEAT-AE-002 `memory_store` evidence.
    /// Skips memory-family tools so `memory_store` cannot self-bootstrap.
    pub fn record_successful_tool_for_memory_gate(&self, tool_name: &str) {
        if tool_name == "pin_memory"
            || tool_name == "unpin_memory"
            || tool_name.starts_with("memory_")
        {
            return;
        }
        self.tool_success_evidence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Resolves session-bound execution context for control-plane flows.
#[derive(Clone)]
pub struct SessionContextResolver {
    session_manager: Arc<SessionManager>,
    project_manager: Arc<ProjectManager>,
}

impl SessionContextResolver {
    /// Create a new resolver.
    #[must_use]
    pub fn new(session_manager: Arc<SessionManager>, project_manager: Arc<ProjectManager>) -> Self {
        Self {
            session_manager,
            project_manager,
        }
    }

    /// Resolve context by session ID.
    ///
    /// # Errors
    ///
    /// Returns an error when session restoration fails.
    pub async fn resolve(
        &self,
        session_id: &str,
        permission_mode: PermissionMode,
        caller: &str,
    ) -> Result<SessionExecutionContext, String> {
        let session = self
            .session_manager
            .restore_session(session_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(self
            .resolve_from_session(&session, permission_mode, caller)
            .await)
    }

    /// Resolve context from an already-loaded session.
    pub async fn resolve_from_session(
        &self,
        app_session: &AppSession,
        permission_mode: PermissionMode,
        caller: &str,
    ) -> SessionExecutionContext {
        let fallback = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let workdir = if app_session.project_id.is_empty() {
            fallback.clone()
        } else {
            match self
                .project_manager
                .get_project(&app_session.project_id)
                .await
            {
                Ok(project) => project.workdir.clone(),
                Err(err) => {
                    tracing::warn!(
                        "[{}] Failed to resolve project '{}' for session '{}': {}. Falling back to current dir '{}'",
                        caller,
                        app_session.project_id,
                        app_session.id,
                        err,
                        fallback.display()
                    );
                    fallback.clone()
                }
            }
        };

        tracing::info!(
            "[{}] Resolved session execution context: session_id='{}', project_id='{}', workdir='{}', permission_mode='{}'",
            caller,
            app_session.id,
            app_session.project_id,
            workdir.display(),
            permission_mode.as_str()
        );

        SessionExecutionContext::new(
            app_session.id.clone(),
            app_session.project_id.clone(),
            workdir,
            permission_mode,
        )
    }
}
