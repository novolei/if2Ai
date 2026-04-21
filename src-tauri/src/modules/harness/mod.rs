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
pub mod compare;
pub mod corpus;
pub mod event_bus;
pub mod gate;
pub mod graders;
pub mod report_persistence;
pub mod run_report;
pub mod session_recorder;
pub mod suite_report;
pub mod telemetry;
pub mod trace_aggregator;

// Re-exported for callers that import directly from `crate::modules::harness`
// rather than via the full `crate::modules::harness::event_bus` path.
pub use compare::{
    compare_reports, AggregateDiff, BaselineVsCandidate, BlockerDiff, CompareError,
    EvidenceSummaryDiff, GraderVerdictDiff, ReportVersionCompatibility, VecLengthDiff,
    HARNESS_COMPARE_VERSION,
};
pub use corpus::{
    load_corpus_from_yaml, CorpusError, CorpusTask, CorpusTier, RegressionCorpus,
    HARNESS_CORPUS_VERSION,
};
#[allow(unused_imports)]
pub use event_bus::{AgentEvent, EventBus};
pub use gate::{
    evaluate_compare as gate_evaluate_compare, evaluate_suite as gate_evaluate_suite, GateDecision,
    GatePolicy, Recommendation, HARNESS_GATE_VERSION,
};
pub use graders::{run_all as run_all_graders, GraderId, GraderVerdict, HARNESS_GRADERS_VERSION};
pub use report_persistence::{
    HarnessReportStore, PersistenceError, RunIndexEntry, HARNESS_REPORT_PERSISTENCE_VERSION,
};
pub use run_report::{
    AggregateMetrics, BlockingFailure, EvidenceBundle, ExecutionModeTrace, HarnessRunReport,
    MemoryAfterTurnTrace, PermissionPromptTrace, PrepareStepTrace, Severity, TaskOutcome,
    TaskRunResult, HARNESS_RUN_REPORT_VERSION,
};
pub use session_recorder::SessionRecorder;
pub use suite_report::{
    aggregate_suite_report, SuiteGrade, SuiteReport, TaskClassification, TaskRunOutcome,
    TierSummary, HARNESS_SUITE_REPORT_VERSION,
};
pub use telemetry::{SessionTelemetry, TelemetryCollector};
pub use trace_aggregator::TraceAggregator;

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
    /// Phase M4-C — governance trace aggregator.  Auto-attached to
    /// the event bus at construction so every `AgentEvent` is
    /// folded into the live `HarnessRunReport`.  Callers obtain
    /// the report via [`TraceAggregator::finalize`] (exposed as
    /// the `harness_finalize_run` IPC).
    pub trace_aggregator: Arc<TraceAggregator>,
    /// Phase M4.6 — durable on-disk store for finalised reports.
    /// `harness_finalize_and_rotate_run` IPC auto-saves each
    /// finalised report so the M4.5 compare flow / M4.8 gate /
    /// future review surface can re-load history without
    /// re-running the trace pipeline.
    pub report_store: Arc<HarnessReportStore>,
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
        // Phase M4-C — single shared aggregator per app run.  Run
        // id derives from a UUID so multiple cold-starts produce
        // distinguishable reports.  Attached to the bus before
        // returning so no events are missed.
        let trace_aggregator = Arc::new(TraceAggregator::new(uuid::Uuid::new_v4().to_string()));
        let _ = trace_aggregator.attach(&bus);
        // Phase M4.6 — durable report store.  Default root lives
        // under the platform data dir (`~/Library/Application
        // Support/if2ai/harness/runs` on macOS); first save
        // creates it lazily.  Cheap `Arc` so all IPC handlers
        // share the same handle.
        let report_store = Arc::new(HarnessReportStore::with_default_root());
        Self {
            event_bus: bus,
            telemetry,
            recorder,
            trace_aggregator,
            report_store,
        }
    }
}
