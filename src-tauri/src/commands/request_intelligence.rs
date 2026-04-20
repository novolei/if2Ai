//! Request intelligence Tauri commands (Phase M2.6).
//!
//! Exposes the deterministic + heuristic classifier in
//! [`crate::modules::application::request_intelligence_service`]
//! as an IPC command so the frontend can ask "what mode would you
//! classify this draft as" without having to wait for an actual
//! `start_agent_stream` call.
//!
//! Honest scope (mirrors the M1.6 backend reality):
//!
//! - The classifier is **deterministic** + heuristic — no LLM
//!   fallback, no per-policy registry. The decision is the same
//!   value `TurnService::prepare_chat_inputs` would produce for the
//!   same input.
//! - The classifier is **advisory** today: `commands/agent.rs` only
//!   logs the decision, the existing single execution path keeps
//!   running. The IPC command surfaces the decision so M2.6 frontend
//!   can render an honest "current judgment" chip — never an "auto
//!   route" UX.
//! - Future m2.7+ slices may grow this command into a Tauri **event**
//!   (`execution_mode_decision_emitted`) tied to actual stream emit;
//!   the IPC contract here will continue to work as a manual fetch
//!   seam.

use std::path::PathBuf;

use crate::modules::application::request_intelligence_service::{
    classify, RequestIntelligenceInput,
};
use crate::modules::runtime::contracts::execution_mode::ExecutionModeDecision;

/// Wire-shape input for the IPC command. Mirrors the
/// [`RequestIntelligenceInput`] but accepts `String` (vs `&str`)
/// because Tauri commands marshal owned values across the IPC
/// boundary.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestIntelligenceClassifyInput {
    pub user_message: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    /// Working directory string (frontend has no `PathBuf` notion).
    #[serde(default)]
    pub workdir: Option<String>,
}

/// Run the deterministic + heuristic classifier and return the
/// canonical [`ExecutionModeDecision`].
///
/// The decision struct serialises as camelCase (per the M0.5
/// contract `#[serde(rename_all = "camelCase")]`), so the wire
/// payload matches the TypeScript `ExecutionModeDecision` interface
/// in [`src/transport/contracts.ts`](../../../src/transport/contracts.ts)
/// without any adapter struct.
#[tauri::command]
pub async fn request_intelligence_classify(
    input: RequestIntelligenceClassifyInput,
) -> Result<ExecutionModeDecision, String> {
    let workdir = input.workdir.map(PathBuf::from);
    let out = classify(RequestIntelligenceInput {
        user_message: input.user_message,
        session_id: input.session_id,
        project_id: input.project_id,
        workdir,
    });
    Ok(out.decision)
}
