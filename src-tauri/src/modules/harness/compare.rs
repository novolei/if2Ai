//! Baseline-vs-candidate report compare foundation (Phase M4.5).
//!
//! Pure function that takes two [`super::run_report::HarnessRunReport`]
//! and produces a typed [`BaselineVsCandidate`] diff.  No
//! recommendation engine, no weighted score, no
//! `promote / hold / reject` decision — those land in M4.8.
//!
//! Honest scope:
//!
//! - **Version compatibility** is checked first.  Mismatched
//!   `report_version`s are not silently coerced — the compare
//!   either returns
//!   [`ReportVersionCompatibility::Identical`] /
//!   [`ReportVersionCompatibility::DifferentMinor`] (allowed) or
//!   refuses with [`CompareError::IncompatibleReportVersion`].
//! - **Grader verdicts** are computed inside `compare_reports` by
//!   running [`super::graders::run_all`] on each report — the
//!   reports themselves don't carry verdicts (kept as a
//!   contract simplicity decision; compare is the consumer that
//!   needs them).
//! - **Aggregate / blocking failures / evidence** diffs are
//!   minimal — just enough for M4.8 gate to read regression
//!   signal.  Detailed per-event diff (e.g. which exact
//!   `MemoryAfterTurn` envelope changed) is out of scope.
//!
//! Out of scope for M4.5:
//!
//! - Multi-run / suite compare (M4.7 corpus territory).
//! - Recommendation engine (`promote / hold / reject`) — M4.8.
//! - Persisted compare history.
//! - UI rendering (M4.6).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::graders::{run_all as run_all_graders, GraderId, GraderVerdict};
#[cfg(test)]
use super::run_report::HARNESS_RUN_REPORT_VERSION;
use super::run_report::{AggregateMetrics, BlockingFailure, HarnessRunReport, Severity};

/// Stable diff contract version.  Bumping is breaking for M4.8
/// gate consumers.
pub const HARNESS_COMPARE_VERSION: &str = "harness-compare@m4.5";

// ───────────────────────── Version compatibility ──────────────────

/// How the two reports' `report_version` strings relate.  Pinned
/// here so M4.8 gate has a typed answer instead of comparing strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportVersionCompatibility {
    /// `baseline.report_version == candidate.report_version`.
    /// Compare is fully comparable.
    Identical,
    /// Versions differ but share the same major bucket
    /// (`harness-run-report@m4.X` family).  M4-D allows this so
    /// minor M4 evolution does not block compare; M4.8 gate may
    /// still demote the resulting verdict.
    DifferentMinor { baseline: String, candidate: String },
}

/// Hard error from [`compare_reports`].  Today only one variant —
/// non-M4 reports (or a future major bump) refuse compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum CompareError {
    IncompatibleReportVersion {
        baseline: String,
        candidate: String,
        reason: String,
    },
}

impl std::fmt::Display for CompareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncompatibleReportVersion {
                baseline,
                candidate,
                reason,
            } => write!(
                f,
                "incompatible report versions (baseline={baseline}, candidate={candidate}): {reason}"
            ),
        }
    }
}

impl std::error::Error for CompareError {}

fn report_family(version: &str) -> Option<&str> {
    // Family discriminator: text before the first `@m` token.
    // E.g. `harness-run-report@m4.4` → `harness-run-report@m4`.
    let idx = version.find('@')?;
    let suffix = &version[idx + 1..];
    let dot = suffix.find('.')?;
    let major = &suffix[..dot];
    Some(version.get(..idx + 1 + major.len()).unwrap_or(version))
}

fn check_version_compatibility(
    baseline: &str,
    candidate: &str,
) -> Result<ReportVersionCompatibility, CompareError> {
    if baseline == candidate {
        return Ok(ReportVersionCompatibility::Identical);
    }
    let bf = report_family(baseline);
    let cf = report_family(candidate);
    match (bf, cf) {
        (Some(b), Some(c)) if b == c => Ok(ReportVersionCompatibility::DifferentMinor {
            baseline: baseline.to_string(),
            candidate: candidate.to_string(),
        }),
        _ => Err(CompareError::IncompatibleReportVersion {
            baseline: baseline.to_string(),
            candidate: candidate.to_string(),
            reason: "report family mismatch (different major version)".to_string(),
        }),
    }
}

// ───────────────────────── Diff sub-shapes ────────────────────────

