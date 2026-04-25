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
use super::contracts::common::{CorrelationIds, RuntimeEventEnvelope, RuntimeEventType};
use super::contracts::memory::MemoryItemProjection;
use super::contracts::prompt::PromptDiagnosticsSummary;
use super::recoverability::ResumeRecoverability;

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

/// Per-turn provider-billable token usage and USD cost. Emitted on
/// `stream_complete` so the chat UI can render a Steward-style
/// `[Zap] 12,040 输入 · 62 输出 · $0.0307` chip beneath the assistant
/// reply. `cost_usd` uses the price table in
/// [`crate::modules::runtime::usage::pricing_for_model`].
#[derive(Serialize, Clone, Debug, Default)]
pub struct TurnCostPayload {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
    pub cost_usd: f64,
    pub model: String,
}

/// Smart-routing decision summary attached to `stream_complete` so the
/// chat UI can render a per-message routing chip (P1-8). `used_cheap_model`
/// is `true` when `apply_complexity_model_routing` swapped to the cheap
/// model id; `effective_model` is the model that actually answered.
#[derive(Serialize, Clone, Debug, Default)]
pub struct RoutingInfoPayload {
    pub complexity_score: f32,
    pub complexity_level: String,
    pub execution_mode: String,
    pub used_cheap_model: bool,
    pub effective_model: String,
}

/// Per-session running totals after a successful `stream_complete`.
/// Lets the ContextBar render「本会话累计」 without a second IPC.
/// Mirrors the persisted `SessionUsageTotals` on `Session`.
#[derive(Serialize, Clone, Debug, Default)]
pub struct SessionUsageTotalsPayload {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cost_usd: f64,
    pub turns: u32,
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
    /// Cross-cutting correlation ids carried alongside the payload.
    ///
    /// Populated by the emitter so downstream consumers (event log,
    /// frontend translator) can correlate events without scraping the
    /// payload body. When `None`, the event is pre-correlation
    /// (legacy path); M2+ will always set this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation: Option<CorrelationIds>,
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
    /// Structured recoverability payload (MIG-021).
    ///
    /// Populated on `stream_complete` / `stream_error` so the
    /// frontend can render typed resume CTAs without parsing
    /// free-form `degraded_reason` strings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoverability: Option<ResumeRecoverability>,
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
    /// Prompt diagnostics summary for this completed turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_diagnostics: Option<PromptDiagnosticsSummary>,
    /// Per-turn token + cost (only on `stream_complete`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_cost: Option<TurnCostPayload>,
    /// Smart-routing decision (P1-8, only on `stream_complete`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_info: Option<RoutingInfoPayload>,
    /// Session running totals after this turn (only on `stream_complete`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_totals: Option<SessionUsageTotalsPayload>,
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
            correlation: None,
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
            recoverability: None,
            context_budget_usage: None,
            memory_context: None,
            prompt_diagnostics: None,
            turn_cost: None,
            routing_info: None,
            session_totals: None,
        }
    }

    /// Convert this wire payload into a canonical
    /// [`RuntimeEventEnvelope`].
    ///
    /// The conversion is lossless: every field on
    /// [`StreamTokenPayload`] is preserved in the envelope's
    /// `payload` JSON, and correlation ids are promoted into the
    /// envelope's `correlation` field. When `self.correlation` is
    /// `None`, a minimal correlation is derived from `stream_id`
    /// (mapped to `stream_id` in [`CorrelationIds`]) for backwards
    /// compatibility.
    ///
    /// Returns `None` when the `event_type` string does not map to
    /// any [`RuntimeEventType`] variant. This should never happen in
    /// production; callers should log a warning on `None`.
    pub fn to_envelope(&self) -> Option<RuntimeEventEnvelope> {
        let event_type = map_event_type_to_runtime(&self.event_type)?;
        let correlation = self.correlation.clone().unwrap_or_else(|| CorrelationIds {
            stream_id: Some(self.stream_id.clone()),
            ..CorrelationIds::default()
        });
        let payload = serde_json::to_value(self).unwrap_or_else(
            |error| serde_json::json!({ "serialization_error": error.to_string() }),
        );

        Some(RuntimeEventEnvelope::new(
            event_type,
            &self.event_type,
            correlation,
            payload,
        ))
    }
}

