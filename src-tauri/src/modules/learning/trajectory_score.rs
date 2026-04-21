//! Trajectory scoring (Phase M5 m5.1, closeout).
//!
//! Typed scoring of one [`HarnessRunReport`] (or a pair of
//! reports) along five governance-aligned axes:
//!
//! 1. **Completion** — task outcome + last-turn success
//! 2. **Recovery quality** — recovered vs hard-failed turns
//! 3. **Tool quality** — tool failure / misuse density
//! 4. **Memory alignment** — memory_after_turn accept/reject ratio
//!    + recall/conflict signals from `EvidenceBundle`
//! 5. **Governance signal** — number of blocking failures + their
//!    aggregate severity
//!
//! Honest scope:
//!
//! - **Pure compute**, no IO.  Consumers (reflection generator,
//!   candidate evaluator, future M5-D activation policy) call
//!   [`score_run_report`] / [`score_compare`] directly.
//! - Composite score is a single `f64 ∈ [0.0, 1.0]` weighted
//!   over the five axes.  Each axis is also returned so
//!   reviewers can see WHY a low score happened.
//! - Latency / token are intentionally **not** part of the
//!   score: per the M5 design spec "score 不能只看 latency/token";
//!   they're counted but never weighted.
//! - Stable contract version pinned via
//!   [`TRAJECTORY_SCORE_VERSION`] so persisted scores can be
//!   refused if the formula evolves breaking-ly.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::modules::harness::{HarnessRunReport, Severity, TaskOutcome};

/// Stable contract version for [`TrajectoryScore`].  Bumping is
/// breaking for any persisted score downstream.
pub const TRAJECTORY_SCORE_VERSION: &str = "trajectory-score@m5.1";

/// Closed enum of scoring axes.  Used by reflection generation
/// to decide which `ReflectionIssueType` to emit (low completion
/// → intent miss / repeated failure; low recovery → recovery
/// issue; etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryAxis {
    Completion,
    RecoveryQuality,
    ToolQuality,
    MemoryAlignment,
    GovernanceSignal,
}

impl TrajectoryAxis {
    /// Default weight used by [`composite_score`].  Completion
    /// dominates (0.6) because outcome is the primary signal for
    /// "did the agent finish the task"; the other four axes
    /// share the remaining 0.4 equally.  Future phases may
    /// tune.  Sum of defaults == 1.0.
    #[must_use]
    pub fn default_weight(self) -> f64 {
        match self {
            Self::Completion => 0.60,
            Self::RecoveryQuality => 0.10,
            Self::ToolQuality => 0.10,
            Self::MemoryAlignment => 0.10,
            Self::GovernanceSignal => 0.10,
        }
    }

    /// Stable wire label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Completion => "completion",
            Self::RecoveryQuality => "recovery_quality",
            Self::ToolQuality => "tool_quality",
            Self::MemoryAlignment => "memory_alignment",
            Self::GovernanceSignal => "governance_signal",
        }
    }
}

/// One axis breakdown.  `value ∈ [0.0, 1.0]` (1.0 == best).
/// `weight` is the contribution to the composite score; sum of
/// weights MUST be > 0 for [`TrajectoryScore::composite`] to
/// produce a meaningful number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AxisBreakdown {
    pub axis: TrajectoryAxis,
    pub value: f64,
    pub weight: f64,
    /// Short human-readable rationale for the value (e.g.
    /// "outcome=Success last_turn_succeeded=true").
    pub rationale: String,
}

/// Top-level score record.  Held as a value (no Arc) so callers
/// can stash it in `ReflectionNote.evidence` / candidate audit
/// records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryScore {
    pub score_version: String,
    /// `run_id` of the report this score was computed from.
    pub run_id: String,
    /// Composite weighted score `∈ [0.0, 1.0]`.
    pub composite: f64,
    /// Per-axis breakdown.  Always carries one entry per
    /// [`TrajectoryAxis`] variant.
    pub axes: Vec<AxisBreakdown>,
    /// Convenience flag: `true` iff `composite < 0.5` — used by
    /// reflection generation to decide whether to emit a note.
    pub is_low_quality: bool,
}

impl TrajectoryScore {
    /// Find one axis by label.  Useful for tests + diagnostics.
    #[must_use]
    pub fn axis(&self, axis: TrajectoryAxis) -> Option<&AxisBreakdown> {
        self.axes.iter().find(|a| a.axis == axis)
    }
}

// ────────────────── public scoring entry points ──────────────────

