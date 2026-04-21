//! Candidate strategy registry contracts.
//!
//! Typed shape for the **registry side** of the M5 self-evolution
//! loop: a versioned record of one candidate strategy and the
//! references that connect it to the M4 governance pipeline
//! (HarnessRunReport / BaselineVsCandidate / SuiteReport /
//! Recommendation).
//!
//! Delivery layers (kept in sync with
//! `docs/exec-plans/active/phase-m5-self-evolution-and-strategy-promotion.yaml`):
//!
//! - **M5-A** — registry foundation (`Draft / Candidate /
//!   Compared / Recommended / Rejected / Deprecated`),
//!   compare-pair refs.
//! - **M5-B** — promotion-gate skeleton states
//!   (`PromotionReady / PromotionBlocked / PromotedCandidate`),
//!   no production activation.
//! - **M5-C round 1** — `Active / RolledBack` + typed
//!   [`ActivationAudit`] / [`RollbackAudit`] / [`RollbackTarget`]
//!   audit chain.  `Active` is **registry / governance truth
//!   only**; production agent-loop mutation lives in M5-D.
//! - **M5-C round 2** — suite-level evaluation refs
//!   ([`SuiteEvaluationRef`] + `last_suite_evaluation_ref` +
//!   `last_suite_recommendation_ref`).  Compare-pair and
//!   suite-driven recommendations are recorded on parallel
//!   tracks (no merge logic — left to M5-D).
//!
//! Honest scope of this module:
//!
//! - Defines the **types only**.  Persistence lives in
//!   [`super::strategy_registry_store`]; high-level register /
//!   attach operations live in
//!   [`super::strategy_registry_service`]; rollout / rollback in
//!   [`super::strategy_rollout`]; promotion gate in
//!   [`super::promotion_gate`].
//! - Recommendation / compare / suite references stored here
//!   are **lightweight summaries** (decision, run/suite ids,
//!   reason codes, timestamps).  The full
//!   [`crate::modules::harness::Recommendation`] /
//!   [`crate::modules::harness::BaselineVsCandidate`] /
//!   [`crate::modules::harness::SuiteReport`] structures stay
//!   in the harness layer and are rehydrated on demand via the
//!   existing `harness_load_report` / `harness_compare_reports`
//!   / `harness_aggregate_suite_report` IPCs.
//!
//! Out of scope (intentional non-goals — do **not** assume any
//! of these exist):
//!
//! - Production active flip into `commands/agent.rs` (M5-D).
//! - Auto-promote / auto-rollback automation (M5-D).
//! - Per-corpus-tier weighted policy (M4 enhancement, M5-D
//!   may consume).
//! - Closed-set evidence_ref scheme (M4 enhancement).
//! - Querying / search UI (M5-D, follow-on phase).
//! - Provenance signing.
//! - Recommendation / activation / rollback **history** stacks
//!   — every `last_*_ref` is "most recent only"; full timeline
//!   is M5-D territory.

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::modules::harness::{GateDecision, Recommendation};
use crate::modules::learning::reflection_note::ReflectionNote;

/// Stable registry contract version.  Bumping is breaking; the
/// on-disk layout (`<root>/<strategy_id>.json`) is keyed by this
/// version so older readers refuse mismatched files.
///
/// M5-C round 1 bump: `RolloutState` extended with `Active` /
/// `RolledBack`; `CandidateStrategy` carries optional
/// [`ActivationAudit`] / [`RollbackAudit`] sub-objects.
///
/// M5-C round 2 bump: `CandidateStrategy` carries optional
/// [`SuiteEvaluationRef`] (`last_suite_evaluation_ref`) and a
/// suite-driven [`RecommendationRef`]
/// (`last_suite_recommendation_ref`).
///
/// M5 closeout bump: `CandidateStrategy` adds typed history
/// vectors (`compare_history`, `recommendation_history`,
/// `suite_evaluation_history`, `activation_history`,
/// `rollback_history`), an executable [`StrategyDefinition`]
/// overlay (`definition`), and an optional [`SupersedeRecord`]
/// audit pointer (`superseded_by`).  All new fields default to
/// empty vec / `None` so existing M5-A / M5-B / M5-C records
/// remain forward-compatible.
pub const STRATEGY_REGISTRY_VERSION: &str = "strategy-registry@m5.closeout";