/// Map a [`StreamTokenPayload`] `event_type` string to a canonical
/// [`RuntimeEventType`] variant.
///
/// Every `event_type` value emitted by the agent loop must appear
/// here. If a new event type is added upstream without updating this
/// function, the `to_envelope` conversion will return `None` and
/// callers should treat it as a contract drift failure.
fn map_event_type_to_runtime(event_type: &str) -> Option<RuntimeEventType> {
    match event_type {
        // Conversation family
        "text_delta"
        | "thinking_delta"
        | "thinking_start"
        | "final_text_override"
        | "stream_complete"
        | "stream_error"
        | "run_started" => Some(RuntimeEventType::Conversation),
        // Tool family
        "tool_call_update" => Some(RuntimeEventType::Tool),
        // Permission family (placeholder — M1.7 will add concrete events)
        "permission_request" | "permission_decision" => Some(RuntimeEventType::Permission),
        // Memory family
        "memory_write_decision" | "memory_after_turn" => Some(RuntimeEventType::Memory),
        // Activation family
        "activation_status_changed" => Some(RuntimeEventType::Activation),
        // Execution-mode family
        "execution_mode_decision" => Some(RuntimeEventType::ExecutionMode),
        // Harness family
        "harness_recording" | "harness_eval" => Some(RuntimeEventType::Harness),
        // System family
        "boot_phase_changed" | "session_opened" | "session_closed" => {
            Some(RuntimeEventType::System)
        }
        _ => None,
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
        assert!(p.prompt_diagnostics.is_none());
        assert!(p.correlation.is_none());
    }

    #[test]
    fn stream_payload_maps_to_runtime_envelope() {
        // Every known event_type must map to a RuntimeEventType variant
        let known_types = [
            "text_delta",
            "thinking_delta",
            "thinking_start",
            "final_text_override",
            "stream_complete",
            "stream_error",
            "run_started",
            "tool_call_update",
        ];
        for et in &known_types {
            let p = StreamTokenPayload::skeleton("s1", *et);
            let envelope = p.to_envelope().unwrap_or_else(|| {
                panic!("event_type '{et}' should map to a RuntimeEventType variant")
            });
            assert_eq!(envelope.payload_family.0, *et);
            // Without explicit correlation, stream_id is promoted
            assert_eq!(envelope.correlation.stream_id.as_deref(), Some("s1"));
        }

        // Unknown event_type returns None
        let unknown = StreamTokenPayload::skeleton("s1", "unknown_event_xyz");
        assert!(unknown.to_envelope().is_none());
    }

    #[test]
    fn stream_payload_to_envelope_preserves_correlation() {
        let correlation = CorrelationIds {
            session_id: Some("sess-1".into()),
            run_id: Some("run-1".into()),
            stream_id: Some("s1".into()),
            project_id: None,
            turn_index: Some(3),
            attempt_id: Some("att-7".into()),
        };
        let mut p = StreamTokenPayload::skeleton("s1", "text_delta");
        p.text = Some("hello".into());
        p.correlation = Some(correlation.clone());

        let envelope = p.to_envelope().expect("text_delta should map");
        assert_eq!(envelope.correlation.run_id.as_deref(), Some("run-1"));
        assert_eq!(envelope.correlation.session_id.as_deref(), Some("sess-1"));
        assert_eq!(envelope.correlation.attempt_id.as_deref(), Some("att-7"));
        assert_eq!(envelope.correlation.turn_index, Some(3));
        assert_eq!(envelope.event_type, RuntimeEventType::Conversation);
    }
}
