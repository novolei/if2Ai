//! Upgrade gate policy + Recommendation contract (Phase M4.8).
//!
//! Pure function over `BaselineVsCandidate` (one-pair compare) or
//! `SuiteReport` (corpus-level aggregation): renders a typed
//! [`GateDecision`] (`Promote / Hold / Reject`) with stable
//! reason codes.
//!
//! Honest scope of this module:
//!
//! - **Default policy is hard-coded.**  M4.8 ships one canonical
//!   policy ([`GatePolicy::default`]) — strict, conservative,
//!   refuses promotion on any blocking regression.  Future
//!   phases may load policies from YAML; the contract is shaped
//!   so that lands non-breaking.
//! - **Refuse over allow.**  Per the M4 runbook §5.9 hard rule
//!   "没有 harness recommendation 不可 promote" — when in doubt,
//!   the gate returns `Hold`, never `Promote`.  Only the
//!   explicit `Promote` decision is permission to ship; `Hold`
//!   means "human review required"; `Reject` means "candidate
//!   regressed, do not ship".
//! - **No side effects.**  This module renders a recommendation;
//!   actual promotion (rolling out a new strategy / model)
//!   belongs to M5.
//!
//! Out of scope:
//!
//! - Loading policy from disk (M5+).
//! - Per-corpus-tier weighted policy (one knob today; future
//!   phases may differentiate).
//! - Auto-promote / auto-rollback (M5).
//! - Reading historical recommendations to detect "flapping".

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::compare::{BaselineVsCandidate, BlockerDiff, GraderVerdictDiff};
use super::graders::GraderId;
use super::run_report::Severity;
use super::suite_report::{SuiteGrade, SuiteReport, TaskClassification};

/// Stable gate contract version.  Bumping is breaking for
/// downstream consumers (M5 auto-promote, future audit trail).
pub const HARNESS_GATE_VERSION: &str = "harness-gate@m4.8";

/// Closed-set decision alphabet.  Adding a variant is a breaking
/// contract bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    /// Candidate is safe to ship.  Only emitted when **no**
    /// blocking regressions and **no** uncertain signal.
    Promote,
    /// Candidate may ship after human review.  Emitted on
    /// warning-level regressions or partial signal.
    Hold,
    /// Candidate must NOT ship.  Emitted on blocking regressions
    /// or hard-fail suite grades.
    Reject,
}

impl GateDecision {
    /// Stable wire label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Promote => "promote",
            Self::Hold => "hold",
            Self::Reject => "reject",
        }
    }
}

/// Policy knobs.  M4.8 ships one canonical default; future
/// phases may grow this struct or wrap it in a per-corpus
/// registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatePolicy {
    /// Stable policy id (e.g. `"default-conservative-m4.8"`).
    /// Pinned in every Recommendation so M5 auto-promote can
    /// refuse mismatched policies.
    pub policy_id: String,
    /// Maximum allowable warning-level regressions before the
    /// decision degrades from `Hold` to `Reject`.  M4.8 default:
    /// 5.
    pub max_warning_regressions: usize,
    /// Maximum acceptable suite-level `regression_weight /
    /// total_weight` ratio for `Promote`.  When **>** this in a
    /// Pass-graded suite → `Hold` (degraded from Promote).  When
    /// **>** this in a Warning-graded suite → adds the
    /// `HOLD_TOO_MANY_WARNING_REGRESSIONS` reason code.  Has no
    /// effect on Fail / Skeleton suites (those are already
    /// non-promotable).  M4.8 default: 0.0 (any regression
    /// weight blocks promote — most conservative).
    pub max_promote_regression_weight_ratio: f64,
}

impl GatePolicy {
    /// M4.8 canonical default — conservative.
    #[must_use]
    pub fn default_conservative() -> Self {
        Self {
            policy_id: "default-conservative-m4.8".to_string(),
            max_warning_regressions: 5,
            max_promote_regression_weight_ratio: 0.0,
        }
    }
}

impl Default for GatePolicy {
    fn default() -> Self {
        Self::default_conservative()
    }
}

