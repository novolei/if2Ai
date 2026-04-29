//! Telemetry Collector
//!
//! Listens to [`EventBus`] and aggregates per-session metrics: tool call counts,
//! token totals, turn timing, and compaction events.
//!
//! # `#[allow(dead_code)]` justification
//! `avg_turn_duration_ms`, `turn_success_rate`, and `clear_session` are
//! public API methods consumed by harness IPC commands and harness-cli.
//! They are not yet wired into all production call sites.

#![allow(dead_code)]
//!
//! # Usage
//! ```ignore
//! let collector = TelemetryCollector::new();
//! collector.attach(&event_bus);  // spawns background task
//! // … later …
//! let snapshot = collector.snapshot("session-id").await;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::event_bus::{AgentEvent, EventBus};

/// Per-session telemetry snapshot.
///
/// All counts are monotonically increasing from session start.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionTelemetry {
    /// Session identifier.
    pub session_id: String,
    /// Total completed turns.
    pub turns_completed: u64,
    /// Total successful turns.
    pub turns_succeeded: u64,
    /// Total LLM calls made.
    pub llm_calls: u64,
    /// Cumulative input tokens (estimate).
    pub input_tokens_total: u64,
    /// Cumulative output tokens.
    pub output_tokens_total: u64,
    /// Per-tool call counts.
    pub tool_calls: HashMap<String, u64>,
    /// Per-tool success counts.
    pub tool_successes: HashMap<String, u64>,
    /// Number of context compaction events.
    pub compaction_events: u64,
    /// Number of permission prompts shown.
    pub permission_prompts: u64,
    /// Number of reflection cycles completed.
    pub reflection_cycles: u64,
    /// Total insights extracted across all reflections.
    pub reflection_insights_total: usize,
    /// Wall-clock timestamp of the last event.
    pub last_event_at: Option<DateTime<Utc>>,
    /// Cumulative turn duration in milliseconds.
    pub total_turn_duration_ms: u64,
}

impl SessionTelemetry {
    fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Default::default()
        }
    }

    /// Average turn duration in milliseconds, or `0` when no turns have
    /// completed yet.
    #[must_use]
    pub fn avg_turn_duration_ms(&self) -> u64 {
        self.total_turn_duration_ms
            .checked_div(self.turns_completed)
            .unwrap_or(0)
    }

    /// Overall turn success rate (0.0–1.0).
    #[must_use]
    pub fn turn_success_rate(&self) -> f64 {
        if self.turns_completed == 0 {
            1.0
        } else {
            self.turns_succeeded as f64 / self.turns_completed as f64
        }
    }
}

/// Global telemetry state shared across the background collector task
/// and query callers.
#[derive(Debug, Default)]
struct TelemetryState {
    /// Map from session_id to accumulated telemetry.
    sessions: HashMap<String, SessionTelemetry>,
}

impl TelemetryState {
    fn apply(&mut self, event: &AgentEvent) {
        let session_id = match event.session_id() {
            Some(id) => id.to_string(),
            None => return,
        };

        let entry = self
            .sessions
            .entry(session_id.clone())
            .or_insert_with(|| SessionTelemetry::new(&session_id));

        match event {
            AgentEvent::TurnFinished {
                success,
                tokens_used,
                duration_ms,
                at,
                ..
            } => {
                entry.turns_completed += 1;
                if *success {
                    entry.turns_succeeded += 1;
                }
                entry.input_tokens_total += *tokens_used as u64;
                entry.total_turn_duration_ms += *duration_ms;
                entry.last_event_at = Some(*at);
            }
            AgentEvent::LlmRequested {
                input_tokens, at, ..
            } => {
                entry.llm_calls += 1;
                entry.input_tokens_total += *input_tokens as u64;
                entry.last_event_at = Some(*at);
            }
            AgentEvent::LlmResponded {
                output_tokens, at, ..
            } => {
                entry.output_tokens_total += *output_tokens as u64;
                entry.last_event_at = Some(*at);
            }
            AgentEvent::ToolCalled { tool_name, at, .. } => {
                *entry.tool_calls.entry(tool_name.clone()).or_insert(0) += 1;
                entry.last_event_at = Some(*at);
            }
            AgentEvent::ToolResult {
                tool_name,
                success,
                at,
                ..
            } => {
                if *success {
                    *entry.tool_successes.entry(tool_name.clone()).or_insert(0) += 1;
                }
                entry.last_event_at = Some(*at);
            }
            AgentEvent::ContextCompacted { at, .. } => {
                entry.compaction_events += 1;
                entry.last_event_at = Some(*at);
            }
            AgentEvent::PermissionPrompted { at, .. } => {
                entry.permission_prompts += 1;
                entry.last_event_at = Some(*at);
            }
            AgentEvent::ReflectionCompleted {
                insights_count, at, ..
            } => {
                entry.reflection_cycles += 1;
                entry.reflection_insights_total += *insights_count;
                entry.last_event_at = Some(*at);
            }
            // TurnStarted carries no counters; ignore.
            AgentEvent::TurnStarted { .. } => {}
            // Phase M4.1 — MemoryAfterTurn carries governance
            // trace data consumed by the M4.2+ trace sinks /
            // future grader components, not by per-session
            // telemetry counters.  Telemetry intentionally
            // ignores it.
            AgentEvent::MemoryAfterTurn { .. } => {}
            // Phase M4-C — governance trace variants (consumed by
            // TraceAggregator only; per-session telemetry counters
            // are intentionally agnostic to them).
            AgentEvent::PrepareStepExecuted { .. }
            | AgentEvent::ExecutionModeJudged { .. }
            | AgentEvent::PermissionResolved { .. }
            | AgentEvent::StreamErrored { .. }
            | AgentEvent::ResumeInvoked { .. } => {}
        }
    }
}

