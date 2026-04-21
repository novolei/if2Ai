//! Suite-level aggregation of multiple [`HarnessRunReport`]s
//! against a [`super::corpus::RegressionCorpus`] (Phase M4.7).
//!
//! Pure aggregation — no execution, no IO.  Caller supplies the
//! corpus + the per-task `HarnessRunReport` (typically loaded
//! from disk via [`super::report_persistence`]) and this module
//! produces a typed [`SuiteReport`] the M4.8 gate can score.
//!
//! Honest scope:
//!
//! - **Aggregation only.**  Driving the agent through corpus
//!   tasks (mapping `prompt → start_agent_stream → run_id`) is
//!   M5 territory and intentionally out of scope.  The caller is
//!   responsible for the task-id → run_id mapping.
//! - **Per-tier roll-up + suite-level grade.**  Failure
//!   classification compares actual outcome against the corpus
//!   author's `expected_outcome` / `expected_blockers`.
//! - Suite grade is **coarse**: `Pass` / `Warning` / `Fail` /
//!   `Skeleton`.  Detailed weighting / promotion logic stays in
//!   M4.8 gate.

#![allow(dead_code)]

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::corpus::{CorpusTier, RegressionCorpus};
use super::run_report::{HarnessRunReport, TaskOutcome};

/// Stable suite contract version.
pub const HARNESS_SUITE_REPORT_VERSION: &str = "harness-suite-report@m4.7";

/// Coarse classification of one task within a suite.  Distinct
/// from `TaskOutcome` (which describes the **agent run** outcome)
/// — `TaskClassification` describes whether the agent's outcome
/// matched the corpus author's intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClassification {
    /// Actual outcome matched expected, no unexpected blockers.
    Pass,
    /// Actual outcome differed from expected, OR an unexpected
    /// blocker code appeared.
    Regression,
    /// Actual outcome was strictly **better** than expected
    /// (e.g. baseline failed, candidate succeeded).  Recorded
    /// for visibility but never blocks promotion.
    Improvement,
    /// Caller did not supply a run report for this task; nothing
    /// could be classified.
    Missing,
    /// Task carried no `expected_outcome` and the run produced
    /// no blockers — task ran but author intent was undefined.
    NoExpectation,
}

/// Per-task outcome inside a suite report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunOutcome {
    pub task_id: String,
    pub tier: CorpusTier,
    /// `None` when no run was supplied (caller didn't run this
    /// task, or the run failed before producing a report).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// `None` when no run was supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_outcome: Option<TaskOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_outcome: Option<TaskOutcome>,
    /// Blocker codes observed in the run that were NOT in the
    /// task's `expected_blockers` whitelist.
    pub unexpected_blocker_codes: Vec<String>,
    /// Total blocking-failures count across all severities (mirror
    /// of `report.blocking_failures.len()`).
    pub blocking_failures_count: usize,
    pub classification: TaskClassification,
    /// Echoed task weight from the corpus (preserved so M4.8 gate
    /// doesn't need to re-walk the corpus).
    pub weight: f64,
}

/// Per-tier roll-up summary.  `pass + regression + improvement +
/// missing + no_expectation == total_tasks`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TierSummary {
    pub tier: Option<CorpusTier>, // None for the "all tiers" total
    pub total_tasks: usize,
    pub pass: usize,
    pub regression: usize,
    pub improvement: usize,
    pub missing: usize,
    pub no_expectation: usize,
    /// Sum of `weight` over `regression` tasks; useful as an
    /// early signal for M4.8 gate weighting.
    pub regression_weight: f64,
    /// Sum of `weight` over all classified tasks (excludes
    /// `Missing`).  Always positive when `total_tasks > missing`.
    pub total_weight: f64,
}

impl TierSummary {
    fn record(&mut self, outcome: &TaskRunOutcome) {
        self.total_tasks += 1;
        match outcome.classification {
            TaskClassification::Pass => self.pass += 1,
            TaskClassification::Regression => self.regression += 1,
            TaskClassification::Improvement => self.improvement += 1,
            TaskClassification::Missing => self.missing += 1,
            TaskClassification::NoExpectation => self.no_expectation += 1,
        }
        if !matches!(outcome.classification, TaskClassification::Missing) {
            self.total_weight += outcome.weight;
        }
        if matches!(outcome.classification, TaskClassification::Regression) {
            self.regression_weight += outcome.weight;
        }
    }
}

/// Coarse suite-level grade rendered by [`aggregate_suite_report`].
/// M4.8 gate may override based on weighted policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuiteGrade {
    /// Every classified task passed; zero regressions.
    Pass,
    /// At least one regression observed.
    Warning,
    /// Total regression weight > 50% of total weight, OR every
    /// task in a tier regressed.
    Fail,
    /// Suite produced zero classified runs (all `Missing` /
    /// `NoExpectation`) — grader cannot judge.
    Skeleton,
}