/// Score one [`HarnessRunReport`].  Pure function.
#[must_use]
pub fn score_run_report(report: &HarnessRunReport) -> TrajectoryScore {
    let completion = score_completion(report);
    let recovery = score_recovery(report);
    let tool = score_tool_quality(report);
    let memory = score_memory_alignment(report);
    let governance = score_governance_signal(report);

    let axes = vec![completion, recovery, tool, memory, governance];
    let composite = composite_score(&axes);

    TrajectoryScore {
        score_version: TRAJECTORY_SCORE_VERSION.to_string(),
        run_id: report.run_id.clone(),
        composite,
        is_low_quality: composite < 0.5,
        axes,
    }
}

/// Score the **delta** between baseline and candidate run
/// reports (positive ⇒ candidate is better).  Returns the
/// candidate score and the delta-from-baseline so callers can
/// detect regression / improvement directly.
#[must_use]
pub fn score_compare(
    baseline: &HarnessRunReport,
    candidate: &HarnessRunReport,
) -> CompareScore {
    let baseline_score = score_run_report(baseline);
    let candidate_score = score_run_report(candidate);
    let delta = candidate_score.composite - baseline_score.composite;
    CompareScore {
        baseline_run_id: baseline.run_id.clone(),
        candidate_run_id: candidate.run_id.clone(),
        baseline: baseline_score,
        candidate: candidate_score,
        composite_delta: delta,
        regression: delta < -0.05,
        improvement: delta > 0.05,
    }
}

/// Compare-pair score wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareScore {
    pub baseline_run_id: String,
    pub candidate_run_id: String,
    pub baseline: TrajectoryScore,
    pub candidate: TrajectoryScore,
    /// `candidate.composite - baseline.composite`.  Negative ⇒
    /// regression.
    pub composite_delta: f64,
    /// `composite_delta < -0.05` (below noise floor).
    pub regression: bool,
    /// `composite_delta > 0.05`.
    pub improvement: bool,
}

// ────────────────── per-axis scorers ──────────────────

fn score_completion(report: &HarnessRunReport) -> AxisBreakdown {
    let value = match report.task.outcome {
        TaskOutcome::Success => {
            if report.task.last_turn_succeeded {
                1.0
            } else {
                0.85
            }
        }
        TaskOutcome::PartialSuccess => 0.5,
        TaskOutcome::Failed => 0.1,
        TaskOutcome::Incomplete => 0.0,
    };
    AxisBreakdown {
        axis: TrajectoryAxis::Completion,
        value,
        weight: TrajectoryAxis::Completion.default_weight(),
        rationale: format!(
            "outcome={:?} last_turn_succeeded={}",
            report.task.outcome, report.task.last_turn_succeeded
        ),
    }
}

fn score_recovery(report: &HarnessRunReport) -> AxisBreakdown {
    // Use turn success rate from aggregate metrics as the
    // recovery proxy: completed turns that succeeded vs total
    // completed turns.  When zero turns ran, treat as neutral
    // (1.0 — vacuously true; matches AggregateMetrics::turn_success_rate).
    let value = report.aggregate.turn_success_rate();
    AxisBreakdown {
        axis: TrajectoryAxis::RecoveryQuality,
        value,
        weight: TrajectoryAxis::RecoveryQuality.default_weight(),
        rationale: format!(
            "turns_completed={} turns_succeeded={}",
            report.aggregate.turns_completed, report.aggregate.turns_succeeded
        ),
    }
}

fn score_tool_quality(report: &HarnessRunReport) -> AxisBreakdown {
    let total: u64 = report.aggregate.tool_calls_total;
    let successes: u64 = report.aggregate.tool_successes_by_name.values().sum();
    let value = if total == 0 {
        1.0
    } else {
        (successes as f64 / total as f64).clamp(0.0, 1.0)
    };
    AxisBreakdown {
        axis: TrajectoryAxis::ToolQuality,
        value,
        weight: TrajectoryAxis::ToolQuality.default_weight(),
        rationale: format!("tool_calls_total={} successes={}", total, successes),
    }
}

fn score_memory_alignment(report: &HarnessRunReport) -> AxisBreakdown {
    let total = report.aggregate.memory_decision_count;
    let accepted = report.aggregate.memory_accepted_count;
    let rejected = report.aggregate.memory_rejected_count;
    let warnings = report.aggregate.memory_warning_count;
    let value = if total == 0 {
        // Neutral: no memory decisions observed.
        1.0
    } else {
        // Reward acceptances; penalise rejects + warnings
        // proportionally.
        let bad = rejected as f64 + 0.5 * warnings as f64;
        let raw = (accepted as f64 - bad) / (total as f64);
        // map [-1.0, 1.0] -> [0.0, 1.0]
        ((raw + 1.0) / 2.0).clamp(0.0, 1.0)
    };
    AxisBreakdown {
        axis: TrajectoryAxis::MemoryAlignment,
        value,
        weight: TrajectoryAxis::MemoryAlignment.default_weight(),
        rationale: format!(
            "memory_decisions={} accepted={} rejected={} warnings={}",
            total, accepted, rejected, warnings
        ),
    }
}

