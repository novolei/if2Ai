//! Agent Loop Integration
//!
//! Provides helper functions that the agent loop calls at key execution
//! boundaries to emit structured [`AgentEvent`]s onto the [`EventBus`].
//!
//! # `#[allow(dead_code)]` justification
//! All emit helpers are public APIs called from `commands/agent.rs` at key
//! turn boundaries. They are not yet wired into all call sites (only the
//! harness-enabled path uses them), so dead_code lint would fire spuriously.

#![allow(dead_code)]
//!
//! All functions are free-standing (no `self`) and use the `Option<&EventBus>`
//! pattern so that callers can pass `None` in environments where the harness
//! is not configured, avoiding any overhead.
//!
//! # Integration points in `commands/agent.rs`
//!
//! | Agent loop step                  | Function to call                  |
//! |----------------------------------|-----------------------------------|
//! | Before first LLM call            | `emit_turn_started`               |
//! | After all tool calls + response  | `emit_turn_finished`              |
//! | Before each LLM request          | `emit_llm_requested`              |
//! | After each LLM response          | `emit_llm_responded`              |
//! | Before each tool execution       | `emit_tool_called`                |
//! | After each tool execution        | `emit_tool_result`                |
//! | After context compaction         | `emit_context_compacted`          |
//! | On permission prompt             | `emit_permission_prompted`        |
//! | After reflection cycle           | `emit_reflection_completed`       |

use chrono::Utc;
use serde::Serialize;

use super::event_bus::{AgentEvent, EventBus};
use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::runtime_event;

/// DT-01 S1.3 — payload for the canonical
/// `conversation:turn_finished` envelope mirrored alongside the
/// harness `AgentEvent::TurnFinished` bus emit.  Closes audit §2
/// (`docs/superpowers/plans/2026-05-05-dt01-s11-schema-audit.md`).
///
/// Carries the minimum field set the
/// [`crate::modules::harness::runlog_projection::fold_run_log_to_report`]
/// fold needs to derive `task.turn_count`,
/// `task.last_turn_succeeded`, `aggregate.turns_completed`,
/// `aggregate.turns_succeeded`,
/// `aggregate.total_turn_duration_ms`, and the per-turn token
/// totals.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnFinishedPayload {
    /// Monotonically increasing turn number within this session.
    pub turn_number: u64,
    /// Whether the turn produced a successful terminal outcome.
    pub succeeded: bool,
    /// Wall-clock duration of the turn in milliseconds.
    pub duration_ms: u64,
    /// Optional terminal status string (`"completed"`, `"failed"`,
    /// `"cancelled"`, ...) when known to the call site.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_status: Option<String>,
    /// Optional input-token count attributed to this turn.
    /// **Wire name** is `inputUsage` (NOT `tokensIn`) so the
    /// `SENSITIVE_JSON_KEYS` redaction filter (`"token"` substring
    /// rule) does not strip the integer payload before the fold
    /// path reads it back from the run-log JSONL.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "inputUsage"
    )]
    pub tokens_in: Option<u64>,
    /// Optional output-token count attributed to this turn.
    /// See note on `tokens_in` re: wire-name choice.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "outputUsage"
    )]
    pub tokens_out: Option<u64>,
}

/// DT-01 S1.3 — dispatch the canonical
/// `conversation:turn_finished` envelope and (best-effort) mirror
/// it onto the run-log JSONL.
///
/// `app_handle` may be `None` when no Tauri channel is reachable
/// (e.g. inside the non-streaming `run_turn` path).  The
/// `run_event_logger` is what carries the run-log mirror; pass
/// `Some(&logger)` whenever a logger is in scope so the fold path
/// (`fold_run_log_to_report`) can recover the turn-level
/// aggregates.  Failures are logged at TRACE; never propagated.
pub fn dispatch_turn_finished_envelope(
    app_handle: Option<&tauri::AppHandle>,
    run_event_logger: Option<&RunEventLogger>,
    session_id: &str,
    run_id: Option<&str>,
    payload: &TurnFinishedPayload,
) {
    let correlation = CorrelationIds {
        session_id: Some(session_id.to_string()),
        run_id: run_id.map(str::to_string),
        turn_index: u32::try_from(payload.turn_number).ok(),
        ..Default::default()
    };
    if let Err(err) = runtime_event::dispatch(
        app_handle,
        RuntimeEventType::Conversation,
        "turn_finished",
        correlation,
        payload,
        run_event_logger,
    ) {
        tracing::trace!(
            error = ?err,
            "[harness] turn_finished envelope dispatch failed (non-fatal)"
        );
    }
}

/// Emit a [`AgentEvent::TurnStarted`] event.
///
/// Call this at the start of each agent turn before any LLM interaction.
pub fn emit_turn_started(bus: Option<&EventBus>, session_id: &str, turn_number: u64) {
    if let Some(b) = bus {
        let event = AgentEvent::TurnStarted {
            turn_number,
            session_id: session_id.to_string(),
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_turn_started failed: {e}");
        }
    }
}

