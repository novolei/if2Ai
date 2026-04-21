//! Gated promotion skeleton (Phase M5-B).
//!
//! Pure function over a [`CandidateStrategy`] that decides
//! whether the candidate is **eligible** to enter the promotion
//! pipeline.  Wraps the existing M4.8 `Recommendation` decision
//! plus a small set of registry-side coherence checks (compare
//! present, compare ↔ recommendation temporal consistency,
//! operator terminal state guard).
//!
//! Honest scope of this module:
//!
//! - **Skeleton only.**  This module declares **allowed /
//!   blocked**; it does **not** activate any strategy in
//!   production.  Even a `PromotedCandidate` transition is a
//!   marker — the real `Active` state and the production
//!   activation path land in M5-C.
//! - **Hard rule ported from M4.8 runbook §5.9:** "no harness
//!   `Recommendation` ⇒ no promote".  The gate preserves the
//!   strict default and adds per-registry coherence checks
//!   (e.g. "candidate has both a compare and a matching
//!   recommendation").
//! - **No side effects** in the pure check.  The wrapper service
//!   methods that mutate the registry call back into
//!   [`StrategyRegistryService`] so persistence / state
//!   transitions stay in one place.
//!
//! Out of scope for M5-B:
//!
//! - Real production rollout (active strategy mutation).
//! - Auto-promotion (this skeleton requires an explicit
//!   operator step to mark `PromotedCandidate`).
//! - Rollback semantics.
//! - Per-corpus-tier weighted policy.
//! - Multi-recommendation flapping detection.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::strategy_registry::{CandidateStrategy, RolloutState};
use super::strategy_registry_service::{StrategyRegistryError, StrategyRegistryService};

/// Stable promotion-gate contract version.  Bumping is breaking
/// for downstream consumers (M5-C activation pipeline / future
/// audit trail).
///
/// M5 closeout bump: introduces [`PromotionBasis`] to formally
/// resolve the compare-pair vs suite recommendation governance
/// question.  Default basis is `RequireBoth` (the most
/// conservative): both compare-pair and suite recommendations
/// must say `promote` for the gate to return `Ready`.
pub const PROMOTION_GATE_VERSION: &str = "promotion-gate@m5.closeout";

/// Closed-set decision alphabet for the promotion gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDecision {
    /// All checks passed: registry-side coherence holds and the
    /// most recent gate `Recommendation` is `Promote`.  Maps to
    /// [`RolloutState::PromotionReady`].
    Ready,
    /// At least one check failed: missing/stale recommendation,
    /// `Hold` / `Reject` decision, missing compare ref, or
    /// version mismatch.  Maps to
    /// [`RolloutState::PromotionBlocked`].
    Blocked,
}

impl PromotionDecision {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Blocked => "blocked",
        }
    }
}

/// Phase M5 closeout — formal governance choice for combining
/// compare-pair and suite recommendations.
///
/// Default is `RequireBoth` — the most conservative path; M5-D
/// auto-promote pipelines may relax to `EitherSufficient`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionBasis {
    /// Promotion requires `last_recommendation_ref.decision ==
    /// "promote"` only; suite recommendation is informational.
    CompareOnly,
    /// Promotion requires `last_suite_recommendation_ref.decision
    /// == "promote"` only; compare-pair recommendation is
    /// informational.
    SuiteOnly,
    /// Promotion requires **both** compare-pair AND suite
    /// recommendations to say `promote`.  Either one missing or
    /// `hold` / `reject` blocks.  This is the M5 closeout
    /// default.
    RequireBoth,
    /// Promotion is allowed when **either** compare-pair OR
    /// suite recommendation says `promote`.  Use with caution.
    EitherSufficient,
}

impl PromotionBasis {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::CompareOnly => "compare_only",
            Self::SuiteOnly => "suite_only",
            Self::RequireBoth => "require_both",
            Self::EitherSufficient => "either_sufficient",
        }
    }
}

impl Default for PromotionBasis {
    fn default() -> Self {
        Self::RequireBoth
    }
}