/// Closed alphabet of provenance kinds.  Adding a variant is
/// non-breaking (`#[serde(other)]` is intentionally NOT used —
/// new sources must land as explicit variants so audits can
/// differentiate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum StrategySource {
    /// Created from a structured [`ReflectionNote`].  Carries the
    /// originating `note_id` so the audit trail can re-resolve
    /// the reflection later.
    Reflection { note_id: String },
    /// Created by an operator via the registry IPC (no
    /// reflection / no curated rule backing).
    Manual,
    /// Materialised from a hand-curated rule library / policy
    /// pack.  M5-A keeps this opaque (no rule_id schema yet).
    CuratedRule {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule_id: Option<String>,
    },
    /// Catch-all for sources not yet covered by the closed
    /// catalogue.  Producers that fall back to `Other` MUST
    /// populate `summary` so the audit trail is not silent.
    Other { summary: String },
}

/// Closed alphabet of rollout states this registry models.
///
/// M5-A established `Draft / Candidate / Compared / Recommended /
/// Rejected / Deprecated`.  M5-B adds `PromotionReady /
/// PromotionBlocked / PromotedCandidate` as a **promotion
/// skeleton** — these states only declare allowed/blocked status
/// + operator marking; they do **not** activate any strategy in
/// production.  Real `Active` / `RolledBack` states land in
/// M5-C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutState {
    /// Just registered.  No compare run, no recommendation.  May
    /// be promoted to `Candidate` once the operator decides it
    /// is worth evaluating.
    Draft,
    /// Earmarked for evaluation.  Compare target may or may not
    /// be set yet.
    Candidate,
    /// Compare run has been recorded against this strategy
    /// (`last_compare_ref` is `Some`).  Awaiting recommendation.
    Compared,
    /// Gate has rendered a recommendation (`last_recommendation_ref`
    /// is `Some`).  The decision may be `Promote`, `Hold`, or
    /// `Reject`; M5-A intentionally **does not** auto-act on it
    /// — the operator (or a future M5-B promotion pipeline)
    /// must consume the recommendation explicitly.
    Recommended,
    /// Phase M5-B — gate eligibility check passed: the most
    /// recent recommendation is `Promote` and a paired compare
    /// reference exists.  This state declares "promotion is
    /// allowed by the gate"; it **does not** imply the strategy
    /// is in production.  The operator must take a separate
    /// explicit action (see `PromotedCandidate`) to mark it as
    /// the winning candidate; even then no production rollout
    /// happens until M5-C `Active`.
    PromotionReady,
    /// Phase M5-B — gate eligibility check failed: the most
    /// recent recommendation is `Hold` / `Reject`, or required
    /// inputs are missing.  Distinct from `Rejected` (operator
    /// hard-refusal) — `PromotionBlocked` is gate-driven and
    /// reversible: a fresh compare + recommendation may move
    /// the candidate back to `PromotionReady`.
    PromotionBlocked,
    /// Phase M5-B — operator has explicitly marked this
    /// candidate as the chosen winner of its evaluation cohort.
    /// **Still NOT active in production** — production rollout
    /// requires the M5-C `Active` state and a real activation
    /// path (M5-D wires the production flip).  Reachable from
    /// `PromotionReady` via
    /// [`super::promotion_gate::PromotionGateService::mark_promoted_candidate`].
    PromotedCandidate,
    /// Phase M5-C round 1 — registry-side governance truth that
    /// this strategy is the **active** candidate.  **Not** wired
    /// to the production agent loop (`commands/agent.rs`) in
    /// this round; M5-D performs the actual production flip.
    /// Reachable only from `PromotedCandidate` via
    /// [`super::strategy_rollout::StrategyRolloutService::activate_promoted_candidate`].
    /// Carries an [`ActivationAudit`] sub-object on the parent
    /// [`CandidateStrategy`].
    Active,
    /// Phase M5-C round 1 — strategy was active and has now been
    /// rolled back.  Reachable only from `Active` via
    /// [`super::strategy_rollout::StrategyRolloutService::rollback_active_strategy`].
    /// Carries a [`RollbackAudit`] sub-object on the parent
    /// [`CandidateStrategy`] with non-empty `reason` and
    /// `initiated_by`.
    RolledBack,
    /// Operator (or recommendation consumer) explicitly retired
    /// this candidate.  Distinct from `Recommended` with a
    /// `Reject` decision so the registry can record explicit
    /// human refusal independently of gate output.
    Rejected,
    /// Set aside for later; not currently in the evaluation
    /// pipeline.  Reversible (operator may move back to
    /// `Candidate`).
    Deprecated,
}