/// Top-level suite report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiteReport {
    pub suite_report_version: String,
    pub suite_id: String,
    pub corpus_name: String,
    pub corpus_version: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub tasks: Vec<TaskRunOutcome>,
    /// Per-tier roll-up.  Always contains entries for every tier
    /// in `CorpusTier::ALL` (zero-counts when the corpus has no
    /// tasks of that tier) so consumers iterate stably.
    pub tier_summaries: Vec<TierSummary>,
    /// Cross-tier total.
    pub overall: TierSummary,
    pub grade: SuiteGrade,
}

/// Aggregate one suite from a corpus + per-task run reports.
///
/// `task_to_run_report` is keyed by `task_id`.  Tasks not present
/// in the map are classified as `Missing`.  Run reports for tasks
/// not in the corpus are silently ignored (the corpus is the
/// source of truth for which tasks count).
///
/// `started_at` / `ended_at` are caller-supplied because suite
/// runners may report wall-clock differently (parallel vs serial
/// execution).
pub fn aggregate_suite_report(
    suite_id: impl Into<String>,
    corpus: &RegressionCorpus,
    task_to_run_report: &BTreeMap<String, HarnessRunReport>,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
) -> SuiteReport {
    let mut tasks: Vec<TaskRunOutcome> = Vec::with_capacity(corpus.tasks.len());
    let mut tier_summaries_map: BTreeMap<CorpusTier, TierSummary> = CorpusTier::ALL
        .iter()
        .map(|t| {
            (
                *t,
                TierSummary {
                    tier: Some(*t),
                    ..Default::default()
                },
            )
        })
        .collect();
    let mut overall = TierSummary {
        tier: None,
        ..Default::default()
    };

    for task in &corpus.tasks {
        let report = task_to_run_report.get(&task.id);
        let outcome = classify_task(task, report);
        if let Some(summary) = tier_summaries_map.get_mut(&task.tier) {
            summary.record(&outcome);
        }
        overall.record(&outcome);
        tasks.push(outcome);
    }

    let tier_summaries: Vec<TierSummary> = CorpusTier::ALL
        .iter()
        .map(|t| {
            tier_summaries_map.remove(t).unwrap_or(TierSummary {
                tier: Some(*t),
                ..Default::default()
            })
        })
        .collect();

    let grade = derive_suite_grade(&overall, &tier_summaries);

    SuiteReport {
        suite_report_version: HARNESS_SUITE_REPORT_VERSION.to_string(),
        suite_id: suite_id.into(),
        corpus_name: corpus.name.clone(),
        corpus_version: corpus.corpus_version.clone(),
        started_at,
        ended_at,
        tasks,
        tier_summaries,
        overall,
        grade,
    }
}

fn classify_task(
    task: &super::corpus::CorpusTask,
    report: Option<&HarnessRunReport>,
) -> TaskRunOutcome {
    let Some(report) = report else {
        return TaskRunOutcome {
            task_id: task.id.clone(),
            tier: task.tier,
            run_id: None,
            actual_outcome: None,
            expected_outcome: task.expected_outcome,
            unexpected_blocker_codes: Vec::new(),
            blocking_failures_count: 0,
            classification: TaskClassification::Missing,
            weight: task.weight,
        };
    };
    let actual = report.task.outcome;
    let unexpected_blockers: Vec<String> = report
        .blocking_failures
        .iter()
        .filter_map(|f| {
            if task.expected_blockers.contains(&f.code) {
                None
            } else {
                Some(f.code.clone())
            }
        })
        .collect();
    let classification = match (task.expected_outcome, actual) {
        (None, _) if unexpected_blockers.is_empty() => TaskClassification::NoExpectation,
        (None, _) => TaskClassification::Regression,
        (Some(expected), actual) if expected == actual && unexpected_blockers.is_empty() => {
            TaskClassification::Pass
        }
        (Some(expected), actual)
            if outcome_rank(actual) > outcome_rank(expected) && unexpected_blockers.is_empty() =>
        {
            TaskClassification::Improvement
        }
        _ => TaskClassification::Regression,
    };

    TaskRunOutcome {
        task_id: task.id.clone(),
        tier: task.tier,
        run_id: Some(report.run_id.clone()),
        actual_outcome: Some(actual),
        expected_outcome: task.expected_outcome,
        unexpected_blocker_codes: unexpected_blockers,
        blocking_failures_count: report.blocking_failures.len(),
        classification,
        weight: task.weight,
    }
}