fn score_governance_signal(report: &HarnessRunReport) -> AxisBreakdown {
    // Penalty grows with severity: Blocking >> Warning > Info.
    let mut penalty: f64 = 0.0;
    let mut blocking = 0usize;
    let mut warning = 0usize;
    for f in &report.blocking_failures {
        match f.severity {
            Severity::Blocking => {
                penalty += 0.4;
                blocking += 1;
            }
            Severity::Warning => {
                penalty += 0.15;
                warning += 1;
            }
            _ => {}
        }
    }
    let value: f64 = (1.0_f64 - penalty).clamp(0.0, 1.0);
    AxisBreakdown {
        axis: TrajectoryAxis::GovernanceSignal,
        value,
        weight: TrajectoryAxis::GovernanceSignal.default_weight(),
        rationale: format!("blocking={} warning={}", blocking, warning),
    }
}

fn composite_score(axes: &[AxisBreakdown]) -> f64 {
    let weight_sum: f64 = axes.iter().map(|a| a.weight).sum();
    if weight_sum <= 0.0 {
        return 0.0;
    }
    let weighted_sum: f64 = axes.iter().map(|a| a.value * a.weight).sum();
    (weighted_sum / weight_sum).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::{
        AggregateMetrics, BlockingFailure, EvidenceBundle, HarnessRunReport, Severity,
        TaskOutcome, TaskRunResult, HARNESS_RUN_REPORT_VERSION,
    };
    use chrono::Utc;

    fn report_with(outcome: TaskOutcome, aggregate: AggregateMetrics) -> HarnessRunReport {
        let now = Utc::now();
        HarnessRunReport {
            report_version: HARNESS_RUN_REPORT_VERSION.to_string(),
            run_id: "r1".into(),
            label: None,
            session_id: None,
            project_id: None,
            started_at: now,
            ended_at: now,
            task: TaskRunResult {
                task_id: "t1".into(),
                outcome,
                turn_count: aggregate.turns_completed,
                last_turn_succeeded: matches!(outcome, TaskOutcome::Success),
                error_summary: None,
            },
            aggregate,
            blocking_failures: Vec::new(),
            evidence: EvidenceBundle::default(),
        }
    }

    #[test]
    fn perfect_run_scores_near_one() {
        let agg = AggregateMetrics {
            turns_completed: 3,
            turns_succeeded: 3,
            tool_calls_total: 4,
            tool_successes_by_name: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("bash".into(), 4);
                m
            },
            ..Default::default()
        };
        let r = report_with(TaskOutcome::Success, agg);
        let s = score_run_report(&r);
        assert!(s.composite > 0.9, "got composite={}", s.composite);
        assert!(!s.is_low_quality);
    }

    #[test]
    fn failed_run_scores_low() {
        let r = report_with(TaskOutcome::Failed, AggregateMetrics::default());
        let s = score_run_report(&r);
        assert!(s.composite < 0.5, "got composite={}", s.composite);
        assert!(s.is_low_quality);
    }

    #[test]
    fn blocking_failures_drag_governance_axis_down() {
        let mut r = report_with(TaskOutcome::Success, AggregateMetrics::default());
        r.blocking_failures.push(BlockingFailure {
            code: "tool_failure".into(),
            severity: Severity::Blocking,
            message: "boom".into(),
            evidence_ref: None,
            observed_at: Utc::now(),
        });
        let s = score_run_report(&r);
        let gov = s.axis(TrajectoryAxis::GovernanceSignal).unwrap();
        assert!(gov.value < 0.7, "got governance={}", gov.value);
    }

    #[test]
    fn compare_score_detects_regression() {
        let baseline = report_with(
            TaskOutcome::Success,
            AggregateMetrics {
                turns_completed: 5,
                turns_succeeded: 5,
                ..Default::default()
            },
        );
        let candidate = report_with(TaskOutcome::Failed, AggregateMetrics::default());
        let cs = score_compare(&baseline, &candidate);
        assert!(cs.regression);
        assert!(!cs.improvement);
        assert!(cs.composite_delta < -0.05);
    }
}
