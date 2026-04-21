//! Agent Loop Event Bus
//!
//! A broadcast channel through which the agent loop emits structured events.
//! Subscribers (TelemetryCollector, SessionRecorder) receive all events without
//! coupling to agent internals.
//!
//! # `#[allow(dead_code)]` justification
//! `emit`, `receiver_count`, and `session_id()` are the public API surface used
//! by agent loop integration helpers and IPC commands. They are not yet called
//! from all code paths in production (only the harness-enabled path).

#![allow(dead_code)]
//!
//! # Design
//! Uses `tokio::sync::broadcast` (bounded, capacity 256) so that slow subscribers
//! do not block the agent loop. A lagged subscriber simply misses old events.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::modules::application::{ConflictResolution, QualityGateResult};
use crate::modules::control_plane::prepare_step_execution::{
    BoundaryDecision, PermissionDecision, PrepareStepOutcome, SandboxPolicy,
};
use crate::modules::runtime::contracts::memory::MemoryWriteDecision;

/// Capacity of the broadcast channel.
/// Chosen to be large enough that a slow subscriber does not lose events in
/// typical conversation turns, but small enough to bound memory usage.
const BUS_CAPACITY: usize = 256;

/// Structured event emitted by the agent loop.
///
/// Each variant carries the minimal data needed by consumers (telemetry,
/// recording, external harness CLI). Variants are non-exhaustive so that
/// new fields can be added without breaking existing `match` arms in callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentEvent {
    /// A new agent turn has started.
    TurnStarted {
        /// Logical turn counter (1-based).
        turn_number: u64,
        /// Session identifier.
        session_id: String,
        /// Wall-clock timestamp.
        at: DateTime<Utc>,
    },

    /// An agent turn completed (success or failure).
    TurnFinished {
        /// Logical turn counter.
        turn_number: u64,
        /// Session identifier.
        session_id: String,
        /// `true` if the turn succeeded without hard errors.
        success: bool,
        /// Approximate tokens used in this turn (0 when unknown).
        tokens_used: u32,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Wall-clock timestamp.
        at: DateTime<Utc>,
    },

    /// The LLM was called.
    LlmRequested {
        session_id: String,
        /// Input token estimate.
        input_tokens: u32,
        at: DateTime<Utc>,
    },

    /// The LLM returned a response.
    LlmResponded {
        session_id: String,
        /// Output token count.
        output_tokens: u32,
        /// Whether the response was streamed.
        streamed: bool,
        at: DateTime<Utc>,
    },

    /// A tool was invoked by the agent.
    ToolCalled {
        session_id: String,
        tool_name: String,
        /// Serialised call arguments (truncated to 1 KB for safety).
        args_snippet: String,
        at: DateTime<Utc>,
    },

    /// A tool returned a result.
    ToolResult {
        session_id: String,
        tool_name: String,
        /// `true` for success, `false` for error.
        success: bool,
        /// Duration of the tool call in milliseconds.
        duration_ms: u64,
        at: DateTime<Utc>,
    },

    /// Context compaction was triggered.
    ContextCompacted {
        session_id: String,
        /// Message count before compaction.
        messages_before: usize,
        /// Message count after compaction.
        messages_after: usize,
        at: DateTime<Utc>,
    },

    /// A permission prompt was shown to the user.
    PermissionPrompted {
        session_id: String,
        tool_name: String,
        at: DateTime<Utc>,
    },

    /// A reflection cycle completed.
    ReflectionCompleted {
        session_id: String,
        insights_count: usize,
        at: DateTime<Utc>,
    },

    /// Phase M4-A — `MemoryCoordinator::after_turn` produced a typed
    /// batch envelope (write-policy decisions + quality-gate result +
    /// per-candidate conflict-resolution outcomes).  Mirrors the
    /// frontend `memory_after_turn` Tauri event family but lives on
    /// the **harness EventBus** so trace sinks / future grader
    /// components can record decisions without subscribing to the
    /// frontend transport.
    ///
    /// Stable trace shape: `caller / policy_version / decided_at /
    /// decisions / quality / conflicts`.  Fires once per real turn
    /// end including when all arrays are empty (the empty batch is
    /// the explicit "no candidates this turn" signal — same
    /// semantics as the M3-C frontend event).
    /// Phase M4-C P2 — `prepare_step_execution` rendered a typed
    /// preflight decision for one tool dispatch.  Today emitted in
    /// **shadow** mode (the seam does not yet replace the existing
    /// per-tool authorize path); the trace records what the seam
    /// *would* have decided so M4 governance can compare against
    /// the actual production outcome.
    PrepareStepExecuted {
        session_id: String,
        tool_name: String,
        outcome: PrepareStepOutcome,
        boundary: BoundaryDecision,
        permission: PermissionDecision,
        sandbox: SandboxPolicy,
        policy_version: String,
        at: DateTime<Utc>,
    },

    /// Phase M4-C P3 — `request_intelligence_classify` produced an
    /// advisory execution-mode judgment.  The classifier is
    /// deterministic + heuristic + advisory only (it does NOT
    /// route the agent today).  This trace records every advisory
    /// decision so M4 governance can compare classifier judgment
    /// vs actual behaviour.
    ExecutionModeJudged {
        session_id: Option<String>,
        execution_mode: String,
        risk_level: String,
        complexity_level: String,
        policy_version: String,
        at: DateTime<Utc>,
    },

    /// Phase M4-C P4 — user resolved a permission prompt.  Pairs
    /// with [`Self::PermissionPrompted`]; `decision` and `scope`
    /// mirror the IPC contract of `respond_permission`.
    PermissionResolved {
        session_id: String,
        /// `tool_name` from the originating prompt when the
        /// IPC caller carried it; `None` when the prompt was
        /// session-wide.
        tool_name: Option<String>,
        /// `"allow"` or `"deny"`.
        decision: String,
        /// `"once"` or `"session"`.
        scope: String,
        at: DateTime<Utc>,
    },

    /// Phase M4-C P5 — agent stream entered a hard error state.
    /// `reason` mirrors the wire-level `stream_error` payload's
    /// reason string (already produced by `format_stream_error_reason`).
    StreamErrored {
        session_id: String,
        reason: String,
        /// `true` iff the error path advertised resume capability
        /// to the frontend (mirrors `resume_available` on the
        /// wire).  Useful for M4 governance to discriminate
        /// recoverable vs terminal errors.
        resume_available: bool,
        at: DateTime<Utc>,
    },

    /// Phase M4-C P5 — resume of a prior errored stream was
    /// invoked.  **No production caller emits this today**; the
    /// variant exists so the resume code path (M4.5+ work) can
    /// land without a contract bump.  Keep this variant unused
    /// rather than synthesising fake resumes.
    ResumeInvoked {
        session_id: String,
        resume_cursor: String,
        at: DateTime<Utc>,
    },

    MemoryAfterTurn {
        /// Stable governance trace contract version.  Pinned so
        /// future M4 graders / replay can compare envelopes across
        /// app versions.
        trace_version: &'static str,
        /// Originating call site tag (`"run_agent_turn"` /
        /// `"start_agent_stream"`).
        caller: &'static str,
        /// Optional session correlation id.
        session_id: Option<String>,
        /// Optional project correlation id.
        project_id: Option<String>,
        /// Stable policy version of the write-policy that produced
        /// the decisions (mirrors `MemoryWriteDecision.policy_version`).
        policy_version: String,
        /// RFC3339 wall-clock timestamp the batch was produced.
        decided_at: DateTime<Utc>,
        /// Pre-write decisions, one per candidate (parallel ordering).
        decisions: Vec<MemoryWriteDecision>,
        /// Quality-gate result over the whole batch.
        quality: QualityGateResult,
        /// Per-candidate conflict-resolution outcomes (parallel ordering).
        conflicts: Vec<ConflictResolution>,
    },
}