/// Rank task outcomes for "improvement vs regression" detection.
/// Higher rank == better.
fn outcome_rank(o: TaskOutcome) -> u8 {
    match o {
        TaskOutcome::Success => 4,
        TaskOutcome::PartialSuccess => 3,
        TaskOutcome::Failed => 2,
        TaskOutcome::Incomplete => 1,
    }
}

fn derive_suite_grade(overall: &TierSummary, tiers: &[TierSummary]) -> SuiteGrade {
    // Reviewer Blocking-2 fix: a task counts as "judged" only
    // when it produced both a run report AND the corpus author
    // declared an `expected_outcome`.  Tasks that are Missing
    // (no run) or NoExpectation (no author intent) carry zero
    // signal for the grader — if every task falls into one of
    // those buckets, the suite grade must honestly degrade to
    // Skeleton instead of pretending Pass.
    let judged = overall.total_tasks - overall.missing - overall.no_expectation;
    if judged == 0 {
        return SuiteGrade::Skeleton;
    }
    if overall.regression == 0 {
        return SuiteGrade::Pass;
    }
    // Hard fail: any tier where every classified task regressed
    // (and the tier is non-empty).
    for tier in tiers {
        let tier_classified = tier.total_tasks - tier.missing;
        if tier_classified > 0 && tier.regression == tier_classified {
            return SuiteGrade::Fail;
        }
    }
    // Hard fail: weighted regression > 50% of total weight.
    if overall.total_weight > 0.0 && overall.regression_weight / overall.total_weight > 0.5 {
        return SuiteGrade::Fail;
    }
    SuiteGrade::Warning
}

#[cfg(test)]
mod tests {
    use super::super::corpus::{CorpusTask, RegressionCorpus};
    use super::super::run_report::{BlockingFailure, HarnessRunReport, Severity, TaskOutcome};
    use super::*;
    use chrono::Utc;

    fn task_with(
        id: &str,
        tier: CorpusTier,
        expected: Option<TaskOutcome>,
        expected_blockers: Vec<String>,
        weight: f64,
    ) -> CorpusTask {
        CorpusTask {
            id: id.to_string(),
            tier,
            prompt: "p".to_string(),
            description: None,
            expected_outcome: expected,
            expected_blockers,
            weight,
            tags: Vec::new(),
        }
    }

    fn report_with(
        run_id: &str,
        outcome: TaskOutcome,
        blocker_codes: Vec<&str>,
    ) -> HarnessRunReport {
        let mut r = HarnessRunReport::new_empty(run_id, Utc::now());
        r.task.outcome = outcome;
        r.task.turn_count = 1;
        r.task.last_turn_succeeded = matches!(outcome, TaskOutcome::Success);
        r.aggregate.turns_completed = 1;
        for code in blocker_codes {
            r.blocking_failures.push(BlockingFailure {
                code: code.to_string(),
                severity: Severity::Warning,
                message: "x".to_string(),
                evidence_ref: None,
                observed_at: Utc::now(),
            });
        }
        r
    }

    #[test]
    fn empty_corpus_grades_skeleton() {
        let corpus = RegressionCorpus::new("c");
        let map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        let now = Utc::now();
        let s = aggregate_suite_report("suite-1", &corpus, &map, now, now);
        assert_eq!(s.grade, SuiteGrade::Skeleton);
        assert_eq!(s.overall.total_tasks, 0);
        assert_eq!(s.tier_summaries.len(), 5);
    }

