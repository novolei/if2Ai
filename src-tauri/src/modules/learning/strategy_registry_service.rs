//! Candidate strategy registry service (Phase M5-A).
//!
//! Thin orchestration layer over [`super::strategy_registry`] +
//! [`super::strategy_registry_store`].  Owns the
//! `ReflectionNote -> CandidateStrategy` seam, the
//! `CompareRef` attachment seam, and the `RecommendationRef`
//! attachment seam.
//!
//! Honest scope of this module:
//!
//! - **No promotion.**  M5-A intentionally **does not** flip a
//!   `Recommended` candidate to active state.  That is M5-B
//!   territory.  The registry only records what compare /
//!   recommendation has been observed.
//! - **No automatic mutation of active strategy / policy.**  The
//!   service never calls back into the production code path —
//!   registry mutations are operator-driven (via IPC).
//! - **No active background producer.**  There is no in-tree
//!   caller that automatically generates `ReflectionNote`s
//!   today; producers must hand the note in explicitly.  This
//!   is by design — M5-A only opens the seam.
//! - **Pure projection** of [`crate::modules::harness::Recommendation`]
//!   into [`super::strategy_registry::RecommendationRef`].  We do
//!   not re-run the gate, do not re-interpret reason codes.
//!
//! Out of scope for M5-A:
//!
//! - Auto-promote / auto-rollback.
//! - Conflict resolution between concurrent registrations.
//! - Lifecycle queries beyond `list / load`.
//! - Per-corpus-tier weighted policy.

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use super::reflection::Reflection;
use super::reflection_note::{from_legacy_reflection, ReflectionNote};
use super::strategy_registry::{
    source_from_reflection, CandidateStrategy, CompareRef, CompareTarget, RecommendationRef,
    RolloutState, StrategyDefinition, StrategyIdentity, StrategySource, SuiteEvaluationRef,
};
use super::strategy_registry_store::{RegistryPersistenceError, StrategyRegistryStore};
use crate::modules::harness::{
    HarnessReportStore, Recommendation, SuiteGrade, SuiteReport, HARNESS_COMPARE_VERSION,
};

/// Service-layer error family.  Distinct from
/// [`RegistryPersistenceError`] so callers can render a clear
/// "ill-formed input" vs "store IO" distinction without
/// pattern-matching IO sub-errors.
#[derive(Debug, Error)]
pub enum StrategyRegistryError {
    #[error("persistence error: {0}")]
    Persistence(#[from] RegistryPersistenceError),
    #[error("ill-formed reflection note: missing note_id / summary / evidence")]
    IllFormedReflectionNote,
    #[error("strategy not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("cross-store validation failed: {0}")]
    CrossStoreValidation(String),
}

/// Optional input parameters for
/// [`StrategyRegistryService::register_from_reflection`] that the
/// caller may want to pin (operator label override, definition
/// pointer, target run ids, etc.).  `Default` ⇒ everything
/// inherited from the reflection note.
#[derive(Debug, Clone, Default)]
pub struct RegisterFromReflectionOpts {
    /// Override the auto-generated `strategy_id`.  `None` ⇒
    /// generate a fresh UUID.
    pub strategy_id: Option<String>,
    /// Override the auto-derived label (default: derived from
    /// the reflection note's `summary`).
    pub label: Option<String>,
    /// Pinned policy version the candidate targets.
    pub policy_version: Option<String>,
    /// Pinned `definition_ref` (e.g. file path / git sha).
    pub definition_ref: Option<String>,
    /// Optional initial compare intent.
    pub compare_target: Option<CompareTarget>,
    /// Free-form operator notes.
    pub notes: Option<String>,
}

/// Optional input parameters for
/// [`StrategyRegistryService::register_manual`].
#[derive(Debug, Clone, Default)]
pub struct RegisterManualOpts {
    pub strategy_id: Option<String>,
    pub policy_version: Option<String>,
    pub definition_ref: Option<String>,
    pub compare_target: Option<CompareTarget>,
    pub notes: Option<String>,
}

/// Service handle.  Cheap to construct (just wraps a
/// [`StrategyRegistryStore`]).  Holds **no** in-memory state —
/// every operation reads & writes the store.
#[derive(Debug, Clone)]
pub struct StrategyRegistryService {
    store: StrategyRegistryStore,
}

impl StrategyRegistryService {
    /// Construct a service with an explicit store (tests).
    #[must_use]
    pub fn new(store: StrategyRegistryStore) -> Self {
        Self { store }
    }

