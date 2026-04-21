//! Harness Run Report contracts (Phase M4.2).
//!
//! Single canonical run-level governance report shape consumed by
//! the M4.3 graders, the future M4.5 baseline-vs-candidate compare,
//! and (later) the M4.6 governance UI.
//!
//! Honest scope of this module:
//!
//! - Defines the **types**.  Wiring (who builds the report, who
//!   pushes evidence, who runs graders) lives in
//!   [`super::trace_aggregator`] and [`super::graders`].
//! - The report is shaped to consume what the EventBus already
//!   produces today, **plus** a typed `EvidenceBundle` for traces
//!   that are observed but not yet on the bus (e.g. the M1.8
//!   `prepare_step_execution` seam, the M1.6
//!   `request_intelligence_classify` seam).  Those evidence vectors
//!   are intentionally allowed to be empty in M4.2 — graders that
//!   depend on them MUST honestly degrade to `Skeleton` / partial
//!   verdicts instead of inventing data.
//! - Stable trace contract version is pinned via
//!   [`HARNESS_RUN_REPORT_VERSION`] so the M4.5 compare flow can
//!   refuse mismatched reports.
//!
//! Out of scope for M4.2 (intentional non-goals):
//!
//! - Persistence / replay (writing the report to disk, reloading it
//!   for compare).  M4.5 + M4.7 territory.
//! - Multi-run / suite aggregation.  M4.7 corpus territory.
//! - Recommendation rendering (`promote / hold / reject`).  M4.8
//!   gate territory.

#![allow(dead_code)]

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::modules::application::{ConflictResolution, QualityGateResult};
use crate::modules::control_plane::prepare_step_execution::{
    BoundaryDecision, PermissionDecision, PrepareStepOutcome, SandboxPolicy,
};
use crate::modules::runtime::contracts::memory::MemoryWriteDecision;

/// Stable governance report contract version.  Bumping is a
/// breaking change; future graders / replay MUST honor it.
///
/// M4-C bump: extended `EvidenceBundle` with `permission_resolved`,
/// `stream_errors`, `resume_invocations`; existing `prepare_step` /
/// `execution_mode` slots are now real-source (no longer SEAM).
pub const HARNESS_RUN_REPORT_VERSION: &str = "harness-run-report@m4.4";

/// Canonical task outcome at the **run** level (an entire eval run
/// usually corresponds to one task; multi-task runs aggregate per
/// task).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOutcome {
    /// All turns succeeded and the agent reached an accepting
    /// terminal state.
    Success,
    /// At least one turn produced output but blocking failures
    /// were observed (e.g. tool-call failure recovered, partial
    /// answer).
    PartialSuccess,
    /// The agent did not produce an accepting terminal state.
    Failed,
    /// Run never finished (cancelled, crashed before TurnFinished).
    Incomplete,
}

/// Severity bucket shared across [`BlockingFailure`] and
/// [`super::graders::GraderVerdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Hard block — the run / candidate cannot pass governance.
    Blocking,
    /// Soft signal — recorded for review but does not block by
    /// itself.  Multiple warnings may aggregate to a blocking
    /// verdict at the M4.8 gate layer.
    Warning,
    /// Informational only — never blocks, never warns; counted for
    /// trend analysis.
    Info,
    /// Grader did not have enough input to render a verdict.
    /// Distinct from `Pass` because the absence of evidence is
    /// itself important signal for governance.
    Skeleton,
    /// Grader verdict is positive — no concerns.
    Pass,
}

/// Per-task result inside [`HarnessRunReport`].  v1 of the contract
/// assumes one task per run; multi-task runs build one
/// `TaskRunResult` per task and the report's top-level `task` field
/// holds the aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunResult {
    /// Stable identifier (corpus task id, harness scenario id, or
    /// fall-back to `run_id`).
    pub task_id: String,
    /// Coarse outcome.
    pub outcome: TaskOutcome,
    /// Number of completed turns the run executed.
    pub turn_count: u64,
    /// `true` iff the **last** observed `TurnFinished` event
    /// reported success.  When the run never produced a
    /// `TurnFinished`, this is `false`.
    pub last_turn_succeeded: bool,
    /// Optional terminal error / degraded reason summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
}

