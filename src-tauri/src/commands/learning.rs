//! Learning IPC commands (Phase M5-A).
//!
//! Thin Tauri command surface over
//! [`crate::modules::learning::StrategyRegistryService`].  Every
//! command constructs the service on-demand against the platform
//! default root — there is **no** in-memory registry state on
//! [`AppState`], so concurrent IPC calls observe a consistent
//! on-disk view (last writer wins per `strategy_id`, mirroring
//! the M4.6 [`crate::modules::harness`] report store pattern).
//!
//! Honest scope (M5-A):
//!
//! - Open the **registry / reflection-to-candidate / recommendation**
//!   seams.  No promote, no rollback, no auto-evolution.
//! - Recommendation projection is **lossless within the
//!   registry's audit subset**: `gate_version` / `policy_id` /
//!   `decision` / `reason_codes` / `summary`.  Full
//!   `BaselineVsCandidate` / `Recommendation` payloads stay in
//!   the harness layer; callers re-fetch via the existing
//!   `harness_*` IPCs when they need detail.
//! - All commands are **safe to call concurrently** — the
//!   underlying store is file-per-record + atomic rename.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::harness::{
    GatePolicy, HarnessReportStore, Recommendation, RegressionCorpus, SuiteReport,
};
use crate::modules::learning::reflection::ReflectionEngine;
use crate::modules::learning::{
    generate_reflection_notes, score_run_report, ActivateInput, ActiveStrategyOverlay,
    ActiveStrategyOverlayResolver, CandidateEvaluationOutcome, CandidateEvaluator,
    CandidateStrategy, CandidateSuiteEvaluationOutcome, ClusteredFailureSet, CompareTarget,
    EvaluationProgress, PromotionEligibility, PromotionGateOutcome, PromotionGateService,
    ReflectionNote, RegisterFromReflectionOpts, RegisterManualOpts, RollbackInput, RollbackTarget,
    RolloutState, StrategyDefinition, StrategyIndexEntry, StrategyRegistryService,
    StrategyRolloutService, TrajectoryScore,
};

fn svc() -> StrategyRegistryService {
    StrategyRegistryService::with_default_root()
}

fn evaluator() -> CandidateEvaluator {
    CandidateEvaluator::with_default_roots()
}

fn map_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ──────────────────────────────────────────────────────────────────────────────
// Input payloads (camelCase wire shape)
// ──────────────────────────────────────────────────────────────────────────────

/// Wire payload for [`learning_register_candidate_from_reflection`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RegisterFromReflectionInput {
    /// Optional override for the auto-generated UUID.
    #[serde(default)]
    pub strategy_id: Option<String>,
    /// Optional label override; default derives from the note's
    /// `summary`.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub policy_version: Option<String>,
    #[serde(default)]
    pub definition_ref: Option<String>,
    #[serde(default)]
    pub compare_target: Option<CompareTarget>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl From<RegisterFromReflectionInput> for RegisterFromReflectionOpts {
    fn from(v: RegisterFromReflectionInput) -> Self {
        Self {
            strategy_id: v.strategy_id,
            label: v.label,
            policy_version: v.policy_version,
            definition_ref: v.definition_ref,
            compare_target: v.compare_target,
            notes: v.notes,
        }
    }
}

