//! Execution-mode contract v1 skeleton (Phase M0.5).
//!
//! Defines the canonical execution-mode taxonomy and decision shape
//! produced by the request intelligence service (M1+) and consumed
//! by the runtime + frontend explainability projection (M2+).
//!
//! Hard rules (mirrored in
//! [`docs/staff-remediation/if2ai-canonical-domain-model.md`](../../../../../docs/staff-remediation/if2ai-canonical-domain-model.md)
//! §3.10 and
//! [`docs/staff-remediation/if2ai-workflow-truth.md`](../../../../../docs/staff-remediation/if2ai-workflow-truth.md)
//! §3.14):
//!
//! 1. `ExecutionMode` is **runtime truth**, produced by a backend
//!    classifier, never recomputed in the frontend.
//! 2. The four canonical values are closed:
//!    `direct_execute / auto_plan_execute / plan_then_confirm /
//!    specialized_surface`.
//! 3. `Chat / Coding / Research / Planning / Review` are **scenario
//!    profiles**, a separate concept (see [`ScenarioProfileHint`]),
//!    never collapsed into `ExecutionMode`.
//! 4. The frontend may **only project** an [`ExecutionModeDecision`];
//!    overrides go through a backend-mediated action, not a UI-side
//!    recomputation.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Canonical, closed set of runtime execution modes.
///
/// Adding a new mode is a breaking contract change and requires a
/// schema version bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Single-step direct execution; no plan, no confirmation.
    DirectExecute,
    /// Auto-generated multi-step plan executed end-to-end without
    /// user confirmation between steps.
    AutoPlanExecute,
    /// Auto-generated plan presented to the user for confirmation
    /// before execution.
    PlanThenConfirm,
    /// Routed to a specialized surface (browser / coding workspace /
    /// research panel / etc.) instead of the chat thread.
    SpecializedSurface,
}

/// Coarse risk taxonomy used by the classifier. Open enum is *not*
/// allowed — frontends rely on these three buckets for explainable
/// chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Coarse complexity taxonomy. Pairs with [`RiskLevel`] in
/// classifier explainability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplexityLevel {
    Trivial,
    Simple,
    Moderate,
    Complex,
}

/// Scenario profile hint, layered above `ExecutionMode`.
///
/// Scenario profiles are user-facing surfaces (Chat / Coding /
/// Research / Planning / Review). The classifier MAY surface a
/// best-guess scenario, but it is advisory: the actual run is
/// always governed by `ExecutionMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioProfileHint {
    Chat,
    Coding,
    Research,
    Planning,
    Review,
}

/// Optional route hint for [`ExecutionMode::SpecializedSurface`] or
/// for cases where a downstream router needs a sub-target (e.g.
/// "browser session", "coding workspace project id").
///
/// Stays a free-form string in M0.5 because the catalog of surfaces
/// is still growing; M2+ will narrow with an enum once stable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RouteHint(pub String);

/// Open reason-code taxonomy.
///
/// Each reason code MUST be a stable `snake_case` identifier that
/// the frontend can look up in its i18n catalog. New codes can be
/// added without bumping schema; removed codes require a bump.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReasonCode(pub String);

impl ReasonCode {
    /// Construct from a static or owned string.
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }
}

/// Classifier evidence snapshot — the explainability backbone.
///
/// Producers SHOULD fill in every field they have. Consumers MUST
/// treat absent fields as "no evidence", not as "unknown false".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifierEvidence {
    /// Stable policy version id (e.g. `"req-cls@2026-04-20-r1"`).
    /// Required so harness eval can correlate decisions to policies.
    pub policy_version: String,
    /// Ids of the rules that fired during classification.
    #[serde(default)]
    pub matched_rule_ids: Vec<String>,
    /// Slot-fill summary in opaque JSON; the frontend renders this
    /// as a key/value table for transparency.
    #[serde(default)]
    pub slot_summary: serde_json::Value,
    /// `true` if the classifier found the request ambiguous and
    /// escalated to a fallback policy or LLM.
    #[serde(default)]
    pub ambiguous_escalated: bool,
    /// Identifier of the escalation source when
    /// `ambiguous_escalated == true` (e.g. `"llm_fallback@gpt-4o"`,
    /// `"manual_user_override"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalation_source: Option<String>,
}