impl RolloutState {
    /// Stable wire label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Candidate => "candidate",
            Self::Compared => "compared",
            Self::Recommended => "recommended",
            Self::PromotionReady => "promotion_ready",
            Self::PromotionBlocked => "promotion_blocked",
            Self::PromotedCandidate => "promoted_candidate",
            Self::Active => "active",
            Self::RolledBack => "rolled_back",
            Self::Rejected => "rejected",
            Self::Deprecated => "deprecated",
        }
    }

    /// Phase M5-B — true for states the operator has declared
    /// terminal (`Rejected` / `Deprecated`).  Attach seams and
    /// state advancers preserve these states.
    #[must_use]
    pub fn is_operator_terminal(self) -> bool {
        matches!(self, Self::Rejected | Self::Deprecated)
    }

    /// Phase M5-B — true for states gated on M4 recommendation
    /// (`PromotionReady` / `PromotionBlocked` / `PromotedCandidate`).
    /// Used by the service-layer state advancer to avoid
    /// silently overwriting a freshly-computed gate verdict
    /// with a re-attached compare ref.
    #[must_use]
    pub fn is_gate_decided(self) -> bool {
        matches!(
            self,
            Self::PromotionReady | Self::PromotionBlocked | Self::PromotedCandidate
        )
    }

    /// Phase M5-C round 1 — true for rollout-managed states
    /// (`Active` / `RolledBack`).  Used by the service-layer
    /// state advancer + `set_state` guard to forbid operators
    /// from bypassing the rollout service.  Also used by the
    /// rollback ↔ active state machine.
    #[must_use]
    pub fn is_rollout_managed(self) -> bool {
        matches!(self, Self::Active | Self::RolledBack)
    }
}

/// Minimal strategy identity.  M5-A keeps the strategy itself
/// opaque (no policy DSL, no diff format) — `definition_ref` is
/// a free-form pointer the operator can resolve out-of-band
/// (file path, git sha, registry id, etc.).  M5-B may tighten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyIdentity {
    /// Stable strategy id (UUID-shaped recommended; the registry
    /// store enforces filesystem-safe characters).
    pub strategy_id: String,
    /// Human-readable label for review surfaces.
    pub label: String,
    /// Optional pinned `policy_version` the strategy targets
    /// (mirrors the harness `policy_version` field in run
    /// reports / recommendations).  Left `None` when the
    /// strategy is policy-agnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<String>,
    /// Optional out-of-band pointer to the strategy definition
    /// (file path, git sha, prompt template id, etc.).  M5-A
    /// does NOT validate or resolve this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_ref: Option<String>,
}

/// Pair of `run_id`s the operator intends to use when comparing
/// this candidate against a baseline.  Stored as part of the
/// candidate so the eventual `attach_compare_ref` call can be
/// validated against the operator's stated intent.
///
/// Either field may be `None` when the operator has chosen one
/// side but not the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompareTarget {
    /// `run_id` of the baseline `HarnessRunReport` (resolvable
    /// via `harness_load_report`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_run_id: Option<String>,
    /// `run_id` of the candidate `HarnessRunReport`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_run_id: Option<String>,
}

