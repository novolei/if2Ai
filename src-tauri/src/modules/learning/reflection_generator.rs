//! Structured reflection generation (Phase M5 m5.3, closeout).
//!
//! Real producer of [`ReflectionNote`]s from harness evidence
//! (run report + trajectory score + clustered failures), in
//! contrast to the legacy [`super::reflection_note::from_legacy_reflection`]
//! adapter which only translates pattern strings.
//!
//! Honest scope:
//!
//! - **Pure compute**.  Inputs are typed (run report + score +
//!   cluster set); output is a `Vec<ReflectionNote>` (zero or
//!   more notes).
//! - **Trigger policy**: the generator emits a note when:
//!   1. trajectory score is "low quality"
//!      ([`TrajectoryScore::is_low_quality`]), AND
//!   2. there is at least one classified failure cluster.
//!   Both must be true so a healthy run does not generate
//!   spurious candidate hints.
//! - **Auto producer mount point**: the recommended hookpoint
//!   is the per-session "session_end" event (one reflection
//!   pass per session, NOT per turn — per-turn would generate
//!   too many candidates and bloat the registry).  The
//!   `agent.rs::dispatch_after_turn` path remains a manual
//!   hand-off; the formal automatic pipeline runs when the
//!   harness produces a finalised report (because that is the
//!   first point at which all evidence is available).  Tests +
//!   the `learning_reflect_session_and_register` IPC are the
//!   auto-trigger surface for now.
//! - **No active strategy mutation**: every note is still a
//!   candidate hint; the candidate registry + promotion gate
//!   own the decision to ship.
//! - Closed mapping from [`FailureCategory`] →
//!   [`ReflectionIssueType`] so issue routing is auditable.

#![allow(dead_code)]

use chrono::Utc;
use uuid::Uuid;

use super::failure_clustering::{cluster_failures, ClusteredFailureSet, FailureCluster};
use super::failure_taxonomy::FailureCategory;
use super::reflection_note::{
    ReflectionEvidenceRef, ReflectionIssueType, ReflectionNote, REFLECTION_NOTE_VERSION,
};
use super::trajectory_score::{score_run_report, TrajectoryAxis, TrajectoryScore};
use crate::modules::harness::HarnessRunReport;
use crate::modules::runtime::contracts::execution_mode::RiskLevel;

/// Stable contract version for the generator output policy.
pub const REFLECTION_GENERATOR_VERSION: &str = "reflection-generator@m5.3";

/// Result bundle: every generated note + the scoring + cluster
/// inputs that drove the decision (so callers / tests can show
/// "why was this note emitted").
#[derive(Debug, Clone)]
pub struct ReflectionGeneration {
    pub generator_version: &'static str,
    pub trajectory_score: TrajectoryScore,
    pub clusters: ClusteredFailureSet,
    pub notes: Vec<ReflectionNote>,
}

/// Pure entry point.  Consumes one [`HarnessRunReport`] and
/// returns whatever notes the trigger policy produces (may be
/// empty).  This is the M5 closeout replacement for the
/// legacy adapter.
#[must_use]
pub fn generate_reflection_notes(report: &HarnessRunReport) -> ReflectionGeneration {
    let score = score_run_report(report);
    let clusters = cluster_failures(&[report]);
    let mut notes: Vec<ReflectionNote> = Vec::new();
    if score.is_low_quality && !clusters.clusters.is_empty() {
        for cluster in &clusters.clusters {
            if let Some(note) = note_from_cluster(report, &score, cluster) {
                notes.push(note);
            }
        }
    }
    ReflectionGeneration {
        generator_version: REFLECTION_GENERATOR_VERSION,
        trajectory_score: score,
        clusters,
        notes,
    }
}