/// Stable reason codes the gate may attach to a
/// [`PromotionEligibility`].  Closed catalogue so M5-C
/// activation + future diagnostics surfaces can render stable
/// badges.
pub mod reason_codes {
    pub const READY_RECOMMENDATION_PROMOTE: &str = "ready_recommendation_promote";
    pub const READY_BOTH_PROMOTE: &str = "ready_both_promote";
    pub const READY_EITHER_PROMOTE: &str = "ready_either_promote";

    pub const BLOCKED_OPERATOR_TERMINAL: &str = "blocked_operator_terminal";
    pub const BLOCKED_NO_RECOMMENDATION: &str = "blocked_no_recommendation";
    pub const BLOCKED_NO_SUITE_RECOMMENDATION: &str = "blocked_no_suite_recommendation";
    pub const BLOCKED_NO_COMPARE: &str = "blocked_no_compare";
    pub const BLOCKED_NO_SUITE_EVALUATION: &str = "blocked_no_suite_evaluation";
    pub const BLOCKED_DECISION_NOT_PROMOTE: &str = "blocked_decision_not_promote";
    pub const BLOCKED_SUITE_DECISION_NOT_PROMOTE: &str = "blocked_suite_decision_not_promote";
    pub const BLOCKED_RECOMMENDATION_OLDER_THAN_COMPARE: &str =
        "blocked_recommendation_older_than_compare";
    pub const BLOCKED_SUITE_RECOMMENDATION_OLDER_THAN_SUITE_EVAL: &str =
        "blocked_suite_recommendation_older_than_suite_eval";
}

/// Typed eligibility result.  Held by value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionEligibility {
    pub gate_version: String,
    pub decision: PromotionDecision,
    pub reason_codes: Vec<String>,
    pub summary: String,
    /// Echo of the upstream M4.8 gate decision string when
    /// available (`promote` / `hold` / `reject`); `None` when
    /// no recommendation is attached.
    pub upstream_decision: Option<String>,
    /// Echo of the upstream M4.8 policy id when available.
    pub upstream_policy_id: Option<String>,
}

impl PromotionEligibility {
    fn ready(reason: &str, summary: impl Into<String>, candidate: &CandidateStrategy) -> Self {
        let rec = candidate.last_recommendation_ref.as_ref();
        Self {
            gate_version: PROMOTION_GATE_VERSION.to_string(),
            decision: PromotionDecision::Ready,
            reason_codes: vec![reason.to_string()],
            summary: summary.into(),
            upstream_decision: rec.map(|r| r.decision.clone()),
            upstream_policy_id: rec.map(|r| r.policy_id.clone()),
        }
    }

    fn blocked(
        reasons: Vec<String>,
        summary: impl Into<String>,
        candidate: &CandidateStrategy,
    ) -> Self {
        let rec = candidate.last_recommendation_ref.as_ref();
        Self {
            gate_version: PROMOTION_GATE_VERSION.to_string(),
            decision: PromotionDecision::Blocked,
            reason_codes: reasons,
            summary: summary.into(),
            upstream_decision: rec.map(|r| r.decision.clone()),
            upstream_policy_id: rec.map(|r| r.policy_id.clone()),
        }
    }
}

/// Pure eligibility check.  No IO, no mutation.
///
/// Uses the **default** [`PromotionBasis::RequireBoth`].  Use
/// [`check_eligibility_with_basis`] to override.
///
/// Operator terminal (`Rejected` / `Deprecated`) blocks first
/// (highest precedence).  Then the basis decides which
/// recommendation tracks must say `promote`:
///
/// - `CompareOnly`: same as M5-B / M5-C round 1 behaviour
///   (reads only `last_recommendation_ref` + `last_compare_ref`).
/// - `SuiteOnly`: reads only `last_suite_recommendation_ref` +
///   `last_suite_evaluation_ref`.
/// - `RequireBoth` **(default)**: both must be `promote` and
///   each must not be stale relative to its own evidence.
/// - `EitherSufficient`: any one promote is enough.
#[must_use]
pub fn check_eligibility(candidate: &CandidateStrategy) -> PromotionEligibility {
    check_eligibility_with_basis(candidate, PromotionBasis::default())
}