    /// Construct a service rooted at the platform default
    /// learning data directory.
    #[must_use]
    pub fn with_default_root() -> Self {
        Self::new(StrategyRegistryStore::with_default_root())
    }

    /// Read-only access to the underlying store (for advanced
    /// callers that need raw `list / load / delete` semantics).
    #[must_use]
    pub fn store(&self) -> &StrategyRegistryStore {
        &self.store
    }

    // ───────────────────────── register seams ─────────────────────────

    /// Register a fresh `Draft` candidate from a structured
    /// [`ReflectionNote`].  This is the **single official seam**
    /// from M3.7 reflection notes into the M5 candidate registry.
    ///
    /// Hard rules:
    ///
    /// - Reflection note MUST be well-formed (non-empty
    ///   `note_id`, `summary`, `evidence`); otherwise returns
    ///   [`StrategyRegistryError::IllFormedReflectionNote`].
    /// - The new record starts in `RolloutState::Draft`.  This
    ///   call **never** mutates active strategy or policy state.
    /// - The reflection note's `proposed_strategy.proposal_id`
    ///   is recorded as `definition_ref` when the caller does
    ///   not override it (lets reviewers re-resolve the
    ///   originating proposal).
    pub async fn register_from_reflection(
        &self,
        note: &ReflectionNote,
        opts: RegisterFromReflectionOpts,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        if !note.is_well_formed() {
            return Err(StrategyRegistryError::IllFormedReflectionNote);
        }
        let strategy_id = opts
            .strategy_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let label = opts.label.unwrap_or_else(|| derive_label(note));
        let definition_ref = opts.definition_ref.or_else(|| {
            note.proposed_strategy
                .as_ref()
                .map(|p| p.proposal_id.clone())
        });
        let identity = StrategyIdentity {
            strategy_id,
            label,
            policy_version: opts.policy_version,
            definition_ref,
        };
        let now = Utc::now();
        let mut record = CandidateStrategy::new_draft(
            identity,
            source_from_reflection(note),
            Some(note.note_id.clone()),
            now,
        );
        record.compare_target = opts.compare_target;
        record.notes = opts.notes;
        self.store.save(&record).await?;
        Ok(record)
    }

    /// Phase M5-B — first auto producer path.  Convert legacy
    /// [`Reflection`] outputs from `ReflectionEngine::analyze_session`
    /// into [`ReflectionNote`]s and register each as a `Draft`
    /// candidate.  Returns the (possibly empty) vector of
    /// records that were successfully created.
    ///
    /// Honest scope:
    ///
    /// - Skips legacy reflections whose `pattern` is empty (no
    ///   well-formed note can be derived).
    /// - Each registered candidate carries
    ///   `source = StrategySource::Reflection { note_id }` so
    ///   the audit trail makes the auto-producer origin
    ///   explicit; the `note_id` is `legacy:<uuid>` (see
    ///   [`from_legacy_reflection`]).
    /// - Never mutates active strategy / policy state.
    /// - On the first persistence failure, the partially-saved
    ///   batch is returned alongside the error (best-effort
    ///   batch — caller may inspect what landed).
    pub async fn auto_register_from_legacy_reflections(
        &self,
        reflections: &[Reflection],
        per_record_opts: RegisterFromReflectionOpts,
    ) -> Result<Vec<CandidateStrategy>, StrategyRegistryError> {
        let mut out = Vec::with_capacity(reflections.len());
        for legacy in reflections {
            let Some(note) = from_legacy_reflection(legacy) else {
                continue;
            };
            // Each candidate gets a fresh strategy_id; the
            // operator-supplied opts apply uniformly to every
            // produced record (label override is intentionally
            // **not** propagated so each candidate keeps its
            // own derived label).
            let mut opts = per_record_opts.clone();
            opts.strategy_id = None;
            opts.label = None;
            let record = self.register_from_reflection(&note, opts).await?;
            out.push(record);
        }
        Ok(out)
    }

