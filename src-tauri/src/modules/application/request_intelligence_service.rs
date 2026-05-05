//! Request intelligence service (Phase M1.6 + Hybrid LLM verification).
//!
//! Application-layer wrapper around the
//! [`crate::modules::control_plane::ingress_classifier`]. It exists
//! so:
//!
//! 1. The IPC adapter (`commands/agent.rs`) and `TurnService` reach
//!    a single typed seam, instead of importing the `control_plane`
//!    classifier directly.
//! 2. When the deterministic classifier produces an ambiguous result
//!    (`Trivial + DirectExecute`), an optional lightweight LLM call
//!    validates whether the message actually requires tool usage and
//!    escalates the execution mode accordingly.
//! 3. M2 frontend explainability projection has a stable service
//!    edge to consume `ExecutionModeDecision` from.
//!
//! Session-level rate-limiting: at most 3 LLM verification calls per
//! service instance (typically per session). After the cap the
//! service silently falls back to the deterministic classification.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::modules::control_plane::ingress_classifier::{classify_request, IngressClassifierInput};
use crate::modules::memory::UtilityLlm;
use crate::modules::runtime::contracts::execution_mode::{
    ComplexityLevel, ExecutionMode, ExecutionModeDecision,
};

/// Maximum number of LLM semantic verification calls allowed per
/// service instance (session). After this cap, the service skips
/// LLM verification and returns the deterministic classification.
const MAX_LLM_VERIFICATIONS_PER_SESSION: u32 = 3;

/// Timeout for the LLM semantic verification call.
const LLM_VERIFICATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

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

/// Stateful request intelligence service holding an optional
/// [`UtilityLlm`] for hybrid semantic verification.
///
/// Create one instance per session so the rate-limit counter resets
/// naturally with session lifetime.
pub struct RequestIntelligenceService {
    utility_llm: Option<Arc<dyn UtilityLlm>>,
    llm_call_count: AtomicU32,
}

impl RequestIntelligenceService {
    /// Build a new service. Pass `None` for `utility_llm` to disable
    /// LLM-based semantic verification entirely (deterministic-only
    /// mode).
    #[must_use]
    pub fn new(utility_llm: Option<Arc<dyn UtilityLlm>>) -> Self {
        Self {
            utility_llm,
            llm_call_count: AtomicU32::new(0),
        }
    }

    /// Run the classifier and, when the result is ambiguous, optionally
    /// escalate via a lightweight LLM semantic check.
    ///
    /// The LLM check is gated on:
    /// 1. The deterministic classifier flagging `classifier_ambiguous_escalated`.
    /// 2. A `UtilityLlm` being available.
    /// 3. The per-session call count being below [`MAX_LLM_VERIFICATIONS_PER_SESSION`].
    ///
    /// On timeout or LLM error the original classification is preserved.
    pub async fn classify_async(
        &self,
        input: RequestIntelligenceInput,
    ) -> RequestIntelligenceOutput {
        let workdir_ref = input.workdir.as_deref();
        let out = classify_request(IngressClassifierInput {
            user_message: &input.user_message,
            session_id: input.session_id.as_deref(),
            project_id: input.project_id.as_deref(),
            workdir: workdir_ref,
        });

        let mut decision = out.decision;

        // Only attempt LLM verification when the classifier flagged
        // ambiguity (Trivial + DirectExecute).
        if decision.classifier_ambiguous_escalated {
            if let Some(ref llm) = self.utility_llm {
                let current_count = self.llm_call_count.load(Ordering::Relaxed);
                if current_count >= MAX_LLM_VERIFICATIONS_PER_SESSION {
                    tracing::info!(
                        count = current_count,
                        "[request-intelligence] LLM verification skipped — session cap reached"
                    );
                } else {
                    self.llm_call_count.fetch_add(1, Ordering::Relaxed);
                    decision = self
                        .try_llm_semantic_check(llm, &input.user_message, decision)
                        .await;
                }
            }
        }

        RequestIntelligenceOutput { decision }
    }