/// Pure eligibility check with explicit [`PromotionBasis`].
#[must_use]
pub fn check_eligibility_with_basis(
    candidate: &CandidateStrategy,
    basis: PromotionBasis,
) -> PromotionEligibility {
    if candidate.rollout_state.is_operator_terminal() {
        return PromotionEligibility::blocked(
            vec![reason_codes::BLOCKED_OPERATOR_TERMINAL.to_string()],
            format!(
                "candidate is in operator-terminal state {:?}; refusing promotion check",
                candidate.rollout_state
            ),
            candidate,
        );
    }

    let compare_verdict = check_compare_track(candidate);
    let suite_verdict = check_suite_track(candidate);

    match basis {
        PromotionBasis::CompareOnly => render_single(
            candidate,
            "compare",
            compare_verdict,
            reason_codes::READY_RECOMMENDATION_PROMOTE,
        ),
        PromotionBasis::SuiteOnly => render_single(
            candidate,
            "suite",
            suite_verdict,
            reason_codes::READY_RECOMMENDATION_PROMOTE,
        ),
        PromotionBasis::RequireBoth => match (&compare_verdict, &suite_verdict) {
            (TrackVerdict::Ready { .. }, TrackVerdict::Ready { .. }) => {
                let summary = format!(
                    "promotion allowed (require_both): compare + suite both promote"
                );
                PromotionEligibility::ready(
                    reason_codes::READY_BOTH_PROMOTE,
                    summary,
                    candidate,
                )
            }
            _ => {
                let mut reasons: Vec<String> = Vec::new();
                let mut summary_parts: Vec<String> = Vec::new();
                if let TrackVerdict::Blocked { codes, summary } = compare_verdict {
                    reasons.extend(codes);
                    summary_parts.push(format!("compare: {summary}"));
                }
                if let TrackVerdict::Blocked { codes, summary } = suite_verdict {
                    reasons.extend(codes);
                    summary_parts.push(format!("suite: {summary}"));
                }
                PromotionEligibility::blocked(
                    reasons,
                    format!("promotion blocked (require_both): {}", summary_parts.join("; ")),
                    candidate,
                )
            }
        },
        PromotionBasis::EitherSufficient => {
            if matches!(compare_verdict, TrackVerdict::Ready { .. })
                || matches!(suite_verdict, TrackVerdict::Ready { .. })
            {
                PromotionEligibility::ready(
                    reason_codes::READY_EITHER_PROMOTE,
                    "promotion allowed (either_sufficient): at least one track promotes"
                        .to_string(),
                    candidate,
                )
            } else {
                let mut reasons: Vec<String> = Vec::new();
                if let TrackVerdict::Blocked { codes, .. } = compare_verdict {
                    reasons.extend(codes);
                }
                if let TrackVerdict::Blocked { codes, .. } = suite_verdict {
                    reasons.extend(codes);
                }
                PromotionEligibility::blocked(
                    reasons,
                    "promotion blocked (either_sufficient): both tracks failed".to_string(),
                    candidate,
                )
            }
        }
    }
}

/// Internal track verdict — distinct from the public
/// [`PromotionEligibility`] so the basis combinator can mix
/// freely.
enum TrackVerdict {
    Ready {
        #[allow(dead_code)]
        decision_summary: String,
    },
    Blocked {
        codes: Vec<String>,
        summary: String,
    },
}

fn check_compare_track(candidate: &CandidateStrategy) -> TrackVerdict {
    let Some(rec) = candidate.last_recommendation_ref.as_ref() else {
        return TrackVerdict::Blocked {
            codes: vec![reason_codes::BLOCKED_NO_RECOMMENDATION.to_string()],
            summary: "no compare-pair recommendation attached".into(),
        };
    };
    let Some(cmp) = candidate.last_compare_ref.as_ref() else {
        return TrackVerdict::Blocked {
            codes: vec![reason_codes::BLOCKED_NO_COMPARE.to_string()],
            summary: "compare-pair recommendation present but no compare ref".into(),
        };
    };
    let mut codes = Vec::new();
    if rec.decision.as_str() != "promote" {
        codes.push(reason_codes::BLOCKED_DECISION_NOT_PROMOTE.to_string());
    }
    if rec.recorded_at < cmp.recorded_at {
        codes.push(reason_codes::BLOCKED_RECOMMENDATION_OLDER_THAN_COMPARE.to_string());
    }
    if codes.is_empty() {
        TrackVerdict::Ready {
            decision_summary: format!("decision = promote (policy {})", rec.policy_id),
        }
    } else {
        TrackVerdict::Blocked {
            codes,
            summary: format!(
                "compare-pair decision = {} (policy {})",
                rec.decision, rec.policy_id
            ),
        }
    }
}