    /// Register a fresh `Draft` candidate manually (no
    /// reflection backing).  Used by operators / tests.
    pub async fn register_manual(
        &self,
        label: String,
        opts: RegisterManualOpts,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        if label.trim().is_empty() {
            return Err(StrategyRegistryError::InvalidInput(
                "label must not be empty".into(),
            ));
        }
        let strategy_id = opts
            .strategy_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let identity = StrategyIdentity {
            strategy_id,
            label,
            policy_version: opts.policy_version,
            definition_ref: opts.definition_ref,
        };
        let now = Utc::now();
        let mut record =
            CandidateStrategy::new_draft(identity, StrategySource::Manual, None, now);
        record.compare_target = opts.compare_target;
        record.notes = opts.notes;
        self.store.save(&record).await?;
        Ok(record)
    }

    // ───────────────────────── attach seams ───────────────────────────

    /// Attach a [`CompareRef`] to an existing candidate.  Records
    /// "this candidate has been compared against this baseline";
    /// transitions `rollout_state` to `Compared` (operator
    /// terminal `Rejected` / `Deprecated` and gate-decided
    /// `PromotionReady` / `PromotionBlocked` / `PromotedCandidate`
    /// are preserved — re-attaching compare alone never undoes a
    /// gate verdict).
    ///
    /// `baseline_run_id` / `candidate_run_id` are taken at face
    /// value — this method does **not** verify they exist in the
    /// harness report store.  Use
    /// [`Self::attach_compare_ref_validated`] (M5-B) to reject
    /// `run_id`s that are not present in the harness report
    /// store.
    pub async fn attach_compare_ref(
        &self,
        strategy_id: &str,
        baseline_run_id: String,
        candidate_run_id: String,
        recorded_at: Option<DateTime<Utc>>,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        self.attach_compare_ref_inner(
            strategy_id,
            baseline_run_id,
            candidate_run_id,
            recorded_at,
            None,
        )
        .await
    }

    /// Phase M5-B — cross-store-validated variant of
    /// [`Self::attach_compare_ref`].  Refuses to record a
    /// compare reference unless **both** `baseline_run_id` and
    /// `candidate_run_id` exist in the supplied harness report
    /// store.  This is the first cross-store guard between the
    /// candidate registry and the M4 governance pipeline.
    pub async fn attach_compare_ref_validated(
        &self,
        strategy_id: &str,
        baseline_run_id: String,
        candidate_run_id: String,
        harness_store: &HarnessReportStore,
        recorded_at: Option<DateTime<Utc>>,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        self.attach_compare_ref_inner(
            strategy_id,
            baseline_run_id,
            candidate_run_id,
            recorded_at,
            Some(harness_store),
        )
        .await
    }

    async fn attach_compare_ref_inner(
        &self,
        strategy_id: &str,
        baseline_run_id: String,
        candidate_run_id: String,
        recorded_at: Option<DateTime<Utc>>,
        harness_store: Option<&HarnessReportStore>,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        if baseline_run_id.is_empty() || candidate_run_id.is_empty() {
            return Err(StrategyRegistryError::InvalidInput(
                "baseline_run_id and candidate_run_id must be non-empty".into(),
            ));
        }
        if let Some(store) = harness_store {
            verify_run_id_present(store, &baseline_run_id, "baseline_run_id").await?;
            verify_run_id_present(store, &candidate_run_id, "candidate_run_id").await?;
        }
        let mut record = self.load_required(strategy_id).await?;
        let now = recorded_at.unwrap_or_else(Utc::now);
        let new_ref = CompareRef {
            baseline_run_id,
            candidate_run_id,
            compare_version: HARNESS_COMPARE_VERSION.to_string(),
            recorded_at: now,
        };
        record.compare_history.push(new_ref.clone());
        record.last_compare_ref = Some(new_ref);
        record.rollout_state = advance_state_on_compare(record.rollout_state);
        record.updated_at = now;
        self.store.save(&record).await?;
        Ok(record)
    }