/// Numeric diff for one [`AggregateMetrics`] field.  Held
/// separately from the full [`AggregateMetrics`] structs (which
/// are also embedded in [`AggregateDiff`]) so M4.8 gate can read
/// regression signal without re-walking the whole struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateDiff {
    /// Verbatim baseline / candidate aggregate snapshots so
    /// reviewers can render full context.
    pub baseline: AggregateMetrics,
    pub candidate: AggregateMetrics,
    /// Selected highlight deltas (`candidate - baseline`).  Negative
    /// values mean candidate did *less* than baseline.  M4.8 gate
    /// will codify which direction is "regression" per metric;
    /// this struct intentionally does not editorialise.
    pub turns_completed_delta: i64,
    pub turns_succeeded_delta: i64,
    pub llm_calls_delta: i64,
    pub input_tokens_total_delta: i64,
    pub output_tokens_total_delta: i64,
    pub tool_calls_total_delta: i64,
    pub permission_prompts_delta: i64,
    pub stream_errors_delta: i64,
    pub memory_decision_count_delta: i64,
    pub memory_accepted_count_delta: i64,
    pub memory_rejected_count_delta: i64,
    pub memory_warning_count_delta: i64,
    pub memory_conflict_prompt_count_delta: i64,
    pub prepare_step_total_delta: i64,
    pub prepare_step_denied_delta: i64,
    pub execution_mode_judgments_delta: i64,
    pub permission_resolved_allow_delta: i64,
    pub permission_resolved_deny_delta: i64,
}

/// Diff of one blocking-failure code.  Aggregated by `code` since
/// the same failure may fire multiple times per run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockerDiff {
    pub code: String,
    pub baseline_count: u64,
    pub candidate_count: u64,
    /// `candidate_count as i64 - baseline_count as i64`.
    pub delta: i64,
    /// Highest severity observed across baseline + candidate runs
    /// for this code.  Ordered by [`severity_rank`] (Blocking
    /// highest, Skeleton lowest).
    pub max_severity: Severity,
    /// `true` iff `delta > 0` — candidate introduced **new**
    /// occurrences of this failure code.  M4.8 gate will use this
    /// to refuse promotion when `regression && severity ==
    /// Blocking`.
    pub regression: bool,
}

/// Diff of one grader's verdict between baseline and candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraderVerdictDiff {
    pub grader_id: GraderId,
    pub baseline_severity: Severity,
    pub candidate_severity: Severity,
    pub baseline_reason_codes: Vec<String>,
    pub candidate_reason_codes: Vec<String>,
    /// `true` iff `severity_rank(candidate) > severity_rank(baseline)`.
    pub regression: bool,
}

/// Per-evidence-vector length diff.  Per-event diff is out of
/// scope for M4.5; consumers that need it can re-walk the
/// underlying `evidence` vectors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VecLengthDiff {
    pub baseline: usize,
    pub candidate: usize,
    pub delta: i64,
}

/// Compact summary of how the two reports' evidence bundles
/// differ in size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSummaryDiff {
    pub memory_after_turn: VecLengthDiff,
    pub prepare_step: VecLengthDiff,
    pub execution_mode: VecLengthDiff,
    pub permission_prompts: VecLengthDiff,
    pub permission_resolved: VecLengthDiff,
    pub stream_errors: VecLengthDiff,
    pub resume_invocations: VecLengthDiff,
}

// ───────────────────────── Top-level compare ──────────────────────

/// One typed `baseline → candidate` diff.  Held by value so it can
/// travel through M4.6 IPC + M4.8 gate without sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaselineVsCandidate {
    pub diff_version: String,
    pub baseline_run_id: String,
    pub candidate_run_id: String,
    pub baseline_report_version: String,
    pub candidate_report_version: String,
    pub version_compatibility: ReportVersionCompatibility,
    pub aggregate: AggregateDiff,
    /// Per-blocking-failure-code diff, sorted by `code` for stable
    /// serialization.
    pub blocking_failures: Vec<BlockerDiff>,
    /// Per-grader verdict diff in stable [`GraderId`] declaration
    /// order.
    pub grader_verdicts: Vec<GraderVerdictDiff>,
    pub evidence_summary: EvidenceSummaryDiff,
    /// `true` iff **any** of (`grader_verdicts.regression`,
    /// `blocking_failures.regression`) is true.  Coarse signal —
    /// M4.8 gate may still classify the regression as
    /// non-promotion-blocking based on severity.
    pub any_regression: bool,
}

