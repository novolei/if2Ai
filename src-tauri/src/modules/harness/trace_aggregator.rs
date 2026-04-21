//! Trace aggregator (Phase M4.4 foundation).
//!
//! Subscribes to the harness [`super::event_bus::EventBus`] and
//! folds the live event stream into a [`super::run_report::HarnessRunReport`]
//! that the M4.3 graders can score.
//!
//! Honest scope of this module:
//!
//! - Real consumption today: `TurnStarted` / `TurnFinished` /
//!   `LlmRequested` / `LlmResponded` / `ToolCalled` / `ToolResult`
//!   / `PermissionPrompted` / `ContextCompacted` /
//!   `MemoryAfterTurn`.  These all flow off the bus today and are
//!   folded into the report's [`super::run_report::AggregateMetrics`]
//!   + [`super::run_report::EvidenceBundle::memory_after_turn`].
//! - SEAM today: `prepare_step_execution` / `execution_mode` /
//!   `permission_resolved` outcomes.  The aggregator carries
//!   public push-side helpers
//!   ([`TraceAggregator::push_prepare_step`] /
//!   [`TraceAggregator::push_execution_mode`]) so the M4.4 P2 / P3
//!   wiring can inject typed traces directly without re-shaping
//!   `AgentEvent`.  Until that wiring lands, the
//!   `EvidenceBundle::prepare_step` / `execution_mode` vectors stay
//!   empty.
//!
//! Out of scope for M4.2 / M4.4:
//!
//! - Multi-run aggregation (M4.7 corpus territory).
//! - Persistence / replay.

#![allow(dead_code)]

use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;

use super::event_bus::{AgentEvent, EventBus};
use super::run_report::{
    AggregateMetrics, BlockingFailure, ExecutionModeTrace, HarnessRunReport, MemoryAfterTurnTrace,
    PermissionPromptTrace, PermissionResolutionTrace, PrepareStepTrace, ResumeInvocationTrace,
    Severity, StreamErrorTrace, TaskOutcome, HARNESS_RUN_REPORT_VERSION,
};
use crate::modules::application::ConflictResolutionOutcome;
use crate::modules::control_plane::prepare_step_execution::PrepareStepOutcome;
use crate::modules::runtime::contracts::memory::MemoryWriteDisposition;

/// Live aggregator state held by [`TraceAggregator`].  Mutated from
/// the background subscriber task and the public `push_*` helpers.
#[derive(Debug, Clone)]
struct AggregatorState {
    report: HarnessRunReport,
}

impl AggregatorState {
    fn new(run_id: String, label: Option<String>) -> Self {
        let started_at = Utc::now();
        let mut report = HarnessRunReport::new_empty(run_id, started_at);
        report.label = label;
        Self { report }
    }