/// Reason codes the gate may attach to a [`Recommendation`].
/// Closed catalogue here so M5 / governance UI can render
/// stable badges.
pub mod reason_codes {
    pub const PROMOTE_NO_REGRESSION: &str = "promote_no_regression";
    pub const HOLD_GRADER_WARNING_REGRESSION: &str = "hold_grader_warning_regression";
    pub const HOLD_BLOCKING_FAILURE_REGRESSION: &str = "hold_blocking_failure_regression";
    pub const HOLD_SUITE_WARNING: &str = "hold_suite_warning";
    pub const HOLD_TOO_MANY_WARNING_REGRESSIONS: &str = "hold_too_many_warning_regressions";
    pub const HOLD_NO_INPUT: &str = "hold_no_input_for_judgment";
    pub const REJECT_GRADER_BLOCKING: &str = "reject_grader_blocking_regression";
    pub const REJECT_BLOCKING_FAILURE: &str = "reject_blocking_failure_regression";
    pub const REJECT_SUITE_FAIL: &str = "reject_suite_fail";
    pub const REJECT_TIER_ALL_REGRESSION: &str = "reject_tier_full_regression";
    pub const REJECT_REGRESSION_WEIGHT_OVER_LIMIT: &str = "reject_regression_weight_over_limit";
}

/// One typed gate recommendation.  Held by value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub gate_version: String,
    pub policy_id: String,
    pub decision: GateDecision,
    pub reason_codes: Vec<String>,
    /// Short human-readable message.  Reviewers / future UI
    /// render this verbatim.
    pub summary: String,
    /// Grader ids that contributed to a non-Promote decision
    /// (in stable [`GraderId`] declaration order when present).
    pub blocking_grader_ids: Vec<GraderId>,
    /// Blocking-failure codes that contributed to a non-Promote
    /// decision (sorted alphabetically).
    pub blocking_failure_codes: Vec<String>,
    /// Compare-side weighted score.  Currently a coarse signal
    /// (count of blocking + warning regressions) — future
    /// phases may evolve to a continuous metric without changing
    /// the contract.
    pub weighted_score: f64,
}

impl Recommendation {
    fn promote(policy_id: &str, summary: impl Into<String>) -> Self {
        Self {
            gate_version: HARNESS_GATE_VERSION.to_string(),
            policy_id: policy_id.to_string(),
            decision: GateDecision::Promote,
            reason_codes: vec![reason_codes::PROMOTE_NO_REGRESSION.to_string()],
            summary: summary.into(),
            blocking_grader_ids: Vec::new(),
            blocking_failure_codes: Vec::new(),
            weighted_score: 0.0,
        }
    }

    fn hold_no_input(policy_id: &str) -> Self {
        Self {
            gate_version: HARNESS_GATE_VERSION.to_string(),
            policy_id: policy_id.to_string(),
            decision: GateDecision::Hold,
            reason_codes: vec![reason_codes::HOLD_NO_INPUT.to_string()],
            summary: "no compare / suite signal available; hold by default".to_string(),
            blocking_grader_ids: Vec::new(),
            blocking_failure_codes: Vec::new(),
            weighted_score: 0.0,
        }
    }
}

/// Severity precedence for "blocking-vs-warning" detection.  Must
/// stay in sync with the graders module + compare module.
fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Blocking => 4,
        Severity::Warning => 3,
        Severity::Info => 2,
        Severity::Pass => 1,
        Severity::Skeleton => 0,
    }
}

// ───────────────────────── Compare-based gate ─────────────────────