    /// Attach a [`RecommendationRef`] (projected from a real
    /// gate [`Recommendation`]) to an existing candidate.
    /// Transitions `rollout_state` to `Recommended` unless the
    /// candidate is already in a terminal operator state
    /// (`Rejected` / `Deprecated`), which are preserved.
    ///
    /// **No promotion** — even when the gate decision is
    /// `Promote`, the registry only records the recommendation.
    /// Active rollout is M5-B territory.
    pub async fn attach_recommendation(
        &self,
        strategy_id: &str,
        recommendation: &Recommendation,
        recorded_at: Option<DateTime<Utc>>,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        let mut record = self.load_required(strategy_id).await?;
        let now = recorded_at.unwrap_or_else(Utc::now);
        let new_ref = RecommendationRef::from_recommendation(recommendation, now);
        record.recommendation_history.push(new_ref.clone());
        record.last_recommendation_ref = Some(new_ref);
        record.rollout_state = advance_state_on_recommendation(record.rollout_state);
        record.updated_at = now;
        self.store.save(&record).await?;
        Ok(record)
    }

    /// Phase M5-C round 2 — attach a suite-level evaluation
    /// reference + (optional) suite-driven recommendation to an
    /// existing candidate.  Mirrors
    /// [`Self::attach_compare_ref`] / [`Self::attach_recommendation`]
    /// for the suite-evaluation track.
    ///
    /// Pure projection of a [`SuiteReport`] (+ optional
    /// [`Recommendation`]) into the registry-side
    /// [`SuiteEvaluationRef`] / [`RecommendationRef`].  Does
    /// **not** re-aggregate the suite, does **not** re-run the
    /// gate.
    ///
    /// State machine: same preservation rules as
    /// [`advance_state_on_recommendation`] when a recommendation
    /// is supplied (operator-terminal + rollout-managed states
    /// preserved; gate-decided states reset to `Recommended`);
    /// when no recommendation is supplied, state is preserved
    /// (the suite-eval ref is treated as a non-mutating audit
    /// record).
    ///
    /// Idempotent under repeated calls with the same
    /// `(strategy_id, suite_id)` — the registry overwrites
    /// `last_suite_evaluation_ref` / `last_suite_recommendation_ref`.
    pub async fn attach_suite_evaluation_ref(
        &self,
        strategy_id: &str,
        suite_report: &SuiteReport,
        suite_recommendation: Option<&Recommendation>,
        recorded_at: Option<DateTime<Utc>>,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        let mut record = self.load_required(strategy_id).await?;
        let now = recorded_at.unwrap_or_else(Utc::now);
        let new_suite_ref = suite_evaluation_ref_from_report(suite_report, now);
        record.suite_evaluation_history.push(new_suite_ref.clone());
        record.last_suite_evaluation_ref = Some(new_suite_ref);
        if let Some(rec) = suite_recommendation {
            let new_rec_ref = RecommendationRef::from_recommendation(rec, now);
            record.suite_recommendation_history.push(new_rec_ref.clone());
            record.last_suite_recommendation_ref = Some(new_rec_ref);
            record.rollout_state = advance_state_on_recommendation(record.rollout_state);
        }
        record.updated_at = now;
        self.store.save(&record).await?;
        Ok(record)
    }

    /// Phase M5 closeout — set the candidate's executable
    /// [`StrategyDefinition`].  Used by operators / IPC to
    /// upgrade a `Noop`-default candidate to one that the
    /// active overlay resolver can actually project.
    ///
    /// Refused on operator-terminal states.  Persists
    /// atomically.  Future M5-D may add validation that the
    /// definition shape matches the candidate's
    /// `policy_version` pin.
    pub async fn set_definition(
        &self,
        strategy_id: &str,
        definition: StrategyDefinition,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        if let StrategyDefinition::PromptOverlay { ref text } = definition {
            if text.trim().is_empty() {
                return Err(StrategyRegistryError::InvalidInput(
                    "PromptOverlay.text must be non-empty".into(),
                ));
            }
        }
        if let StrategyDefinition::DiscourageTool { ref tool_name } = definition {
            if tool_name.trim().is_empty() {
                return Err(StrategyRegistryError::InvalidInput(
                    "DiscourageTool.tool_name must be non-empty".into(),
                ));
            }
        }
        let mut record = self.load_required(strategy_id).await?;
        if record.rollout_state.is_operator_terminal() {
            return Err(StrategyRegistryError::InvalidInput(format!(
                "candidate is in operator-terminal state {:?}; refusing definition update",
                record.rollout_state
            )));
        }
        record.definition = definition;
        record.updated_at = Utc::now();
        self.store.save(&record).await?;
        Ok(record)
    }