/// Run two reports through the M4.5 compare foundation.  Returns
/// either a typed [`BaselineVsCandidate`] diff or a typed
/// [`CompareError`] when the compare cannot proceed (today: only
/// version family mismatch).
pub fn compare_reports(
    baseline: &HarnessRunReport,
    candidate: &HarnessRunReport,
) -> Result<BaselineVsCandidate, CompareError> {
    let version_compatibility =
        check_version_compatibility(&baseline.report_version, &candidate.report_version)?;

    let baseline_grades = run_all_graders(baseline);
    let candidate_grades = run_all_graders(candidate);
    let grader_verdicts = diff_grader_verdicts(&baseline_grades, &candidate_grades);

    let blocking_failures =
        diff_blocking_failures(&baseline.blocking_failures, &candidate.blocking_failures);

    let aggregate = diff_aggregate(&baseline.aggregate, &candidate.aggregate);

    let evidence_summary = EvidenceSummaryDiff {
        memory_after_turn: vec_length_diff(
            baseline.evidence.memory_after_turn.len(),
            candidate.evidence.memory_after_turn.len(),
        ),
        prepare_step: vec_length_diff(
            baseline.evidence.prepare_step.len(),
            candidate.evidence.prepare_step.len(),
        ),
        execution_mode: vec_length_diff(
            baseline.evidence.execution_mode.len(),
            candidate.evidence.execution_mode.len(),
        ),
        permission_prompts: vec_length_diff(
            baseline.evidence.permission_prompts.len(),
            candidate.evidence.permission_prompts.len(),
        ),
        permission_resolved: vec_length_diff(
            baseline.evidence.permission_resolved.len(),
            candidate.evidence.permission_resolved.len(),
        ),
        stream_errors: vec_length_diff(
            baseline.evidence.stream_errors.len(),
            candidate.evidence.stream_errors.len(),
        ),
        resume_invocations: vec_length_diff(
            baseline.evidence.resume_invocations.len(),
            candidate.evidence.resume_invocations.len(),
        ),
    };

    let any_regression = grader_verdicts.iter().any(|g| g.regression)
        || blocking_failures.iter().any(|b| b.regression);

    Ok(BaselineVsCandidate {
        diff_version: HARNESS_COMPARE_VERSION.to_string(),
        baseline_run_id: baseline.run_id.clone(),
        candidate_run_id: candidate.run_id.clone(),
        baseline_report_version: baseline.report_version.clone(),
        candidate_report_version: candidate.report_version.clone(),
        version_compatibility,
        aggregate,
        blocking_failures,
        grader_verdicts,
        evidence_summary,
        any_regression,
    })
}

// ───────────────────────── Internal helpers ───────────────────────

fn diff_aggregate(b: &AggregateMetrics, c: &AggregateMetrics) -> AggregateDiff {
    AggregateDiff {
        baseline: b.clone(),
        candidate: c.clone(),
        turns_completed_delta: c.turns_completed as i64 - b.turns_completed as i64,
        turns_succeeded_delta: c.turns_succeeded as i64 - b.turns_succeeded as i64,
        llm_calls_delta: c.llm_calls as i64 - b.llm_calls as i64,
        input_tokens_total_delta: c.input_tokens_total as i64 - b.input_tokens_total as i64,
        output_tokens_total_delta: c.output_tokens_total as i64 - b.output_tokens_total as i64,
        tool_calls_total_delta: c.tool_calls_total as i64 - b.tool_calls_total as i64,
        permission_prompts_delta: c.permission_prompts as i64 - b.permission_prompts as i64,
        stream_errors_delta: c.stream_errors as i64 - b.stream_errors as i64,
        memory_decision_count_delta: c.memory_decision_count as i64
            - b.memory_decision_count as i64,
        memory_accepted_count_delta: c.memory_accepted_count as i64
            - b.memory_accepted_count as i64,
        memory_rejected_count_delta: c.memory_rejected_count as i64
            - b.memory_rejected_count as i64,
        memory_warning_count_delta: c.memory_warning_count as i64 - b.memory_warning_count as i64,
        memory_conflict_prompt_count_delta: c.memory_conflict_prompt_count as i64
            - b.memory_conflict_prompt_count as i64,
        prepare_step_total_delta: c.prepare_step_total as i64 - b.prepare_step_total as i64,
        prepare_step_denied_delta: c.prepare_step_denied as i64 - b.prepare_step_denied as i64,
        execution_mode_judgments_delta: c.execution_mode_judgments as i64
            - b.execution_mode_judgments as i64,
        permission_resolved_allow_delta: c.permission_resolved_allow as i64
            - b.permission_resolved_allow as i64,
        permission_resolved_deny_delta: c.permission_resolved_deny as i64
            - b.permission_resolved_deny as i64,
    }
}