/// Lightweight summary of one `BaselineVsCandidate` compare that
/// has been recorded against this strategy.  Stays in the
/// registry so reviewers can see the most recent compare
/// without rerunning the diff.
///
/// Full diff is **not** mirrored here — callers re-load it from
/// the harness report store + `harness_compare_reports` when
/// they need detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareRef {
    /// `run_id` of the baseline `HarnessRunReport` consumed.
    pub baseline_run_id: String,
    /// `run_id` of the candidate `HarnessRunReport` consumed.
    pub candidate_run_id: String,
    /// Stable harness compare contract version
    /// (`HARNESS_COMPARE_VERSION`).  Pinned so registry readers
    /// can refuse mismatched expectations without rehydrating.
    pub compare_version: String,
    /// Wall-clock time the compare was attached to the
    /// candidate.
    pub recorded_at: DateTime<Utc>,
}

/// Lightweight summary of one gate `Recommendation` attached to
/// this strategy.  Mirrors the closed-set fields callers care
/// about for audit; the original [`Recommendation`] can be
/// rebuilt by re-running the gate via `harness_evaluate_compare`
/// / `harness_evaluate_suite`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationRef {
    /// Stable harness gate contract version (`HARNESS_GATE_VERSION`).
    pub gate_version: String,
    /// Pinned policy id from the originating `Recommendation`
    /// (`GatePolicy::policy_id`).
    pub policy_id: String,
    /// Closed-set decision (`promote` / `hold` / `reject`).  Stored
    /// as the wire label so registry consumers do not have to
    /// import the harness enum.
    pub decision: String,
    /// Stable reason codes from the gate (e.g.
    /// `"hold_grader_warning_regression"`).
    pub reason_codes: Vec<String>,
    /// Short human-readable summary (verbatim from the gate).
    pub summary: String,
    /// Wall-clock time the recommendation was attached to the
    /// candidate.
    pub recorded_at: DateTime<Utc>,
}

impl RecommendationRef {
    /// Build a registry-side reference from the harness-side
    /// [`Recommendation`].  Pure projection — no decision logic.
    #[must_use]
    pub fn from_recommendation(rec: &Recommendation, recorded_at: DateTime<Utc>) -> Self {
        Self {
            gate_version: rec.gate_version.clone(),
            policy_id: rec.policy_id.clone(),
            decision: gate_decision_label(rec.decision).to_string(),
            reason_codes: rec.reason_codes.clone(),
            summary: rec.summary.clone(),
            recorded_at,
        }
    }
}

/// Phase M5-C round 2 — lightweight suite-level evaluation
/// reference attached to a candidate.  Records "this candidate
/// has been evaluated against suite X (corpus Y@version Z) with
/// grade G".  The full [`crate::modules::harness::SuiteReport`]
/// stays in the harness layer; callers re-aggregate on demand
/// via `harness_aggregate_suite_report`.
///
/// Companion of [`RecommendationRef`]: when the suite-level
/// gate (`harness_evaluate_suite`) renders a recommendation, the
/// candidate carries that recommendation in the parallel
/// `last_suite_recommendation_ref` field — the registry **does
/// not** merge suite + compare-pair recommendations
/// automatically (M5-D may add merge semantics).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiteEvaluationRef {
    /// Caller-supplied suite identifier (mirrors
    /// `SuiteReport.suite_id`).
    pub suite_id: String,
    /// Corpus name + version pair (mirrors
    /// `SuiteReport.corpus_name` / `corpus_version`).  Held so
    /// reviewers can detect "the candidate was evaluated against
    /// an old corpus version".
    pub corpus_name: String,
    pub corpus_version: String,
    /// Stable harness suite-report contract version
    /// (`HARNESS_SUITE_REPORT_VERSION`).  Pinned so registry
    /// consumers can refuse mismatched expectations without
    /// rehydrating.
    pub suite_report_version: String,
    /// Coarse suite-level grade as a stable wire label
    /// (`pass` / `warning` / `fail` / `skeleton`) — mirrors
    /// `SuiteGrade` but stored as `String` so registry
    /// consumers do not need to import the harness enum.
    pub suite_grade: String,
    /// Number of tasks the suite covered (mirrors
    /// `SuiteReport.tasks.len()`).
    pub task_count: usize,
    /// Number of regression-classified tasks (mirrors
    /// `SuiteReport.overall.regression`).  Held for quick
    /// at-a-glance filtering.
    pub regression_count: usize,
    /// Wall-clock time the suite ref was attached to the
    /// candidate.
    pub recorded_at: DateTime<Utc>,
}