/// Emit a [`AgentEvent::TurnFinished`] event.
///
/// Call this after the turn completes (success or error).
pub fn emit_turn_finished(
    bus: Option<&EventBus>,
    session_id: &str,
    turn_number: u64,
    success: bool,
    tokens_used: u32,
    duration_ms: u64,
) {
    if let Some(b) = bus {
        let event = AgentEvent::TurnFinished {
            turn_number,
            session_id: session_id.to_string(),
            success,
            tokens_used,
            duration_ms,
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_turn_finished failed: {e}");
        }
    }
}

/// Emit a [`AgentEvent::LlmRequested`] event.
///
/// Call this immediately before sending a request to the LLM provider.
pub fn emit_llm_requested(bus: Option<&EventBus>, session_id: &str, input_tokens: u32) {
    if let Some(b) = bus {
        let event = AgentEvent::LlmRequested {
            session_id: session_id.to_string(),
            input_tokens,
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_llm_requested failed: {e}");
        }
    }
}

/// Emit a [`AgentEvent::LlmResponded`] event.
///
/// Call this after the LLM returns a full response (streaming or non-streaming).
pub fn emit_llm_responded(
    bus: Option<&EventBus>,
    session_id: &str,
    output_tokens: u32,
    streamed: bool,
) {
    if let Some(b) = bus {
        let event = AgentEvent::LlmResponded {
            session_id: session_id.to_string(),
            output_tokens,
            streamed,
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_llm_responded failed: {e}");
        }
    }
}

/// Emit a [`AgentEvent::ToolCalled`] event.
///
/// `args_snippet` is a short preview of the tool arguments (truncated to 512
/// chars by the caller to avoid emitting secrets or large blobs).
pub fn emit_tool_called(
    bus: Option<&EventBus>,
    session_id: &str,
    tool_name: &str,
    args_snippet: &str,
) {
    if let Some(b) = bus {
        let event = AgentEvent::ToolCalled {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            args_snippet: args_snippet.chars().take(512).collect(),
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_tool_called failed: {e}");
        }
    }
}

/// Emit a [`AgentEvent::ToolResult`] event.
///
/// Call this after the tool execution completes, regardless of outcome.
pub fn emit_tool_result(
    bus: Option<&EventBus>,
    session_id: &str,
    tool_name: &str,
    success: bool,
    duration_ms: u64,
) {
    if let Some(b) = bus {
        let event = AgentEvent::ToolResult {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            success,
            duration_ms,
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_tool_result failed: {e}");
        }
    }
}

/// Emit a [`AgentEvent::ContextCompacted`] event.
///
/// Call this after context compaction completes with the message counts
/// before and after.
pub fn emit_context_compacted(
    bus: Option<&EventBus>,
    session_id: &str,
    messages_before: usize,
    messages_after: usize,
) {
    if let Some(b) = bus {
        let event = AgentEvent::ContextCompacted {
            session_id: session_id.to_string(),
            messages_before,
            messages_after,
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_context_compacted failed: {e}");
        }
    }
}

/// Emit a [`AgentEvent::PermissionPrompted`] event.
pub fn emit_permission_prompted(bus: Option<&EventBus>, session_id: &str, tool_name: &str) {
    if let Some(b) = bus {
        let event = AgentEvent::PermissionPrompted {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_permission_prompted failed: {e}");
        }
    }
}

/// Emit a [`AgentEvent::ReflectionCompleted`] event.
pub fn emit_reflection_completed(bus: Option<&EventBus>, session_id: &str, insights_count: usize) {
    if let Some(b) = bus {
        let event = AgentEvent::ReflectionCompleted {
            session_id: session_id.to_string(),
            insights_count,
            at: Utc::now(),
        };
        if let Err(e) = b.emit(event) {
            tracing::debug!("[harness] emit_reflection_completed failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::event_bus::EventBus;

    #[tokio::test]
    async fn emit_with_none_bus_is_noop() {
        // Must not panic.
        emit_turn_started(None, "s1", 1);
        emit_turn_finished(None, "s1", 1, true, 0, 0);
        emit_llm_requested(None, "s1", 100);
        emit_llm_responded(None, "s1", 50, false);
        emit_tool_called(None, "s1", "bash", "echo hi");
        emit_tool_result(None, "s1", "bash", true, 10);
        emit_context_compacted(None, "s1", 20, 5);
        emit_permission_prompted(None, "s1", "bash");
        emit_reflection_completed(None, "s1", 3);
    }

    #[tokio::test]
    async fn emit_tool_called_truncates_long_args() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        let long_args = "x".repeat(1024);
        emit_tool_called(Some(&bus), "sess", "my_tool", &long_args);

        if let Ok(AgentEvent::ToolCalled { args_snippet, .. }) = rx.try_recv() {
            assert!(args_snippet.len() <= 512);
        } else {
            panic!("expected ToolCalled event");
        }
    }
}