/// Aggregate counters over the whole run.  Sourced from real
/// EventBus traffic (`TurnFinished` / `LlmRequested` /
/// `LlmResponded` / `ToolCalled` / `ToolResult` / `MemoryAfterTurn`
/// / `PermissionPrompted` / `ContextCompacted`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AggregateMetrics {
    pub turns_completed: u64,
    pub turns_succeeded: u64,
    pub llm_calls: u64,
    pub input_tokens_total: u64,
    pub output_tokens_total: u64,
    /// Total tool invocations across all tool names.
    pub tool_calls_total: u64,
    /// Per-tool-name tool invocation counts (sorted for stable
    /// serialization across runs).
    pub tool_calls_by_name: BTreeMap<String, u64>,
    /// Per-tool-name success counts.
    pub tool_successes_by_name: BTreeMap<String, u64>,
    pub permission_prompts: u64,
    pub compaction_events: u64,
    pub total_turn_duration_ms: u64,
    /// Phase M4-A — counts derived from `MemoryAfterTurn`
    /// envelopes.  Zero when no `memory_store` tool calls fired.
    pub memory_decision_count: u64,
    pub memory_accepted_count: u64,
    pub memory_rejected_count: u64,
    pub memory_warning_count: u64,
    pub memory_conflict_prompt_count: u64,
    pub memory_conflict_replace_count: u64,
    pub memory_conflict_keep_existing_count: u64,
    pub memory_conflict_no_conflict_count: u64,
    /// Phase M4-C P2 — counters derived from
    /// `PrepareStepExecuted` events.
    pub prepare_step_total: u64,
    pub prepare_step_granted: u64,
    pub prepare_step_requires_approval: u64,
    pub prepare_step_denied: u64,
    /// Phase M4-C P3 — count of advisory execution-mode judgments.
    pub execution_mode_judgments: u64,
    /// Phase M4-C P4 — count of permission resolutions split by
    /// decision.
    pub permission_resolved_allow: u64,
    pub permission_resolved_deny: u64,
    /// Phase M4-C P5 — count of hard stream errors observed.
    pub stream_errors: u64,
    /// Phase M4-C P5 — count of resume invocations (always 0
    /// today; reserved for M4.5+ resume code path).
    pub resume_invocations: u64,
}

impl AggregateMetrics {
    /// Average turn duration in milliseconds, `0` when no turns
    /// completed.
    #[must_use]
    pub fn avg_turn_duration_ms(&self) -> u64 {
        if self.turns_completed == 0 {
            0
        } else {
            self.total_turn_duration_ms / self.turns_completed
        }
    }

    /// Turn success rate (0.0–1.0); `1.0` when no turns completed
    /// (vacuously true; graders that care about success rate MUST
    /// also check `turns_completed > 0`).
    #[must_use]
    pub fn turn_success_rate(&self) -> f64 {
        if self.turns_completed == 0 {
            1.0
        } else {
            self.turns_succeeded as f64 / self.turns_completed as f64
        }
    }
}

/// One structured failure observed during the run.  Distinct from a
/// grader verdict — failures are **observations** the aggregator
/// recorded directly off the EventBus; verdicts are graders'
/// **judgments** built on top of failures + metrics + evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockingFailure {
    /// Stable failure code (e.g. `"tool_failure"`,
    /// `"memory_rejected"`, `"prepare_step_denied"`).
    pub code: String,
    pub severity: Severity,
    pub message: String,
    /// Optional evidence pointer the grader / reviewer can follow
    /// to pull the originating envelope from `EvidenceBundle`.
    /// Today: free-form (e.g. `"memory_after_turn:0"`,
    /// `"tool_result:bash:1"`).  Stable scheme lands in M4.5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_ref: Option<String>,
    /// Wall-clock time the originating event was observed.
    pub observed_at: DateTime<Utc>,
}

/// Typed evidence the report carries forward so graders / replay /
/// compare can re-inspect raw envelopes without re-subscribing to
/// the EventBus.
///
/// Honest framing: only `memory_after_turn` is fully populated
/// today (M4-A wire).  The other vectors are SEAMS — the field
/// exists so M4.4+ wiring can plug in without a contract bump, but
/// today they will typically be empty.  Graders that depend on the
/// empty seams MUST report `Severity::Skeleton`, not `Pass`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceBundle {
    /// Phase M4-A — every `AgentEvent::MemoryAfterTurn` envelope
    /// observed during the run, in observation order.
    pub memory_after_turn: Vec<MemoryAfterTurnTrace>,
    /// Phase M4-C P2 — every `prepare_step_execution` decision
    /// observed during the run (real source via
    /// `AgentEvent::PrepareStepExecuted`).
    pub prepare_step: Vec<PrepareStepTrace>,
    /// Phase M4-C P3 — every advisory `ExecutionModeDecision`
    /// rendered during the run (real source via
    /// `AgentEvent::ExecutionModeJudged`).
    pub execution_mode: Vec<ExecutionModeTrace>,
    /// Phase M4-A — every permission prompt observed (real source
    /// via `AgentEvent::PermissionPrompted`).
    pub permission_prompts: Vec<PermissionPromptTrace>,
    /// Phase M4-C P4 — every permission resolution observed (real
    /// source via `AgentEvent::PermissionResolved`).  Pairs with
    /// `permission_prompts` by `tool_name` + ordering.
    pub permission_resolved: Vec<PermissionResolutionTrace>,
    /// Phase M4-C P5 — every hard stream-error observed (real
    /// source via `AgentEvent::StreamErrored`).
    pub stream_errors: Vec<StreamErrorTrace>,
    /// Phase M4-C P5 — every resume invocation observed.  Empty
    /// today; production resume code path lands in M4.5+.
    pub resume_invocations: Vec<ResumeInvocationTrace>,
}