    /// Explicit operator transition.  Allowed targets:
    ///
    /// - `Draft` (rare; mostly used in test fixtures / resets)
    /// - `Candidate` (operator earmarks for evaluation)
    /// - `Deprecated` (operator sets aside)
    /// - `Rejected` (operator hard-refusal)
    ///
    /// Refused with `InvalidInput`:
    ///
    /// - `Compared` / `Recommended` — managed by the attach
    ///   seams (`attach_compare_ref` / `attach_recommendation`).
    /// - `PromotionReady` / `PromotionBlocked` / `PromotedCandidate`
    ///   — managed by the M5-B promotion gate (see
    ///   [`super::promotion_gate`]).  Operators must not bypass
    ///   the gate.
    pub async fn set_state(
        &self,
        strategy_id: &str,
        next: RolloutState,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        match next {
            RolloutState::Compared
            | RolloutState::Recommended
            | RolloutState::PromotionReady
            | RolloutState::PromotionBlocked
            | RolloutState::PromotedCandidate
            | RolloutState::Active
            | RolloutState::RolledBack => {
                return Err(StrategyRegistryError::InvalidInput(format!(
                    "operator may not set state directly to {next:?}; use the attach / promotion_gate / strategy_rollout seams"
                )));
            }
            _ => {}
        }
        let mut record = self.load_required(strategy_id).await?;
        record.rollout_state = next;
        record.updated_at = Utc::now();
        self.store.save(&record).await?;
        Ok(record)
    }

    /// Phase M5-B — internal seam used by the promotion gate to
    /// transition the candidate into one of the gate-managed
    /// states.  **Not** part of the public IPC; operators must
    /// go through [`super::promotion_gate`] which wraps this
    /// with the actual eligibility logic.
    pub(crate) async fn force_state_internal(
        &self,
        strategy_id: &str,
        next: RolloutState,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        let mut record = self.load_required(strategy_id).await?;
        if record.rollout_state.is_operator_terminal() {
            return Err(StrategyRegistryError::InvalidInput(format!(
                "candidate is in operator-terminal state {:?}; refusing to overwrite",
                record.rollout_state
            )));
        }
        record.rollout_state = next;
        record.updated_at = Utc::now();
        self.store.save(&record).await?;
        Ok(record)
    }

    /// Replace the operator-stated `compare_target` (intent
    /// only; does **not** touch `last_compare_ref`).
    pub async fn set_compare_target(
        &self,
        strategy_id: &str,
        target: Option<CompareTarget>,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        let mut record = self.load_required(strategy_id).await?;
        record.compare_target = target;
        record.updated_at = Utc::now();
        self.store.save(&record).await?;
        Ok(record)
    }

    /// Append-style notes update (replaces the field).
    pub async fn set_notes(
        &self,
        strategy_id: &str,
        notes: Option<String>,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        let mut record = self.load_required(strategy_id).await?;
        record.notes = notes;
        record.updated_at = Utc::now();
        self.store.save(&record).await?;
        Ok(record)
    }

    // ───────────────────────── helpers ────────────────────────────────