impl AgentEvent {
    /// Return the `session_id` embedded in this event, if present.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::TurnStarted { session_id, .. }
            | Self::TurnFinished { session_id, .. }
            | Self::LlmRequested { session_id, .. }
            | Self::LlmResponded { session_id, .. }
            | Self::ToolCalled { session_id, .. }
            | Self::ToolResult { session_id, .. }
            | Self::ContextCompacted { session_id, .. }
            | Self::PermissionPrompted { session_id, .. }
            | Self::ReflectionCompleted { session_id, .. }
            | Self::PrepareStepExecuted { session_id, .. }
            | Self::PermissionResolved { session_id, .. }
            | Self::StreamErrored { session_id, .. }
            | Self::ResumeInvoked { session_id, .. } => Some(session_id.as_str()),
            Self::MemoryAfterTurn { session_id, .. }
            | Self::ExecutionModeJudged { session_id, .. } => session_id.as_deref(),
        }
    }
}

/// Shared event bus for the agent loop.
///
/// Clone this to share across tasks. The underlying `broadcast::Sender` is
/// reference-counted so all clones share the same channel.
#[derive(Clone, Debug)]
pub struct EventBus {
    sender: Arc<broadcast::Sender<AgentEvent>>,
}

impl EventBus {
    /// Create a new `EventBus` with the default channel capacity.
    #[must_use]
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BUS_CAPACITY);
        Self {
            sender: Arc::new(tx),
        }
    }

    /// Emit an event to all current subscribers.
    ///
    /// Returns `Ok(n)` with the number of active receivers. Returns `Ok(0)` if
    /// there are no subscribers (not an error).
    ///
    /// # Errors
    /// Returns `Err` only when the channel is closed, which cannot happen while
    /// `self` holds a reference to the `Sender`.
    pub fn emit(
        &self,
        event: AgentEvent,
    ) -> Result<usize, broadcast::error::SendError<AgentEvent>> {
        match self.sender.send(event) {
            Ok(n) => Ok(n),
            // SendError means no receivers — treat as 0 rather than an error.
            Err(e) if self.sender.receiver_count() == 0 => {
                drop(e);
                Ok(0)
            }
            Err(e) => Err(e),
        }
    }

    /// Subscribe to all future events.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    /// Number of active subscribers.
    #[must_use]
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn emit_reaches_subscriber() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        let event = AgentEvent::TurnStarted {
            turn_number: 1,
            session_id: "s1".to_string(),
            at: Utc::now(),
        };

        bus.emit(event).expect("emit should succeed");

        let received = rx.try_recv().expect("should have received event");
        assert!(matches!(
            received,
            AgentEvent::TurnStarted { turn_number: 1, .. }
        ));
    }

    #[tokio::test]
    async fn emit_with_no_subscribers_returns_zero() {
        let bus = EventBus::new();
        // No subscribers.
        let result = bus.emit(AgentEvent::TurnStarted {
            turn_number: 1,
            session_id: "s1".to_string(),
            at: Utc::now(),
        });
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn memory_after_turn_event_reaches_subscriber() {
        use crate::modules::application::{
            ConflictResolution, ConflictResolutionOutcome, QualityGateResult,
            MEMORY_CONFLICT_RESOLVER_VERSION, MEMORY_QUALITY_GATE_VERSION,
        };
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let event = AgentEvent::MemoryAfterTurn {
            trace_version: "memory-after-turn-trace@m4.1",
            caller: "test_caller",
            session_id: Some("sess-1".to_string()),
            project_id: None,
            policy_version: "memory-write-policy@m3.3-skeleton".to_string(),
            decided_at: Utc::now(),
            decisions: Vec::new(),
            quality: QualityGateResult {
                accepted: Vec::new(),
                rejected: Vec::new(),
                warnings: Vec::new(),
                policy_version: MEMORY_QUALITY_GATE_VERSION.to_string(),
            },
            conflicts: vec![ConflictResolution {
                outcome: ConflictResolutionOutcome::NoConflict,
                reason_codes: vec!["no_conflict".to_string()],
                policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
            }],
        };
        bus.emit(event).expect("emit should succeed");
        let received = rx.try_recv().expect("should have received event");
        match received {
            AgentEvent::MemoryAfterTurn {
                trace_version,
                caller,
                session_id,
                conflicts,
                ..
            } => {
                assert_eq!(trace_version, "memory-after-turn-trace@m4.1");
                assert_eq!(caller, "test_caller");
                assert_eq!(session_id.as_deref(), Some("sess-1"));
                assert_eq!(conflicts.len(), 1);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
        // Session id accessor still works for the new variant.
        let event2 = AgentEvent::MemoryAfterTurn {
            trace_version: "v",
            caller: "c",
            session_id: Some("s".to_string()),
            project_id: None,
            policy_version: String::new(),
            decided_at: Utc::now(),
            decisions: Vec::new(),
            quality: QualityGateResult {
                accepted: Vec::new(),
                rejected: Vec::new(),
                warnings: Vec::new(),
                policy_version: String::new(),
            },
            conflicts: Vec::new(),
        };
        assert_eq!(event2.session_id(), Some("s"));
    }

    #[test]
    fn session_id_accessor_returns_correct_value() {
        let event = AgentEvent::ToolCalled {
            session_id: "abc".to_string(),
            tool_name: "bash".to_string(),
            args_snippet: "".to_string(),
            at: Utc::now(),
        };
        assert_eq!(event.session_id(), Some("abc"));
    }
}