/// Render a recommendation for one baseline-vs-candidate compare
/// result.  Uses the supplied policy (or pass
/// [`GatePolicy::default`] for the M4.8 canonical).
///
/// Decision precedence (highest first):
///
///   1. Any grader regression to `Severity::Blocking` → Reject
///   2. Any blocking-failure regression with `max_severity ==
///      Blocking` → Reject
///   3. Warning regression count > policy.max_warning_regressions
///      → Reject
///   4. Any grader regression at `Warning` → Hold
///   5. Any blocking-failure regression at `Warning` → Hold
///   6. No regression at all → Promote
#[must_use]
pub fn evaluate_compare(diff: &BaselineVsCandidate, policy: &GatePolicy) -> Recommendation {
    let mut reject_codes: Vec<String> = Vec::new();
    let mut hold_codes: Vec<String> = Vec::new();
    let mut blocking_grader_ids: Vec<GraderId> = Vec::new();
    let mut blocking_failure_codes: Vec<String> = Vec::new();

    // (1) Grader regressions split by severity.
    let mut grader_blocking_regressions: Vec<&GraderVerdictDiff> = Vec::new();
    let mut grader_warning_regressions: Vec<&GraderVerdictDiff> = Vec::new();
    for g in &diff.grader_verdicts {
        if !g.regression {
            continue;
        }
        match g.candidate_severity {
            Severity::Blocking => grader_blocking_regressions.push(g),
            Severity::Warning => grader_warning_regressions.push(g),
            _ => {} // Info / Pass / Skeleton regressions never trigger gate
        }
    }

    // (2) Blocking-failure regressions split by max severity.
    let mut blocker_blocking_regressions: Vec<&BlockerDiff> = Vec::new();
    let mut blocker_warning_regressions: Vec<&BlockerDiff> = Vec::new();
    for b in &diff.blocking_failures {
        if !b.regression {
            continue;
        }
        match b.max_severity {
            Severity::Blocking => blocker_blocking_regressions.push(b),
            Severity::Warning => blocker_warning_regressions.push(b),
            _ => {}
        }
    }

    if !grader_blocking_regressions.is_empty() {
        reject_codes.push(reason_codes::REJECT_GRADER_BLOCKING.to_string());
        for g in &grader_blocking_regressions {
            blocking_grader_ids.push(g.grader_id);
        }
    }
    if !blocker_blocking_regressions.is_empty() {
        reject_codes.push(reason_codes::REJECT_BLOCKING_FAILURE.to_string());
        for b in &blocker_blocking_regressions {
            blocking_failure_codes.push(b.code.clone());
        }
    }

    let warning_regression_count =
        grader_warning_regressions.len() + blocker_warning_regressions.len();
    let warning_count_over_limit = warning_regression_count > policy.max_warning_regressions;
    if warning_count_over_limit {
        reject_codes.push(reason_codes::REJECT_REGRESSION_WEIGHT_OVER_LIMIT.to_string());
    } else {
        if !grader_warning_regressions.is_empty() {
            hold_codes.push(reason_codes::HOLD_GRADER_WARNING_REGRESSION.to_string());
        }
        if !blocker_warning_regressions.is_empty() {
            hold_codes.push(reason_codes::HOLD_BLOCKING_FAILURE_REGRESSION.to_string());
        }
    }
    // Reviewer Warning-2 fix: contributing IDs are collected
    // unconditionally so escalated-Reject paths still surface
    // the audit trail (the Hold-vs-Reject distinction is in
    // `decision`, not in `blocking_*_ids`).
    for g in &grader_warning_regressions {
        blocking_grader_ids.push(g.grader_id);
    }
    for b in &blocker_warning_regressions {
        blocking_failure_codes.push(b.code.clone());
    }

    blocking_grader_ids.sort();
    blocking_grader_ids.dedup();
    blocking_failure_codes.sort();
    blocking_failure_codes.dedup();

    let weighted_score = compute_compare_score(
        grader_blocking_regressions.len(),
        grader_warning_regressions.len(),
        blocker_blocking_regressions.len(),
        blocker_warning_regressions.len(),
    );

    let (decision, reason_codes, summary) = if !reject_codes.is_empty() {
        // Reviewer Warning-1 fix: distinguish hard-blocking
        // Reject (grader/blocker at Severity::Blocking) from
        // count-escalation Reject (too many warning regressions).
        let summary = if !grader_blocking_regressions.is_empty()
            || !blocker_blocking_regressions.is_empty()
        {
            format!(
                "candidate REJECTED — {} blocking grader regression(s), {} blocking failure code(s)",
                grader_blocking_regressions.len(),
                blocker_blocking_regressions.len()
            )
        } else {
            format!(
                "candidate REJECTED — {} warning regression(s) exceed policy limit ({})",
                warning_regression_count, policy.max_warning_regressions
            )
        };
        (GateDecision::Reject, reject_codes, summary)
    } else if !hold_codes.is_empty() {
        (
            GateDecision::Hold,
            hold_codes,
            format!(
                "candidate HELD — {} warning regression(s) require human review",
                warning_regression_count
            ),
        )
    } else {
        return Recommendation::promote(
            &policy.policy_id,
            "candidate PROMOTED — no regression observed",
        );
    };

    Recommendation {
        gate_version: HARNESS_GATE_VERSION.to_string(),
        policy_id: policy.policy_id.clone(),
        decision,
        reason_codes,
        summary,
        blocking_grader_ids,
        blocking_failure_codes,
        weighted_score,
    }
}

