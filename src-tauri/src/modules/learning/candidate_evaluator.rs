//! Offline candidate evaluator (Phase M5-B foundation).
//!
//! Single orchestration seam that drives one candidate strategy
//! through the **existing M4 governance pipeline**:
//!
//! 1. Load both `HarnessRunReport`s (baseline + candidate) from
//!    the harness report store.
//! 2. Run [`compare_reports`] to produce the typed
//!    `BaselineVsCandidate` diff.
//! 3. Run [`evaluate_compare`] (with the supplied or default
//!    [`GatePolicy`]) to produce the typed `Recommendation`.
//! 4. Persist both as registry-side refs on the candidate via
//!    [`StrategyRegistryService::attach_compare_ref_validated`]
//!    and [`StrategyRegistryService::attach_recommendation`].
//!
//! Honest scope of this module:
//!
//! - **Foundation only.**  The evaluator does **not** generate
//!   the candidate `HarnessRunReport` from a candidate strategy
//!   DSL — there is no executable strategy DSL yet (M5-C+
//!   territory).  The caller is responsible for sourcing both
//!   `run_id`s; the evaluator wires them through compare + gate
//!   + persistence.
//! - **Pure compose** of M4 contracts (`compare_reports`,
//!   `evaluate_compare`).  No new evaluation rules; no
//!   re-interpretation of reason codes.
//! - **No promotion.**  The evaluator stops at "recommendation
//!   recorded".  The promotion gate
//!   ([`super::promotion_gate`]) is a separate explicit step.
//! - **Cross-store validated.**  Refuses if the supplied
//!   `run_id`s are not present in the harness report store
//!   (first guard against typo / stale id).
//!
//! Out of scope for M5-B:
//!
//! - Driving the agent through corpus tasks (requires
//!   executable strategy DSL).
//! - Multi-candidate cohort comparison.
//!
//! M5-C round 2 additions:
//!
//! - `evaluate_candidate_against_suite(...)` orchestrates
//!   `harness_aggregate_suite_report` + `harness_evaluate_suite`
//!   and writes a [`SuiteEvaluationRef`] (+ optional
//!   suite-driven `RecommendationRef`) back to the candidate.
//!   Compare-pair eval and suite eval are recorded on parallel
//!   tracks (no merge logic).
//! - Typed [`EvaluationStep`] + `StepFailed` error variant so
//!   callers can identify exactly which step failed during a
//!   partial-state run.
//! - [`CandidateEvaluator::inspect_evaluation_progress`] +
//!   [`EvaluationProgress`] expose what already landed for a
//!   given `(strategy_id, baseline_run_id, candidate_run_id)`
//!   triple, enabling retry-safe resume.
//! - Idempotency contract: re-running `evaluate_candidate` /
//!   `evaluate_candidate_against_suite` with the same inputs is
//!   safe — every persistence step overwrites "last" refs and
//!   the underlying file store is atomic-rename per record.
//!   See [`EvaluationStep`] doc-comment for the per-step
//!   guarantees.

#![allow(dead_code)]

use std::collections::BTreeMap;

use chrono::Utc;
use thiserror::Error;

use super::strategy_registry::CandidateStrategy;
use super::strategy_registry_service::{StrategyRegistryError, StrategyRegistryService};
use crate::modules::harness::{
    aggregate_suite_report, compare_reports, gate_evaluate_compare, gate_evaluate_suite,
    BaselineVsCandidate, CompareError, GatePolicy, HarnessReportStore, HarnessRunReport,
    Recommendation, RegressionCorpus, SuiteReport,
};