    #[test]
    fn missing_run_is_classified_missing() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(task_with(
            "smoke-001",
            CorpusTier::Smoke,
            Some(TaskOutcome::Success),
            Vec::new(),
            1.0,
        ));
        let map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        let now = Utc::now();
        let s = aggregate_suite_report("suite-1", &corpus, &map, now, now);
        assert_eq!(s.tasks.len(), 1);
        assert_eq!(s.tasks[0].classification, TaskClassification::Missing);
        assert_eq!(s.grade, SuiteGrade::Skeleton);
    }

    #[test]
    fn matching_outcome_is_pass() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(task_with(
            "t1",
            CorpusTier::Smoke,
            Some(TaskOutcome::Success),
            Vec::new(),
            1.0,
        ));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Success, Vec::new()),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.tasks[0].classification, TaskClassification::Pass);
        assert_eq!(s.grade, SuiteGrade::Pass);
    }

    #[test]
    fn unexpected_blocker_marks_regression_even_when_outcome_matches() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(task_with(
            "t1",
            CorpusTier::Smoke,
            Some(TaskOutcome::Success),
            vec![], // no expected blockers
            1.0,
        ));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Success, vec!["unexpected_failure"]),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.tasks[0].classification, TaskClassification::Regression);
        assert_eq!(
            s.tasks[0].unexpected_blocker_codes,
            vec!["unexpected_failure".to_string()]
        );
        assert_eq!(s.grade, SuiteGrade::Warning);
    }

    #[test]
    fn whitelisted_blocker_does_not_regress() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(task_with(
            "t1",
            CorpusTier::ToolRisk,
            Some(TaskOutcome::Success),
            vec!["tool_failure".to_string()],
            1.0,
        ));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Success, vec!["tool_failure"]),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.tasks[0].classification, TaskClassification::Pass);
        assert!(s.tasks[0].unexpected_blocker_codes.is_empty());
    }

    #[test]
    fn improvement_when_actual_better_than_expected() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(task_with(
            "t1",
            CorpusTier::Smoke,
            Some(TaskOutcome::PartialSuccess),
            Vec::new(),
            1.0,
        ));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Success, Vec::new()),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.tasks[0].classification, TaskClassification::Improvement);
        // Improvement should NOT degrade grade.
        assert_eq!(s.grade, SuiteGrade::Pass);
    }

    #[test]
    fn full_tier_regression_grades_fail() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(task_with(
            "t1",
            CorpusTier::Smoke,
            Some(TaskOutcome::Success),
            Vec::new(),
            1.0,
        ));
        corpus.tasks.push(task_with(
            "t2",
            CorpusTier::CriticalPath,
            Some(TaskOutcome::Success),
            Vec::new(),
            1.0,
        ));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        // smoke tier — single task regresses → tier 100% regress
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Failed, Vec::new()),
        );
        // critical-path tier passes
        map.insert(
            "t2".to_string(),
            report_with("r2", TaskOutcome::Success, Vec::new()),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.grade, SuiteGrade::Fail);
    }

    #[test]
    fn weighted_majority_regression_fails() {
        let mut corpus = RegressionCorpus::new("c");
        // 1.0 weight passes
        corpus.tasks.push(task_with(
            "t1",
            CorpusTier::Smoke,
            Some(TaskOutcome::Success),
            Vec::new(),
            1.0,
        ));
        // 5.0 weight regresses → 5/6 = 83% > 50%
        corpus.tasks.push(task_with(
            "t2",
            CorpusTier::CriticalPath,
            Some(TaskOutcome::Success),
            Vec::new(),
            5.0,
        ));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Success, Vec::new()),
        );
        map.insert(
            "t2".to_string(),
            report_with("r2", TaskOutcome::Failed, Vec::new()),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.grade, SuiteGrade::Fail);
    }

    #[test]
    fn all_no_expectation_grades_skeleton() {
        // Reviewer Blocking-2 regression: corpus exists, runs
        // exist, but no task carries `expected_outcome` and no
        // unexpected blockers fired → grader has no signal →
        // Skeleton (not Pass).
        let mut corpus = RegressionCorpus::new("c");
        corpus
            .tasks
            .push(task_with("t1", CorpusTier::Smoke, None, Vec::new(), 1.0));
        corpus
            .tasks
            .push(task_with("t2", CorpusTier::ToolRisk, None, Vec::new(), 1.0));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Success, Vec::new()),
        );
        map.insert(
            "t2".to_string(),
            report_with("r2", TaskOutcome::Success, Vec::new()),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.overall.no_expectation, 2);
        assert_eq!(s.overall.regression, 0);
        assert_eq!(s.grade, SuiteGrade::Skeleton);
    }

    #[test]
    fn no_expectation_with_unexpected_blocker_grades_warning() {
        // Even without an author-declared expected_outcome, an
        // unexpected blocker still counts as a real Regression
        // signal (the corpus author whitelisted nothing).
        let mut corpus = RegressionCorpus::new("c");
        corpus
            .tasks
            .push(task_with("t1", CorpusTier::Smoke, None, Vec::new(), 1.0));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Success, vec!["unexpected_failure"]),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.tasks[0].classification, TaskClassification::Regression);
        assert_eq!(s.grade, SuiteGrade::Fail); // single-tier 100% regression
    }

    #[test]
    fn extra_run_for_unknown_task_is_silently_ignored() {
        let mut corpus = RegressionCorpus::new("c");
        corpus.tasks.push(task_with(
            "t1",
            CorpusTier::Smoke,
            Some(TaskOutcome::Success),
            Vec::new(),
            1.0,
        ));
        let mut map: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        map.insert(
            "t1".to_string(),
            report_with("r1", TaskOutcome::Success, Vec::new()),
        );
        // run for non-corpus task — should be ignored
        map.insert(
            "ghost".to_string(),
            report_with("r2", TaskOutcome::Failed, Vec::new()),
        );
        let now = Utc::now();
        let s = aggregate_suite_report("suite", &corpus, &map, now, now);
        assert_eq!(s.tasks.len(), 1);
        assert_eq!(s.grade, SuiteGrade::Pass);
    }
}
