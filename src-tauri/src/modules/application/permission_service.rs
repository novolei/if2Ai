//! Permission service — bridges sync `PermissionPrompter` trait with
//! async Tauri IPC by emitting `permission-request` events to the
//! frontend and blocking on an mpsc channel for the response.
//!
//! Extracted from `commands/agent.rs` in GFR-003 (pure structural move,
//! function bodies byte-identical). The
//! [`respond_to_permission_prompt`] helper at the bottom is the
//! application-layer body for the `respond_permission` IPC
//! command — moved out of `commands/agent.rs` as the MIG-001
//! follow-up cleanup.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::Emitter;

use crate::modules::harness::{AgentEvent, HarnessState};
use crate::modules::runtime::event_log::RunEventLogger;
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
    event_logger: Option<RunEventLogger>,
}

#[allow(dead_code)]
impl TauriPermissionPrompter {
    /// Create a new TauriPermissionPrompter.
    pub(crate) fn new(
        window: tauri::WebviewWindow,
        session_id: String,
        receiver: std::sync::mpsc::Receiver<PermissionPromptDecision>,
        event_logger: Option<RunEventLogger>,
    ) -> Self {
        Self {
            window,
            session_id,
            receiver,
            event_logger,
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
        if let Some(event_logger) = &self.event_logger {
            let _ = event_logger.append_sync(
                "permission_requested",
                serde_json::json!({
                    "tool_name": request.tool_name,
                    "required_mode": request.required_mode.as_str(),
                    "current_mode": request.current_mode.as_str(),
                }),
            );
        }

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
                if let Some(event_logger) = &self.event_logger {
                    let (decision_label, deny_reason) = match &decision {
                        PermissionPromptDecision::Allow => ("allow", None),
                        PermissionPromptDecision::Deny { reason } => {
                            ("deny", Some(reason.as_str()))
                        }
                    };
                    let _ = event_logger.append_sync(
                        "permission_resolved",
                        serde_json::json!({
                            "tool_name": request.tool_name,
                            "decision": decision_label,
                            "reason": deny_reason,
                        }),
                    );
                }
                decision
            }
            Err(_) => {
                if let Some(event_logger) = &self.event_logger {
                    let _ = event_logger.append_sync(
                        "permission_resolved",
                        serde_json::json!({
                            "tool_name": request.tool_name,
                            "decision": "deny",
                            "reason": "timeout",
                        }),
                    );
                }
                PermissionPromptDecision::Deny {
                    reason: "Permission request timed out".to_string(),
                }
            }
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

/// Application-layer body for the `respond_permission` IPC
/// command.
///
/// Decodes the frontend `decision` string into a
/// [`PermissionPromptDecision`], looks up the pending mpsc sender
/// for `session_id`, forwards the decision, optionally records a
/// session-scoped "remember this tool" override, and emits a
/// harness `PermissionResolved` event when the harness is wired.
///
/// Extracted from `commands::agent::respond_permission` so the IPC
/// command stays a thin adapter (CHARTER §2.1: command layer
/// owns IPC contract, application layer owns business logic).
pub(crate) fn respond_to_permission_prompt(
    permission_senders: &Mutex<HashMap<String, std::sync::mpsc::Sender<PermissionPromptDecision>>>,
    permission_overrides: &Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>,
    harness: Option<&Arc<HarnessState>>,
    session_id: String,
    decision: String,
    tool_name: Option<String>,
    scope: Option<String>,
) -> Result<(), String> {
    tracing::info!(
        "[permission] respond session_id={}, decision={}, scope={}",
        session_id,
        decision,
        scope.as_deref().unwrap_or("once")
    );
    let decision_enum = match decision.as_str() {
        "allow" => PermissionPromptDecision::Allow,
        _ => PermissionPromptDecision::Deny {
            reason: "User denied permission".to_string(),
        },
    };

    let senders = permission_senders
        .lock()
        .map_err(|e| format!("Failed to lock permission senders: {e}"))?;

    let sender = senders
        .get(&session_id)
        .ok_or_else(|| "No pending permission request for this session".to_string())?;

    sender
        .send(decision_enum)
        .map_err(|_| "Failed to send permission decision".to_string())?;

    // Phase M4-C P4 — emit harness `PermissionResolved` event so the
    // trace aggregator pairs this resolution with the earlier
    // `PermissionPrompted` event. Zero-cost no-op when harness is
    // not initialised.
    if let Some(harness) = harness {
        let _ = harness.event_bus.emit(AgentEvent::PermissionResolved {
            session_id: session_id.clone(),
            tool_name: tool_name.clone(),
            decision: decision.clone(),
            scope: scope.clone().unwrap_or_else(|| "once".to_string()),
            at: chrono::Utc::now(),
        });
    }

    // Optional session-scoped remember decision.
    if scope.as_deref() == Some("session") {
        // Store by latest requested tool in this session if known.
        // We cannot extract tool_name from the mpsc payload here,
        // so we keep a coarse fallback decision bucket under "*"
        // to be read by the prompter side.
        let mut overrides = permission_overrides
            .lock()
            .map_err(|e| format!("Failed to lock permission overrides: {e}"))?;
        let session_map = overrides.entry(session_id).or_default();
        let key = tool_name.unwrap_or_else(|| "*".to_string());
        session_map.insert(
            key,
            match decision.as_str() {
                "allow" => PermissionPromptDecision::Allow,
                _ => PermissionPromptDecision::Deny {
                    reason: "User denied permission (session policy)".to_string(),
                },
            },
        );
    }

    Ok(())
}