fn check_suite_track(candidate: &CandidateStrategy) -> TrackVerdict {
    let Some(rec) = candidate.last_suite_recommendation_ref.as_ref() else {
        return TrackVerdict::Blocked {
            codes: vec![reason_codes::BLOCKED_NO_SUITE_RECOMMENDATION.to_string()],
            summary: "no suite recommendation attached".into(),
        };
    };
    let Some(eval) = candidate.last_suite_evaluation_ref.as_ref() else {
        return TrackVerdict::Blocked {
            codes: vec![reason_codes::BLOCKED_NO_SUITE_EVALUATION.to_string()],
            summary: "suite recommendation present but no suite evaluation ref".into(),
        };
    };
    let mut codes = Vec::new();
    if rec.decision.as_str() != "promote" {
        codes.push(reason_codes::BLOCKED_SUITE_DECISION_NOT_PROMOTE.to_string());
    }
    if rec.recorded_at < eval.recorded_at {
        codes.push(reason_codes::BLOCKED_SUITE_RECOMMENDATION_OLDER_THAN_SUITE_EVAL.to_string());
    }
    if codes.is_empty() {
        TrackVerdict::Ready {
            decision_summary: format!("suite decision = promote (policy {})", rec.policy_id),
        }
    } else {
        TrackVerdict::Blocked {
            codes,
            summary: format!(
                "suite decision = {} (policy {})",
                rec.decision, rec.policy_id
            ),
        }
    }
}

fn render_single(
    candidate: &CandidateStrategy,
    track: &str,
    verdict: TrackVerdict,
    ready_code: &str,
) -> PromotionEligibility {
    match verdict {
        TrackVerdict::Ready { decision_summary } => PromotionEligibility::ready(
            ready_code,
            format!("promotion allowed ({track}_only): {decision_summary}"),
            candidate,
        ),
        TrackVerdict::Blocked { codes, summary } => {
            PromotionEligibility::blocked(codes, format!("{track}_only basis: {summary}"), candidate)
        }
    }
}

/// Promotion gate service-layer error.
#[derive(Debug, Error)]
pub enum PromotionGateError {
    #[error("registry error: {0}")]
    Registry(#[from] StrategyRegistryError),
    #[error("promotion blocked: {0}")]
    Blocked(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

/// Bundled outcome for the apply / mark service methods.  Held
/// so callers can see both the eligibility verdict and the
/// post-mutation candidate snapshot.
#[derive(Debug, Clone)]
pub struct PromotionGateOutcome {
    pub eligibility: PromotionEligibility,
    pub candidate_after: CandidateStrategy,
}

/// Service-layer wrapper that applies the gate to a stored
/// candidate and persists the resulting state transition.
///
/// - `apply_eligibility(strategy_id)` runs the pure check and
///   transitions the candidate to `PromotionReady` or
///   `PromotionBlocked` (preserves operator terminal states —
///   surfaced as `Blocked` with the corresponding reason
///   code).
/// - `mark_promoted_candidate(strategy_id)` is the explicit
///   operator step that flips a `PromotionReady` candidate to
///   `PromotedCandidate`.  **No production rollout** — this is
///   purely a marker for M5-C to consume.
pub struct PromotionGateService<'a> {
    registry: &'a StrategyRegistryService,
}

impl<'a> PromotionGateService<'a> {
    #[must_use]
    pub fn new(registry: &'a StrategyRegistryService) -> Self {
        Self { registry }
    }

    /// Run the eligibility check on a stored candidate and
    /// transition the registry-side state accordingly.  Always
    /// returns the verdict (whether `Ready` or `Blocked`); the
    /// candidate's `rollout_state` is updated to match.
    pub async fn apply_eligibility(
        &self,
        strategy_id: &str,
    ) -> Result<PromotionGateOutcome, PromotionGateError> {
        if strategy_id.is_empty() {
            return Err(PromotionGateError::InvalidInput(
                "strategy_id must be non-empty".into(),
            ));
        }
        let candidate = match self.registry.store().load(strategy_id).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Err(PromotionGateError::Registry(
                    StrategyRegistryError::NotFound(strategy_id.to_string()),
                ));
            }
            Err(e) => {
                return Err(PromotionGateError::Registry(
                    StrategyRegistryError::Persistence(e),
                ));
            }
        };
        let eligibility = check_eligibility(&candidate);
        // Operator terminal states are preserved — do NOT bump
        // them to PromotionBlocked (which is a reversible
        // gate-driven state).
        // Rollout-managed states (`Active` / `RolledBack`)
        // are also preserved — re-running the gate on an
        // already-active strategy must not silently demote it.
        // Operators that want to re-evaluate must explicitly
        // rollback first.
        if candidate.rollout_state.is_operator_terminal()
            || candidate.rollout_state.is_rollout_managed()
        {
            return Ok(PromotionGateOutcome {
                eligibility,
                candidate_after: candidate,
            });
        }
        let target = match eligibility.decision {
            PromotionDecision::Ready => RolloutState::PromotionReady,
            PromotionDecision::Blocked => RolloutState::PromotionBlocked,
        };
        let candidate_after = self
            .registry
            .force_state_internal(strategy_id, target)
            .await?;
        Ok(PromotionGateOutcome {
            eligibility,
            candidate_after,
        })
    }