/// Stable wire label for [`GateDecision`].  Duplicated from the
/// gate module so the registry never imports the gate's private
/// label helper; bump together if the harness ever changes the
/// labels (it's a contract break for both layers).
fn gate_decision_label(d: GateDecision) -> &'static str {
    match d {
        GateDecision::Promote => "promote",
        GateDecision::Hold => "hold",
        GateDecision::Reject => "reject",
    }
}

/// Phase M5-C round 1 — typed audit metadata captured when a
/// candidate transitions into [`RolloutState::Active`].  Held as
/// an `Option` on [`CandidateStrategy`] so historic records
/// remain forward-compatible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationAudit {
    /// Wall-clock time the activation was recorded.
    pub activated_at: DateTime<Utc>,
    /// Operator / agent identifier that triggered the
    /// activation (must be non-empty; the rollout service
    /// rejects empty strings).
    pub activated_by: String,
    /// Echo of the gate `Recommendation.policy_id` that
    /// authorised the activation (defensive audit — operators
    /// can confirm the active strategy matches the policy
    /// version it was promoted under).  May be `None` when the
    /// candidate's `last_recommendation_ref` was missing
    /// `policy_id` at activation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_policy_id: Option<String>,
    /// Echo of the harness gate version
    /// (`HARNESS_GATE_VERSION`) at activation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_gate_version: Option<String>,
    /// Free-form operator note recorded at activation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Phase M5-C round 1 — typed audit metadata captured when a
/// candidate transitions into [`RolloutState::RolledBack`].
/// Every field except `target` is required by the rollout
/// service (see
/// [`super::strategy_rollout::StrategyRolloutService::rollback_active_strategy`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackAudit {
    /// Wall-clock time the rollback was recorded.
    pub rolled_back_at: DateTime<Utc>,
    /// Operator / agent identifier that triggered the rollback
    /// (must be non-empty).
    pub initiated_by: String,
    /// Free-form non-empty rollback reason (the rollout service
    /// rejects empty strings — rollback must always have an
    /// explanation).
    pub reason: String,
    /// Optional pointer to the strategy / version we are
    /// rolling **to**.  Free-form today: a `strategy_id` of
    /// another candidate, a baseline policy version pin, or any
    /// out-of-band identifier the operator chooses.  Held so
    /// the audit trail records "rolled back from X to Y".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<RollbackTarget>,
}

/// Phase M5-C round 1 — typed pointer for the rollback target.
/// Open variants today; M5-C round 2 may tighten as the
/// strategy DSL / version registry evolves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RollbackTarget {
    /// Roll back to another candidate strategy in this
    /// registry.
    Candidate { strategy_id: String },
    /// Roll back to a pinned baseline policy version (free-form
    /// label; operator-defined).
    BaselinePolicy { policy_version: String },
    /// Catch-all when the operator wants to record a target
    /// that does not match the closed catalogue.
    Other { description: String },
}

/// Phase M5 closeout — minimal **executable** strategy
/// definition that the active overlay resolver can apply at
/// runtime.  Closed catalogue: each variant is a small,
/// observable effect that can be unit-tested without depending
/// on a full strategy DSL.
///
/// Adding a variant is non-breaking (serde tagged), but the
/// overlay resolver MUST learn how to project the new variant
/// otherwise it will be silently ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum StrategyDefinition {
    /// No-op strategy.  Carried for candidates that are
    /// registered for governance audit only (e.g. comparing
    /// telemetry shapes) without any runtime effect.
    Noop,
    /// Append a fixed system-prompt fragment to the static
    /// prompt plan (`PromptBlockKind::ActiveStrategyOverlay`).
    /// `text` MUST be non-empty; the rollout service rejects
    /// activation when an empty payload is supplied.
    PromptOverlay { text: String },
    /// Discourage / disable a tool by name.  Surfaces as an
    /// extra system-prompt note today (the overlay resolver
    /// renders "Avoid tool X unless strictly necessary").
    /// True tool-mask enforcement is M6+ territory.
    DiscourageTool { tool_name: String },
}