fn note_from_cluster(
    report: &HarnessRunReport,
    score: &TrajectoryScore,
    cluster: &FailureCluster,
) -> Option<ReflectionNote> {
    let issue_type = map_category_to_issue(cluster.category);
    let summary = format!(
        "{} cluster ({} failures, max severity {:?}); composite score {:.2}",
        cluster.category.label(),
        cluster.total_failures,
        cluster.max_severity,
        score.composite,
    );
    // Build evidence refs from the top signatures in the
    // cluster — keep at most 3 so registries stay small.
    let evidence: Vec<ReflectionEvidenceRef> = cluster
        .signatures
        .iter()
        .take(3)
        .map(|sig| ReflectionEvidenceRef {
            trace_id: sig.run_id.clone(),
            kind: format!("blocking_failure:{}", sig.code),
            excerpt: Some(format!(
                "occurrences={} severity={:?}",
                sig.occurrences, sig.max_severity
            )),
        })
        .collect();
    if evidence.is_empty() {
        return None;
    }
    // Risk level: blocking-severe clusters bump to Medium;
    // warning-only stays Low.
    let risk_level = match cluster.max_severity {
        crate::modules::harness::Severity::Blocking => RiskLevel::Medium,
        _ => RiskLevel::Low,
    };
    // Expected gain: derived from the lowest-scoring axis so
    // reviewers see "fix would lift X axis".
    let expected_gain = lowest_axis_label(score).map(|l| format!("lift_{l}"));
    Some(ReflectionNote {
        note_id: format!("gen:{}:{}", cluster.category.label(), Uuid::new_v4()),
        issue_type,
        summary,
        evidence,
        proposed_strategy: None,
        expected_gain,
        risk_level,
        created_at: Utc::now().to_rfc3339(),
        contract_version: REFLECTION_NOTE_VERSION.to_string(),
    })
    .filter(|_| !report.run_id.is_empty())
}

fn map_category_to_issue(c: FailureCategory) -> ReflectionIssueType {
    match c {
        FailureCategory::IntentMiss => ReflectionIssueType::Other,
        FailureCategory::PolicyIssue => ReflectionIssueType::Other,
        FailureCategory::MemoryIssue => ReflectionIssueType::MissedRecall,
        FailureCategory::RecoveryIssue => ReflectionIssueType::RepeatedFailure,
        FailureCategory::ToolMisuse => ReflectionIssueType::ToolMismatch,
        FailureCategory::Other => ReflectionIssueType::Other,
    }
}

fn lowest_axis_label(score: &TrajectoryScore) -> Option<&'static str> {
    score
        .axes
        .iter()
        .min_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))
        .map(|a| match a.axis {
            TrajectoryAxis::Completion => "completion",
            TrajectoryAxis::RecoveryQuality => "recovery_quality",
            TrajectoryAxis::ToolQuality => "tool_quality",
            TrajectoryAxis::MemoryAlignment => "memory_alignment",
            TrajectoryAxis::GovernanceSignal => "governance_signal",
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::{
        AggregateMetrics, BlockingFailure, EvidenceBundle, HarnessRunReport, Severity,
        TaskOutcome, TaskRunResult, HARNESS_RUN_REPORT_VERSION,
    };
    use chrono::Utc;

    fn make_report(outcome: TaskOutcome, failures: Vec<BlockingFailure>) -> HarnessRunReport {
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
                turn_count: 1,
                last_turn_succeeded: matches!(outcome, TaskOutcome::Success),
                error_summary: None,
            },
            aggregate: AggregateMetrics::default(),
            blocking_failures: failures,
            evidence: EvidenceBundle::default(),
        }
    }

    fn fail(code: &str, sev: Severity) -> BlockingFailure {
        BlockingFailure {
            code: code.into(),
            severity: sev,
            message: String::new(),
            evidence_ref: None,
            observed_at: Utc::now(),
        }
    }

    #[test]
    fn healthy_run_emits_no_notes() {
        let r = make_report(TaskOutcome::Success, vec![]);
        let g = generate_reflection_notes(&r);
        assert!(g.notes.is_empty());
    }

    #[test]
    fn failed_run_with_failures_emits_typed_notes() {
        let r = make_report(
            TaskOutcome::Failed,
            vec![
                fail("memory_rejected", Severity::Blocking),
                fail("tool_failure", Severity::Warning),
            ],
        );
        let g = generate_reflection_notes(&r);
        assert!(!g.notes.is_empty());
        // At least one note tagged MissedRecall (memory cluster)
        // and one ToolMismatch (tool cluster).
        let kinds: std::collections::HashSet<_> = g.notes.iter().map(|n| n.issue_type).collect();
        assert!(kinds.contains(&ReflectionIssueType::MissedRecall));
        assert!(kinds.contains(&ReflectionIssueType::ToolMismatch));
        // Every emitted note carries non-empty evidence.
        for n in &g.notes {
            assert!(!n.evidence.is_empty());
            assert!(n.is_well_formed());
        }
    }

    #[test]
    fn failed_run_without_failures_emits_no_notes() {
        // Trigger requires BOTH low-quality score AND clusters.
        let r = make_report(TaskOutcome::Failed, vec![]);
        let g = generate_reflection_notes(&r);
        assert!(g.notes.is_empty());
    }
}