    /// Explicit operator step: mark a `PromotionReady`
    /// candidate as `PromotedCandidate`.  Refuses every other
    /// state — the operator must run `apply_eligibility` first
    /// and only call this after observing `Ready`.
    ///
    /// **No production rollout.**  This only flips a registry
    /// label so M5-C can later wire activation.
    pub async fn mark_promoted_candidate(
        &self,
        strategy_id: &str,
    ) -> Result<PromotionGateOutcome, PromotionGateError> {
        if strategy_id.is_empty() {
            return Err(PromotionGateError::InvalidInput(
                "strategy_id must be non-empty".into(),
            ));
        }
        let candidate = match self.registry.store().load(strategy_id).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Err(PromotionGateError::Registry(
                    StrategyRegistryError::NotFound(strategy_id.to_string()),
                ));
            }
            Err(e) => {
                return Err(PromotionGateError::Registry(
                    StrategyRegistryError::Persistence(e),
                ));
            }
        };
        if candidate.rollout_state != RolloutState::PromotionReady {
            return Err(PromotionGateError::Blocked(format!(
                "candidate must be in PromotionReady to be marked PromotedCandidate; current = {:?}",
                candidate.rollout_state
            )));
        }
        // Re-run the pure check to defend against drift between
        // "apply_eligibility" and this call (e.g. operator
        // attached a fresh recommendation in between).
        let eligibility = check_eligibility(&candidate);
        if eligibility.decision != PromotionDecision::Ready {
            return Err(PromotionGateError::Blocked(format!(
                "eligibility re-check failed: {}",
                eligibility.summary
            )));
        }
        let candidate_after = self
            .registry
            .force_state_internal(strategy_id, RolloutState::PromotedCandidate)
            .await?;
        Ok(PromotionGateOutcome {
            eligibility,
            candidate_after,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::HARNESS_GATE_VERSION;
    use crate::modules::learning::strategy_registry::{
        CompareRef, RecommendationRef, StrategyIdentity, StrategySource,
    };
    use crate::modules::learning::strategy_registry_service::RegisterManualOpts;
    use crate::modules::learning::strategy_registry_store::StrategyRegistryStore;
    use chrono::{Duration, Utc};
    use tempfile::tempdir;

    fn fresh_candidate() -> CandidateStrategy {
        CandidateStrategy::new_draft(
            StrategyIdentity {
                strategy_id: "p-1".into(),
                label: "promotion-skeleton".into(),
                policy_version: None,
                definition_ref: None,
            },
            StrategySource::Manual,
            None,
            Utc::now(),
        )
    }

    fn cmp_ref(at: chrono::DateTime<Utc>) -> CompareRef {
        CompareRef {
            baseline_run_id: "base".into(),
            candidate_run_id: "cand".into(),
            compare_version: "harness-compare@m4.5".into(),
            recorded_at: at,
        }
    }

    fn rec_ref(decision: &str, at: chrono::DateTime<Utc>) -> RecommendationRef {
        RecommendationRef {
            gate_version: HARNESS_GATE_VERSION.to_string(),
            policy_id: "default-conservative-m4.8".into(),
            decision: decision.into(),
            reason_codes: vec!["promote_no_regression".into()],
            summary: "ok".into(),
            recorded_at: at,
        }
    }

    fn suite_eval_ref(at: chrono::DateTime<Utc>) -> crate::modules::learning::strategy_registry::SuiteEvaluationRef {
        crate::modules::learning::strategy_registry::SuiteEvaluationRef {
            suite_id: "s".into(),
            corpus_name: "c".into(),
            corpus_version: "v".into(),
            suite_report_version: "harness-suite-report@m4.7".into(),
            suite_grade: "pass".into(),
            task_count: 1,
            regression_count: 0,
            recorded_at: at,
        }
    }

    /// Produce a fully-promotable candidate (both tracks
    /// promote, both not stale).
    fn ready_candidate() -> CandidateStrategy {
        let mut c = fresh_candidate();
        let t0 = Utc::now();
        c.last_compare_ref = Some(cmp_ref(t0));
        c.last_recommendation_ref = Some(rec_ref("promote", t0 + Duration::seconds(1)));
        c.last_suite_evaluation_ref = Some(suite_eval_ref(t0));
        c.last_suite_recommendation_ref = Some(rec_ref("promote", t0 + Duration::seconds(2)));
        c
    }

    #[test]
    fn no_recommendation_is_blocked_under_compare_only() {
        let c = fresh_candidate();
        let e = check_eligibility_with_basis(&c, PromotionBasis::CompareOnly);
        assert_eq!(e.decision, PromotionDecision::Blocked);
        assert!(e
            .reason_codes
            .iter()
            .any(|r| r == reason_codes::BLOCKED_NO_RECOMMENDATION));
    }

    #[test]
    fn hold_decision_is_blocked_under_compare_only() {
        let mut c = fresh_candidate();
        let now = Utc::now();
        c.last_compare_ref = Some(cmp_ref(now));
        c.last_recommendation_ref = Some(rec_ref("hold", now));
        let e = check_eligibility_with_basis(&c, PromotionBasis::CompareOnly);
        assert_eq!(e.decision, PromotionDecision::Blocked);
        assert!(e
            .reason_codes
            .iter()
            .any(|r| r == reason_codes::BLOCKED_DECISION_NOT_PROMOTE));
    }

    #[test]
    fn compare_only_promote_is_ready() {
        let mut c = fresh_candidate();
        let t0 = Utc::now();
        c.last_compare_ref = Some(cmp_ref(t0));
        c.last_recommendation_ref = Some(rec_ref("promote", t0 + Duration::seconds(1)));
        let e = check_eligibility_with_basis(&c, PromotionBasis::CompareOnly);
        assert_eq!(e.decision, PromotionDecision::Ready);
    }

    #[test]
    fn require_both_blocks_when_only_compare_promotes() {
        let mut c = fresh_candidate();
        let t0 = Utc::now();
        c.last_compare_ref = Some(cmp_ref(t0));
        c.last_recommendation_ref = Some(rec_ref("promote", t0 + Duration::seconds(1)));
        // Suite missing → require_both blocks.
        let e = check_eligibility(&c);
        assert_eq!(e.decision, PromotionDecision::Blocked);
        assert!(e
            .reason_codes
            .iter()
            .any(|r| r == reason_codes::BLOCKED_NO_SUITE_RECOMMENDATION));
    }

    #[test]
    fn require_both_ready_when_both_promote() {
        let c = ready_candidate();
        let e = check_eligibility(&c);
        assert_eq!(e.decision, PromotionDecision::Ready);
        assert!(e
            .reason_codes
            .iter()
            .any(|r| r == reason_codes::READY_BOTH_PROMOTE));
    }

    #[test]
    fn either_sufficient_passes_with_only_compare() {
        let mut c = fresh_candidate();
        let t0 = Utc::now();
        c.last_compare_ref = Some(cmp_ref(t0));
        c.last_recommendation_ref = Some(rec_ref("promote", t0 + Duration::seconds(1)));
        let e = check_eligibility_with_basis(&c, PromotionBasis::EitherSufficient);
        assert_eq!(e.decision, PromotionDecision::Ready);
    }

    #[test]
    fn stale_recommendation_is_blocked_under_compare_only() {
        let mut c = fresh_candidate();
        let t0 = Utc::now();
        c.last_compare_ref = Some(cmp_ref(t0));
        c.last_recommendation_ref = Some(rec_ref("promote", t0 - Duration::seconds(60)));
        let e = check_eligibility_with_basis(&c, PromotionBasis::CompareOnly);
        assert_eq!(e.decision, PromotionDecision::Blocked);
        assert!(e
            .reason_codes
            .iter()
            .any(|r| r == reason_codes::BLOCKED_RECOMMENDATION_OLDER_THAN_COMPARE));
    }

    #[test]
    fn operator_terminal_is_blocked_first() {
        let mut c = fresh_candidate();
        c.rollout_state = RolloutState::Rejected;
        let e = check_eligibility(&c);
        assert_eq!(e.decision, PromotionDecision::Blocked);
        assert!(e
            .reason_codes
            .iter()
            .any(|r| r == reason_codes::BLOCKED_OPERATOR_TERMINAL));
    }

    #[tokio::test]
    async fn apply_eligibility_ready_then_mark_promoted_candidate() {
        let tmp = tempdir().unwrap();
        let svc = StrategyRegistryService::new(StrategyRegistryStore::new(tmp.path()));
        let r = svc
            .register_manual("apply-1".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let t0 = Utc::now();
        // Manually attach refs through the service.
        svc.attach_compare_ref(
            &r.identity.strategy_id,
            "base".into(),
            "cand".into(),
            Some(t0),
        )
        .await
        .unwrap();
        // Build a real Recommendation via JSON (mirrors
        // existing test pattern in registry tests).
        let json = serde_json::json!({
            "gateVersion": HARNESS_GATE_VERSION,
            "policyId": "default-conservative-m4.8",
            "decision": "promote",
            "reasonCodes": ["promote_no_regression"],
            "summary": "ok",
            "blockingGraderIds": [],
            "blockingFailureCodes": [],
            "weightedScore": 0.0,
        });
        let rec: crate::modules::harness::Recommendation = serde_json::from_value(json).unwrap();
        svc.attach_recommendation(
            &r.identity.strategy_id,
            &rec,
            Some(t0 + Duration::seconds(1)),
        )
        .await
        .unwrap();
        // M5 closeout: default basis is RequireBoth; attach a
        // suite eval + suite recommendation so the gate returns
        // Ready end-to-end.
        let suite_report = crate::modules::harness::SuiteReport {
            suite_report_version: "harness-suite-report@m4.7".into(),
            suite_id: "suite-apply-1".into(),
            corpus_name: "c".into(),
            corpus_version: "v".into(),
            started_at: t0,
            ended_at: t0,
            tasks: vec![],
            tier_summaries: vec![],
            overall: Default::default(),
            grade: crate::modules::harness::SuiteGrade::Pass,
        };
        svc.attach_suite_evaluation_ref(
            &r.identity.strategy_id,
            &suite_report,
            Some(&rec),
            Some(t0 + Duration::seconds(2)),
        )
        .await
        .unwrap();

        let gate = PromotionGateService::new(&svc);
        let outcome = gate
            .apply_eligibility(&r.identity.strategy_id)
            .await
            .unwrap();
        assert_eq!(outcome.eligibility.decision, PromotionDecision::Ready);
        assert_eq!(
            outcome.candidate_after.rollout_state,
            RolloutState::PromotionReady
        );

        let promoted = gate
            .mark_promoted_candidate(&r.identity.strategy_id)
            .await
            .unwrap();
        assert_eq!(
            promoted.candidate_after.rollout_state,
            RolloutState::PromotedCandidate
        );
    }

    #[tokio::test]
    async fn mark_promoted_candidate_refuses_non_ready_state() {
        let tmp = tempdir().unwrap();
        let svc = StrategyRegistryService::new(StrategyRegistryStore::new(tmp.path()));
        let r = svc
            .register_manual("apply-2".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let gate = PromotionGateService::new(&svc);
        let res = gate.mark_promoted_candidate(&r.identity.strategy_id).await;
        assert!(matches!(res, Err(PromotionGateError::Blocked(_))));
    }
}