impl StrategyDefinition {
    /// Stable wire label.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::PromptOverlay { .. } => "prompt_overlay",
            Self::DiscourageTool { .. } => "discourage_tool",
        }
    }

    /// True iff the definition carries a real runtime effect.
    /// Used by the overlay resolver to skip noop entries.
    #[must_use]
    pub fn has_runtime_effect(&self) -> bool {
        !matches!(self, Self::Noop)
    }
}

impl Default for StrategyDefinition {
    fn default() -> Self {
        Self::Noop
    }
}

/// Phase M5 closeout — audit pointer recorded on the candidate
/// when it is superseded by a newer activation.  `superseded_by`
/// holds the `strategy_id` of the new active candidate; the
/// rollback service writes a [`RollbackAudit`] alongside this
/// pointer with `target = Candidate { strategy_id: <new id> }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupersedeRecord {
    pub superseded_by: String,
    pub superseded_at: DateTime<Utc>,
}

/// Top-level registry record.  One file per record on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateStrategy {
    /// Stable contract version (`STRATEGY_REGISTRY_VERSION`).
    pub registry_version: String,
    /// Identity (`strategy_id` + `label` + optional pins).
    pub identity: StrategyIdentity,
    /// Provenance — where this candidate came from.
    pub source: StrategySource,
    /// Current rollout state.  See [`RolloutState`].
    pub rollout_state: RolloutState,
    /// RFC3339-style timestamp the record was first registered.
    pub created_at: DateTime<Utc>,
    /// RFC3339-style timestamp of the most recent mutation.
    pub updated_at: DateTime<Utc>,
    /// Convenience pointer to the originating reflection note
    /// (mirrors `source` when `StrategySource::Reflection`).
    /// Held outside `source` so non-reflection sources can still
    /// link to a related note when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on_reflection_note: Option<String>,
    /// Operator-stated compare intent.  May be `None` until the
    /// candidate is paired with a baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare_target: Option<CompareTarget>,
    /// Most-recent compare reference attached to this candidate.
    /// `None` until `attach_compare_ref` succeeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_compare_ref: Option<CompareRef>,
    /// Most-recent recommendation reference attached to this
    /// candidate.  `None` until `attach_recommendation` succeeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_recommendation_ref: Option<RecommendationRef>,
    /// Phase M5-C round 2 — most-recent suite-level evaluation
    /// reference attached to this candidate.  Recorded by
    /// [`super::candidate_evaluator::CandidateEvaluator::evaluate_candidate_against_suite`]
    /// or by the low-level
    /// [`super::strategy_registry_service::StrategyRegistryService::attach_suite_evaluation_ref`]
    /// seam.  `None` until the first suite eval lands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_suite_evaluation_ref: Option<SuiteEvaluationRef>,
    /// Phase M5-C round 2 — most-recent **suite-driven**
    /// recommendation projection (output of
    /// `harness_evaluate_suite`).  Recorded on a parallel
    /// track from `last_recommendation_ref` (which is the
    /// compare-pair-driven recommendation from
    /// `harness_evaluate_compare`).  Promotion gate continues
    /// to read `last_recommendation_ref` only; M5-D may merge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_suite_recommendation_ref: Option<RecommendationRef>,
    /// Phase M5-C round 1 — activation audit, populated when
    /// the candidate transitions into `Active`.  Cleared on
    /// rollback (rollback writes [`Self::rollback_audit`]
    /// instead).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_audit: Option<ActivationAudit>,
    /// Phase M5-C round 1 — rollback audit, populated when the
    /// candidate transitions into `RolledBack`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_audit: Option<RollbackAudit>,
    /// Phase M5 closeout — executable strategy definition.
    /// Defaults to [`StrategyDefinition::Noop`] for backward
    /// compatibility; the activation path requires a non-Noop
    /// definition before flipping into `Active`.
    #[serde(default)]
    pub definition: StrategyDefinition,
    /// Phase M5 closeout — superseded-by audit pointer recorded
    /// when this candidate's `Active` lifecycle ended because
    /// another candidate was activated under singleton-active
    /// policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<SupersedeRecord>,
    /// Phase M5 closeout — append-only history of every
    /// compare ref attached.  `last_compare_ref` is preserved
    /// as a quick projection / cache.  Old records load with
    /// an empty vec via `serde(default)`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compare_history: Vec<CompareRef>,
    /// Phase M5 closeout — append-only history of every
    /// recommendation ref attached (compare-pair gate output).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommendation_history: Vec<RecommendationRef>,
    /// Phase M5 closeout — append-only history of every suite
    /// evaluation ref attached.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suite_evaluation_history: Vec<SuiteEvaluationRef>,
    /// Phase M5 closeout — append-only history of every
    /// suite-driven recommendation ref attached.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suite_recommendation_history: Vec<RecommendationRef>,
    /// Phase M5 closeout — append-only history of every
    /// activation event.  Each entry mirrors the
    /// `activation_audit` projection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activation_history: Vec<ActivationAudit>,
    /// Phase M5 closeout — append-only history of every
    /// rollback event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rollback_history: Vec<RollbackAudit>,
    /// Free-form operator notes (audit trail / decision log).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl CandidateStrategy {
    /// Build a fresh `Draft` candidate.  Used by the service
    /// layer; tests may call directly.
    #[must_use]
    pub fn new_draft(
        identity: StrategyIdentity,
        source: StrategySource,
        based_on_reflection_note: Option<String>,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            registry_version: STRATEGY_REGISTRY_VERSION.to_string(),
            identity,
            source,
            rollout_state: RolloutState::Draft,
            created_at: now,
            updated_at: now,
            based_on_reflection_note,
            compare_target: None,
            last_compare_ref: None,
            last_recommendation_ref: None,
            last_suite_evaluation_ref: None,
            last_suite_recommendation_ref: None,
            activation_audit: None,
            rollback_audit: None,
            definition: StrategyDefinition::Noop,
            superseded_by: None,
            compare_history: Vec::new(),
            recommendation_history: Vec::new(),
            suite_evaluation_history: Vec::new(),
            suite_recommendation_history: Vec::new(),
            activation_history: Vec::new(),
            rollback_history: Vec::new(),
            notes: None,
        }
    }

    /// True iff the record is structurally well-formed.  The
    /// store rejects ill-formed records on save.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        !self.identity.strategy_id.is_empty()
            && !self.identity.label.is_empty()
            && self.registry_version == STRATEGY_REGISTRY_VERSION
    }
}