/// Canonical execution-mode decision.
///
/// This is the contract the runtime emits per request and the
/// frontend projects onto its "why this mode" explainer chip.
///
/// Field naming on the wire is `camelCase` so the TS twin in
/// `src/transport/contracts.ts` can use it unchanged.
//
// `Eq` intentionally omitted: `complexity_score: f32` does not
// satisfy `Eq`. Compare via `PartialEq` only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionModeDecision {
    /// Chosen execution mode.
    pub execution_mode: ExecutionMode,
    /// Coarse risk bucket.
    pub risk_level: RiskLevel,
    /// Coarse complexity bucket.
    pub complexity_level: ComplexityLevel,
    /// Numeric complexity score the classifier produced (0.0..=1.0).
    /// Frontends MUST treat this as advisory only — bucket
    /// (`complexity_level`) is the authoritative projection.
    pub complexity_score: f32,
    /// Stable reason codes explaining why this mode was chosen.
    #[serde(default)]
    pub reason_codes: Vec<ReasonCode>,
    /// Optional route hint for downstream routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_hint: Option<RouteHint>,
    /// Whether the chosen mode mandates a plan-confirm step.
    pub requires_plan: bool,
    /// Optional scenario profile hint, advisory only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_profile_hint: Option<ScenarioProfileHint>,
    /// Stable policy version id for harness correlation.
    pub classifier_policy_version: String,
    /// Ids of the rules that fired during classification.
    #[serde(default)]
    pub classifier_matched_rule_ids: Vec<String>,
    /// Slot-fill summary as opaque JSON.
    #[serde(default)]
    pub classifier_slot_summary: serde_json::Value,
    /// `true` when the classifier escalated due to ambiguity.
    #[serde(default)]
    pub classifier_ambiguous_escalated: bool,
    /// Source identifier when `classifier_ambiguous_escalated == true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifier_escalation_source: Option<String>,
}

impl ExecutionModeDecision {
    /// Convenience: extract a [`ClassifierEvidence`] view from the
    /// decision. Useful for harness trace projection without
    /// duplicating fields.
    #[must_use]
    pub fn evidence(&self) -> ClassifierEvidence {
        ClassifierEvidence {
            policy_version: self.classifier_policy_version.clone(),
            matched_rule_ids: self.classifier_matched_rule_ids.clone(),
            slot_summary: self.classifier_slot_summary.clone(),
            ambiguous_escalated: self.classifier_ambiguous_escalated,
            escalation_source: self.classifier_escalation_source.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_round_trips() {
        let d = ExecutionModeDecision {
            execution_mode: ExecutionMode::PlanThenConfirm,
            risk_level: RiskLevel::Medium,
            complexity_level: ComplexityLevel::Moderate,
            complexity_score: 0.42,
            reason_codes: vec![
                ReasonCode::new("multi_step_detected"),
                ReasonCode::new("user_intent_ambiguous"),
            ],
            route_hint: None,
            requires_plan: true,
            scenario_profile_hint: Some(ScenarioProfileHint::Coding),
            classifier_policy_version: "req-cls@2026-04-20-r1".into(),
            classifier_matched_rule_ids: vec!["R12".into(), "R30".into()],
            classifier_slot_summary: serde_json::json!({ "verb": "refactor" }),
            classifier_ambiguous_escalated: false,
            classifier_escalation_source: None,
        };
        let s = serde_json::to_string(&d).unwrap();
        let back: ExecutionModeDecision = serde_json::from_str(&s).unwrap();
        assert_eq!(d, back);
        assert_eq!(d.execution_mode, ExecutionMode::PlanThenConfirm);
        assert_eq!(d.evidence().matched_rule_ids.len(), 2);
    }

    #[test]
    fn execution_mode_is_closed_set() {
        let modes = [
            ExecutionMode::DirectExecute,
            ExecutionMode::AutoPlanExecute,
            ExecutionMode::PlanThenConfirm,
            ExecutionMode::SpecializedSurface,
        ];
        for m in modes {
            let s = serde_json::to_string(&m).unwrap();
            let back: ExecutionMode = serde_json::from_str(&s).unwrap();
            assert_eq!(m, back);
        }
    }
}