/// Wire payload for [`learning_register_candidate_manual`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RegisterManualInput {
    #[serde(default)]
    pub strategy_id: Option<String>,
    #[serde(default)]
    pub policy_version: Option<String>,
    #[serde(default)]
    pub definition_ref: Option<String>,
    #[serde(default)]
    pub compare_target: Option<CompareTarget>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl From<RegisterManualInput> for RegisterManualOpts {
    fn from(v: RegisterManualInput) -> Self {
        Self {
            strategy_id: v.strategy_id,
            policy_version: v.policy_version,
            definition_ref: v.definition_ref,
            compare_target: v.compare_target,
            notes: v.notes,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// IPC commands
// ──────────────────────────────────────────────────────────────────────────────

/// Phase M5-A — register a fresh `Draft` candidate from a
/// structured [`ReflectionNote`].  Single official seam from
/// M3.7 reflection notes into the candidate registry.
///
/// Refuses ill-formed notes (empty `note_id` / `summary` /
/// `evidence`).  Never mutates active strategy / policy state.
#[tauri::command]
pub async fn learning_register_candidate_from_reflection(
    note: ReflectionNote,
    opts: Option<RegisterFromReflectionInput>,
) -> Result<CandidateStrategy, String> {
    svc()
        .register_from_reflection(&note, opts.unwrap_or_default().into())
        .await
        .map_err(map_err)
}

/// Phase M5-A — register a fresh `Draft` candidate manually
/// (operator-driven, no reflection backing).
#[tauri::command]
pub async fn learning_register_candidate_manual(
    label: String,
    opts: Option<RegisterManualInput>,
) -> Result<CandidateStrategy, String> {
    svc()
        .register_manual(label, opts.unwrap_or_default().into())
        .await
        .map_err(map_err)
}

/// Phase M5-A — list every persisted candidate as a lightweight
/// summary, sorted newest-first by `updatedAt`.  Skips corrupt
/// files silently (logged at WARN by the store).
#[tauri::command]
pub async fn learning_list_candidates() -> Result<Vec<StrategyIndexEntry>, String> {
    svc().store().list().await.map_err(map_err)
}

/// Phase M5-A — load one candidate by `strategy_id`.  Returns
/// `Ok(None)` when the record does not exist.  Returns `Err`
/// when the file exists but cannot be parsed.
#[tauri::command]
pub async fn learning_get_candidate(
    strategy_id: String,
) -> Result<Option<CandidateStrategy>, String> {
    svc().store().load(&strategy_id).await.map_err(map_err)
}

/// Phase M5-A — delete one candidate by `strategy_id`.  Returns
/// `Ok(false)` when the record did not exist (idempotent).
#[tauri::command]
pub async fn learning_delete_candidate(strategy_id: String) -> Result<bool, String> {
    svc().store().delete(&strategy_id).await.map_err(map_err)
}

/// Phase M5-A — attach a [`crate::modules::learning::CompareRef`]
/// to an existing candidate.  Records "this candidate has been
/// compared against this baseline" and bumps `rolloutState` to
/// `Compared` (operator terminal states `Rejected` / `Deprecated`
/// are preserved).
#[tauri::command]
pub async fn learning_attach_compare_ref(
    strategy_id: String,
    baseline_run_id: String,
    candidate_run_id: String,
) -> Result<CandidateStrategy, String> {
    svc()
        .attach_compare_ref(&strategy_id, baseline_run_id, candidate_run_id, None)
        .await
        .map_err(map_err)
}

/// Phase M5-A — attach a gate [`Recommendation`] (typically the
/// return value of `harness_evaluate_compare` /
/// `harness_evaluate_suite`) to an existing candidate.  Stored as
/// a registry-side projection (`RecommendationRef`); the full
/// recommendation can be re-derived by re-running the gate.
///
/// **Never auto-promotes** — even when `decision == "promote"`,
/// the registry only records.  Active rollout is M5-B
/// territory.
#[tauri::command]
pub async fn learning_attach_recommendation(
    strategy_id: String,
    recommendation: Recommendation,
) -> Result<CandidateStrategy, String> {
    svc()
        .attach_recommendation(&strategy_id, &recommendation, None)
        .await
        .map_err(map_err)
}

/// Phase M5-A — explicit operator transition.  Allowed targets:
/// `draft` / `candidate` / `rejected` / `deprecated`.  Requests
/// for `compared` / `recommended` are refused — those states
/// are managed by the attach seams.
#[tauri::command]
pub async fn learning_set_candidate_state(
    strategy_id: String,
    state: RolloutState,
) -> Result<CandidateStrategy, String> {
    svc().set_state(&strategy_id, state).await.map_err(map_err)
}

/// Phase M5-A — replace the operator-stated `compareTarget`
/// (intent only; does **not** touch `lastCompareRef`).
#[tauri::command]
pub async fn learning_set_candidate_compare_target(
    strategy_id: String,
    target: Option<CompareTarget>,
) -> Result<CandidateStrategy, String> {
    svc()
        .set_compare_target(&strategy_id, target)
        .await
        .map_err(map_err)
}

/// Phase M5-A — replace the candidate's free-form operator
/// notes.
#[tauri::command]
pub async fn learning_set_candidate_notes(
    strategy_id: String,
    notes: Option<String>,
) -> Result<CandidateStrategy, String> {
    svc().set_notes(&strategy_id, notes).await.map_err(map_err)
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase M5-B — auto producer / evaluator / promotion gate IPCs
// ──────────────────────────────────────────────────────────────────────────────

/// Phase M5-B — first auto-producer path.  Loads the named
/// session via [`crate::modules::session::SessionManager`],
/// runs the configured [`crate::modules::learning::reflection::ReflectionEngine`]
/// over it, converts each legacy `Reflection` into a
/// well-formed `ReflectionNote`, and registers each as a
/// `Draft` candidate.
///
/// Returns the (possibly empty) vector of candidates that were
/// successfully created.  Reflections whose `pattern` is empty
/// are skipped silently.
///
/// **Honest scope**: the legacy reflection engine emits
/// pattern/insight strings (not yet structured failure
/// taxonomies); the adapter classifies them with a small
/// keyword heuristic.  M5-C `failure_taxonomy.rs` will tighten
/// this without changing the IPC shape.
///
/// Requires:
///   - `state.session_manager` (always present)
///   - `state.learning_module` (must be `Some`); returns an
///     empty vector when absent (graceful no-op).
#[tauri::command]
pub async fn learning_reflect_session_and_register(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<CandidateStrategy>, String> {
    if session_id.trim().is_empty() {
        return Err("session_id must be non-empty".to_string());
    }
    let Some(learning) = state.learning_module.clone() else {
        tracing::warn!(
            target: "learning.auto_producer",
            session_id = %session_id,
            "[reflect_session_and_register] learning_module is None; nothing to register"
        );
        return Ok(Vec::new());
    };
    let persisted = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| format!("failed to load session {session_id}: {e}"))?;
    // Bridge: the reflection engine consumes the runtime
    // `Session` shape; the session manager hands back the
    // persistence shape.  Both share `ConversationMessage`, so
    // we forward the messages and pin a fresh `version`.
    let runtime_session = crate::modules::runtime::session::Session {
        version: 1,
        messages: persisted.messages,
    };

    let reflections = {
        let learning = learning.lock().await;
        learning
            .reflection_engine
            .analyze_session(&runtime_session)
            .await
            .map_err(|e| format!("reflection engine failed: {e}"))?
    };
    // Tag every reflection with the source session so the note
    // adapter can carry it through to the evidence ref.
    let reflections: Vec<crate::modules::learning::reflection::Reflection> = reflections
        .into_iter()
        .map(|mut r| {
            if r.source_session.is_empty() {
                r.source_session = session_id.clone();
            }
            r
        })
        .collect();

    svc()
        .auto_register_from_legacy_reflections(&reflections, RegisterFromReflectionOpts::default())
        .await
        .map_err(map_err)
}

/// Output payload for [`learning_evaluate_candidate`] /
/// [`learning_evaluate_candidate_with_policy`].  Mirrors
/// [`CandidateEvaluationOutcome`] but stable on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningEvaluateCandidateResponse {
    pub compare: crate::modules::harness::BaselineVsCandidate,
    pub recommendation: Recommendation,
    pub candidate_after: CandidateStrategy,
}

impl From<CandidateEvaluationOutcome> for LearningEvaluateCandidateResponse {
    fn from(v: CandidateEvaluationOutcome) -> Self {
        Self {
            compare: v.compare,
            recommendation: v.recommendation,
            candidate_after: v.candidate_after,
        }
    }
}

/// Phase M5-B — drive one candidate through the
/// `compare -> gate -> persist` pipeline using the **default
/// conservative** [`GatePolicy`].
///
/// Refuses when either `baseline_run_id` or `candidate_run_id`
/// is not present in the harness report store (cross-store
/// guard).  Recommendation is recorded as a registry-side ref;
/// **never auto-promotes** — promotion is a separate explicit
/// step (`learning_apply_promotion_gate`).
#[tauri::command]
pub async fn learning_evaluate_candidate(
    strategy_id: String,
    baseline_run_id: String,
    candidate_run_id: String,
) -> Result<LearningEvaluateCandidateResponse, String> {
    evaluator()
        .evaluate_candidate(&strategy_id, &baseline_run_id, &candidate_run_id, None)
        .await
        .map(LearningEvaluateCandidateResponse::from)
        .map_err(map_err)
}

/// Phase M5-B — variant of [`learning_evaluate_candidate`]
/// that accepts an explicit `GatePolicy`.  Useful for
/// regression test fixtures and operator scenarios where the
/// conservative default is not what is being evaluated.  Same
/// no-promote semantics.
#[tauri::command]
pub async fn learning_evaluate_candidate_with_policy(
    strategy_id: String,
    baseline_run_id: String,
    candidate_run_id: String,
    policy: GatePolicy,
) -> Result<LearningEvaluateCandidateResponse, String> {
    evaluator()
        .evaluate_candidate(
            &strategy_id,
            &baseline_run_id,
            &candidate_run_id,
            Some(&policy),
        )
        .await
        .map(LearningEvaluateCandidateResponse::from)
        .map_err(map_err)
}

/// Wire payload for [`learning_apply_promotion_gate`] /
/// [`learning_mark_promoted_candidate`] return value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningPromotionGateResponse {
    pub eligibility: PromotionEligibility,
    pub candidate_after: CandidateStrategy,
}

impl From<PromotionGateOutcome> for LearningPromotionGateResponse {
    fn from(v: PromotionGateOutcome) -> Self {
        Self {
            eligibility: v.eligibility,
            candidate_after: v.candidate_after,
        }
    }
}

/// Phase M5-B — apply the promotion gate to a stored candidate.
///
/// Runs the pure eligibility check and transitions the
/// candidate to `PromotionReady` or `PromotionBlocked`
/// (operator terminal states `Rejected` / `Deprecated` are
/// preserved and surfaced as `Blocked`).
///
/// **No production rollout.**  This only declares whether
/// promotion is allowed.  The operator must call
/// [`learning_mark_promoted_candidate`] separately to mark a
/// `Ready` candidate as the chosen winner — and even that only
/// flips a registry label; production activation is M5-C.
#[tauri::command]
pub async fn learning_apply_promotion_gate(
    strategy_id: String,
) -> Result<LearningPromotionGateResponse, String> {
    let s = svc();
    let gate = PromotionGateService::new(&s);
    gate.apply_eligibility(&strategy_id)
        .await
        .map(LearningPromotionGateResponse::from)
        .map_err(map_err)
}

/// Phase M5-B — explicit operator step that flips a
/// `PromotionReady` candidate to `PromotedCandidate`.  Refuses
/// any other state.
///
/// **No production rollout.**  This is purely a registry-side
/// marker for M5-C to consume; no active strategy is mutated.
#[tauri::command]
pub async fn learning_mark_promoted_candidate(
    strategy_id: String,
) -> Result<LearningPromotionGateResponse, String> {
    let s = svc();
    let gate = PromotionGateService::new(&s);
    gate.mark_promoted_candidate(&strategy_id)
        .await
        .map(LearningPromotionGateResponse::from)
        .map_err(map_err)
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase M5-C round 1 — Active rollout + rollback IPCs
//
// Honest scope: these IPCs flip *registry / governance truth*
// only.  They do NOT mutate the production agent loop
// (commands/agent.rs).  M5-D performs the real production flip.
// ──────────────────────────────────────────────────────────────────────────────

/// Wire payload for [`learning_activate_promoted_candidate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningActivateInput {
    pub strategy_id: String,
    /// Operator / agent identifier (must be non-empty).
    pub activated_by: String,
    /// Optional free-form note recorded on the activation
    /// audit.
    #[serde(default)]
    pub note: Option<String>,
}

impl From<LearningActivateInput> for ActivateInput {
    fn from(v: LearningActivateInput) -> Self {
        Self {
            strategy_id: v.strategy_id,
            activated_by: v.activated_by,
            note: v.note,
        }
    }
}

/// Wire payload for [`learning_rollback_active_strategy`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningRollbackInput {
    pub strategy_id: String,
    /// Operator / agent identifier (must be non-empty).
    pub initiated_by: String,
    /// Non-empty rollback reason.
    pub reason: String,
    /// Optional pointer to the strategy / version we are
    /// rolling **to**.
    #[serde(default)]
    pub target: Option<RollbackTarget>,
}

impl From<LearningRollbackInput> for RollbackInput {
    fn from(v: LearningRollbackInput) -> Self {
        Self {
            strategy_id: v.strategy_id,
            initiated_by: v.initiated_by,
            reason: v.reason,
            target: v.target,
        }
    }
}

/// Phase M5-C round 1 — flip a `PromotedCandidate` into
/// `Active` and record an `ActivationAudit`.
///
/// Refuses every state other than `PromotedCandidate`.
/// Re-runs the promotion gate eligibility check defensively;
/// refuses if the gate now blocks.  **Active is registry /
/// governance truth only** — no production agent-loop
/// mutation in this round.
#[tauri::command]
pub async fn learning_activate_promoted_candidate(
    input: LearningActivateInput,
) -> Result<CandidateStrategy, String> {
    let s = svc();
    let rs = StrategyRolloutService::new(&s);
    rs.activate_promoted_candidate(input.into())
        .await
        .map(|o| o.candidate_after)
        .map_err(map_err)
}

/// Phase M5-C round 1 — flip an `Active` candidate into
/// `RolledBack` and record a `RollbackAudit`.
///
/// Refuses every state other than `Active`.  Refuses empty
/// `initiatedBy` / `reason`.  The pre-existing
/// `activationAudit` is preserved so reviewers can see the
/// full "active → rolled back" trail.
#[tauri::command]
pub async fn learning_rollback_active_strategy(
    input: LearningRollbackInput,
) -> Result<CandidateStrategy, String> {
    let s = svc();
    let rs = StrategyRolloutService::new(&s);
    rs.rollback_active_strategy(input.into())
        .await
        .map(|o| o.candidate_after)
        .map_err(map_err)
}

/// Phase M5-C round 1 — list every persisted record currently
/// in `RolloutState::Active`.  Singleton-active is **not**
/// enforced in this round; M5-D may add the invariant.
#[tauri::command]
pub async fn learning_get_active_strategies() -> Result<Vec<CandidateStrategy>, String> {
    let s = svc();
    let rs = StrategyRolloutService::new(&s);
    rs.list_active_strategies().await.map_err(map_err)
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase M5-C round 2 — suite-level evaluation seam +
// partial-state recovery inspector.
// ──────────────────────────────────────────────────────────────────────────────

/// Wire payload for [`learning_evaluate_candidate_against_suite`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningEvaluateCandidateAgainstSuiteInput {
    pub strategy_id: String,
    pub suite_id: String,
    /// In-line corpus payload (caller-supplied, e.g. via
    /// `harness_load_corpus` first).
    pub corpus: RegressionCorpus,
    /// `task_id -> run_id` map.  Tasks missing from this map
    /// (or whose `run_id` is absent from the harness report
    /// store) are classified `Missing`.
    pub task_to_run_id: std::collections::BTreeMap<String, String>,
    /// Optional gate policy override.  `None` ⇒
    /// [`GatePolicy::default_conservative`].
    #[serde(default)]
    pub policy: Option<GatePolicy>,
    /// When `false`, skip the suite gate step (suite eval ref
    /// recorded as audit only; `last_suite_recommendation_ref`
    /// is left untouched).  Defaults to `true`.
    #[serde(default = "default_attach_recommendation")]
    pub attach_recommendation: bool,
}

fn default_attach_recommendation() -> bool {
    true
}

/// Wire shape for the suite evaluation response.  Mirrors
/// [`CandidateSuiteEvaluationOutcome`] with stable serde tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningEvaluateCandidateAgainstSuiteResponse {
    pub suite_report: SuiteReport,
    /// `None` when the caller asked to skip the gate step.
    pub recommendation: Option<Recommendation>,
    pub candidate_after: CandidateStrategy,
}