    async fn load_required(
        &self,
        strategy_id: &str,
    ) -> Result<CandidateStrategy, StrategyRegistryError> {
        match self.store.load(strategy_id).await? {
            Some(r) => Ok(r),
            None => Err(StrategyRegistryError::NotFound(strategy_id.to_string())),
        }
    }
}

/// Default label derivation: truncate the reflection note's
/// summary to a reasonable length so registry lists stay
/// readable.  Keeps Unicode boundaries intact.
fn derive_label(note: &ReflectionNote) -> String {
    const MAX_CHARS: usize = 80;
    let s: String = note.summary.chars().take(MAX_CHARS).collect();
    if note.summary.chars().count() > MAX_CHARS {
        format!("{s}…")
    } else {
        s
    }
}

/// State machine for `attach_compare_ref`.  Preserves:
///
/// - operator terminal states (`Rejected` / `Deprecated`)
/// - already-`Recommended` (re-attaching compare alone never
///   un-recommends)
/// - gate-decided states (`PromotionReady` / `PromotionBlocked`
///   / `PromotedCandidate`) — re-attaching a compare ref alone
///   is not enough to invalidate a gate verdict; the operator
///   must run `attach_recommendation` again to refresh.
/// - rollout-managed states (`Active` / `RolledBack`) — once
///   activated, attaching a fresh compare for audit purposes
///   does not silently demote the active strategy.  Operators
///   that want to re-evaluate must explicitly rollback first.
fn advance_state_on_compare(current: RolloutState) -> RolloutState {
    if current.is_operator_terminal()
        || current.is_gate_decided()
        || current.is_rollout_managed()
    {
        return current;
    }
    match current {
        RolloutState::Recommended => RolloutState::Recommended,
        _ => RolloutState::Compared,
    }
}

/// State machine for `attach_recommendation`.  Preserves
/// operator terminal **and** rollout-managed states (a fresh
/// recommendation on an already-active strategy does not
/// auto-demote); resets any gate-decided state (a fresh
/// recommendation invalidates the prior verdict and requires
/// the operator to re-run the promotion gate); bumps every
/// other non-terminal state to `Recommended`.
fn advance_state_on_recommendation(current: RolloutState) -> RolloutState {
    if current.is_operator_terminal() || current.is_rollout_managed() {
        return current;
    }
    RolloutState::Recommended
}

/// Phase M5-C round 2 — projection helper.  Mirrors the
/// `RecommendationRef::from_recommendation` pattern.  Pure;
/// no IO.
fn suite_evaluation_ref_from_report(
    suite: &SuiteReport,
    recorded_at: DateTime<Utc>,
) -> SuiteEvaluationRef {
    SuiteEvaluationRef {
        suite_id: suite.suite_id.clone(),
        corpus_name: suite.corpus_name.clone(),
        corpus_version: suite.corpus_version.clone(),
        suite_report_version: suite.suite_report_version.clone(),
        suite_grade: suite_grade_label(suite.grade).to_string(),
        task_count: suite.tasks.len(),
        regression_count: suite.overall.regression,
        recorded_at,
    }
}

fn suite_grade_label(g: SuiteGrade) -> &'static str {
    match g {
        SuiteGrade::Pass => "pass",
        SuiteGrade::Warning => "warning",
        SuiteGrade::Fail => "fail",
        SuiteGrade::Skeleton => "skeleton",
    }
}

