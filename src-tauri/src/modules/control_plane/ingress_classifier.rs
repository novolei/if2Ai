//! Ingress classifier (Phase M1.6 skeleton).
//!
//! Deterministic + heuristic gate that decides which canonical
//! [`crate::modules::runtime::contracts::execution_mode::ExecutionMode`]
//! a chat request should run under.
//!
//! Lives under `control_plane` (rather than `application`) because
//! routing decisions are part of the platform's runtime truth — the
//! application service layer only consumes this output, it does not
//! own the rules. M4 governance / policy work will plug additional
//! rule sources into this same edge.
//!
//! Strict rule (mirrored in
//! [`docs/staff-remediation/if2ai-canonical-domain-model.md`](../../../../../docs/staff-remediation/if2ai-canonical-domain-model.md)
//! §3.10):
//!
//! 1. The frontend MUST NOT recompute execution mode. This module
//!    is the single canonical source.
//! 2. Rules added here must be deterministic / heuristic only —
//!    LLM-backed classification is deferred to a later slice and
//!    will plug in via a separate trait.
//! 3. Adding a [`ClassifierRuleId`] is *not* a contract break;
//!    removing one is. Keep ids stable so harness traces remain
//!    comparable across policy versions.
//!
//! Out of scope for this skeleton:
//! - LLM fallback escalation path (M1.6 follow-up).
//! - Per-skill / per-scenario route hints (M2 specialized surface).
//! - Profile hints (`Chat / Coding / Research / ...`) come from a
//!   separate user-controlled layer; they are advisory only.

#![allow(dead_code)]

use crate::modules::runtime::contracts::execution_mode::{
    ClassifierEvidence, ComplexityLevel, ExecutionMode, ExecutionModeDecision, ReasonCode,
    RiskLevel, RouteHint, ScenarioProfileHint,
};

/// Stable policy version emitted by this skeleton classifier. Kept
/// `pub const` so the harness eval surface (M4) can pin against it.
pub const CLASSIFIER_POLICY_VERSION: &str = "ingress-classifier@m1.6-skeleton";

/// Stable rule ids.  Adding a new id is non-breaking; removing one
/// is a contract break (harness baselines reference these strings).
pub mod rule_ids {
    pub const HIGH_RISK_MUTATION: &str = "high_risk_mutation_verb_detected";
    pub const PLANNING_VERB: &str = "planning_verb_detected";
    pub const MULTI_STEP_HINT: &str = "multi_step_hint_detected";
    pub const SHORT_DIRECT_REQUEST: &str = "short_direct_request";
    pub const SPECIALIZED_SURFACE_HINT: &str = "specialized_surface_hint";
    pub const DEFAULT_AUTO_PLAN: &str = "default_auto_plan_execute";
}

/// Stable rule-id discriminator. Carried inside the
/// [`ClassifierEvidence::matched_rule_ids`] vector for explainability.
pub type ClassifierRuleId = &'static str;

/// Inputs needed to classify one chat request.
pub struct IngressClassifierInput<'a> {
    pub user_message: &'a str,
    pub session_id: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub workdir: Option<&'a std::path::Path>,
}

/// Outputs of the classifier.
///
/// Held as a struct so future slices can attach additional metadata
/// (e.g. score histograms, candidate alternatives) without changing
/// the call shape.
pub struct IngressClassifierOutput {
    pub decision: ExecutionModeDecision,
    pub evidence: ClassifierEvidence,
}