    fn apply(&mut self, event: &AgentEvent) {
        let now = Utc::now();
        // Update session/project on first observation that carries
        // them; later turns overwrite to match the most recent
        // value (sessions / projects don't change mid-run today).
        if let Some(sid) = event.session_id() {
            if self.report.session_id.is_none() {
                self.report.session_id = Some(sid.to_string());
            }
        }

        match event {
            AgentEvent::TurnStarted { .. } => { /* counters update on TurnFinished */ }
            AgentEvent::TurnFinished {
                success,
                tokens_used,
                duration_ms,
                ..
            } => {
                self.report.aggregate.turns_completed += 1;
                if *success {
                    self.report.aggregate.turns_succeeded += 1;
                }
                self.report.aggregate.input_tokens_total += u64::from(*tokens_used);
                self.report.aggregate.total_turn_duration_ms += *duration_ms;
                self.report.task.turn_count = self.report.aggregate.turns_completed;
                self.report.task.last_turn_succeeded = *success;
                self.report.ended_at = now;
                if !*success {
                    self.report.blocking_failures.push(BlockingFailure {
                        code: "turn_failed".to_string(),
                        severity: Severity::Warning,
                        message: "TurnFinished reported success=false".to_string(),
                        evidence_ref: None,
                        observed_at: now,
                    });
                }
            }
            AgentEvent::LlmRequested { input_tokens, .. } => {
                self.report.aggregate.llm_calls += 1;
                self.report.aggregate.input_tokens_total += u64::from(*input_tokens);
            }
            AgentEvent::LlmResponded { output_tokens, .. } => {
                self.report.aggregate.output_tokens_total += u64::from(*output_tokens);
            }
            AgentEvent::ToolCalled { tool_name, .. } => {
                self.report.aggregate.tool_calls_total += 1;
                *self
                    .report
                    .aggregate
                    .tool_calls_by_name
                    .entry(tool_name.clone())
                    .or_insert(0) += 1;
            }
            AgentEvent::ToolResult {
                tool_name, success, ..
            } => {
                if *success {
                    *self
                        .report
                        .aggregate
                        .tool_successes_by_name
                        .entry(tool_name.clone())
                        .or_insert(0) += 1;
                } else {
                    self.report.blocking_failures.push(BlockingFailure {
                        code: "tool_failure".to_string(),
                        severity: Severity::Warning,
                        message: format!("Tool '{tool_name}' returned success=false"),
                        evidence_ref: Some(format!("tool_result:{tool_name}")),
                        observed_at: now,
                    });
                }
            }
            AgentEvent::ContextCompacted { .. } => {
                self.report.aggregate.compaction_events += 1;
            }
            AgentEvent::PermissionPrompted { tool_name, at, .. } => {
                self.report.aggregate.permission_prompts += 1;
                self.report
                    .evidence
                    .permission_prompts
                    .push(PermissionPromptTrace {
                        tool_name: tool_name.clone(),
                        observed_at: *at,
                    });
            }
            AgentEvent::ReflectionCompleted { .. } => { /* no aggregate counter today */ }
            AgentEvent::MemoryAfterTurn {
                trace_version,
                caller,
                session_id,
                project_id,
                policy_version,
                decided_at,
                decisions,
                quality,
                conflicts,
            } => {
                // Aggregate counters first so the grader can read
                // them without walking `evidence`.
                self.report.aggregate.memory_decision_count += decisions.len() as u64;
                self.report.aggregate.memory_accepted_count += quality.accepted.len() as u64;
                self.report.aggregate.memory_rejected_count += quality.rejected.len() as u64;
                self.report.aggregate.memory_warning_count += quality.warnings.len() as u64;
                for c in conflicts {
                    match c.outcome {
                        ConflictResolutionOutcome::RequirePrompt => {
                            self.report.aggregate.memory_conflict_prompt_count += 1;
                        }
                        ConflictResolutionOutcome::AcceptReplacement => {
                            self.report.aggregate.memory_conflict_replace_count += 1;
                        }
                        ConflictResolutionOutcome::KeepExisting
                        | ConflictResolutionOutcome::RejectCandidate => {
                            self.report.aggregate.memory_conflict_keep_existing_count += 1;
                        }
                        ConflictResolutionOutcome::NoConflict => {
                            self.report.aggregate.memory_conflict_no_conflict_count += 1;
                        }
                    }
                }
                // `Deny` decisions surface as a blocking failure so
                // the M4.5 compare can short-circuit on regressions.
                let deny_count = decisions
                    .iter()
                    .filter(|d| matches!(d.disposition, MemoryWriteDisposition::Deny))
                    .count();
                if deny_count > 0 {
                    self.report.blocking_failures.push(BlockingFailure {
                        code: "memory_write_denied".to_string(),
                        severity: Severity::Blocking,
                        message: format!("{deny_count} memory candidate(s) denied by write policy"),
                        evidence_ref: Some(format!(
                            "memory_after_turn:{}",
                            self.report.evidence.memory_after_turn.len()
                        )),
                        observed_at: now,
                    });
                }
                if project_id.is_some() && self.report.project_id.is_none() {
                    self.report.project_id = project_id.clone();
                }
                self.report
                    .evidence
                    .memory_after_turn
                    .push(MemoryAfterTurnTrace {
                        trace_version: trace_version.to_string(),
                        caller: caller.to_string(),
                        session_id: session_id.clone(),
                        project_id: project_id.clone(),
                        policy_version: policy_version.clone(),
                        decided_at: *decided_at,
                        decisions: decisions.clone(),
                        quality: quality.clone(),
                        conflicts: conflicts.clone(),
                    });
            }
            AgentEvent::PrepareStepExecuted {
                tool_name,
                outcome,
                boundary,
                permission,
                sandbox,
                policy_version,
                at,
                ..
            } => {
                self.report.aggregate.prepare_step_total += 1;
                match outcome {
                    PrepareStepOutcome::Granted => {
                        self.report.aggregate.prepare_step_granted += 1;
                    }
                    PrepareStepOutcome::RequiresApproval => {
                        self.report.aggregate.prepare_step_requires_approval += 1;
                    }
                    PrepareStepOutcome::Denied => {
                        self.report.aggregate.prepare_step_denied += 1;
                        // Record as Warning (not Blocking) — the
                        // M1.8 seam runs in shadow mode and does
                        // not actually block tool dispatch.  M4.8
                        // gate may upgrade this to Blocking once
                        // enforcement lands.
                        self.report.blocking_failures.push(BlockingFailure {
                            code: "prepare_step_denied".to_string(),
                            severity: Severity::Warning,
                            message: format!(
                                "prepare_step_execution would have denied tool '{tool_name}'"
                            ),
                            evidence_ref: Some(format!(
                                "prepare_step:{}",
                                self.report.evidence.prepare_step.len()
                            )),
                            observed_at: *at,
                        });
                    }
                }
                self.report.evidence.prepare_step.push(PrepareStepTrace {
                    tool_name: tool_name.clone(),
                    outcome: outcome.clone(),
                    boundary: boundary.clone(),
                    permission: permission.clone(),
                    sandbox: sandbox.clone(),
                    policy_version: policy_version.clone(),
                    observed_at: *at,
                });
            }
            AgentEvent::ExecutionModeJudged {
                execution_mode,
                risk_level,
                complexity_level,
                policy_version,
                at,
                ..
            } => {
                self.report.aggregate.execution_mode_judgments += 1;
                self.report
                    .evidence
                    .execution_mode
                    .push(ExecutionModeTrace {
                        execution_mode: execution_mode.clone(),
                        risk_level: risk_level.clone(),
                        complexity_level: complexity_level.clone(),
                        policy_version: policy_version.clone(),
                        observed_at: *at,
                    });
            }
            AgentEvent::PermissionResolved {
                tool_name,
                decision,
                scope,
                at,
                ..
            } => {
                if decision == "allow" {
                    self.report.aggregate.permission_resolved_allow += 1;
                } else {
                    self.report.aggregate.permission_resolved_deny += 1;
                }
                self.report
                    .evidence
                    .permission_resolved
                    .push(PermissionResolutionTrace {
                        tool_name: tool_name.clone(),
                        decision: decision.clone(),
                        scope: scope.clone(),
                        observed_at: *at,
                    });
            }
            AgentEvent::StreamErrored {
                reason,
                resume_available,
                at,
                ..
            } => {
                self.report.aggregate.stream_errors += 1;
                self.report.evidence.stream_errors.push(StreamErrorTrace {
                    reason: reason.clone(),
                    resume_available: *resume_available,
                    observed_at: *at,
                });
                // Record as Warning when resume is advertised
                // (recoverable), Blocking when terminal.
                let severity = if *resume_available {
                    Severity::Warning
                } else {
                    Severity::Blocking
                };
                self.report.blocking_failures.push(BlockingFailure {
                    code: "stream_error".to_string(),
                    severity,
                    message: format!("stream errored: {reason}"),
                    evidence_ref: Some(format!(
                        "stream_errors:{}",
                        self.report.evidence.stream_errors.len() - 1
                    )),
                    observed_at: *at,
                });
            }
            AgentEvent::ResumeInvoked {
                resume_cursor, at, ..
            } => {
                self.report.aggregate.resume_invocations += 1;
                self.report
                    .evidence
                    .resume_invocations
                    .push(ResumeInvocationTrace {
                        resume_cursor: resume_cursor.clone(),
                        observed_at: *at,
                    });
            }
        }
    }