/// Phase M5-B — first cross-store guard.  Refuses when the
/// `run_id` cannot be loaded from the supplied harness report
/// store; surfaces parse errors verbatim so corrupt reports do
/// not silently pass.
async fn verify_run_id_present(
    store: &HarnessReportStore,
    run_id: &str,
    field: &'static str,
) -> Result<(), StrategyRegistryError> {
    match store.load(run_id).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(StrategyRegistryError::CrossStoreValidation(format!(
            "{field} '{run_id}' not present in harness report store"
        ))),
        Err(e) => Err(StrategyRegistryError::CrossStoreValidation(format!(
            "{field} '{run_id}' load failed: {e}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::HARNESS_GATE_VERSION;
    use crate::modules::learning::reflection_note::{
        ReflectionEvidenceRef, ReflectionIssueType, ReflectionNote, StrategyProposal,
        REFLECTION_NOTE_VERSION,
    };
    use crate::modules::runtime::contracts::execution_mode::RiskLevel;
    use tempfile::tempdir;

    fn sample_note() -> ReflectionNote {
        ReflectionNote {
            note_id: "note-svc-1".into(),
            issue_type: ReflectionIssueType::ToolMismatch,
            summary: "prefer rg over grep".into(),
            evidence: vec![ReflectionEvidenceRef {
                trace_id: "turn-3".into(),
                kind: "tool_chosen".into(),
                excerpt: None,
            }],
            proposed_strategy: Some(StrategyProposal {
                proposal_id: "use-rg-by-default".into(),
                summary: "prefer rg".into(),
                definition_ref: None,
            }),
            expected_gain: Some("latency_reduction".into()),
            risk_level: RiskLevel::Low,
            created_at: "2026-04-21T00:00:00+00:00".into(),
            contract_version: REFLECTION_NOTE_VERSION.into(),
        }
    }

    fn fixture_recommendation(decision: &str) -> Recommendation {
        let json = serde_json::json!({
            "gateVersion": HARNESS_GATE_VERSION,
            "policyId": "default-conservative-m4.8",
            "decision": decision,
            "reasonCodes": ["promote_no_regression"],
            "summary": format!("decision = {decision}"),
            "blockingGraderIds": [],
            "blockingFailureCodes": [],
            "weightedScore": 0.0,
        });
        serde_json::from_value(json).unwrap()
    }

    fn svc() -> (StrategyRegistryService, tempfile::TempDir) {
        let tmp = tempdir().unwrap();
        let svc = StrategyRegistryService::new(StrategyRegistryStore::new(tmp.path()));
        (svc, tmp)
    }

    #[tokio::test]
    async fn register_from_reflection_creates_draft() {
        let (svc, _tmp) = svc();
        let note = sample_note();
        let r = svc
            .register_from_reflection(&note, RegisterFromReflectionOpts::default())
            .await
            .unwrap();
        assert_eq!(r.rollout_state, RolloutState::Draft);
        assert_eq!(r.based_on_reflection_note.as_deref(), Some("note-svc-1"));
        assert!(matches!(r.source, StrategySource::Reflection { .. }));
        assert_eq!(r.identity.definition_ref.as_deref(), Some("use-rg-by-default"));
    }

    #[tokio::test]
    async fn register_from_reflection_refuses_ill_formed() {
        let (svc, _tmp) = svc();
        let mut note = sample_note();
        note.evidence.clear();
        let res = svc
            .register_from_reflection(&note, RegisterFromReflectionOpts::default())
            .await;
        assert!(matches!(
            res,
            Err(StrategyRegistryError::IllFormedReflectionNote)
        ));
    }

    #[tokio::test]
    async fn attach_compare_ref_advances_state() {
        let (svc, _tmp) = svc();
        let r = svc
            .register_manual("manual-1".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let after = svc
            .attach_compare_ref(
                &r.identity.strategy_id,
                "base-1".into(),
                "cand-1".into(),
                None,
            )
            .await
            .unwrap();
        assert_eq!(after.rollout_state, RolloutState::Compared);
        let cmp = after.last_compare_ref.unwrap();
        assert_eq!(cmp.baseline_run_id, "base-1");
        assert_eq!(cmp.candidate_run_id, "cand-1");
    }

    #[tokio::test]
    async fn attach_recommendation_advances_state_but_does_not_promote() {
        let (svc, _tmp) = svc();
        let r = svc
            .register_manual("manual-2".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let rec = fixture_recommendation("promote");
        let after = svc
            .attach_recommendation(&r.identity.strategy_id, &rec, None)
            .await
            .unwrap();
        // Even with `promote`, M5-A only records — never auto-activates.
        assert_eq!(after.rollout_state, RolloutState::Recommended);
        let r_ref = after.last_recommendation_ref.unwrap();
        assert_eq!(r_ref.decision, "promote");
        assert_eq!(r_ref.policy_id, "default-conservative-m4.8");
    }

    #[tokio::test]
    async fn rejected_state_is_preserved_across_attach() {
        let (svc, _tmp) = svc();
        let r = svc
            .register_manual("manual-3".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        svc.set_state(&r.identity.strategy_id, RolloutState::Rejected)
            .await
            .unwrap();
        let after_cmp = svc
            .attach_compare_ref(
                &r.identity.strategy_id,
                "b".into(),
                "c".into(),
                None,
            )
            .await
            .unwrap();
        assert_eq!(after_cmp.rollout_state, RolloutState::Rejected);
        let rec = fixture_recommendation("hold");
        let after_rec = svc
            .attach_recommendation(&r.identity.strategy_id, &rec, None)
            .await
            .unwrap();
        assert_eq!(after_rec.rollout_state, RolloutState::Rejected);
    }

    #[tokio::test]
    async fn set_state_refuses_compared_or_recommended() {
        let (svc, _tmp) = svc();
        let r = svc
            .register_manual("manual-4".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let res = svc
            .set_state(&r.identity.strategy_id, RolloutState::Compared)
            .await;
        assert!(matches!(res, Err(StrategyRegistryError::InvalidInput(_))));
        let res = svc
            .set_state(&r.identity.strategy_id, RolloutState::Recommended)
            .await;
        assert!(matches!(res, Err(StrategyRegistryError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn missing_strategy_load_surfaces_not_found() {
        let (svc, _tmp) = svc();
        let res = svc
            .attach_compare_ref("nope", "b".into(), "c".into(), None)
            .await;
        assert!(matches!(res, Err(StrategyRegistryError::NotFound(_))));
    }

    // ──────────────── M5-B additions ────────────────

    #[tokio::test]
    async fn auto_register_from_legacy_reflections_skips_empty_patterns() {
        use chrono::Utc;
        let (svc, _tmp) = svc();
        let reflections = vec![
            Reflection {
                pattern: "".into(),
                insight: "".into(),
                confidence: 0.0,
                source_session: "sess-1".into(),
                timestamp: Utc::now(),
            },
            Reflection {
                pattern: "Tool errors: bash".into(),
                insight: "3 errors in this session".into(),
                confidence: 0.6,
                source_session: "sess-1".into(),
                timestamp: Utc::now(),
            },
            Reflection {
                pattern: "Tool sequence: read -> write".into(),
                insight: "Observed 4 times".into(),
                confidence: 0.4,
                source_session: "sess-1".into(),
                timestamp: Utc::now(),
            },
        ];
        let recs = svc
            .auto_register_from_legacy_reflections(
                &reflections,
                RegisterFromReflectionOpts::default(),
            )
            .await
            .unwrap();
        assert_eq!(recs.len(), 2);
        for r in &recs {
            assert_eq!(r.rollout_state, RolloutState::Draft);
            assert!(matches!(r.source, StrategySource::Reflection { .. }));
        }
    }

    #[tokio::test]
    async fn attach_compare_ref_validated_refuses_unknown_run_id() {
        use crate::modules::harness::HarnessReportStore;
        let (svc, _tmp) = svc();
        let r = svc
            .register_manual("manual-validated".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let store_dir = tempdir().unwrap();
        let harness_store = HarnessReportStore::new(store_dir.path());
        let res = svc
            .attach_compare_ref_validated(
                &r.identity.strategy_id,
                "missing-base".into(),
                "missing-cand".into(),
                &harness_store,
                None,
            )
            .await;
        assert!(matches!(
            res,
            Err(StrategyRegistryError::CrossStoreValidation(_))
        ));
    }

    #[tokio::test]
    async fn attach_recommendation_resets_gate_decided_state() {
        let (svc, _tmp) = svc();
        let r = svc
            .register_manual("manual-reset".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        // Force into PromotionReady through the internal seam.
        svc.force_state_internal(&r.identity.strategy_id, RolloutState::PromotionReady)
            .await
            .unwrap();
        let rec = fixture_recommendation("hold");
        let after = svc
            .attach_recommendation(&r.identity.strategy_id, &rec, None)
            .await
            .unwrap();
        // Re-attaching a fresh recommendation must reset the
        // gate verdict — operator must re-run promotion gate.
        assert_eq!(after.rollout_state, RolloutState::Recommended);
    }

    #[tokio::test]
    async fn set_state_refuses_gate_managed_states() {
        let (svc, _tmp) = svc();
        let r = svc
            .register_manual("manual-gate".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        for s in [
            RolloutState::PromotionReady,
            RolloutState::PromotionBlocked,
            RolloutState::PromotedCandidate,
            RolloutState::Active,
            RolloutState::RolledBack,
        ] {
            let res = svc.set_state(&r.identity.strategy_id, s).await;
            assert!(
                matches!(res, Err(StrategyRegistryError::InvalidInput(_))),
                "{s:?} should be refused"
            );
        }
    }
}