    /// Attempt a lightweight LLM semantic verification with a 2-second
    /// timeout. On success the decision may be escalated; on
    /// timeout / error the original decision is returned unchanged.
    async fn try_llm_semantic_check(
        &self,
        llm: &Arc<dyn UtilityLlm>,
        user_message: &str,
        mut decision: ExecutionModeDecision,
    ) -> ExecutionModeDecision {
        let prompt = format!(
            "Analyze this user message and determine if it requires creating, \
             modifying, or executing files/commands.\n\
             User message: \"{}\"\n\
             Answer with ONLY \"YES\" or \"NO\".",
            user_message
        );

        let llm_clone = Arc::clone(llm);
        let llm_future = async move {
            llm_clone
                .complete("You are a request classifier assistant.", &prompt, 128, 0.0)
                .await
        };

        match tokio::time::timeout(LLM_VERIFICATION_TIMEOUT, llm_future).await {
            Ok(Ok(response)) => {
                let answer = response.trim().to_uppercase();
                if answer.contains("YES") {
                    tracing::info!(
                        original_mode = ?decision.execution_mode,
                        "[request-intelligence] LLM semantic check → YES, escalating to AutoPlanExecute"
                    );
                    decision.execution_mode = ExecutionMode::AutoPlanExecute;
                    decision.complexity_level = ComplexityLevel::Moderate;
                    decision.classifier_escalation_source = Some("llm_semantic_check".to_string());
                } else {
                    tracing::info!(
                        "[request-intelligence] LLM semantic check → NO, keeping original classification"
                    );
                }
            }
            Ok(Err(e)) => {
                tracing::info!(
                    error = %e,
                    "[request-intelligence] LLM semantic check failed, keeping original classification"
                );
            }
            Err(_) => {
                tracing::info!(
                    "[request-intelligence] LLM semantic check timed out (2s), keeping original classification"
                );
            }
        }

        decision
    }

    /// Return the current LLM verification call count (useful for
    /// diagnostics / testing).
    #[must_use]
    pub fn llm_call_count(&self) -> u32 {
        self.llm_call_count.load(Ordering::Relaxed)
    }

    /// Reset the per-session LLM call counter (e.g. on session reset).
    pub fn reset_llm_counter(&self) {
        self.llm_call_count.store(0, Ordering::Relaxed);
    }
}

/// Run the deterministic + heuristic classifier and return a typed
/// [`ExecutionModeDecision`].
///
/// MIG-002-a — this decision now drives real route behavior at
/// `TurnService` entry. `SpecializedSurface` mode short-circuits
/// before entering `ConversationRuntime`. Always succeeds: the
/// deterministic gate is total.
///
/// This synchronous variant does **not** perform LLM semantic
/// verification. Use [`RequestIntelligenceService::classify_async`]
/// for the hybrid path.
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
    use crate::modules::memory::MockUtilityLlm;
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

    #[tokio::test]
    async fn llm_yes_escalates_to_auto_plan_execute() {
        let llm = Arc::new(MockUtilityLlm::new(vec!["YES".to_string()]));
        let svc = RequestIntelligenceService::new(Some(llm));
        let out = svc
            .classify_async(RequestIntelligenceInput {
                user_message: "fix the bug".into(),
                session_id: None,
                project_id: None,
                workdir: None,
            })
            .await;
        assert_eq!(out.decision.execution_mode, ExecutionMode::AutoPlanExecute);
        assert_eq!(out.decision.complexity_level, ComplexityLevel::Moderate);
        assert_eq!(
            out.decision.classifier_escalation_source.as_deref(),
            Some("llm_semantic_check")
        );
        assert_eq!(svc.llm_call_count(), 1);
    }

    #[tokio::test]
    async fn llm_no_keeps_direct_execute() {
        let llm = Arc::new(MockUtilityLlm::new(vec!["NO".to_string()]));
        let svc = RequestIntelligenceService::new(Some(llm));
        let out = svc
            .classify_async(RequestIntelligenceInput {
                user_message: "hello".into(),
                session_id: None,
                project_id: None,
                workdir: None,
            })
            .await;
        assert_eq!(out.decision.execution_mode, ExecutionMode::DirectExecute);
        assert!(out.decision.classifier_escalation_source.is_none());
    }

    #[tokio::test]
    async fn session_cap_stops_llm_calls() {
        let llm = Arc::new(MockUtilityLlm::new(vec![
            "YES".to_string(),
            "YES".to_string(),
            "YES".to_string(),
            "YES".to_string(), // should not be consumed
        ]));
        let svc = RequestIntelligenceService::new(Some(llm.clone()));
        for _ in 0..3 {
            let _ = svc
                .classify_async(RequestIntelligenceInput {
                    user_message: "hi".into(),
                    session_id: None,
                    project_id: None,
                    workdir: None,
                })
                .await;
        }
        assert_eq!(svc.llm_call_count(), 3);
        // Fourth call should be skipped
        let out = svc
            .classify_async(RequestIntelligenceInput {
                user_message: "hi".into(),
                session_id: None,
                project_id: None,
                workdir: None,
            })
            .await;
        // Stays DirectExecute because LLM was not called
        assert_eq!(out.decision.execution_mode, ExecutionMode::DirectExecute);
        assert_eq!(svc.llm_call_count(), 3);
    }

    #[tokio::test]
    async fn no_llm_falls_back_to_deterministic() {
        let svc = RequestIntelligenceService::new(None);
        let out = svc
            .classify_async(RequestIntelligenceInput {
                user_message: "hi".into(),
                session_id: None,
                project_id: None,
                workdir: None,
            })
            .await;
        assert_eq!(out.decision.execution_mode, ExecutionMode::DirectExecute);
        assert_eq!(svc.llm_call_count(), 0);
    }
}