impl From<CandidateSuiteEvaluationOutcome> for LearningEvaluateCandidateAgainstSuiteResponse {
    fn from(v: CandidateSuiteEvaluationOutcome) -> Self {
        Self {
            suite_report: v.suite_report,
            recommendation: v.recommendation,
            candidate_after: v.candidate_after,
        }
    }
}

/// Phase M5-C round 2 — drive one candidate through a
/// suite-level evaluation: aggregate per-task run reports
/// against the corpus, optionally render a `Recommendation` via
/// `harness_evaluate_suite`, then write `last_suite_evaluation_ref`
/// + (optional) `last_suite_recommendation_ref` back to the
/// candidate.  Compare-pair refs (`last_compare_ref` /
/// `last_recommendation_ref`) are **not** modified — the two
/// tracks are parallel.
///
/// **No promotion.**  Even when the suite gate decision is
/// `Promote`, the registry only records.  Promotion gate
/// continues to read compare-pair refs only (M5-D may merge).
#[tauri::command]
pub async fn learning_evaluate_candidate_against_suite(
    input: LearningEvaluateCandidateAgainstSuiteInput,
) -> Result<LearningEvaluateCandidateAgainstSuiteResponse, String> {
    evaluator()
        .evaluate_candidate_against_suite(
            &input.strategy_id,
            &input.suite_id,
            &input.corpus,
            input.task_to_run_id,
            input.policy.as_ref(),
            input.attach_recommendation,
        )
        .await
        .map(LearningEvaluateCandidateAgainstSuiteResponse::from)
        .map_err(map_err)
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase M5 closeout — definition / overlay / scoring / generator IPCs
// ──────────────────────────────────────────────────────────────────────────────

/// Phase M5 closeout — set the candidate's executable
/// [`StrategyDefinition`].  Required before activation: the
/// rollout service refuses to flip a candidate carrying a
/// `Noop` definition into `Active`.
#[tauri::command]
pub async fn learning_set_candidate_definition(
    strategy_id: String,
    definition: StrategyDefinition,
) -> Result<CandidateStrategy, String> {
    svc()
        .set_definition(&strategy_id, definition)
        .await
        .map_err(map_err)
}

/// Phase M5 closeout — resolve the active-strategy overlay for
/// the current registry truth.  Returns the typed
/// [`ActiveStrategyOverlay`] used by the prompt planner; UI
/// callers can use the same shape to render diagnostics.
#[tauri::command]
pub async fn learning_resolve_active_overlay() -> Result<ActiveStrategyOverlay, String> {
    let resolver = ActiveStrategyOverlayResolver::with_default_root();
    Ok(resolver.resolve().await)
}

/// Phase M5 closeout — score one [`HarnessRunReport`] (loaded
/// from the harness report store by `run_id`) along the five
/// trajectory axes.  Returns `Ok(None)` when the run is not
/// present.
#[tauri::command]
pub async fn learning_score_run_report(run_id: String) -> Result<Option<TrajectoryScore>, String> {
    let store = HarnessReportStore::with_default_root();
    let report = store.load(&run_id).await.map_err(map_err)?;
    Ok(report.as_ref().map(score_run_report))
}

/// Phase M5 closeout — generate structured reflection notes
/// from the named run report (if present) using the typed
/// pipeline (trajectory score + failure clustering).  Returns
/// the typed [`ReflectionGenerationResponse`] payload — even
/// when zero notes are emitted callers can inspect the score
/// + cluster breakdown so reviewers see WHY no note fired.
#[tauri::command]
pub async fn learning_generate_reflection_for_report(
    run_id: String,
) -> Result<Option<ReflectionGenerationResponse>, String> {
    let store = HarnessReportStore::with_default_root();
    let Some(report) = store.load(&run_id).await.map_err(map_err)? else {
        return Ok(None);
    };
    let g = generate_reflection_notes(&report);
    Ok(Some(ReflectionGenerationResponse {
        trajectory_score: g.trajectory_score,
        clusters: g.clusters,
        notes: g.notes,
    }))
}

/// Wire payload for [`learning_generate_reflection_for_report`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflectionGenerationResponse {
    pub trajectory_score: TrajectoryScore,
    pub clusters: ClusteredFailureSet,
    pub notes: Vec<ReflectionNote>,
}

/// Phase M5-C round 2 — inspect what already landed for a
/// `(strategy_id, baseline_run_id, candidate_run_id)` triple.
/// Returns an [`EvaluationProgress`] snapshot whose
/// `resumeFrom` field tells the caller which
/// [`crate::modules::learning::EvaluationStep`] to resume from
/// (`null` ⇒ complete, no retry needed).  Pure read — no
/// mutation.
#[tauri::command]
pub async fn learning_inspect_evaluation_progress(
    strategy_id: String,
    baseline_run_id: String,
    candidate_run_id: String,
) -> Result<EvaluationProgress, String> {
    evaluator()
        .inspect_evaluation_progress(&strategy_id, &baseline_run_id, &candidate_run_id)
        .await
        .map_err(map_err)
}
