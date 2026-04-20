//! Request intelligence service (Phase M1.6).
//!
//! Application-layer thin wrapper around the
//! [`crate::modules::control_plane::ingress_classifier`]. It exists
//! so:
//!
//! 1. The IPC adapter (`commands/agent.rs`) and `TurnService` reach
//!    a single typed seam, instead of importing the `control_plane`
//!    classifier directly.
//! 2. Future slices can plug LLM-backed escalation, scenario
//!    profile resolution, and policy-version routing in **here**
//!    without touching the deterministic gate.
//! 3. M2 frontend explainability projection has a stable service
//!    edge to consume `ExecutionModeDecision` from.
//!
//! Out of scope:
//! - LLM fallback escalation (deferred — `classifier_ambiguous_escalated`
//!   stays `false`).
//! - Per-user / per-project profile resolution (M2 specialized surface).
//! - Mutation-policy gating (M4 governance).

#![allow(dead_code)]

use std::path::PathBuf;

use crate::modules::control_plane::ingress_classifier::{
    classify_request, IngressClassifierInput,
};
use crate::modules::runtime::contracts::execution_mode::ExecutionModeDecision;

/// Per-turn input. Held as owned values so the service does not
/// borrow from the IPC adapter's session lock.
pub struct RequestIntelligenceInput {
    pub user_message: String,
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub workdir: Option<PathBuf>,
}

/// Output bundle produced by the service.
pub struct RequestIntelligenceOutput {
    pub decision: ExecutionModeDecision,
}

/// Run the deterministic + heuristic classifier and return a typed
/// [`ExecutionModeDecision`].
///
/// Always succeeds: the deterministic gate is total. Future LLM
/// escalation will surface `Result<...>`; until then there is
/// nothing to fail on.
#[must_use]
pub fn classify(input: RequestIntelligenceInput) -> RequestIntelligenceOutput {
    let workdir_ref = input.workdir.as_deref();
    let out = classify_request(IngressClassifierInput {
        user_message: &input.user_message,
        session_id: input.session_id.as_deref(),
        project_id: input.project_id.as_deref(),
        workdir: workdir_ref,
    });
    RequestIntelligenceOutput {
        decision: out.decision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::execution_mode::ExecutionMode;

    #[test]
    fn classify_returns_typed_decision() {
        let out = classify(RequestIntelligenceInput {
            user_message: "ls .".into(),
            session_id: None,
            project_id: None,
            workdir: None,
        });
        assert_eq!(out.decision.execution_mode, ExecutionMode::DirectExecute);
        assert!(!out.decision.classifier_policy_version.is_empty());
        assert!(!out.decision.classifier_matched_rule_ids.is_empty());
    }
}