/// Build a `StrategySource::Reflection` from a
/// [`ReflectionNote`].  Convenience for the service layer; tests
/// may call directly.
#[must_use]
pub fn source_from_reflection(note: &ReflectionNote) -> StrategySource {
    StrategySource::Reflection {
        note_id: note.note_id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::HARNESS_GATE_VERSION;

    fn sample_identity() -> StrategyIdentity {
        StrategyIdentity {
            strategy_id: "strat-1".to_string(),
            label: "prefer rg over grep".to_string(),
            policy_version: None,
            definition_ref: None,
        }
    }

    #[test]
    fn draft_record_is_well_formed_and_carries_pinned_version() {
        let now = Utc::now();
        let s = CandidateStrategy::new_draft(sample_identity(), StrategySource::Manual, None, now);
        assert!(s.is_well_formed());
        assert_eq!(s.registry_version, STRATEGY_REGISTRY_VERSION);
        assert_eq!(s.rollout_state, RolloutState::Draft);
    }

    #[test]
    fn empty_strategy_id_is_not_well_formed() {
        let now = Utc::now();
        let mut s =
            CandidateStrategy::new_draft(sample_identity(), StrategySource::Manual, None, now);
        s.identity.strategy_id.clear();
        assert!(!s.is_well_formed());
    }

    #[test]
    fn record_round_trips_through_serde() {
        let now = Utc::now();
        let s = CandidateStrategy::new_draft(
            sample_identity(),
            StrategySource::Reflection {
                note_id: "note-7".to_string(),
            },
            Some("note-7".to_string()),
            now,
        );
        let bytes = serde_json::to_string(&s).unwrap();
        let back: CandidateStrategy = serde_json::from_str(&bytes).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn rollout_state_labels_are_stable() {
        assert_eq!(RolloutState::Draft.label(), "draft");
        assert_eq!(RolloutState::Candidate.label(), "candidate");
        assert_eq!(RolloutState::Compared.label(), "compared");
        assert_eq!(RolloutState::Recommended.label(), "recommended");
        assert_eq!(RolloutState::PromotionReady.label(), "promotion_ready");
        assert_eq!(RolloutState::PromotionBlocked.label(), "promotion_blocked");
        assert_eq!(
            RolloutState::PromotedCandidate.label(),
            "promoted_candidate"
        );
        assert_eq!(RolloutState::Active.label(), "active");
        assert_eq!(RolloutState::RolledBack.label(), "rolled_back");
        assert_eq!(RolloutState::Rejected.label(), "rejected");
        assert_eq!(RolloutState::Deprecated.label(), "deprecated");
    }

    #[test]
    fn rollout_managed_only_covers_active_and_rolled_back() {
        assert!(RolloutState::Active.is_rollout_managed());
        assert!(RolloutState::RolledBack.is_rollout_managed());
        for s in [
            RolloutState::Draft,
            RolloutState::Candidate,
            RolloutState::Compared,
            RolloutState::Recommended,
            RolloutState::PromotionReady,
            RolloutState::PromotionBlocked,
            RolloutState::PromotedCandidate,
            RolloutState::Rejected,
            RolloutState::Deprecated,
        ] {
            assert!(
                !s.is_rollout_managed(),
                "{s:?} should not be rollout-managed"
            );
        }
    }

    #[test]
    fn operator_terminal_only_covers_rejected_and_deprecated() {
        assert!(RolloutState::Rejected.is_operator_terminal());
        assert!(RolloutState::Deprecated.is_operator_terminal());
        for s in [
            RolloutState::Draft,
            RolloutState::Candidate,
            RolloutState::Compared,
            RolloutState::Recommended,
            RolloutState::PromotionReady,
            RolloutState::PromotionBlocked,
            RolloutState::PromotedCandidate,
        ] {
            assert!(!s.is_operator_terminal(), "{s:?} should not be terminal");
        }
    }

    #[test]
    fn recommendation_ref_projects_from_gate_recommendation() {
        // Build a synthetic Recommendation using the public type
        // (we cannot invoke `Recommendation::promote` because it
        // is private, so we serde a small JSON to construct one
        // in a black-box-friendly way).
        let now = Utc::now();
        let json = serde_json::json!({
            "gateVersion": HARNESS_GATE_VERSION,
            "policyId": "default-conservative-m4.8",
            "decision": "hold",
            "reasonCodes": ["hold_grader_warning_regression"],
            "summary": "warn-level grader regression",
            "blockingGraderIds": [],
            "blockingFailureCodes": [],
            "weightedScore": 1.0,
        });
        let rec: Recommendation = serde_json::from_value(json).unwrap();
        let r = RecommendationRef::from_recommendation(&rec, now);
        assert_eq!(r.decision, "hold");
        assert_eq!(r.gate_version, HARNESS_GATE_VERSION);
        assert_eq!(r.policy_id, "default-conservative-m4.8");
        assert_eq!(r.reason_codes, vec!["hold_grader_warning_regression"]);
    }
}