/// Telemetry collector — subscribes to the [`EventBus`] and accumulates metrics.
///
/// Cloning a `TelemetryCollector` shares the same underlying state.
#[derive(Clone, Debug)]
pub struct TelemetryCollector {
    state: Arc<RwLock<TelemetryState>>,
}

impl TelemetryCollector {
    /// Create a new, empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(TelemetryState::default())),
        }
    }

    /// Attach to an [`EventBus`] and start processing events in a background task.
    ///
    /// The background task runs until the `EventBus` is dropped (i.e. the
    /// `Sender` is closed). Lagged events (channel overflow) are silently
    /// skipped with a warning.
    pub fn attach(&self, bus: &EventBus) {
        let state = Arc::clone(&self.state);
        let mut rx = bus.subscribe();

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        state.write().await.apply(&event);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(
                            "[TelemetryCollector] missed {n} events due to channel overflow"
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            tracing::debug!("[TelemetryCollector] background task finished");
        });
    }

    /// Return a snapshot of telemetry for the given session, or `None` if no
    /// events have been recorded for that session.
    pub async fn snapshot(&self, session_id: &str) -> Option<SessionTelemetry> {
        self.state.read().await.sessions.get(session_id).cloned()
    }

    /// Return snapshots for all sessions seen so far.
    pub async fn all_snapshots(&self) -> Vec<SessionTelemetry> {
        self.state.read().await.sessions.values().cloned().collect()
    }

    /// Clear telemetry for a specific session (e.g., after it ends).
    pub async fn clear_session(&self, session_id: &str) {
        self.state.write().await.sessions.remove(session_id);
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::event_bus::EventBus;

    #[tokio::test]
    async fn telemetry_counts_tool_calls() {
        let bus = EventBus::new();
        let collector = TelemetryCollector::new();
        collector.attach(&bus);

        bus.emit(AgentEvent::ToolCalled {
            session_id: "s1".to_string(),
            tool_name: "bash".to_string(),
            args_snippet: String::new(),
            at: Utc::now(),
        })
        .ok();

        bus.emit(AgentEvent::ToolResult {
            session_id: "s1".to_string(),
            tool_name: "bash".to_string(),
            success: true,
            duration_ms: 50,
            at: Utc::now(),
        })
        .ok();

        // Give the background task time to process.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let snap = collector.snapshot("s1").await.expect("should have data");
        assert_eq!(*snap.tool_calls.get("bash").unwrap_or(&0), 1);
        assert_eq!(*snap.tool_successes.get("bash").unwrap_or(&0), 1);
    }

    #[tokio::test]
    async fn turn_success_rate_is_correct() {
        let bus = EventBus::new();
        let collector = TelemetryCollector::new();
        collector.attach(&bus);

        for success in [true, true, false] {
            bus.emit(AgentEvent::TurnFinished {
                turn_number: 1,
                session_id: "s2".to_string(),
                success,
                tokens_used: 100,
                duration_ms: 200,
                at: Utc::now(),
            })
            .ok();
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let snap = collector.snapshot("s2").await.expect("should have data");
        assert_eq!(snap.turns_completed, 3);
        assert_eq!(snap.turns_succeeded, 2);
        let rate = snap.turn_success_rate();
        assert!((rate - 2.0 / 3.0).abs() < 1e-6, "rate={rate}");
    }
}
