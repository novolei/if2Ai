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
            | Self::ReflectionCompleted { session_id, .. } => Some(session_id.as_str()),
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