    /// Finalise the report and snapshot it.  Computes the coarse
    /// `TaskOutcome` from observed turns + blocking failures.
    fn finalize(&mut self) -> HarnessRunReport {
        self.report.ended_at = Utc::now();
        self.report.task.outcome = derive_task_outcome(&self.report);
        self.report.task.task_id = self.report.run_id.clone();
        // Clone so the aggregator can be reused (e.g. live snapshot
        // queries during a long-running suite).
        self.report.clone()
    }
}

fn derive_task_outcome(r: &HarnessRunReport) -> TaskOutcome {
    if r.aggregate.turns_completed == 0 {
        return TaskOutcome::Incomplete;
    }
    let has_blocking = r
        .blocking_failures
        .iter()
        .any(|f| f.severity == Severity::Blocking);
    if has_blocking {
        return TaskOutcome::Failed;
    }
    let has_warning = r
        .blocking_failures
        .iter()
        .any(|f| f.severity == Severity::Warning);
    match (r.task.last_turn_succeeded, has_warning) {
        (true, false) => TaskOutcome::Success,
        (true, true) => TaskOutcome::PartialSuccess,
        (false, _) => TaskOutcome::Failed,
    }
}

/// Live aggregator that subscribes to the bus and produces a
/// [`HarnessRunReport`] on demand.
///
/// Phase M4-D — the aggregator now carries a real **run boundary**.
/// One `TraceAggregator` instance lives for the whole app, but the
/// `state` it folds events into can be **rotated** per run via
/// [`Self::begin_run`] / [`Self::reset_run`].  Without rotation,
/// the aggregator would be an app-lifetime accumulator and any
/// two reports it emits would compare against the wrong baseline.
///
/// Lifecycle:
///
/// ```text
///   new(run_id)
///       ↓
///   attach(&bus)                  // events start folding into run #1
///       ↓
///   finalize() → HarnessRunReport // snapshot run #1
///       ↓
///   begin_run(run_id_2)           // rotate; run #1 frozen, run #2 starts
///       ↓
///   finalize() → HarnessRunReport // snapshot run #2
///       …
/// ```
///
/// `attach` is called once at app start and the same background
/// task keeps draining the bus across rotations (the rotation only
/// swaps the inner `AggregatorState`).
#[derive(Debug, Clone)]
pub struct TraceAggregator {
    state: Arc<RwLock<AggregatorState>>,
}