/// Phase M5-C round 2 — closed alphabet of evaluation pipeline
/// steps.  Used in [`CandidateEvaluatorError::StepFailed`] so
/// callers know exactly where a partial-state run aborted, and
/// in [`EvaluationProgress`] so retry callers know what already
/// landed.
///
/// Per-step idempotency contract:
///
/// | Step                       | Side effect                                        | Retry-safe? |
/// |----------------------------|----------------------------------------------------|-------------|
/// | `LoadBaselineReport`       | none (read-only)                                   | yes         |
/// | `LoadCandidateReport`      | none (read-only)                                   | yes         |
/// | `RunCompare`               | none (pure compute)                                | yes         |
/// | `RunGate`                  | none (pure compute)                                | yes         |
/// | `AttachCompareRef`         | overwrite `last_compare_ref` (atomic file rename)  | yes         |
/// | `AttachRecommendationRef`  | overwrite `last_recommendation_ref` (atomic save)  | yes         |
/// | `LoadCorpus`               | none (read-only)                                   | yes         |
/// | `AggregateSuiteReport`     | none (pure compute)                                | yes         |
/// | `RunSuiteGate`             | none (pure compute)                                | yes         |
/// | `AttachSuiteEvaluationRef` | overwrite `last_suite_evaluation_ref` (+ optional
///                              | `last_suite_recommendation_ref`) atomically        | yes         |
///
/// Every persistence step is idempotent: re-running the
/// pipeline with the same `(strategy_id, baseline_run_id,
/// candidate_run_id)` (or `(strategy_id, suite_id, corpus,
/// task_to_run_id)`) overwrites the corresponding `last_*_ref`
/// fields; the underlying file store does atomic tmp+rename so
/// a crash mid-write never leaves a corrupt file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStep {
    LoadBaselineReport,
    LoadCandidateReport,
    RunCompare,
    RunGate,
    AttachCompareRef,
    AttachRecommendationRef,
    LoadCorpus,
    AggregateSuiteReport,
    RunSuiteGate,
    AttachSuiteEvaluationRef,
}

impl EvaluationStep {
    /// Stable wire label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::LoadBaselineReport => "load_baseline_report",
            Self::LoadCandidateReport => "load_candidate_report",
            Self::RunCompare => "run_compare",
            Self::RunGate => "run_gate",
            Self::AttachCompareRef => "attach_compare_ref",
            Self::AttachRecommendationRef => "attach_recommendation_ref",
            Self::LoadCorpus => "load_corpus",
            Self::AggregateSuiteReport => "aggregate_suite_report",
            Self::RunSuiteGate => "run_suite_gate",
            Self::AttachSuiteEvaluationRef => "attach_suite_evaluation_ref",
        }
    }
}