fn compute_compare_score(
    grader_blocking: usize,
    grader_warning: usize,
    blocker_blocking: usize,
    blocker_warning: usize,
) -> f64 {
    // Coarse weighted: blocking == 10, warning == 1.  Future
    // phases may swap in a continuous metric without changing the
    // Recommendation contract.
    (10 * grader_blocking + grader_warning + 10 * blocker_blocking + blocker_warning) as f64
}

// ───────────────────────── Suite-based gate ───────────────────────

/// Render a recommendation for a suite-level aggregation.
///
/// Decision precedence:
///
///   1. `SuiteGrade::Fail` → Reject
///   2. `SuiteGrade::Skeleton` → Hold (no input)
///   3. `regression_weight / total_weight >
///      policy.max_promote_regression_weight_ratio` → Hold
///   4. `SuiteGrade::Warning` → Hold (any regression observed)
///   5. `SuiteGrade::Pass` → Promote
#[must_use]
pub fn evaluate_suite(suite: &SuiteReport, policy: &GatePolicy) -> Recommendation {
    match suite.grade {
        SuiteGrade::Fail => {
            // Identify which tiers fully regressed for the audit
            // trail; otherwise just emit the suite_fail code.
            let mut codes = vec![reason_codes::REJECT_SUITE_FAIL.to_string()];
            for tier in &suite.tier_summaries {
                let tier_classified = tier.total_tasks - tier.missing - tier.no_expectation;
                if tier_classified > 0 && tier.regression == tier_classified {
                    codes.push(reason_codes::REJECT_TIER_ALL_REGRESSION.to_string());
                    break;
                }
            }
            let weighted = if suite.overall.total_weight > 0.0 {
                suite.overall.regression_weight / suite.overall.total_weight
            } else {
                0.0
            };
            Recommendation {
                gate_version: HARNESS_GATE_VERSION.to_string(),
                policy_id: policy.policy_id.clone(),
                decision: GateDecision::Reject,
                reason_codes: codes,
                summary: format!(
                    "suite '{}' REJECTED — {} regression(s) of {} task(s)",
                    suite.corpus_name, suite.overall.regression, suite.overall.total_tasks
                ),
                blocking_grader_ids: Vec::new(),
                blocking_failure_codes: failed_task_codes(suite),
                weighted_score: weighted,
            }
        }
        SuiteGrade::Skeleton => Recommendation::hold_no_input(&policy.policy_id),
        SuiteGrade::Warning => {
            let weighted = if suite.overall.total_weight > 0.0 {
                suite.overall.regression_weight / suite.overall.total_weight
            } else {
                0.0
            };
            let mut codes = vec![reason_codes::HOLD_SUITE_WARNING.to_string()];
            if weighted > policy.max_promote_regression_weight_ratio {
                codes.push(reason_codes::HOLD_TOO_MANY_WARNING_REGRESSIONS.to_string());
            }
            Recommendation {
                gate_version: HARNESS_GATE_VERSION.to_string(),
                policy_id: policy.policy_id.clone(),
                decision: GateDecision::Hold,
                reason_codes: codes,
                summary: format!(
                    "suite '{}' HELD — {} regression(s) of {} task(s) require review",
                    suite.corpus_name, suite.overall.regression, suite.overall.total_tasks
                ),
                blocking_grader_ids: Vec::new(),
                blocking_failure_codes: failed_task_codes(suite),
                weighted_score: weighted,
            }
        }
        SuiteGrade::Pass => {
            // Reviewer Blocking-1 fix: enforce
            // `max_promote_regression_weight_ratio` here too.
            // SuiteGrade::Pass guarantees `regression == 0` so
            // `regression_weight == 0.0`, which means with the
            // M4.8 default policy (`0.0`) this branch always
            // promotes.  When M5+ relaxes the policy to a
            // non-zero ratio, the field is now wired into the
            // Pass path and the behaviour will match the field
            // documentation.
            let weighted = if suite.overall.total_weight > 0.0 {
                suite.overall.regression_weight / suite.overall.total_weight
            } else {
                0.0
            };
            if weighted > policy.max_promote_regression_weight_ratio {
                let mut codes = vec![
                    reason_codes::HOLD_SUITE_WARNING.to_string(),
                    reason_codes::HOLD_TOO_MANY_WARNING_REGRESSIONS.to_string(),
                ];
                codes.dedup();
                return Recommendation {
                    gate_version: HARNESS_GATE_VERSION.to_string(),
                    policy_id: policy.policy_id.clone(),
                    decision: GateDecision::Hold,
                    reason_codes: codes,
                    summary: format!(
                        "suite '{}' HELD — regression weight ratio {:.3} exceeds policy ({:.3})",
                        suite.corpus_name, weighted, policy.max_promote_regression_weight_ratio
                    ),
                    blocking_grader_ids: Vec::new(),
                    blocking_failure_codes: failed_task_codes(suite),
                    weighted_score: weighted,
                };
            }
            Recommendation::promote(
                &policy.policy_id,
                format!(
                    "suite '{}' PROMOTED — {} task(s) passed cleanly",
                    suite.corpus_name, suite.overall.pass
                ),
            )
        }
    }
}