/// Phase M4-A trace — verbatim envelope of one
/// `MemoryCoordinator::after_turn` invocation.  Mirrors
/// [`crate::modules::harness::event_bus::AgentEvent::MemoryAfterTurn`]
/// shape but owned (no borrows) so the report is freely cloneable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAfterTurnTrace {
    pub trace_version: String,
    pub caller: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub policy_version: String,
    pub decided_at: DateTime<Utc>,
    pub decisions: Vec<MemoryWriteDecision>,
    pub quality: QualityGateResult,
    pub conflicts: Vec<ConflictResolution>,
}

/// Phase M4-C P2 — typed trace of one `prepare_step_execution`
/// invocation.  Mirrors the AgentEvent::PrepareStepExecuted shape
/// but owned (no borrows) so the report is freely cloneable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareStepTrace {
    pub tool_name: String,
    pub outcome: PrepareStepOutcome,
    pub boundary: BoundaryDecision,
    pub permission: PermissionDecision,
    pub sandbox: SandboxPolicy,
    pub policy_version: String,
    pub observed_at: DateTime<Utc>,
}

/// Phase M4-C P3 — trace of one `request_intelligence_classify`
/// advisory judgment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionModeTrace {
    pub execution_mode: String,
    pub risk_level: String,
    pub complexity_level: String,
    pub policy_version: String,
    pub observed_at: DateTime<Utc>,
}

/// Phase M4-A — one observed permission prompt fired.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPromptTrace {
    pub tool_name: String,
    pub observed_at: DateTime<Utc>,
}

/// Phase M4-C P4 — one observed permission resolution (user
/// answered an outstanding prompt via `respond_permission`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResolutionTrace {
    /// Tool name when the IPC caller carried it; `None` when the
    /// prompt was session-wide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// `"allow"` / `"deny"`.
    pub decision: String,
    /// `"once"` / `"session"`.
    pub scope: String,
    pub observed_at: DateTime<Utc>,
}

/// Phase M4-C P5 — one hard stream-error observed during the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamErrorTrace {
    /// Wire-level reason string (e.g. `"network_timeout"`,
    /// `"provider_error: ..."`, `"cancelled_by_user"`).
    pub reason: String,
    /// Mirrors the `resume_available` flag advertised on the wire.
    pub resume_available: bool,
    pub observed_at: DateTime<Utc>,
}

/// Phase M4-C P5 — one resume invocation observed.  No production
/// caller emits this today; field set is held for forward compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeInvocationTrace {
    pub resume_cursor: String,
    pub observed_at: DateTime<Utc>,
}

/// Top-level report shape.  Held by value so it can travel through
/// the M4.5 compare flow without sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessRunReport {
    /// Stable contract version (`HARNESS_RUN_REPORT_VERSION`).
    pub report_version: String,
    /// Stable run id (UUID generated by the aggregator's
    /// `start_run`).
    pub run_id: String,
    /// Optional human-readable label (e.g. corpus name + tag).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub task: TaskRunResult,
    pub aggregate: AggregateMetrics,
    pub blocking_failures: Vec<BlockingFailure>,
    pub evidence: EvidenceBundle,
}

impl HarnessRunReport {
    /// Build an empty / placeholder report.  Used by tests and by
    /// the aggregator before any events are observed.
    #[must_use]
    pub fn new_empty(run_id: impl Into<String>, started_at: DateTime<Utc>) -> Self {
        let run_id = run_id.into();
        Self {
            report_version: HARNESS_RUN_REPORT_VERSION.to_string(),
            run_id: run_id.clone(),
            label: None,
            session_id: None,
            project_id: None,
            started_at,
            ended_at: started_at,
            task: TaskRunResult {
                task_id: run_id,
                outcome: TaskOutcome::Incomplete,
                turn_count: 0,
                last_turn_succeeded: false,
                error_summary: None,
            },
            aggregate: AggregateMetrics::default(),
            blocking_failures: Vec::new(),
            evidence: EvidenceBundle::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_has_pinned_version_and_incomplete_outcome() {
        let started = Utc::now();
        let r = HarnessRunReport::new_empty("run-1", started);
        assert_eq!(r.report_version, HARNESS_RUN_REPORT_VERSION);
        assert_eq!(r.task.outcome, TaskOutcome::Incomplete);
        assert_eq!(r.aggregate.turns_completed, 0);
        assert_eq!(r.aggregate.avg_turn_duration_ms(), 0);
        assert!((r.aggregate.turn_success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn report_round_trips_through_serde() {
        let started = Utc::now();
        let r = HarnessRunReport::new_empty("run-2", started);
        let s = serde_json::to_string(&r).unwrap();
        let back: HarnessRunReport = serde_json::from_str(&s).unwrap();
        assert_eq!(back.report_version, r.report_version);
        assert_eq!(back.run_id, r.run_id);
    }
}