fn diff_blocking_failures(
    baseline: &[BlockingFailure],
    candidate: &[BlockingFailure],
) -> Vec<BlockerDiff> {
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Bucket {
        baseline_count: u64,
        candidate_count: u64,
        max_severity: Option<Severity>,
    }
    let mut buckets: BTreeMap<String, Bucket> = BTreeMap::new();
    for f in baseline {
        let entry = buckets.entry(f.code.clone()).or_default();
        entry.baseline_count += 1;
        entry.max_severity = Some(merge_severity(entry.max_severity, f.severity));
    }
    for f in candidate {
        let entry = buckets.entry(f.code.clone()).or_default();
        entry.candidate_count += 1;
        entry.max_severity = Some(merge_severity(entry.max_severity, f.severity));
    }
    buckets
        .into_iter()
        .map(|(code, bucket)| {
            let delta = bucket.candidate_count as i64 - bucket.baseline_count as i64;
            BlockerDiff {
                code,
                baseline_count: bucket.baseline_count,
                candidate_count: bucket.candidate_count,
                delta,
                max_severity: bucket.max_severity.unwrap_or(Severity::Info),
                regression: delta > 0,
            }
        })
        .collect()
}

fn diff_grader_verdicts(
    baseline: &[GraderVerdict],
    candidate: &[GraderVerdict],
) -> Vec<GraderVerdictDiff> {
    use std::collections::BTreeMap;
    let bmap: BTreeMap<GraderId, &GraderVerdict> =
        baseline.iter().map(|v| (v.grader_id, v)).collect();
    let cmap: BTreeMap<GraderId, &GraderVerdict> =
        candidate.iter().map(|v| (v.grader_id, v)).collect();
    // Stable order: walk the full GraderId alphabet via candidate
    // first, then any baseline-only ids that don't appear in
    // candidate (defensive — both maps should have the same key
    // set today since `run_all_graders` is deterministic).
    let mut out: Vec<GraderVerdictDiff> = Vec::with_capacity(candidate.len());
    let mut seen: std::collections::BTreeSet<GraderId> = Default::default();
    for v in candidate {
        let baseline_v = bmap.get(&v.grader_id).copied();
        let baseline_severity = baseline_v.map(|b| b.severity).unwrap_or(Severity::Skeleton);
        let baseline_reason_codes = baseline_v
            .map(|b| b.reason_codes.clone())
            .unwrap_or_default();
        let regression = severity_rank(v.severity) > severity_rank(baseline_severity);
        out.push(GraderVerdictDiff {
            grader_id: v.grader_id,
            baseline_severity,
            candidate_severity: v.severity,
            baseline_reason_codes,
            candidate_reason_codes: v.reason_codes.clone(),
            regression,
        });
        seen.insert(v.grader_id);
    }
    for v in baseline {
        if !seen.contains(&v.grader_id) {
            // Defensive: baseline had a grader candidate didn't —
            // flag candidate as Skeleton with no codes.
            out.push(GraderVerdictDiff {
                grader_id: v.grader_id,
                baseline_severity: v.severity,
                candidate_severity: Severity::Skeleton,
                baseline_reason_codes: v.reason_codes.clone(),
                candidate_reason_codes: Vec::new(),
                regression: false,
            });
        }
    }
    out.sort_by_key(|d| d.grader_id);
    let _ = cmap; // used implicitly via candidate vector iteration order
    out
}

fn vec_length_diff(baseline: usize, candidate: usize) -> VecLengthDiff {
    VecLengthDiff {
        baseline,
        candidate,
        delta: candidate as i64 - baseline as i64,
    }
}

/// Severity precedence for regression detection.  Higher rank ==
/// worse.  Must stay in sync with the `graders` module.
fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Blocking => 4,
        Severity::Warning => 3,
        Severity::Info => 2,
        Severity::Pass => 1,
        Severity::Skeleton => 0,
    }
}

fn merge_severity(prev: Option<Severity>, new: Severity) -> Severity {
    match prev {
        None => new,
        Some(p) if severity_rank(new) > severity_rank(p) => new,
        Some(p) => p,
    }
}

/// Re-export the report-version constant so consumers that
/// only depend on `super::compare` can refuse mismatched reports
/// without pulling in `super::run_report`.
pub const COMPARE_REPORT_VERSION_FAMILY_PREFIX: &str = "harness-run-report@m4";