/// Classify one chat request into a canonical
/// [`ExecutionModeDecision`].
///
/// Heuristic order (shadows the future deterministic gate):
///
/// 1. **High-risk mutation verbs** (`rm / delete / drop / truncate
///    / nuke / format / reset`) → [`ExecutionMode::PlanThenConfirm`]
///    + [`RiskLevel::High`] + [`ComplexityLevel::Moderate`].
/// 2. **Planning verbs** (`plan / design / architect / refactor /
///    migrate / evaluate`) → [`ExecutionMode::PlanThenConfirm`] +
///    [`ComplexityLevel::Complex`].
/// 3. **Specialized-surface hints** (`browser / search the web /
///    open the page`) → [`ExecutionMode::SpecializedSurface`] with
///    a [`RouteHint`] of `"browser"` (kept loose in the skeleton).
/// 4. **Multi-step hints** (`first ... then ... finally`,
///    numbered lists, `&&`, semicolons in shell commands) →
///    [`ExecutionMode::AutoPlanExecute`].
/// 5. **Short single-step request** (≤ 80 chars, no multi-step
///    keywords) → [`ExecutionMode::DirectExecute`] +
///    [`ComplexityLevel::Trivial`].
/// 6. **Default fallback** → [`ExecutionMode::AutoPlanExecute`] +
///    [`RiskLevel::Medium`] + [`ComplexityLevel::Moderate`].
#[must_use]
pub fn classify_request(input: IngressClassifierInput<'_>) -> IngressClassifierOutput {
    let msg = input.user_message.trim();
    let lower = msg.to_lowercase();

    let mut matched_rule_ids: Vec<String> = Vec::new();
    let mut reason_codes: Vec<ReasonCode> = Vec::new();
    let mut route_hint: Option<RouteHint> = None;
    let mut scenario_hint: Option<ScenarioProfileHint> = None;

    let (mode, risk, complexity, complexity_score, requires_plan) = if contains_any(
        &lower,
        &[
            "rm -rf",
            " rm ",
            "delete all",
            "drop table",
            "truncate",
            "format ",
            "nuke ",
            "reset --hard",
            "force push",
        ],
    ) {
        matched_rule_ids.push(rule_ids::HIGH_RISK_MUTATION.into());
        reason_codes.push(ReasonCode::new("high_risk_mutation_verb"));
        scenario_hint = Some(ScenarioProfileHint::Coding);
        (
            ExecutionMode::PlanThenConfirm,
            RiskLevel::High,
            ComplexityLevel::Moderate,
            0.7_f32,
            true,
        )
    } else if contains_any(
        &lower,
        &[
            "plan ",
            "design ",
            "architect",
            "refactor",
            "migrate",
            "redesign",
            "evaluate",
            "review the architecture",
        ],
    ) {
        matched_rule_ids.push(rule_ids::PLANNING_VERB.into());
        reason_codes.push(ReasonCode::new("planning_verb_detected"));
        scenario_hint = Some(ScenarioProfileHint::Planning);
        (
            ExecutionMode::PlanThenConfirm,
            RiskLevel::Medium,
            ComplexityLevel::Complex,
            0.85_f32,
            true,
        )
    } else if contains_any(
        &lower,
        &[
            "browser ",
            "open the page",
            "search the web",
            "navigate to ",
        ],
    ) {
        matched_rule_ids.push(rule_ids::SPECIALIZED_SURFACE_HINT.into());
        reason_codes.push(ReasonCode::new("specialized_surface_hint"));
        route_hint = Some(RouteHint("browser".to_string()));
        scenario_hint = Some(ScenarioProfileHint::Research);
        (
            ExecutionMode::SpecializedSurface,
            RiskLevel::Low,
            ComplexityLevel::Simple,
            0.4_f32,
            false,
        )
    } else if contains_any(
        &lower,
        &[
            "first ", "then ", "finally", "step 1", "step1", "1.", "&&", "; then",
        ],
    ) {
        matched_rule_ids.push(rule_ids::MULTI_STEP_HINT.into());
        reason_codes.push(ReasonCode::new("multi_step_hint_detected"));
        (
            ExecutionMode::AutoPlanExecute,
            RiskLevel::Medium,
            ComplexityLevel::Moderate,
            0.55_f32,
            false,
        )
    } else if msg.chars().count() <= 80
        && !contains_any(&lower, &[" and ", " then ", " also ", " ; ", "&&"])
    {
        matched_rule_ids.push(rule_ids::SHORT_DIRECT_REQUEST.into());
        reason_codes.push(ReasonCode::new("short_single_step_request"));
        (
            ExecutionMode::DirectExecute,
            RiskLevel::Low,
            ComplexityLevel::Trivial,
            0.15_f32,
            false,
        )
    } else {
        matched_rule_ids.push(rule_ids::DEFAULT_AUTO_PLAN.into());
        reason_codes.push(ReasonCode::new("default_auto_plan_execute"));
        (
            ExecutionMode::AutoPlanExecute,
            RiskLevel::Medium,
            ComplexityLevel::Moderate,
            0.5_f32,
            false,
        )
    };

    let slot_summary = serde_json::json!({
        "session_id_present": input.session_id.is_some(),
        "project_id_present": input.project_id.is_some(),
        "workdir_present": input.workdir.is_some(),
        "user_message_chars": msg.chars().count(),
    });

    let decision = ExecutionModeDecision {
        execution_mode: mode,
        risk_level: risk,
        complexity_level: complexity,
        complexity_score,
        reason_codes,
        route_hint,
        requires_plan,
        scenario_profile_hint: scenario_hint,
        classifier_policy_version: CLASSIFIER_POLICY_VERSION.to_string(),
        classifier_matched_rule_ids: matched_rule_ids.clone(),
        classifier_slot_summary: slot_summary.clone(),
        classifier_ambiguous_escalated: false,
        classifier_escalation_source: None,
    };
    let evidence = ClassifierEvidence {
        policy_version: CLASSIFIER_POLICY_VERSION.to_string(),
        matched_rule_ids,
        slot_summary,
        ambiguous_escalated: false,
        escalation_source: None,
    };

    IngressClassifierOutput { decision, evidence }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(msg: &str) -> IngressClassifierOutput {
        classify_request(IngressClassifierInput {
            user_message: msg,
            session_id: None,
            project_id: None,
            workdir: None,
        })
    }

    #[test]
    fn high_risk_mutation_routes_to_plan_then_confirm() {
        let out = classify("please rm -rf node_modules");
        assert_eq!(out.decision.execution_mode, ExecutionMode::PlanThenConfirm);
        assert_eq!(out.decision.risk_level, RiskLevel::High);
        assert!(out.decision.requires_plan);
    }

    #[test]
    fn planning_verb_routes_to_plan_then_confirm_complex() {
        let out = classify("design the new authentication module");
        assert_eq!(out.decision.execution_mode, ExecutionMode::PlanThenConfirm);
        assert_eq!(out.decision.complexity_level, ComplexityLevel::Complex);
    }

    #[test]
    fn short_request_routes_to_direct_execute() {
        let out = classify("ls .");
        assert_eq!(out.decision.execution_mode, ExecutionMode::DirectExecute);
        assert_eq!(out.decision.complexity_level, ComplexityLevel::Trivial);
    }

    #[test]
    fn multi_step_hint_routes_to_auto_plan_execute() {
        let out = classify("first lint, then run tests, finally commit");
        assert_eq!(out.decision.execution_mode, ExecutionMode::AutoPlanExecute);
    }

    #[test]
    fn specialized_surface_hint_carries_route_hint() {
        let out = classify("open the page https://example.com in browser");
        assert_eq!(
            out.decision.execution_mode,
            ExecutionMode::SpecializedSurface
        );
        assert!(out.decision.route_hint.is_some());
    }

    #[test]
    fn default_falls_back_to_auto_plan_execute() {
        let out =
            classify("here is a long-ish ambiguous sentence without any clear single-step verb");
        assert_eq!(out.decision.execution_mode, ExecutionMode::AutoPlanExecute);
        assert_eq!(out.evidence.policy_version, CLASSIFIER_POLICY_VERSION);
    }
}