impl TraceAggregator {
    /// Construct a fresh aggregator for one run.  `run_id` SHOULD
    /// be a UUID generated by the caller; if no stable id is
    /// available, fall back to a timestamp string.
    #[must_use]
    pub fn new(run_id: impl Into<String>) -> Self {
        Self {
            state: Arc::new(RwLock::new(AggregatorState::new(run_id.into(), None))),
        }
    }

    /// Convenience — build with an optional human-readable label.
    #[must_use]
    pub fn with_label(run_id: impl Into<String>, label: impl Into<String>) -> Self {
        let state = AggregatorState::new(run_id.into(), Some(label.into()));
        Self {
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Subscribe the aggregator to the bus.  Spawns a background
    /// task that runs until `attach`-er drops the returned handle
    /// (the join handle holds the receiver alive; dropping it ends
    /// the task gracefully).
    ///
    /// Idempotent within one process — calling `attach` twice
    /// produces two background tasks both writing into the same
    /// state.  Callers should attach once per run.
    pub fn attach(&self, bus: &EventBus) -> tokio::task::JoinHandle<()> {
        let state = self.state.clone();
        let mut rx = bus.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                let mut guard = state.write().await;
                guard.apply(&event);
            }
        })
    }

    /// Phase M4-D — start a new run, replacing the in-flight
    /// state.  Any events that arrive after this call fold into
    /// the **new** run; the previous run's state is **discarded**.
    /// Callers that want to preserve the previous run MUST call
    /// [`Self::finalize`] (or the atomic
    /// [`Self::finalize_and_reset_run`] helper) **before**
    /// `begin_run`.
    ///
    /// `label` is an optional human-readable tag stored on the
    /// new report (e.g. corpus task id + tag).
    pub async fn begin_run(&self, run_id: impl Into<String>, label: Option<String>) {
        let mut guard = self.state.write().await;
        *guard = AggregatorState::new(run_id.into(), label);
    }

    /// Convenience alias of [`Self::begin_run`] meant for the
    /// "rotate to next run" intent.  Same semantics — the old
    /// state is dropped if not finalised first.
    pub async fn reset_run(&self, next_run_id: impl Into<String>, label: Option<String>) {
        self.begin_run(next_run_id, label).await;
    }

    /// Phase M4-D — atomic [`Self::finalize`] + [`Self::reset_run`].
    /// Snapshots the current report, then rotates the inner state
    /// to a new run, all under one write lock so concurrent
    /// EventBus folding cannot interleave.
    pub async fn finalize_and_reset_run(
        &self,
        next_run_id: impl Into<String>,
        label: Option<String>,
    ) -> HarnessRunReport {
        let mut guard = self.state.write().await;
        let snapshot = guard.finalize();
        *guard = AggregatorState::new(next_run_id.into(), label);
        snapshot
    }

    /// Phase M4-D — read-only access to the current run id without
    /// finalising / rotating.  Useful for IPC callers that want to
    /// correlate events emitted out-of-band.
    pub async fn current_run_id(&self) -> String {
        self.state.read().await.report.run_id.clone()
    }

    /// SEAM helper — push a typed `prepare_step_execution` trace
    /// the EventBus does not yet carry.  Used by M4.4 P2 wiring.
    pub async fn push_prepare_step(&self, trace: PrepareStepTrace) {
        self.state
            .write()
            .await
            .report
            .evidence
            .prepare_step
            .push(trace);
    }

    /// SEAM helper — push a typed `execution_mode` advisory trace.
    /// Used by M4.4 P3 wiring.
    pub async fn push_execution_mode(&self, trace: ExecutionModeTrace) {
        self.state
            .write()
            .await
            .report
            .evidence
            .execution_mode
            .push(trace);
    }

    /// Finalise + snapshot the current report.  Safe to call
    /// multiple times; each call returns a fresh snapshot.
    pub async fn finalize(&self) -> HarnessRunReport {
        self.state.write().await.finalize()
    }

    /// Read-only snapshot of the current aggregate metrics
    /// without finalising.  Used by live observation surfaces.
    pub async fn snapshot_metrics(&self) -> AggregateMetrics {
        self.state.read().await.report.aggregate.clone()
    }

    /// Read-only access to the pinned report contract version.
    #[must_use]
    pub fn report_version(&self) -> &'static str {
        HARNESS_RUN_REPORT_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::application::{
        ConflictResolution, ConflictResolutionOutcome, QualityGateAccepted, QualityGateResult,
        MEMORY_CONFLICT_RESOLVER_VERSION, MEMORY_QUALITY_GATE_VERSION,
    };
    use crate::modules::runtime::contracts::memory::{
        MemoryObjectKind, MemoryScope, MemoryWriteCandidate, MemoryWriteDecision,
        MemoryWriteDisposition,
    };

    fn synth_decision(disposition: MemoryWriteDisposition) -> MemoryWriteDecision {
        MemoryWriteDecision {
            disposition,
            reason_codes: vec!["test".to_string()],
            object_kind: MemoryObjectKind::Fact,
            scope: MemoryScope::Session,
            evidence_id: None,
            policy_version: "memory-write-policy@m3.3-skeleton".to_string(),
            decided_at: "2026-04-20T00:00:00Z".to_string(),
        }
    }

    fn synth_candidate() -> MemoryWriteCandidate {
        MemoryWriteCandidate {
            object_kind: MemoryObjectKind::Fact,
            scope: MemoryScope::Session,
            content_preview: "k: v".to_string(),
            evidence_id: None,
            source: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn turn_finished_updates_counters_and_outcome() {
        let bus = EventBus::new();
        let agg = TraceAggregator::new("run-1");
        let _h = agg.attach(&bus);
        bus.emit(AgentEvent::TurnStarted {
            turn_number: 1,
            session_id: "s".to_string(),
            at: Utc::now(),
        })
        .unwrap();
        bus.emit(AgentEvent::TurnFinished {
            turn_number: 1,
            session_id: "s".to_string(),
            success: true,
            tokens_used: 100,
            duration_ms: 250,
            at: Utc::now(),
        })
        .unwrap();
        // Give the spawned task a moment to drain.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let report = agg.finalize().await;
        assert_eq!(report.aggregate.turns_completed, 1);
        assert_eq!(report.aggregate.turns_succeeded, 1);
        assert_eq!(report.task.outcome, TaskOutcome::Success);
        assert_eq!(report.session_id.as_deref(), Some("s"));
    }

    #[tokio::test]
    async fn memory_after_turn_aggregates_counts_and_evidence() {
        let bus = EventBus::new();
        let agg = TraceAggregator::new("run-2");
        let _h = agg.attach(&bus);
        bus.emit(AgentEvent::MemoryAfterTurn {
            trace_version: "memory-after-turn-trace@m4.1",
            caller: "test",
            session_id: Some("s".to_string()),
            project_id: None,
            policy_version: "memory-write-policy@m3.3-skeleton".to_string(),
            decided_at: Utc::now(),
            decisions: vec![
                synth_decision(MemoryWriteDisposition::Allow),
                synth_decision(MemoryWriteDisposition::Deny),
            ],
            quality: QualityGateResult {
                accepted: vec![QualityGateAccepted {
                    candidate: synth_candidate(),
                    decision: synth_decision(MemoryWriteDisposition::Allow),
                }],
                rejected: Vec::new(),
                warnings: Vec::new(),
                policy_version: MEMORY_QUALITY_GATE_VERSION.to_string(),
            },
            conflicts: vec![ConflictResolution {
                outcome: ConflictResolutionOutcome::RequirePrompt,
                reason_codes: vec!["same_fact_different_value".to_string()],
                policy_version: MEMORY_CONFLICT_RESOLVER_VERSION.to_string(),
            }],
        })
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let report = agg.finalize().await;
        assert_eq!(report.aggregate.memory_decision_count, 2);
        assert_eq!(report.aggregate.memory_accepted_count, 1);
        assert_eq!(report.aggregate.memory_conflict_prompt_count, 1);
        assert_eq!(report.evidence.memory_after_turn.len(), 1);
        // One Deny → one blocking failure.
        assert!(report
            .blocking_failures
            .iter()
            .any(|f| f.code == "memory_write_denied" && f.severity == Severity::Blocking));
    }

    #[tokio::test]
    async fn m4c_governance_traces_round_trip_through_bus() {
        use crate::modules::control_plane::prepare_step_execution::{
            BoundaryDecision, PermissionDecision, PrepareStepOutcome, SandboxPolicy,
        };
        let bus = EventBus::new();
        let agg = TraceAggregator::new("run-m4c");
        let _h = agg.attach(&bus);
        bus.emit(AgentEvent::PrepareStepExecuted {
            session_id: "s".to_string(),
            tool_name: "bash".to_string(),
            outcome: PrepareStepOutcome::Granted,
            boundary: BoundaryDecision::WithinWorkdir,
            permission: PermissionDecision::Allow,
            sandbox: SandboxPolicy::None,
            policy_version: "prepare-step@m1.8-skeleton".to_string(),
            at: Utc::now(),
        })
        .unwrap();
        bus.emit(AgentEvent::ExecutionModeJudged {
            session_id: Some("s".to_string()),
            execution_mode: "direct_execute".to_string(),
            risk_level: "low".to_string(),
            complexity_level: "trivial".to_string(),
            policy_version: "ingress-classifier@m1.6".to_string(),
            at: Utc::now(),
        })
        .unwrap();
        bus.emit(AgentEvent::PermissionResolved {
            session_id: "s".to_string(),
            tool_name: Some("bash".to_string()),
            decision: "allow".to_string(),
            scope: "session".to_string(),
            at: Utc::now(),
        })
        .unwrap();
        bus.emit(AgentEvent::StreamErrored {
            session_id: "s".to_string(),
            reason: "network_timeout".to_string(),
            resume_available: true,
            at: Utc::now(),
        })
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let report = agg.finalize().await;
        assert_eq!(report.aggregate.prepare_step_total, 1);
        assert_eq!(report.aggregate.prepare_step_granted, 1);
        assert_eq!(report.aggregate.execution_mode_judgments, 1);
        assert_eq!(report.aggregate.permission_resolved_allow, 1);
        assert_eq!(report.aggregate.stream_errors, 1);
        assert_eq!(report.evidence.prepare_step.len(), 1);
        assert_eq!(report.evidence.execution_mode.len(), 1);
        assert_eq!(report.evidence.permission_resolved.len(), 1);
        assert_eq!(report.evidence.stream_errors.len(), 1);
        // Resume-available stream error → Warning (not Blocking).
        assert!(report
            .blocking_failures
            .iter()
            .any(|f| f.code == "stream_error" && f.severity == Severity::Warning));
    }

    #[tokio::test]
    async fn run_lifecycle_rotation_isolates_runs() {
        let bus = EventBus::new();
        let agg = TraceAggregator::new("run-1");
        let _h = agg.attach(&bus);
        // Run 1: one TurnFinished + one MemoryAfterTurn-style
        // bookkeeping (use TurnFinished to keep test small).
        bus.emit(AgentEvent::TurnFinished {
            turn_number: 1,
            session_id: "s".to_string(),
            success: true,
            tokens_used: 10,
            duration_ms: 100,
            at: Utc::now(),
        })
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        let r1 = agg
            .finalize_and_reset_run("run-2", Some("second".to_string()))
            .await;
        assert_eq!(r1.run_id, "run-1");
        assert_eq!(r1.aggregate.turns_completed, 1);

        // Run 2 starts fresh — counters reset to zero.
        let snapshot_before_event = agg.finalize().await;
        assert_eq!(snapshot_before_event.run_id, "run-2");
        assert_eq!(snapshot_before_event.aggregate.turns_completed, 0);
        assert_eq!(snapshot_before_event.label.as_deref(), Some("second"));

        // Run 2 receives a new TurnFinished.
        bus.emit(AgentEvent::TurnFinished {
            turn_number: 1,
            session_id: "s".to_string(),
            success: false,
            tokens_used: 5,
            duration_ms: 50,
            at: Utc::now(),
        })
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        let r2 = agg.finalize().await;
        assert_eq!(r2.aggregate.turns_completed, 1);
        assert_eq!(r2.aggregate.turns_succeeded, 0);
        // Crucially: r2 does NOT carry r1's counters.
        assert_eq!(r2.run_id, "run-2");
    }

    #[tokio::test]
    async fn current_run_id_returns_active_run() {
        let agg = TraceAggregator::new("run-x");
        assert_eq!(agg.current_run_id().await, "run-x");
        agg.begin_run("run-y", None).await;
        assert_eq!(agg.current_run_id().await, "run-y");
    }

    #[tokio::test]
    async fn seam_helpers_push_into_evidence_bundle() {
        let agg = TraceAggregator::new("run-3");
        use crate::modules::control_plane::prepare_step_execution::{
            BoundaryDecision, PermissionDecision, PrepareStepOutcome, SandboxPolicy,
        };
        agg.push_prepare_step(PrepareStepTrace {
            tool_name: "bash".to_string(),
            outcome: PrepareStepOutcome::Granted,
            boundary: BoundaryDecision::WithinWorkdir,
            permission: PermissionDecision::Allow,
            sandbox: SandboxPolicy::None,
            policy_version: "prepare-step@m1.8-skeleton".to_string(),
            observed_at: Utc::now(),
        })
        .await;
        agg.push_execution_mode(ExecutionModeTrace {
            execution_mode: "direct_execute".to_string(),
            risk_level: "low".to_string(),
            complexity_level: "trivial".to_string(),
            policy_version: "ingress-classifier@m1.6-skeleton".to_string(),
            observed_at: Utc::now(),
        })
        .await;
        let report = agg.finalize().await;
        assert_eq!(report.evidence.prepare_step.len(), 1);
        assert_eq!(report.evidence.execution_mode.len(), 1);
    }
}
