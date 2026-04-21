//! Permission service — bridges sync `PermissionPrompter` trait with
//! async Tauri IPC by emitting `permission-request` events to the
//! frontend and blocking on an mpsc channel for the response.
//!
//! Extracted from `commands/agent.rs` in GFR-003 (pure structural move,
//! function bodies byte-identical).

use tauri::Emitter;

use crate::modules::runtime::permissions::{
    PermissionPrompter, PermissionPromptDecision, PermissionRequest,
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