fn failed_task_codes(suite: &SuiteReport) -> Vec<String> {
    let mut codes: Vec<String> = suite
        .tasks
        .iter()
        .filter(|t| matches!(t.classification, TaskClassification::Regression))
        .flat_map(|t| t.unexpected_blocker_codes.clone())
        .collect();
    codes.sort();
    codes.dedup();
    codes
}

#[cfg(test)]
mod tests {
    use super::super::compare::{
        AggregateDiff, BaselineVsCandidate, BlockerDiff, EvidenceSummaryDiff, GraderVerdictDiff,
        ReportVersionCompatibility, VecLengthDiff,
    };
    use super::super::corpus::{CorpusTask, CorpusTier, RegressionCorpus};
    use super::super::run_report::{
        AggregateMetrics, HarnessRunReport, Severity as S, TaskOutcome,
    };
    use super::super::suite_report::aggregate_suite_report;
    use super::*;
    use chrono::Utc;

    fn empty_diff() -> BaselineVsCandidate {
        BaselineVsCandidate {
            diff_version: "harness-compare@m4.5".to_string(),
            baseline_run_id: "b".to_string(),
            candidate_run_id: "c".to_string(),
            baseline_report_version: "harness-run-report@m4.4".to_string(),
            candidate_report_version: "harness-run-report@m4.4".to_string(),
            version_compatibility: ReportVersionCompatibility::Identical,
            aggregate: AggregateDiff {
                baseline: AggregateMetrics::default(),
                candidate: AggregateMetrics::default(),
                turns_completed_delta: 0,
                turns_succeeded_delta: 0,
                llm_calls_delta: 0,
                input_tokens_total_delta: 0,
                output_tokens_total_delta: 0,
                tool_calls_total_delta: 0,
                permission_prompts_delta: 0,
                stream_errors_delta: 0,
                memory_decision_count_delta: 0,
                memory_accepted_count_delta: 0,
                memory_rejected_count_delta: 0,
                memory_warning_count_delta: 0,
                memory_conflict_prompt_count_delta: 0,
                prepare_step_total_delta: 0,
                prepare_step_denied_delta: 0,
                execution_mode_judgments_delta: 0,
                permission_resolved_allow_delta: 0,
                permission_resolved_deny_delta: 0,
            },
            blocking_failures: Vec::new(),
            grader_verdicts: Vec::new(),
            evidence_summary: EvidenceSummaryDiff {
                memory_after_turn: VecLengthDiff {
                    baseline: 0,
                    candidate: 0,
                    delta: 0,
                },
                prepare_step: VecLengthDiff {
                    baseline: 0,
                    candidate: 0,
                    delta: 0,
                },
                execution_mode: VecLengthDiff {
                    baseline: 0,
                    candidate: 0,
                    delta: 0,
                },
                permission_prompts: VecLengthDiff {
                    baseline: 0,
                    candidate: 0,
                    delta: 0,
                },
                permission_resolved: VecLengthDiff {
                    baseline: 0,
                    candidate: 0,
                    delta: 0,
                },
                stream_errors: VecLengthDiff {
                    baseline: 0,
                    candidate: 0,
                    delta: 0,
                },
                resume_invocations: VecLengthDiff {
                    baseline: 0,
                    candidate: 0,
                    delta: 0,
                },
            },
            any_regression: false,
        }
    }

