//! Agent stream emitter boundary (Phase M1.5).
//!
//! Owns the single point at which agent-loop runtime events leave
//! the backend and reach the frontend over Tauri's IPC. The
//! emitter:
//!
//! - Centralises the canonical event names (`agent-token` for
//!   per-token / per-tool / per-completion updates).
//! - Centralises the wire payload shape ([`StreamTokenPayload`])
//!   so M2 frontend translator has exactly **one** struct to
//!   register against, instead of having to scrape ~13 ad-hoc
//!   `window.emit("agent-token", ...)` call sites in `commands/agent.rs`.
//! - Provides typed convenience methods for the simple emission
//!   shapes (`text_delta` / `thinking_delta` / `thinking_start`)
//!   while still letting complex sites build a full
//!   [`StreamTokenPayload`] and pass it through
//!   [`AgentStreamEmitter::emit_payload`].
//!
//! Out of scope for this skeleton (deferred to later slices):
//!
//! - **Permission prompt event (`permission-request`)** stays in
//!   [`crate::commands::agent::TauriPermissionPrompter`] for now —
//!   M1.7 activation / permission service work touches it.
//! - **Memory event (`memory_event`)** has its own canonical
//!   audit-emitter path
//!   ([`crate::modules::memory::audit::register_app_handle`]),
//!   not part of this M1.5 boundary.
//! - **Canonical [`crate::modules::runtime::contracts::RuntimeEventEnvelope`]
//!   migration**: M0.3 already defined the envelope, but actually
//!   wrapping every emission in it without breaking the frontend
//!   `listenToStream` consumer is a multi-PR effort. M1.5 keeps the
//!   legacy `agent-token` + [`StreamTokenPayload`] wire shape and
//!   defers the envelope migration to M2 once the frontend
//!   translator exists.
//!
//! Hard rules:
//! 1. This module MUST NOT import from `crate::commands::*`.
//! 2. The wire shape of [`StreamTokenPayload`] MUST NOT change in
//!    M1.5 — frontend `listenToStream` depends on every field
//!    name. Any change here is a contract break.
//! 3. The canonical event name [`AGENT_TOKEN_EVENT`] MUST be the
//!    only string in this crate that emits to that channel. Any new
//!    runtime event name belongs in this module too.

// Typed factories / convenience methods below are part of the M1.5
// public surface and will be consumed by later slices (M1.6
// request-intelligence event emission, M2 frontend translator
// reverse engineering). Suppress unused warnings until then.
#![allow(dead_code)]

use serde::Serialize;
use tauri::{Emitter, WebviewWindow};

// Phase M1.6 — `MemoryItemProjection` lives in the runtime contract
// (`super::contracts::memory`) so this module no longer depends on
// the `application` layer.  Reverse `runtime -> application`
// dependencies are forbidden.
use super::contracts::memory::MemoryItemProjection;

/// Canonical Tauri event name used by the agent-loop streaming
/// path. Held as a `pub const` so the M2 frontend translator can
/// `import { AGENT_TOKEN_EVENT } from ...` once we wire the TS
/// twin (in `src/transport/contracts.ts` / a future
/// `src/transport/events.ts`).
pub const AGENT_TOKEN_EVENT: &str = "agent-token";

/// Phase M3-C closeout — canonical Tauri event name for the
/// **batch envelope** emitted by
/// [`crate::modules::application::memory_coordinator::MemoryCoordinator::after_turn`]
/// at every turn end.
///
/// Replaces the short-lived M3-B `memory_write_decision` event.
/// Fires **once per turn** regardless of how many candidates the
/// coordinator produced — even an empty batch is a real event
/// (`after_turn ran but no candidates were extracted`), so the
/// "empty batch is unobservable" gap from the M3-B audit is
/// closed.
///
/// Payload shape (JSON, camelCase):
///
/// ```json
/// {
///   "caller": "run_agent_turn",
///   "policyVersion": "memory-write-policy@m3.3-skeleton",
///   "decidedAt": "2026-04-20T...",
///   "decisions":  [/* MemoryWriteDecision */],
///   "quality":    { "accepted":[…], "rejected":[…], "warnings":[…],
///                   "policyVersion":"…" },
///   "conflicts":  [/* ConflictResolution */]
/// }
/// ```
///
/// Frontend bridge fans this out into a single batch canonical
/// event (`memory_after_turn`) plus N per-decision canonical
/// events (`memory_write_decision`).
pub const MEMORY_AFTER_TURN_EVENT: &str = "memory_after_turn";