/// Evaluator error family.  Distinct from
/// [`StrategyRegistryError`] so callers can render typed
/// diagnostics without unwrapping the registry error.
#[derive(Debug, Error)]
pub enum CandidateEvaluatorError {
    #[error("registry error: {0}")]
    Registry(#[from] StrategyRegistryError),
    #[error("harness compare error: {0}")]
    Compare(#[from] CompareError),
    #[error("harness store error: {0}")]
    HarnessStore(#[from] crate::modules::harness::PersistenceError),
    #[error("harness corpus error: {0}")]
    Corpus(#[from] crate::modules::harness::CorpusError),
    #[error("baseline run_id not found in harness report store: {0}")]
    BaselineNotFound(String),
    #[error("candidate run_id not found in harness report store: {0}")]
    CandidateNotFound(String),
    #[error("strategy not found: {0}")]
    StrategyNotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Phase M5-C round 2 — the pipeline aborted at a specific
    /// step.  Carries the step label + the underlying cause so
    /// callers (and a future retry helper) can resume from a
    /// known point.  See [`EvaluationStep`] for the idempotency
    /// contract.
    #[error("evaluation step '{}' failed: {source}", step.label())]
    StepFailed {
        step: EvaluationStep,
        #[source]
        source: Box<CandidateEvaluatorError>,
    },
}

/// Phase M5-C round 2 — typed snapshot of "what already landed
/// for this (strategy, baseline, candidate) triple".  Returned
/// by [`CandidateEvaluator::inspect_evaluation_progress`] so a
/// retry caller can decide whether to re-run the whole pipeline
/// or skip steps known to have succeeded.
///
/// `compare_ref_present_for_inputs` and
/// `recommendation_ref_fresher_than_compare` lets the caller
/// distinguish:
///
/// - "nothing happened" — `last_compare_ref` is `None` or
///   targets different `run_id`s
/// - "compare landed but recommendation did not" —
///   `compare_ref_present_for_inputs == true` and
///   `recommendation_ref_present == false`
/// - "complete" — both are present and the recommendation is
///   not older than the compare ref
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationProgress {
    pub strategy_id: String,
    pub baseline_run_id: String,
    pub candidate_run_id: String,
    /// `true` iff the candidate has a `last_compare_ref` whose
    /// `(baseline_run_id, candidate_run_id)` matches the
    /// caller's inputs.
    pub compare_ref_present_for_inputs: bool,
    /// `true` iff the candidate has any `last_recommendation_ref`.
    pub recommendation_ref_present: bool,
    /// `true` iff the recommendation ref's `recorded_at` is
    /// `>=` the compare ref's `recorded_at` (i.e. the
    /// recommendation is the verdict on **this** compare, not
    /// a stale one).  `false` when either ref is missing.
    pub recommendation_ref_not_stale: bool,
    /// Convenience: the step a retry caller should resume from.
    /// Computed from the three flags above.  `None` means
    /// "complete; no retry needed".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_from: Option<EvaluationStep>,
}

impl EvaluationProgress {
    fn classify(
        strategy_id: &str,
        baseline_run_id: &str,
        candidate_run_id: &str,
        candidate: &CandidateStrategy,
    ) -> Self {
        let compare_match = candidate
            .last_compare_ref
            .as_ref()
            .map(|c| c.baseline_run_id == baseline_run_id && c.candidate_run_id == candidate_run_id)
            .unwrap_or(false);
        let rec_present = candidate.last_recommendation_ref.is_some();
        let rec_not_stale = match (
            candidate.last_compare_ref.as_ref(),
            candidate.last_recommendation_ref.as_ref(),
        ) {
            (Some(c), Some(r)) => r.recorded_at >= c.recorded_at,
            _ => false,
        };
        let resume_from = if !compare_match {
            // Nothing usable for this input pair — restart from
            // the very first step.
            Some(EvaluationStep::LoadBaselineReport)
        } else if !rec_present {
            Some(EvaluationStep::AttachRecommendationRef)
        } else if !rec_not_stale {
            // Stale recommendation — re-run the gate against
            // current compare to refresh.
            Some(EvaluationStep::RunGate)
        } else {
            None
        };
        Self {
            strategy_id: strategy_id.to_string(),
            baseline_run_id: baseline_run_id.to_string(),
            candidate_run_id: candidate_run_id.to_string(),
            compare_ref_present_for_inputs: compare_match,
            recommendation_ref_present: rec_present,
            recommendation_ref_not_stale: rec_not_stale,
            resume_from,
        }
    }
}

/// Combined output of one offline evaluation run.  Carries the
/// full [`BaselineVsCandidate`] + [`Recommendation`] (so callers
/// can render them) **plus** the post-attach
/// [`CandidateStrategy`] snapshot (so the caller can confirm
/// the registry-side state transition).
#[derive(Debug, Clone)]
pub struct CandidateEvaluationOutcome {
    pub compare: BaselineVsCandidate,
    pub recommendation: Recommendation,
    pub candidate_after: CandidateStrategy,
}

/// Phase M5-C round 2 — combined output of one suite-level
/// evaluation run.
#[derive(Debug, Clone)]
pub struct CandidateSuiteEvaluationOutcome {
    pub suite_report: SuiteReport,
    /// `None` when the caller asked the evaluator to skip the
    /// gate step (suite eval recorded as audit only).
    pub recommendation: Option<Recommendation>,
    pub candidate_after: CandidateStrategy,
}

/// Service handle.  Cheap to construct.  Holds **no** in-memory
/// state — every call composes the registry service + harness
/// report store on the fly.
#[derive(Debug, Clone)]
pub struct CandidateEvaluator {
    registry: StrategyRegistryService,
    harness_store: HarnessReportStore,
}

impl CandidateEvaluator {
    /// Construct an evaluator with explicit collaborators.
    #[must_use]
    pub fn new(registry: StrategyRegistryService, harness_store: HarnessReportStore) -> Self {
        Self {
            registry,
            harness_store,
        }
    }

    /// Construct an evaluator using the platform default roots
    /// for both stores.
    #[must_use]
    pub fn with_default_roots() -> Self {
        Self::new(
            StrategyRegistryService::with_default_root(),
            HarnessReportStore::with_default_root(),
        )
    }

    /// Drive one candidate through the
    /// `compare -> gate -> persist` pipeline.
    ///
    /// `policy` defaults to [`GatePolicy::default_conservative`]
    /// when `None` (mirrors the harness `harness_evaluate_compare`
    /// IPC default).
    ///
    /// Returns the typed [`CandidateEvaluationOutcome`] once
    /// every persistence step has succeeded.
    ///
    /// **Idempotency** (M5-C round 2): re-running with the
    /// same `(strategy_id, baseline_run_id, candidate_run_id)`
    /// is safe — every persistence step overwrites
    /// `last_compare_ref` / `last_recommendation_ref` and the
    /// underlying file store is atomic-rename per record.  See
    /// [`EvaluationStep`] for per-step guarantees.
    ///
    /// On failure: the error is wrapped in
    /// [`CandidateEvaluatorError::StepFailed`] so the caller
    /// knows which step aborted; use
    /// [`Self::inspect_evaluation_progress`] to see what
    /// already landed and decide whether to retry.
    pub async fn evaluate_candidate(
        &self,
        strategy_id: &str,
        baseline_run_id: &str,
        candidate_run_id: &str,
        policy: Option<&GatePolicy>,
    ) -> Result<CandidateEvaluationOutcome, CandidateEvaluatorError> {
        if strategy_id.is_empty() || baseline_run_id.is_empty() || candidate_run_id.is_empty() {
            return Err(CandidateEvaluatorError::InvalidInput(
                "strategy_id, baseline_run_id, candidate_run_id must be non-empty".into(),
            ));
        }

        // (1) load both reports up front so we never persist a
        // partial reference for a run_id that does not exist.
        let baseline = wrap_step(EvaluationStep::LoadBaselineReport, async {
            self.harness_store
                .load(baseline_run_id)
                .await?
                .ok_or_else(|| {
                    CandidateEvaluatorError::BaselineNotFound(baseline_run_id.to_string())
                })
        })
        .await?;
        let candidate_report = wrap_step(EvaluationStep::LoadCandidateReport, async {
            self.harness_store
                .load(candidate_run_id)
                .await?
                .ok_or_else(|| {
                    CandidateEvaluatorError::CandidateNotFound(candidate_run_id.to_string())
                })
        })
        .await?;

        // (2) compose M4 compare + gate (default policy if not
        // supplied — same default the harness IPC uses).
        let compare = wrap_step(EvaluationStep::RunCompare, async {
            compare_reports(&baseline, &candidate_report).map_err(CandidateEvaluatorError::Compare)
        })
        .await?;
        let policy_owned;
        let policy_ref = match policy {
            Some(p) => p,
            None => {
                policy_owned = GatePolicy::default();
                &policy_owned
            }
        };
        let recommendation = wrap_step(EvaluationStep::RunGate, async {
            Ok::<_, CandidateEvaluatorError>(gate_evaluate_compare(&compare, policy_ref))
        })
        .await?;

        // (3) persist refs back into the registry.  Use the
        // validated attach so the cross-store guard fires once
        // more (cheap; both reports are already in cache by the
        // OS at this point).  This second pass also catches the
        // window where a report was deleted between (1) and (3).
        let now = Utc::now();
        wrap_step(EvaluationStep::AttachCompareRef, async {
            self.registry
                .attach_compare_ref_validated(
                    strategy_id,
                    baseline_run_id.to_string(),
                    candidate_run_id.to_string(),
                    &self.harness_store,
                    Some(now),
                )
                .await
                .map_err(|e| match e {
                    StrategyRegistryError::NotFound(id) => {
                        CandidateEvaluatorError::StrategyNotFound(id)
                    }
                    other => CandidateEvaluatorError::Registry(other),
                })
        })
        .await?;
        let candidate_after = wrap_step(EvaluationStep::AttachRecommendationRef, async {
            self.registry
                .attach_recommendation(strategy_id, &recommendation, Some(now))
                .await
                .map_err(|e| match e {
                    StrategyRegistryError::NotFound(id) => {
                        CandidateEvaluatorError::StrategyNotFound(id)
                    }
                    other => CandidateEvaluatorError::Registry(other),
                })
        })
        .await?;

        Ok(CandidateEvaluationOutcome {
            compare,
            recommendation,
            candidate_after,
        })
    }

    /// Phase M5-C round 2 — drive one candidate through a
    /// suite-level evaluation: aggregate the corpus + per-task
    /// run reports into a [`SuiteReport`], optionally render a
    /// [`Recommendation`] via `gate_evaluate_suite`, then
    /// persist both as registry-side refs on the candidate.
    ///
    /// Composition: pure reuse of M4 contracts — no new gate
    /// rules, no new aggregation logic.
    ///
    /// `task_to_run_id` maps `task_id → run_id`.  Tasks
    /// missing from the map (or whose `run_id` is not present
    /// in the harness report store) are classified `Missing`
    /// (mirrors the M4.7 `harness_aggregate_suite_report` IPC).
    ///
    /// `policy` defaults to [`GatePolicy::default_conservative`]
    /// when `None`.  When `attach_recommendation == false`, the
    /// gate is **not** invoked (suite eval recorded as audit
    /// only); the suite-driven recommendation field is left
    /// untouched.
    ///
    /// **No promotion.**  Even when the suite gate decision is
    /// `Promote`, the registry only records the
    /// recommendation; the compare-pair-driven
    /// `last_recommendation_ref` and the suite-driven
    /// `last_suite_recommendation_ref` are stored on parallel
    /// tracks; promotion gate continues to read compare-pair
    /// only (M5-D may merge).
    pub async fn evaluate_candidate_against_suite(
        &self,
        strategy_id: &str,
        suite_id: &str,
        corpus: &RegressionCorpus,
        task_to_run_id: BTreeMap<String, String>,
        policy: Option<&GatePolicy>,
        attach_recommendation: bool,
    ) -> Result<CandidateSuiteEvaluationOutcome, CandidateEvaluatorError> {
        if strategy_id.is_empty() || suite_id.is_empty() {
            return Err(CandidateEvaluatorError::InvalidInput(
                "strategy_id and suite_id must be non-empty".into(),
            ));
        }

        // (1) confirm strategy exists before doing any expensive
        // work — load failures here surface as StrategyNotFound.
        let _ = wrap_step(EvaluationStep::LoadCorpus, async {
            match self.registry.store().load(strategy_id).await {
                Ok(Some(_)) => Ok(()),
                Ok(None) => Err(CandidateEvaluatorError::StrategyNotFound(
                    strategy_id.to_string(),
                )),
                Err(e) => Err(CandidateEvaluatorError::Registry(
                    StrategyRegistryError::Persistence(e),
                )),
            }
        })
        .await?;

        // (2) load every supplied run report.  Missing reports
        // are classified `Missing` by aggregate_suite_report —
        // they do NOT abort the evaluation (mirrors the M4.7
        // harness IPC behaviour).
        let mut task_to_report: BTreeMap<String, HarnessRunReport> = BTreeMap::new();
        for (task_id, run_id) in &task_to_run_id {
            match self.harness_store.load(run_id).await {
                Ok(Some(report)) => {
                    task_to_report.insert(task_id.clone(), report);
                }
                Ok(None) => {
                    tracing::warn!(
                        target: "learning.suite_eval",
                        task_id = %task_id,
                        run_id = %run_id,
                        "[evaluate_candidate_against_suite] run report missing; task will be classified Missing"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        target: "learning.suite_eval",
                        task_id = %task_id,
                        run_id = %run_id,
                        error = %e,
                        "[evaluate_candidate_against_suite] run report load failed; task will be classified Missing"
                    );
                }
            }
        }

        // (3) aggregate suite report + (optionally) render gate
        // recommendation.  Both are pure compute.
        let started_at = Utc::now();
        let suite_report = wrap_step(EvaluationStep::AggregateSuiteReport, async {
            Ok::<_, CandidateEvaluatorError>(aggregate_suite_report(
                suite_id.to_string(),
                corpus,
                &task_to_report,
                started_at,
                Utc::now(),
            ))
        })
        .await?;

        let recommendation = if attach_recommendation {
            let policy_owned;
            let policy_ref = match policy {
                Some(p) => p,
                None => {
                    policy_owned = GatePolicy::default();
                    &policy_owned
                }
            };
            let rec = wrap_step(EvaluationStep::RunSuiteGate, async {
                Ok::<_, CandidateEvaluatorError>(gate_evaluate_suite(&suite_report, policy_ref))
            })
            .await?;
            Some(rec)
        } else {
            None
        };

        // (4) persist refs.
        let now = Utc::now();
        let candidate_after = wrap_step(EvaluationStep::AttachSuiteEvaluationRef, async {
            self.registry
                .attach_suite_evaluation_ref(
                    strategy_id,
                    &suite_report,
                    recommendation.as_ref(),
                    Some(now),
                )
                .await
                .map_err(|e| match e {
                    StrategyRegistryError::NotFound(id) => {
                        CandidateEvaluatorError::StrategyNotFound(id)
                    }
                    other => CandidateEvaluatorError::Registry(other),
                })
        })
        .await?;

        Ok(CandidateSuiteEvaluationOutcome {
            suite_report,
            recommendation,
            candidate_after,
        })
    }

    /// Phase M5-C round 2 — inspect what already landed for a
    /// `(strategy_id, baseline_run_id, candidate_run_id)`
    /// triple.  Pure read — no IO beyond loading the candidate
    /// record.  Use the returned [`EvaluationProgress.resume_from`]
    /// to decide whether to call [`Self::evaluate_candidate`]
    /// again or skip.
    pub async fn inspect_evaluation_progress(
        &self,
        strategy_id: &str,
        baseline_run_id: &str,
        candidate_run_id: &str,
    ) -> Result<EvaluationProgress, CandidateEvaluatorError> {
        if strategy_id.is_empty() {
            return Err(CandidateEvaluatorError::InvalidInput(
                "strategy_id must be non-empty".into(),
            ));
        }
        let candidate = self
            .registry
            .store()
            .load(strategy_id)
            .await
            .map_err(|e| CandidateEvaluatorError::Registry(StrategyRegistryError::Persistence(e)))?
            .ok_or_else(|| CandidateEvaluatorError::StrategyNotFound(strategy_id.to_string()))?;
        Ok(EvaluationProgress::classify(
            strategy_id,
            baseline_run_id,
            candidate_run_id,
            &candidate,
        ))
    }
}

/// Phase M5-C round 2 — wrap one pipeline step's `Result` so
/// failures surface as [`CandidateEvaluatorError::StepFailed`]
/// with the step label preserved.  Pass-through on success.
async fn wrap_step<T, F>(step: EvaluationStep, fut: F) -> Result<T, CandidateEvaluatorError>
where
    F: std::future::Future<Output = Result<T, CandidateEvaluatorError>>,
{
    fut.await.map_err(|source| match source {
        // Avoid double-wrapping when an inner step already
        // labelled itself.
        CandidateEvaluatorError::StepFailed { .. } => source,
        other => CandidateEvaluatorError::StepFailed {
            step,
            source: Box::new(other),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::{
        AggregateMetrics, EvidenceBundle, HarnessRunReport, TaskOutcome, TaskRunResult,
        HARNESS_RUN_REPORT_VERSION,
    };
    use crate::modules::learning::strategy_registry::RolloutState;
    use crate::modules::learning::strategy_registry_service::RegisterManualOpts;
    use crate::modules::learning::strategy_registry_store::StrategyRegistryStore;
    use chrono::Utc;
    use tempfile::tempdir;

    fn synthetic_report(run_id: &str) -> HarnessRunReport {
        let now = Utc::now();
        HarnessRunReport {
            report_version: HARNESS_RUN_REPORT_VERSION.to_string(),
            run_id: run_id.to_string(),
            label: None,
            session_id: None,
            project_id: None,
            started_at: now,
            ended_at: now,
            task: TaskRunResult {
                task_id: run_id.to_string(),
                outcome: TaskOutcome::Success,
                turn_count: 1,
                last_turn_succeeded: true,
                error_summary: None,
            },
            aggregate: AggregateMetrics::default(),
            blocking_failures: Vec::new(),
            evidence: EvidenceBundle::default(),
        }
    }

    #[tokio::test]
    async fn evaluate_candidate_persists_compare_and_recommendation() {
        let registry_dir = tempdir().unwrap();
        let harness_dir = tempdir().unwrap();
        let registry =
            StrategyRegistryService::new(StrategyRegistryStore::new(registry_dir.path()));
        let harness_store = HarnessReportStore::new(harness_dir.path());

        // Seed two synthetic reports.
        let baseline = synthetic_report("base-1");
        let candidate_report = synthetic_report("cand-1");
        harness_store.save(&baseline).await.unwrap();
        harness_store.save(&candidate_report).await.unwrap();

        // Register a manual candidate.
        let candidate = registry
            .register_manual("eval-foundation".into(), RegisterManualOpts::default())
            .await
            .unwrap();

        let evaluator = CandidateEvaluator::new(registry.clone(), harness_store);
        let outcome = evaluator
            .evaluate_candidate(&candidate.identity.strategy_id, "base-1", "cand-1", None)
            .await
            .unwrap();

        // Identical synthetic reports → no regression → Promote.
        assert_eq!(
            outcome.recommendation.decision,
            crate::modules::harness::GateDecision::Promote
        );
        assert_eq!(
            outcome.candidate_after.rollout_state,
            RolloutState::Recommended
        );
        assert!(outcome.candidate_after.last_compare_ref.is_some());
        assert!(outcome.candidate_after.last_recommendation_ref.is_some());
    }

    #[tokio::test]
    async fn evaluate_candidate_refuses_missing_baseline_report() {
        let registry_dir = tempdir().unwrap();
        let harness_dir = tempdir().unwrap();
        let registry =
            StrategyRegistryService::new(StrategyRegistryStore::new(registry_dir.path()));
        let harness_store = HarnessReportStore::new(harness_dir.path());

        let candidate = registry
            .register_manual("eval-missing".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let evaluator = CandidateEvaluator::new(registry.clone(), harness_store);
        let res = evaluator
            .evaluate_candidate(
                &candidate.identity.strategy_id,
                "missing-base",
                "missing-cand",
                None,
            )
            .await;
        // M5-C round 2: failure is wrapped in StepFailed so
        // callers can identify the aborted step.  The original
        // BaselineNotFound is preserved as the source.
        let err = res.unwrap_err();
        match err {
            CandidateEvaluatorError::StepFailed { step, source } => {
                assert_eq!(step, EvaluationStep::LoadBaselineReport);
                assert!(matches!(
                    *source,
                    CandidateEvaluatorError::BaselineNotFound(_)
                ));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    // ───────────────────── M5-C round 2 tests ─────────────────────

    use crate::modules::harness::{
        CorpusTask, CorpusTier, RegressionCorpus, HARNESS_CORPUS_VERSION,
    };

    fn synthetic_corpus(suite_tag: &str) -> RegressionCorpus {
        RegressionCorpus {
            corpus_version: HARNESS_CORPUS_VERSION.to_string(),
            name: format!("{suite_tag}-corpus"),
            description: None,
            loaded_at: Utc::now(),
            source_path: None,
            tasks: vec![CorpusTask {
                id: "task-1".into(),
                tier: CorpusTier::Smoke,
                prompt: "noop".into(),
                expected_outcome: Some(crate::modules::harness::TaskOutcome::Success),
                expected_blockers: Vec::new(),
                weight: 1.0,
                description: None,
                tags: Vec::new(),
            }],
        }
    }

    #[tokio::test]
    async fn evaluate_candidate_against_suite_writes_suite_refs() {
        let registry_dir = tempdir().unwrap();
        let harness_dir = tempdir().unwrap();
        let registry =
            StrategyRegistryService::new(StrategyRegistryStore::new(registry_dir.path()));
        let harness_store = HarnessReportStore::new(harness_dir.path());

        // Seed a single per-task run report.
        let mut report = synthetic_report("suite-run-1");
        report.task.task_id = "task-1".into();
        harness_store.save(&report).await.unwrap();

        let candidate = registry
            .register_manual("suite-eval-1".into(), RegisterManualOpts::default())
            .await
            .unwrap();

        let evaluator = CandidateEvaluator::new(registry.clone(), harness_store);
        let corpus = synthetic_corpus("smoke");
        let mut t2r = std::collections::BTreeMap::new();
        t2r.insert("task-1".to_string(), "suite-run-1".to_string());

        let outcome = evaluator
            .evaluate_candidate_against_suite(
                &candidate.identity.strategy_id,
                "suite-smoke-1",
                &corpus,
                t2r,
                None,
                true,
            )
            .await
            .unwrap();

        assert!(outcome.recommendation.is_some());
        let after = outcome.candidate_after;
        let suite_ref = after.last_suite_evaluation_ref.expect("suite ref present");
        assert_eq!(suite_ref.suite_id, "suite-smoke-1");
        assert_eq!(suite_ref.corpus_name, "smoke-corpus");
        assert_eq!(suite_ref.task_count, 1);
        assert!(after.last_suite_recommendation_ref.is_some());
        // Compare-pair refs untouched.
        assert!(after.last_compare_ref.is_none());
        assert!(after.last_recommendation_ref.is_none());
    }

    #[tokio::test]
    async fn evaluate_candidate_against_suite_skips_recommendation_when_disabled() {
        let registry_dir = tempdir().unwrap();
        let harness_dir = tempdir().unwrap();
        let registry =
            StrategyRegistryService::new(StrategyRegistryStore::new(registry_dir.path()));
        let harness_store = HarnessReportStore::new(harness_dir.path());

        let candidate = registry
            .register_manual("suite-eval-2".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let evaluator = CandidateEvaluator::new(registry.clone(), harness_store);
        let corpus = synthetic_corpus("noop");

        let outcome = evaluator
            .evaluate_candidate_against_suite(
                &candidate.identity.strategy_id,
                "suite-noop",
                &corpus,
                std::collections::BTreeMap::new(),
                None,
                false,
            )
            .await
            .unwrap();

        assert!(outcome.recommendation.is_none());
        assert!(outcome.candidate_after.last_suite_evaluation_ref.is_some());
        assert!(outcome
            .candidate_after
            .last_suite_recommendation_ref
            .is_none());
    }

    #[tokio::test]
    async fn inspect_progress_classifies_pristine_candidate_as_load_baseline_report() {
        let registry_dir = tempdir().unwrap();
        let harness_dir = tempdir().unwrap();
        let registry =
            StrategyRegistryService::new(StrategyRegistryStore::new(registry_dir.path()));
        let harness_store = HarnessReportStore::new(harness_dir.path());
        let candidate = registry
            .register_manual("inspect-1".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let evaluator = CandidateEvaluator::new(registry.clone(), harness_store);
        let progress = evaluator
            .inspect_evaluation_progress(&candidate.identity.strategy_id, "b", "c")
            .await
            .unwrap();
        assert!(!progress.compare_ref_present_for_inputs);
        assert!(!progress.recommendation_ref_present);
        assert_eq!(
            progress.resume_from,
            Some(EvaluationStep::LoadBaselineReport)
        );
    }

    #[tokio::test]
    async fn inspect_progress_after_full_evaluation_is_complete() {
        let registry_dir = tempdir().unwrap();
        let harness_dir = tempdir().unwrap();
        let registry =
            StrategyRegistryService::new(StrategyRegistryStore::new(registry_dir.path()));
        let harness_store = HarnessReportStore::new(harness_dir.path());

        let baseline = synthetic_report("inspect-base");
        let candidate_report = synthetic_report("inspect-cand");
        harness_store.save(&baseline).await.unwrap();
        harness_store.save(&candidate_report).await.unwrap();

        let candidate = registry
            .register_manual("inspect-2".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let evaluator = CandidateEvaluator::new(registry.clone(), harness_store);
        evaluator
            .evaluate_candidate(
                &candidate.identity.strategy_id,
                "inspect-base",
                "inspect-cand",
                None,
            )
            .await
            .unwrap();

        let progress = evaluator
            .inspect_evaluation_progress(
                &candidate.identity.strategy_id,
                "inspect-base",
                "inspect-cand",
            )
            .await
            .unwrap();
        assert!(progress.compare_ref_present_for_inputs);
        assert!(progress.recommendation_ref_present);
        assert!(progress.recommendation_ref_not_stale);
        assert_eq!(progress.resume_from, None);
    }

    #[tokio::test]
    async fn evaluate_candidate_is_idempotent_under_retry() {
        // Re-running with the same triple must not error out
        // and must leave registry state consistent.
        let registry_dir = tempdir().unwrap();
        let harness_dir = tempdir().unwrap();
        let registry =
            StrategyRegistryService::new(StrategyRegistryStore::new(registry_dir.path()));
        let harness_store = HarnessReportStore::new(harness_dir.path());
        harness_store
            .save(&synthetic_report("idem-base"))
            .await
            .unwrap();
        harness_store
            .save(&synthetic_report("idem-cand"))
            .await
            .unwrap();
        let candidate = registry
            .register_manual("idem-1".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let evaluator = CandidateEvaluator::new(registry.clone(), harness_store);

        let first = evaluator
            .evaluate_candidate(
                &candidate.identity.strategy_id,
                "idem-base",
                "idem-cand",
                None,
            )
            .await
            .unwrap();
        let second = evaluator
            .evaluate_candidate(
                &candidate.identity.strategy_id,
                "idem-base",
                "idem-cand",
                None,
            )
            .await
            .unwrap();
        // Same recommendation decision (synthetic reports are
        // identical -> Promote both times).
        assert_eq!(
            first.recommendation.decision,
            second.recommendation.decision
        );
        // Registry shows the most recent ref pair.
        let after = second.candidate_after;
        let cmp = after.last_compare_ref.unwrap();
        assert_eq!(cmp.baseline_run_id, "idem-base");
        assert_eq!(cmp.candidate_run_id, "idem-cand");
    }
}
