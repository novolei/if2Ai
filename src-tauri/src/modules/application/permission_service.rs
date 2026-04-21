//! Permission service — bridges sync `PermissionPrompter` trait with
//! async Tauri IPC by emitting `permission-request` events to the
//! frontend and blocking on an mpsc channel for the response.
//!
//! Extracted from `commands/agent.rs` in GFR-003 (pure structural move,
//! function bodies byte-identical).

use tauri::Emitter;

use crate::modules::runtime::permissions::{
    PermissionMode, PermissionPolicy, PermissionPromptDecision, PermissionPrompter,
    PermissionRequest,
};

/// TauriPermissionPrompter — bridges the sync PermissionPrompter trait
/// with async Tauri IPC. Emits a `permission-request` event to the
/// frontend and blocks on an mpsc channel until the user responds.
///
/// Usage: register `respond_permission` on the frontend side and have it
/// invoke with `{ sessionId, decision: "allow" | "deny", scope?: "once" | "session" }`.
pub(crate) struct TauriPermissionPrompter {
    window: tauri::WebviewWindow,
    session_id: String,
    receiver: std::sync::mpsc::Receiver<PermissionPromptDecision>,
}

#[allow(dead_code)]
impl TauriPermissionPrompter {
    /// Create a new TauriPermissionPrompter.
    pub(crate) fn new(
        window: tauri::WebviewWindow,
        session_id: String,
        receiver: std::sync::mpsc::Receiver<PermissionPromptDecision>,
    ) -> Self {
        Self {
            window,
            session_id,
            receiver,
        }
    }
}

impl PermissionPrompter for TauriPermissionPrompter {
    fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision {
        tracing::info!(
            "[permission] request session_id={}, tool={}, current={}, required={}",
            self.session_id,
            request.tool_name,
            request.current_mode.as_str(),
            request.required_mode.as_str()
        );
        // 1. emit confirmation event to the frontend
        let _ = self.window.emit(
            "permission-request",
            serde_json::json!({
                "session_id": self.session_id,
                "tool_name": request.tool_name,
                "permission_mode": request.required_mode.as_str(),
                "current_mode": request.current_mode.as_str(),
                "message": format!(
                    "Tool '{}' requires {} permission (current: {})",
                    request.tool_name,
                    request.required_mode.as_str(),
                    request.current_mode.as_str()
                ),
            }),
        );

        // 2. block waiting for frontend response (mpsc blocks — acceptable in sync context)
        match self
            .receiver
            .recv_timeout(std::time::Duration::from_secs(60))
        {
            Ok(decision) => {
                tracing::info!(
                    "[permission] decision received for session_id={}",
                    self.session_id
                );
                decision
            }
            Err(_) => PermissionPromptDecision::Deny {
                reason: "Permission request timed out".to_string(),
            },
        }
    }
}

/// Parse a permission_mode string into PermissionMode enum.
pub(crate) fn parse_permission_mode(mode: Option<&str>) -> PermissionMode {
    match mode {
        Some("readOnly") | Some("read-only") | Some("read_only") => PermissionMode::ReadOnly,
        Some("workspaceWrite") | Some("workspace-write") | Some("workspace_write") => {
            PermissionMode::WorkspaceWrite
        }
        Some("prompt") => PermissionMode::Prompt,
        Some("dangerFullAccess")
        | Some("danger-full-access")
        | Some("danger_full_access")
        | None => PermissionMode::DangerFullAccess,
        _ => PermissionMode::DangerFullAccess,
    }
}

/// Build tool-level permission policy for the active mode.
///
/// Read-only tools are allowed in all modes.
/// Workspace-write tools require at least WorkspaceWrite.
/// Dangerous/system tools require DangerFullAccess (or prompt escalation).
pub(crate) fn build_permission_policy(mode: PermissionMode) -> PermissionPolicy {
    PermissionPolicy::new(mode)
        // Read-only tools
        .with_tool_requirement("read_file", PermissionMode::ReadOnly)
        .with_tool_requirement("glob_search", PermissionMode::ReadOnly)
        .with_tool_requirement("grep_search", PermissionMode::ReadOnly)
        .with_tool_requirement("content_search", PermissionMode::ReadOnly)
        .with_tool_requirement("web_fetch", PermissionMode::ReadOnly)
        .with_tool_requirement("web_search", PermissionMode::ReadOnly)
        .with_tool_requirement("WebFetch", PermissionMode::ReadOnly)
        .with_tool_requirement("WebSearch", PermissionMode::ReadOnly)
        .with_tool_requirement("tool_search", PermissionMode::ReadOnly)
        .with_tool_requirement("ToolSearch", PermissionMode::ReadOnly)
        .with_tool_requirement("json_parse", PermissionMode::ReadOnly)
        .with_tool_requirement("skill", PermissionMode::ReadOnly)
        .with_tool_requirement("skill_search", PermissionMode::ReadOnly)
        .with_tool_requirement("skill_find", PermissionMode::ReadOnly)
        .with_tool_requirement("skill_view", PermissionMode::ReadOnly)
        .with_tool_requirement("skills_categories", PermissionMode::ReadOnly)
        .with_tool_requirement("skill_manage", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("memory_recall", PermissionMode::ReadOnly)
        .with_tool_requirement("memory_export", PermissionMode::ReadOnly)
        .with_tool_requirement("cron_list", PermissionMode::ReadOnly)
        .with_tool_requirement("sleep", PermissionMode::ReadOnly)
        .with_tool_requirement("Sleep", PermissionMode::ReadOnly)
        .with_tool_requirement("SendUserMessage", PermissionMode::ReadOnly)
        .with_tool_requirement("structured_output", PermissionMode::ReadOnly)
        .with_tool_requirement("StructuredOutput", PermissionMode::ReadOnly)
        // Workspace-write tools
        .with_tool_requirement("file_write", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("write_file", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("file_edit", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("edit_file", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("NotebookEdit", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("memory_store", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("memory_forget", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("memory_purge", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("TodoWrite", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("Config", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("cron_add", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("cron_remove", PermissionMode::WorkspaceWrite)
        .with_tool_requirement("cron_run", PermissionMode::WorkspaceWrite)
        // Dangerous/system tools
        .with_tool_requirement("bash", PermissionMode::DangerFullAccess)
        .with_tool_requirement("PowerShell", PermissionMode::DangerFullAccess)
        .with_tool_requirement("REPL", PermissionMode::DangerFullAccess)
        .with_tool_requirement("http_request", PermissionMode::DangerFullAccess)
        .with_tool_requirement("agent", PermissionMode::DangerFullAccess)
}
