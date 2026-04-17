//! Agent Loop Harness
//!
//! Provides observability infrastructure for the If2Ai agent loop:
//!
//! | Sub-module               | Responsibility                                         |
//! |--------------------------|--------------------------------------------------------|
//! | [`event_bus`]            | Broadcast channel of [`AgentEvent`]s                   |
//! | [`telemetry`]            | Aggregate per-session metrics from the event stream    |
//! | [`session_recorder`]     | Persist events to JSONL trace files                    |
//! | [`agent_loop_integration`] | Emit helpers for `commands/agent.rs` call sites      |
//!
//! ## Wiring in `AppState`
//!
//! `AppState` holds an `Option<Arc<HarnessState>>` that is `None` in
//! production by default (no-op path) and `Some(...)` when harness recording
//! is enabled (e.g. during eval runs or developer mode). The IPC command
//! `start_harness_recording` toggles recording on/off via [`HarnessControl`].
//!
//! ## EventBus lifetime
//!
//! The [`EventBus`] lives for the duration of the application. All agent
//! turns emit events onto it; zero overhead when no subscribers are present.

pub mod agent_loop_integration;
pub mod event_bus;
pub mod session_recorder;
pub mod telemetry;

#[allow(unused_imports)]
pub use event_bus::{AgentEvent, EventBus};
pub use session_recorder::SessionRecorder;
pub use telemetry::{SessionTelemetry, TelemetryCollector};

use std::sync::Arc;

/// Bundled harness state held by `AppState`.
///
/// `HarnessState` is cheap to clone (all fields are `Arc`-backed).
#[derive(Clone, Debug)]
pub struct HarnessState {
    /// Shared event bus — the single source of truth for agent events.
    pub event_bus: EventBus,
    /// Telemetry collector — processes events into per-session metrics.
    pub telemetry: TelemetryCollector,
    /// Session recorder — writes events to JSONL trace files.
    pub recorder: Arc<SessionRecorder>,
}

impl HarnessState {
    /// Construct and wire a new `HarnessState`.
    ///
    /// `trace_dir` is the directory where JSONL session traces are written.
    #[must_use]
    pub fn new(trace_dir: impl Into<std::path::PathBuf>) -> Self {
        let bus = EventBus::new();
        let telemetry = TelemetryCollector::new();
        // Attach collector to bus before returning so no events are missed.
        telemetry.attach(&bus);
        let recorder = Arc::new(SessionRecorder::new(trace_dir));
        Self {
            event_bus: bus,
            telemetry,
            recorder,
        }
    }
}