/// Token-budget breakdown emitted alongside the final
/// `stream_complete` event so the frontend `ContextBar` can render
/// usage without an extra IPC round-trip.
///
/// Mirrors the TypeScript `ContextBudgetUsage` interface in
/// [`src/lib/tauri.ts`](../../../../../src/lib/tauri.ts).
#[derive(Serialize, Clone, Debug, Default)]
pub struct ContextBudgetUsagePayload {
    pub total_budget: usize,
    pub system_tokens: usize,
    pub history_tokens: usize,
    pub memory_tokens: usize,
    pub output_reserve: usize,
    pub remaining: usize,
}

/// Canonical wire payload for every `agent-token` emission.
///
/// One struct holds every field every event variant might want to
/// fill — frontends route on `event_type`. Keep field names
/// `snake_case` (matches the TS interface in `src/lib/tauri.ts`).
///
/// Constructable via [`StreamTokenPayload::skeleton`] which
/// pre-fills `stream_id` and zeroes everything else; per-event
/// constructors live as factory functions on this module
/// ([`text_delta`], [`thinking_delta`], [`thinking_start`]) for the
/// simple shapes.
#[derive(Serialize, Clone, Debug)]
pub struct StreamTokenPayload {
    pub stream_id: String,
    pub text: Option<String>,
    pub thinking: Option<String>,
    pub event_type: String,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    /// `"queued" | "running" | "completed" | "error"`.
    pub tool_status: Option<String>,
    pub tool_args: Option<serde_json::Value>,
    pub tool_result: Option<String>,
    pub tool_duration_ms: Option<u64>,
    pub effective_workdir: Option<String>,
    pub policy_decision: Option<String>,
    pub evidence_id: Option<String>,
    pub request_id: Option<String>,
    pub task_outcome: Option<String>,
    pub degraded_reason: Option<String>,
    pub resume_available: Option<bool>,
    pub resume_cursor: Option<String>,
    /// Token-budget breakdown for this turn — populated on
    /// `stream_complete` so the frontend `ContextBar` can render
    /// usage live. Never present on per-token `text_delta` events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_budget_usage: Option<ContextBudgetUsagePayload>,
    /// Memory items recalled for this turn — populated on
    /// `stream_complete` alongside `context_budget_usage`. Drives
    /// the `MemoryChip` / `MemoryEvidencePanel` UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_context: Option<Vec<MemoryItemProjection>>,
}

impl StreamTokenPayload {
    /// Build a payload with `stream_id` set and everything else
    /// `None`. Useful for complex emit sites that fill in many
    /// optional fields; simple sites should prefer the per-event
    /// factories ([`text_delta`] etc.).
    #[must_use]
    pub fn skeleton(stream_id: impl Into<String>, event_type: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            text: None,
            thinking: None,
            event_type: event_type.into(),
            tool_call_id: None,
            tool_name: None,
            tool_status: None,
            tool_args: None,
            tool_result: None,
            tool_duration_ms: None,
            effective_workdir: None,
            policy_decision: None,
            evidence_id: None,
            request_id: None,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            context_budget_usage: None,
            memory_context: None,
        }
    }
}

/// Per-event factory: `text_delta`.
#[must_use]
pub fn text_delta(stream_id: impl Into<String>, text: String) -> StreamTokenPayload {
    let mut p = StreamTokenPayload::skeleton(stream_id, "text_delta");
    p.text = Some(text);
    p
}

/// Per-event factory: `thinking_delta`.
#[must_use]
pub fn thinking_delta(stream_id: impl Into<String>, thinking: String) -> StreamTokenPayload {
    let mut p = StreamTokenPayload::skeleton(stream_id, "thinking_delta");
    p.thinking = Some(thinking);
    p
}