    fn grader_diff(id: GraderId, regression: bool, candidate: S) -> GraderVerdictDiff {
        GraderVerdictDiff {
            grader_id: id,
            baseline_severity: S::Pass,
            candidate_severity: candidate,
            baseline_reason_codes: Vec::new(),
            candidate_reason_codes: Vec::new(),
            regression,
        }
    }

    fn blocker_diff(code: &str, regression: bool, max_severity: S) -> BlockerDiff {
        BlockerDiff {
            code: code.to_string(),
            baseline_count: 0,
            candidate_count: if regression { 1 } else { 0 },
            delta: if regression { 1 } else { 0 },
            max_severity,
            regression,
        }
    }

    #[test]
    fn no_regression_promotes() {
        let mut diff = empty_diff();
        diff.grader_verdicts
            .push(grader_diff(GraderId::TaskSuccess, false, S::Pass));
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Promote);
        assert!(r
            .reason_codes
            .contains(&"promote_no_regression".to_string()));
        assert_eq!(r.weighted_score, 0.0);
    }

    #[test]
    fn grader_blocking_regression_rejects() {
        let mut diff = empty_diff();
        diff.grader_verdicts
            .push(grader_diff(GraderId::MemoryAlignment, true, S::Blocking));
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Reject);
        assert!(r
            .reason_codes
            .contains(&"reject_grader_blocking_regression".to_string()));
        assert!(r.blocking_grader_ids.contains(&GraderId::MemoryAlignment));
        assert_eq!(r.weighted_score, 10.0);
    }

    #[test]
    fn blocker_failure_blocking_rejects() {
        let mut diff = empty_diff();
        diff.blocking_failures
            .push(blocker_diff("memory_write_denied", true, S::Blocking));
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Reject);
        assert!(r
            .reason_codes
            .contains(&"reject_blocking_failure_regression".to_string()));
        assert_eq!(
            r.blocking_failure_codes,
            vec!["memory_write_denied".to_string()]
        );
    }

    #[test]
    fn grader_warning_regression_holds() {
        let mut diff = empty_diff();
        diff.grader_verdicts
            .push(grader_diff(GraderId::ResourceEfficiency, true, S::Warning));
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Hold);
        assert!(r
            .reason_codes
            .contains(&"hold_grader_warning_regression".to_string()));
        assert_eq!(r.weighted_score, 1.0);
    }

    #[test]
    fn many_warning_regressions_escalate_to_reject() {
        let mut diff = empty_diff();
        for _ in 0..6 {
            diff.blocking_failures
                .push(blocker_diff("tool_failure", true, S::Warning));
        }
        // policy default: max_warning_regressions = 5
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Reject);
        assert!(r
            .reason_codes
            .contains(&"reject_regression_weight_over_limit".to_string()));
    }

    #[test]
    fn blocking_takes_precedence_over_warning() {
        let mut diff = empty_diff();
        diff.grader_verdicts
            .push(grader_diff(GraderId::MemoryAlignment, true, S::Blocking));
        diff.grader_verdicts
            .push(grader_diff(GraderId::ResourceEfficiency, true, S::Warning));
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Reject);
    }

    #[test]
    fn suite_pass_promotes() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(CorpusTask {
            id: "t1".to_string(),
            tier: CorpusTier::Smoke,
            prompt: "p".to_string(),
            description: None,
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        });
        let mut report = HarnessRunReport::new_empty("r1", Utc::now());
        report.task.outcome = TaskOutcome::Success;
        report.task.last_turn_succeeded = true;
        report.task.turn_count = 1;
        report.aggregate.turns_completed = 1;
        let mut map = std::collections::BTreeMap::new();
        map.insert("t1".to_string(), report);
        let now = Utc::now();
        let suite = aggregate_suite_report("s", &corpus, &map, now, now);
        let r = evaluate_suite(&suite, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Promote);
    }

    #[test]
    fn suite_skeleton_holds_with_no_input_code() {
        let corpus = RegressionCorpus::new("empty");
        let map = std::collections::BTreeMap::new();
        let now = Utc::now();
        let suite = aggregate_suite_report("s", &corpus, &map, now, now);
        let r = evaluate_suite(&suite, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Hold);
        assert!(r
            .reason_codes
            .contains(&"hold_no_input_for_judgment".to_string()));
    }

    #[test]
    fn default_policy_id_is_pinned() {
        let p = GatePolicy::default();
        assert_eq!(p.policy_id, "default-conservative-m4.8");
        let r = evaluate_compare(&empty_diff(), &p);
        assert_eq!(r.policy_id, "default-conservative-m4.8");
        assert_eq!(r.gate_version, HARNESS_GATE_VERSION);
    }

    #[test]
    fn gate_decision_label_is_stable() {
        assert_eq!(GateDecision::Promote.label(), "promote");
        assert_eq!(GateDecision::Hold.label(), "hold");
        assert_eq!(GateDecision::Reject.label(), "reject");
    }

    /// Reviewer Warning-4 regression: suite Warning grade → Hold.
    #[test]
    fn suite_warning_holds() {
        // 2 tasks, 1 regresses → SuiteGrade::Warning (not Fail
        // because not 100% of the tier regressed and not >50%
        // weighted).
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(CorpusTask {
            id: "t1".to_string(),
            tier: CorpusTier::Smoke,
            prompt: "p".to_string(),
            description: None,
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        });
        corpus.tasks.push(CorpusTask {
            id: "t2".to_string(),
            tier: CorpusTier::CriticalPath,
            prompt: "p".to_string(),
            description: None,
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        });
        let mut r1 = HarnessRunReport::new_empty("r1", Utc::now());
        r1.task.outcome = TaskOutcome::Success;
        r1.task.last_turn_succeeded = true;
        r1.task.turn_count = 1;
        r1.aggregate.turns_completed = 1;
        let mut r2 = HarnessRunReport::new_empty("r2", Utc::now());
        r2.task.outcome = TaskOutcome::Failed;
        r2.task.turn_count = 1;
        r2.aggregate.turns_completed = 1;
        let mut map = std::collections::BTreeMap::new();
        map.insert("t1".to_string(), r1);
        map.insert("t2".to_string(), r2);
        let now = Utc::now();
        let suite = aggregate_suite_report("s", &corpus, &map, now, now);
        // Sanity: Fail because critical_path tier 100% regressed.
        // We need a non-Fail Warning; rebuild with non-tier-fatal layout.
        // (Both tasks in same tier, only one regresses → tier regression < 100%)
        let mut corpus2 = RegressionCorpus::new("c");
        corpus2.tasks.push(CorpusTask {
            id: "a".to_string(),
            tier: CorpusTier::Smoke,
            prompt: "p".to_string(),
            description: None,
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        });
        corpus2.tasks.push(CorpusTask {
            id: "b".to_string(),
            tier: CorpusTier::Smoke,
            prompt: "p".to_string(),
            description: None,
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        });
        let mut ra = HarnessRunReport::new_empty("ra", Utc::now());
        ra.task.outcome = TaskOutcome::Success;
        ra.task.last_turn_succeeded = true;
        ra.task.turn_count = 1;
        ra.aggregate.turns_completed = 1;
        let mut rb = HarnessRunReport::new_empty("rb", Utc::now());
        rb.task.outcome = TaskOutcome::Failed;
        rb.task.turn_count = 1;
        rb.aggregate.turns_completed = 1;
        let mut map2 = std::collections::BTreeMap::new();
        map2.insert("a".to_string(), ra);
        map2.insert("b".to_string(), rb);
        let suite2 = aggregate_suite_report("s2", &corpus2, &map2, now, now);
        assert_eq!(suite2.grade, SuiteGrade::Warning);
        let r = evaluate_suite(&suite2, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Hold);
        assert!(r.reason_codes.contains(&"hold_suite_warning".to_string()));
        // Use the original `suite` variable to reach Reject below.
        let _ = suite;
    }

    /// Reviewer Warning-4 regression: suite Fail grade → Reject.
    #[test]
    fn suite_fail_rejects_with_tier_full_regression_code() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(CorpusTask {
            id: "smoke-1".to_string(),
            tier: CorpusTier::Smoke,
            prompt: "p".to_string(),
            description: None,
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        });
        let mut report = HarnessRunReport::new_empty("r1", Utc::now());
        report.task.outcome = TaskOutcome::Failed;
        report.task.turn_count = 1;
        report.aggregate.turns_completed = 1;
        let mut map = std::collections::BTreeMap::new();
        map.insert("smoke-1".to_string(), report);
        let now = Utc::now();
        let suite = aggregate_suite_report("s", &corpus, &map, now, now);
        assert_eq!(suite.grade, SuiteGrade::Fail);
        let r = evaluate_suite(&suite, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Reject);
        assert!(r.reason_codes.contains(&"reject_suite_fail".to_string()));
        assert!(r
            .reason_codes
            .contains(&"reject_tier_full_regression".to_string()));
    }

    /// Reviewer Warning-4 regression: blocker-Warning regression
    /// (no grader regression) still produces a Hold.
    #[test]
    fn blocker_warning_regression_holds() {
        let mut diff = empty_diff();
        diff.blocking_failures
            .push(blocker_diff("tool_failure", true, S::Warning));
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Hold);
        assert!(r
            .reason_codes
            .contains(&"hold_blocking_failure_regression".to_string()));
        assert_eq!(r.blocking_failure_codes, vec!["tool_failure".to_string()]);
    }

    /// Reviewer Warning-2 regression: escalated Reject (too many
    /// warnings) still surfaces contributing IDs in
    /// `blocking_grader_ids` / `blocking_failure_codes`.
    #[test]
    fn escalated_reject_preserves_contributing_ids() {
        let mut diff = empty_diff();
        for _ in 0..6 {
            diff.blocking_failures
                .push(blocker_diff("tool_failure", true, S::Warning));
        }
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Reject);
        // Escalated path used to drop contributing IDs; now they
        // are preserved.
        assert_eq!(r.blocking_failure_codes, vec!["tool_failure".to_string()]);
    }

    /// Reviewer Warning-1 regression: escalated-Reject summary
    /// must mention warning-count overage, not "0 blocking
    /// regression(s)".
    #[test]
    fn escalated_reject_summary_mentions_warning_count() {
        let mut diff = empty_diff();
        for _ in 0..6 {
            diff.grader_verdicts
                .push(grader_diff(GraderId::ResourceEfficiency, true, S::Warning));
        }
        let r = evaluate_compare(&diff, &GatePolicy::default());
        assert_eq!(r.decision, GateDecision::Reject);
        assert!(
            r.summary.contains("warning regression"),
            "summary should describe warning escalation, got: {}",
            r.summary
        );
    }

    /// Reviewer Blocking-1 regression: when policy lifts
    /// `max_promote_regression_weight_ratio` above 0, a
    /// SuiteGrade::Pass with regression_weight > ratio should
    /// degrade to Hold.  Today SuiteGrade::Pass guarantees
    /// regression_weight == 0 so the field is harmless on
    /// default; this test wires it via a synthetic suite where
    /// the policy itself enforces the boundary.
    #[test]
    fn pass_grade_with_relaxed_policy_still_hold_when_weight_over_limit() {
        // Construct a Pass suite by aggregating a clean run.
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(CorpusTask {
            id: "t1".to_string(),
            tier: CorpusTier::Smoke,
            prompt: "p".to_string(),
            description: None,
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            tags: Vec::new(),
        });
        let mut r1 = HarnessRunReport::new_empty("r1", Utc::now());
        r1.task.outcome = TaskOutcome::Success;
        r1.task.last_turn_succeeded = true;
        r1.task.turn_count = 1;
        r1.aggregate.turns_completed = 1;
        let mut map = std::collections::BTreeMap::new();
        map.insert("t1".to_string(), r1);
        let now = Utc::now();
        let mut suite = aggregate_suite_report("s", &corpus, &map, now, now);
        // Synthesise a non-zero regression_weight on the Pass
        // suite to exercise the relaxed-policy code path.  This
        // is a test-only injection; production aggregation never
        // produces this state, but the code path must still
        // honour the policy threshold.
        suite.overall.regression_weight = 0.4;
        suite.overall.total_weight = 1.0;
        let policy = GatePolicy {
            policy_id: "test-policy".to_string(),
            max_warning_regressions: 5,
            max_promote_regression_weight_ratio: 0.3,
        };
        let r = evaluate_suite(&suite, &policy);
        // weighted = 0.4, threshold = 0.3 → Hold
        assert_eq!(r.decision, GateDecision::Hold);
        assert!(
            r.reason_codes
                .contains(&"hold_too_many_warning_regressions".to_string()),
            "expected weight-over-limit code, got {:?}",
            r.reason_codes
        );
    }
}