/// Sanity check the constant against the live report version.
#[cfg(test)]
fn _assert_family_in_sync() {
    debug_assert!(
        HARNESS_RUN_REPORT_VERSION.starts_with(COMPARE_REPORT_VERSION_FAMILY_PREFIX),
        "report version family changed; update compare module"
    );
}

#[cfg(test)]
mod tests {
    use super::super::run_report::{
        HarnessRunReport, MemoryAfterTurnTrace, Severity as S, TaskOutcome,
    };
    use super::*;
    use crate::modules::application::{
        ConflictResolution, ConflictResolutionOutcome, QualityGateResult,
        MEMORY_CONFLICT_RESOLVER_VERSION, MEMORY_QUALITY_GATE_VERSION,
    };
    use crate::modules::runtime::contracts::memory::{
        MemoryObjectKind, MemoryScope, MemoryWriteDecision, MemoryWriteDisposition,
    };
    use chrono::Utc;

    fn empty_report(run_id: &str) -> HarnessRunReport {
        HarnessRunReport::new_empty(run_id, Utc::now())
    }

    fn synth_decision(disp: MemoryWriteDisposition) -> MemoryWriteDecision {
        MemoryWriteDecision {
            disposition: disp,
            reason_codes: Vec::new(),
            object_kind: MemoryObjectKind::Fact,
            scope: MemoryScope::Session,
            evidence_id: None,
            policy_version: "memory-write-policy@m3.3-skeleton".to_string(),
            decided_at: "2026-04-20T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn identical_versions_produce_identical_compatibility() {
        let b = empty_report("r1");
        let c = empty_report("r2");
        let diff = compare_reports(&b, &c).expect("compare ok");
        assert!(matches!(
            diff.version_compatibility,
            ReportVersionCompatibility::Identical
        ));
        assert!(!diff.any_regression);
        assert_eq!(diff.diff_version, HARNESS_COMPARE_VERSION);
    }

    #[test]
    fn aggregate_deltas_are_signed_correctly() {
        let mut b = empty_report("r1");
        let mut c = empty_report("r2");
        b.aggregate.turns_completed = 4;
        b.aggregate.input_tokens_total = 100;
        c.aggregate.turns_completed = 6;
        c.aggregate.input_tokens_total = 80;
        let diff = compare_reports(&b, &c).unwrap();
        assert_eq!(diff.aggregate.turns_completed_delta, 2);
        assert_eq!(diff.aggregate.input_tokens_total_delta, -20);
    }

    #[test]
    fn blocking_failure_diff_buckets_by_code_and_flags_regression() {
        let mut b = empty_report("r1");
        let mut c = empty_report("r2");
        b.blocking_failures.push(BlockingFailure {
            code: "tool_failure".to_string(),
            severity: S::Warning,
            message: "x".to_string(),
            evidence_ref: None,
            observed_at: Utc::now(),
        });
        c.blocking_failures.push(BlockingFailure {
            code: "tool_failure".to_string(),
            severity: S::Warning,
            message: "x".to_string(),
            evidence_ref: None,
            observed_at: Utc::now(),
        });
        c.blocking_failures.push(BlockingFailure {
            code: "tool_failure".to_string(),
            severity: S::Warning,
            message: "y".to_string(),
            evidence_ref: None,
            observed_at: Utc::now(),
        });
        c.blocking_failures.push(BlockingFailure {
            code: "memory_write_denied".to_string(),
            severity: S::Blocking,
            message: "z".to_string(),
            evidence_ref: None,
            observed_at: Utc::now(),
        });
        let diff = compare_reports(&b, &c).unwrap();
        let by_code: std::collections::BTreeMap<_, _> = diff
            .blocking_failures
            .iter()
            .map(|d| (&d.code, d))
            .collect();
        let tool = by_code.get(&"tool_failure".to_string()).unwrap();
        assert_eq!(tool.baseline_count, 1);
        assert_eq!(tool.candidate_count, 2);
        assert_eq!(tool.delta, 1);
        assert!(tool.regression);
        let mem = by_code.get(&"memory_write_denied".to_string()).unwrap();
        assert_eq!(mem.delta, 1);
        assert_eq!(mem.max_severity, S::Blocking);
        assert!(diff.any_regression);
    }

    #[test]
    fn grader_verdict_diff_detects_regression_and_improvement() {
        // Baseline: clean memory envelope → MemoryAlignment Pass.
        // Candidate: deny envelope → MemoryAlignment Blocking.
        let mut b = empty_report("baseline");
        b.aggregate.turns_completed = 1;
        b.aggregate.turns_succeeded = 1;
        b.task.outcome = TaskOutcome::Success;
        b.task.last_turn_succeeded = true;
        b.evidence.memory_after_turn.push(MemoryAfterTurnTrace {
            trace_version: "memory-after-turn-trace@m4.1".to_string(),
            caller: "t".to_string(),
            session_id: None,
            project_id: None,
            policy_version: "memory-write-policy@m3.3-skeleton".to_string(),
            decided_at: Utc::now(),
            decisions: vec![synth_decision(MemoryWriteDisposition::Allow)],
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
        });
        let mut c = empty_report("candidate");
        c.aggregate.turns_completed = 1;
        c.aggregate.turns_succeeded = 1;
        c.task.outcome = TaskOutcome::Success;
        c.task.last_turn_succeeded = true;
        c.evidence.memory_after_turn.push(MemoryAfterTurnTrace {
            trace_version: "memory-after-turn-trace@m4.1".to_string(),
            caller: "t".to_string(),
            session_id: None,
            project_id: None,
            policy_version: "memory-write-policy@m3.3-skeleton".to_string(),
            decided_at: Utc::now(),
            decisions: vec![synth_decision(MemoryWriteDisposition::Deny)],
            quality: QualityGateResult {
                accepted: Vec::new(),
                rejected: Vec::new(),
                warnings: Vec::new(),
                policy_version: MEMORY_QUALITY_GATE_VERSION.to_string(),
            },
            conflicts: Vec::new(),
        });
        let diff = compare_reports(&b, &c).unwrap();
        let mem = diff
            .grader_verdicts
            .iter()
            .find(|g| g.grader_id == GraderId::MemoryAlignment)
            .unwrap();
        assert_eq!(mem.baseline_severity, S::Pass);
        assert_eq!(mem.candidate_severity, S::Blocking);
        assert!(mem.regression);
        assert!(diff.any_regression);
    }

    #[test]
    fn evidence_summary_diff_records_length_deltas() {
        let mut b = empty_report("r1");
        let mut c = empty_report("r2");
        b.evidence.memory_after_turn.push(MemoryAfterTurnTrace {
            trace_version: "v".to_string(),
            caller: "t".to_string(),
            session_id: None,
            project_id: None,
            policy_version: "v".to_string(),
            decided_at: Utc::now(),
            decisions: Vec::new(),
            quality: QualityGateResult {
                accepted: Vec::new(),
                rejected: Vec::new(),
                warnings: Vec::new(),
                policy_version: "v".to_string(),
            },
            conflicts: Vec::new(),
        });
        for _ in 0..3 {
            c.evidence.memory_after_turn.push(MemoryAfterTurnTrace {
                trace_version: "v".to_string(),
                caller: "t".to_string(),
                session_id: None,
                project_id: None,
                policy_version: "v".to_string(),
                decided_at: Utc::now(),
                decisions: Vec::new(),
                quality: QualityGateResult {
                    accepted: Vec::new(),
                    rejected: Vec::new(),
                    warnings: Vec::new(),
                    policy_version: "v".to_string(),
                },
                conflicts: Vec::new(),
            });
        }
        let diff = compare_reports(&b, &c).unwrap();
        assert_eq!(diff.evidence_summary.memory_after_turn.delta, 2);
        assert_eq!(diff.evidence_summary.memory_after_turn.baseline, 1);
        assert_eq!(diff.evidence_summary.memory_after_turn.candidate, 3);
    }

    #[test]
    fn incompatible_major_version_refuses_compare() {
        let mut b = empty_report("r1");
        let mut c = empty_report("r2");
        b.report_version = "harness-run-report@m4.2".to_string();
        c.report_version = "harness-run-report@m5.0".to_string();
        let err = compare_reports(&b, &c).unwrap_err();
        match err {
            CompareError::IncompatibleReportVersion {
                baseline,
                candidate,
                ..
            } => {
                assert_eq!(baseline, "harness-run-report@m4.2");
                assert_eq!(candidate, "harness-run-report@m5.0");
            }
        }
    }

    #[test]
    fn different_minor_versions_allow_compare() {
        let mut b = empty_report("r1");
        let mut c = empty_report("r2");
        b.report_version = "harness-run-report@m4.2".to_string();
        c.report_version = "harness-run-report@m4.4".to_string();
        let diff = compare_reports(&b, &c).unwrap();
        assert!(matches!(
            diff.version_compatibility,
            ReportVersionCompatibility::DifferentMinor { .. }
        ));
    }
}