/// Per-event factory: `thinking_start` (a content-less marker).
#[must_use]
pub fn thinking_start(stream_id: impl Into<String>) -> StreamTokenPayload {
    StreamTokenPayload::skeleton(stream_id, "thinking_start")
}

/// Single boundary at which agent-loop events leave the backend.
///
/// Held by value (cheap — wraps a `WebviewWindow` clone) so the
/// streaming task can move it across an `async move` boundary
/// without re-cloning the window at every emit site.
#[derive(Clone)]
pub struct AgentStreamEmitter {
    window: WebviewWindow,
}

impl AgentStreamEmitter {
    /// Construct an emitter over the given window.
    #[must_use]
    pub fn new(window: WebviewWindow) -> Self {
        Self { window }
    }

    /// Borrow the underlying window. Call sites that need to emit a
    /// custom event (other than `agent-token`) can use this escape
    /// hatch, but the M1.5 boundary contract is that everything
    /// flowing into the streaming projection goes through this
    /// emitter.
    #[must_use]
    pub fn window(&self) -> &WebviewWindow {
        &self.window
    }

    /// Low-level emission of any [`StreamTokenPayload`].
    ///
    /// Per-site convenience methods (e.g. [`AgentStreamEmitter::emit_text_delta`])
    /// build a payload through the module-level factories and call
    /// this. Errors from the underlying `Emitter` impl are logged
    /// at TRACE — emission failure is non-fatal for the agent
    /// loop, matching the legacy `let _ = window.emit(...)` shape.
    pub fn emit_payload(&self, payload: StreamTokenPayload) {
        self.emit_event(AGENT_TOKEN_EVENT, payload);
    }

    /// Emit a `text_delta` event.
    pub fn emit_text_delta(&self, stream_id: impl Into<String>, text: String) {
        self.emit_payload(text_delta(stream_id, text));
    }

    /// Emit a `thinking_delta` event.
    pub fn emit_thinking_delta(&self, stream_id: impl Into<String>, thinking: String) {
        self.emit_payload(thinking_delta(stream_id, thinking));
    }

    /// Emit a `thinking_start` marker.
    pub fn emit_thinking_start(&self, stream_id: impl Into<String>) {
        self.emit_payload(thinking_start(stream_id));
    }

    /// Generic emission for non-`agent-token` events. Used by the
    /// permission prompt path during the M1 transition until M1.7
    /// migrates it to a dedicated boundary.
    pub fn emit_event<T: Serialize + Clone>(&self, event_name: &str, payload: T) {
        if let Err(e) = self.window.emit(event_name, payload) {
            tracing::trace!(
                event = event_name,
                error = %e,
                "[stream_emitter] window.emit failed (non-fatal)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_delta_factory_sets_event_type_and_text() {
        let p = text_delta("s1", "hello".to_string());
        assert_eq!(p.stream_id, "s1");
        assert_eq!(p.event_type, "text_delta");
        assert_eq!(p.text.as_deref(), Some("hello"));
        assert!(p.thinking.is_none());
        assert!(p.tool_call_id.is_none());
    }

    #[test]
    fn thinking_delta_factory_sets_event_type_and_thinking() {
        let p = thinking_delta("s2", "ponder".to_string());
        assert_eq!(p.event_type, "thinking_delta");
        assert_eq!(p.thinking.as_deref(), Some("ponder"));
        assert!(p.text.is_none());
    }

    #[test]
    fn thinking_start_factory_is_marker_only() {
        let p = thinking_start("s3");
        assert_eq!(p.event_type, "thinking_start");
        assert!(p.thinking.is_none());
        assert!(p.text.is_none());
    }

    #[test]
    fn skeleton_pre_fills_stream_id_and_event_type() {
        let p = StreamTokenPayload::skeleton("s4", "stream_complete");
        assert_eq!(p.stream_id, "s4");
        assert_eq!(p.event_type, "stream_complete");
        assert!(p.task_outcome.is_none());
        assert!(p.memory_context.is_none());
    }
}
