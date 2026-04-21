//! Strategy rollout + rollback service skeleton (Phase M5-C
//! round 1).
//!
//! Two operations + one query, sharing one strict state machine
//! and the registry-side audit chain:
//!
//! 1. [`StrategyRolloutService::activate_promoted_candidate`] —
//!    flip a `PromotedCandidate` into `Active`, recording an
//!    [`ActivationAudit`].  Re-runs
//!    [`super::promotion_gate::check_eligibility`] as a
//!    defensive last-check (operators may have attached a fresh
//!    recommendation between `mark_promoted_candidate` and
//!    `activate_promoted_candidate`; if the gate now blocks, we
//!    refuse).
//! 2. [`StrategyRolloutService::rollback_active_strategy`] —
//!    flip an `Active` candidate into `RolledBack`, recording a
//!    [`RollbackAudit`].  Refuses empty `reason` /
//!    `initiated_by`.  Refuses every state other than `Active`.
//! 3. [`StrategyRolloutService::list_active_strategies`] —
//!    enumerate every record currently in `RolloutState::Active`
//!    (registry / governance truth).  Singleton-active is
//!    **not** enforced in M5-C round 1; M5-D may add the
//!    invariant as part of the production active flip.
//!
//! Honest scope of this module:
//!
//! - **Skeleton + governance truth only.**  `Active` here is a
//!   registry label.  No mutation of the production agent loop
//!   (`commands/agent.rs`), no policy version flip in any
//!   running runtime.  M5-D performs the actual production
//!   flip; this module exposes the seam.
//! - **No auto-rollback.**  Rollback is operator-driven (or, in
//!   the future, M5-D autonomic loop).  Today the IPC requires
//!   an explicit `initiated_by` + non-empty `reason`.
//! - **No suite-level eval ref persistence.**  Activation
//!   eligibility re-uses the existing M5-B `last_compare_ref` /
//!   `last_recommendation_ref`.  Suite-level refs land in
//!   M5-C round 2.
//! - **No singleton-active enforcement.**  Multiple records can
//!   currently sit in `Active` simultaneously (e.g. distinct
//!   policy scopes).  M5-D may tighten this once the
//!   production flip happens.
//!
//! Out of scope for M5-C round 1:
//!
//! - Real production `commands/agent.rs` mutation.
//! - Auto-rollback / health-driven rollback automation.
//! - Per-strategy traffic split / canary semantics.
//! - Cross-corpus suite-level activation gates.

#![allow(dead_code)]

use chrono::Utc;
use thiserror::Error;

use super::promotion_gate::{check_eligibility, PromotionDecision};
use super::strategy_registry::{
    ActivationAudit, CandidateStrategy, RollbackAudit, RollbackTarget, RolloutState,
    SupersedeRecord,
};
use super::strategy_registry_service::{StrategyRegistryError, StrategyRegistryService};

/// Stable rollout-service contract version.  Bumping is
/// breaking for downstream consumers (future audit trail).
///
/// M5 closeout bump: enforces **singleton-active** invariant
/// (only one record may sit in `RolloutState::Active` at any
/// time).  Activating a new candidate while another is Active
/// auto-supersedes the prior Active via the typed audit chain
/// (rollback_audit + superseded_by record on both sides).
pub const STRATEGY_ROLLOUT_VERSION: &str = "strategy-rollout@m5.closeout";

/// Inputs for [`StrategyRolloutService::activate_promoted_candidate`].
#[derive(Debug, Clone)]
pub struct ActivateInput {
    pub strategy_id: String,
    /// Operator identifier (must be non-empty).
    pub activated_by: String,
    /// Optional free-form note recorded on the activation
    /// audit.
    pub note: Option<String>,
}

/// Inputs for [`StrategyRolloutService::rollback_active_strategy`].
#[derive(Debug, Clone)]
pub struct RollbackInput {
    pub strategy_id: String,
    /// Operator identifier (must be non-empty).
    pub initiated_by: String,
    /// Non-empty rollback reason.
    pub reason: String,
    /// Optional pointer to the strategy / version we are
    /// rolling **to**.
    pub target: Option<RollbackTarget>,
}

/// Bundled outcome for activate / rollback operations.  Held so
/// callers can see the post-mutation candidate snapshot in one
/// IPC round-trip.
#[derive(Debug, Clone)]
pub struct RolloutOutcome {
    pub candidate_after: CandidateStrategy,
}

/// Service-layer error family.
#[derive(Debug, Error)]
pub enum StrategyRolloutError {
    #[error("registry error: {0}")]
    Registry(#[from] StrategyRegistryError),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("illegal state transition: {0}")]
    IllegalTransition(String),
    #[error("eligibility re-check failed: {0}")]
    EligibilityFailed(String),
    /// Phase M5-C round 2 — rollback target failed cross-store
    /// or self-reference validation.
    #[error("invalid rollback target: {0}")]
    InvalidRollbackTarget(String),
}

/// Service handle.  Holds **no** in-memory state — every call
/// reads & writes the registry store.
pub struct StrategyRolloutService<'a> {
    registry: &'a StrategyRegistryService,
}

impl<'a> StrategyRolloutService<'a> {
    #[must_use]
    pub fn new(registry: &'a StrategyRegistryService) -> Self {
        Self { registry }
    }

    /// Phase M5-C round 1 — flip a `PromotedCandidate` into
    /// `Active`, recording an [`ActivationAudit`].
    ///
    /// Hard rules:
    ///
    /// 1. Refuse empty `strategy_id` / `activated_by`.
    /// 2. Refuse every state other than
    ///    [`RolloutState::PromotedCandidate`].
    /// 3. Re-run [`check_eligibility`] as a defensive last
    ///    check; if the gate now blocks, refuse with
    ///    [`StrategyRolloutError::EligibilityFailed`].
    /// 4. Active is **registry / governance truth only** —
    ///    no production agent-loop mutation in this round.
    pub async fn activate_promoted_candidate(
        &self,
        input: ActivateInput,
    ) -> Result<RolloutOutcome, StrategyRolloutError> {
        if input.strategy_id.trim().is_empty() {
            return Err(StrategyRolloutError::InvalidInput(
                "strategy_id must be non-empty".into(),
            ));
        }
        if input.activated_by.trim().is_empty() {
            return Err(StrategyRolloutError::InvalidInput(
                "activated_by must be non-empty".into(),
            ));
        }
        let mut candidate = self.load_required(&input.strategy_id).await?;
        if candidate.rollout_state != RolloutState::PromotedCandidate {
            return Err(StrategyRolloutError::IllegalTransition(format!(
                "candidate must be in PromotedCandidate to be activated; current = {:?}",
                candidate.rollout_state
            )));
        }
        if !candidate.definition.has_runtime_effect() {
            return Err(StrategyRolloutError::IllegalTransition(
                "candidate carries Noop definition; refuse activation (set a non-Noop StrategyDefinition first)".into(),
            ));
        }
        let eligibility = check_eligibility(&candidate);
        if eligibility.decision != PromotionDecision::Ready {
            return Err(StrategyRolloutError::EligibilityFailed(eligibility.summary));
        }
        let now = Utc::now();
        let supersede_note = self
            .auto_supersede_existing_active(&input.strategy_id, &input.activated_by, now)
            .await?;
        let (source_policy_id, source_gate_version) = candidate
            .last_recommendation_ref
            .as_ref()
            .map(|r| (Some(r.policy_id.clone()), Some(r.gate_version.clone())))
            .unwrap_or((None, None));
        let activation_note = match (input.note.as_deref(), supersede_note.as_deref()) {
            (Some(user), Some(sup)) => Some(format!("{user} | {sup}")),
            (Some(user), None) => Some(user.to_string()),
            (None, Some(sup)) => Some(sup.to_string()),
            (None, None) => None,
        };
        let audit = ActivationAudit {
            activated_at: now,
            activated_by: input.activated_by,
            source_policy_id,
            source_gate_version,
            note: activation_note,
        };
        candidate.activation_audit = Some(audit.clone());
        candidate.activation_history.push(audit);
        candidate.rollback_audit = None;
        candidate.superseded_by = None;
        candidate.rollout_state = RolloutState::Active;
        candidate.updated_at = now;
        self.registry
            .store()
            .save(&candidate)
            .await
            .map_err(|e| StrategyRolloutError::Registry(StrategyRegistryError::Persistence(e)))?;
        Ok(RolloutOutcome {
            candidate_after: candidate,
        })
    }

    /// Phase M5 closeout — singleton-active enforcement helper.
    /// Walks the registry for any record currently in
    /// `RolloutState::Active`; for every such record (other
    /// than `incoming_strategy_id`), writes a typed
    /// [`RollbackAudit`] + [`SupersedeRecord`] and flips it to
    /// `RolledBack`.  Returns a short note for the new
    /// candidate's activation audit.
    async fn auto_supersede_existing_active(
        &self,
        incoming_strategy_id: &str,
        operator: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<Option<String>, StrategyRolloutError> {
        let entries = self.registry.store().list().await.map_err(|e| {
            StrategyRolloutError::Registry(StrategyRegistryError::Persistence(e))
        })?;
        let mut superseded_ids: Vec<String> = Vec::new();
        for entry in entries {
            if entry.rollout_state != RolloutState::Active.label() {
                continue;
            }
            if entry.strategy_id == incoming_strategy_id {
                continue;
            }
            let mut prior = match self.registry.store().load(&entry.strategy_id).await {
                Ok(Some(c)) => c,
                Ok(None) => continue,
                Err(e) => {
                    return Err(StrategyRolloutError::Registry(
                        StrategyRegistryError::Persistence(e),
                    ));
                }
            };
            if prior.rollout_state != RolloutState::Active {
                continue;
            }
            let audit = RollbackAudit {
                rolled_back_at: now,
                initiated_by: operator.to_string(),
                reason: format!("superseded by {incoming_strategy_id}"),
                target: Some(RollbackTarget::Candidate {
                    strategy_id: incoming_strategy_id.to_string(),
                }),
            };
            prior.rollback_audit = Some(audit.clone());
            prior.rollback_history.push(audit);
            prior.superseded_by = Some(SupersedeRecord {
                superseded_by: incoming_strategy_id.to_string(),
                superseded_at: now,
            });
            prior.rollout_state = RolloutState::RolledBack;
            prior.updated_at = now;
            self.registry
                .store()
                .save(&prior)
                .await
                .map_err(|e| StrategyRolloutError::Registry(StrategyRegistryError::Persistence(e)))?;
            superseded_ids.push(prior.identity.strategy_id);
        }
        if superseded_ids.is_empty() {
            Ok(None)
        } else {
            Ok(Some(format!(
                "superseded prior Active strategies: {}",
                superseded_ids.join(", ")
            )))
        }
    }

    /// Phase M5-C round 1 — flip an `Active` candidate into
    /// `RolledBack`, recording a [`RollbackAudit`].
    ///
    /// Hard rules:
    ///
    /// 1. Refuse empty `strategy_id` / `initiated_by` /
    ///    `reason`.
    /// 2. Refuse every state other than
    ///    [`RolloutState::Active`].
    /// 3. Phase M5-C round 2 — when `target ==
    ///    Some(RollbackTarget::Candidate { strategy_id })`:
    ///    - Refuse self-targeting (rolling back X with target =
    ///      X is incoherent).
    ///    - Refuse non-existent target (load fails →
    ///      [`StrategyRolloutError::InvalidRollbackTarget`]).
    /// 4. Rollback writes to the audit chain — never silently
    ///    drops audit metadata.  Existing
    ///    `activation_audit` is preserved (operators / future
    ///    diagnostics may want to see "was active from X,
    ///    rolled back at Y").
    /// 5. Rollback is operator-driven; **no** auto-rollback in
    ///    this round.
    pub async fn rollback_active_strategy(
        &self,
        input: RollbackInput,
    ) -> Result<RolloutOutcome, StrategyRolloutError> {
        if input.strategy_id.trim().is_empty() {
            return Err(StrategyRolloutError::InvalidInput(
                "strategy_id must be non-empty".into(),
            ));
        }
        if input.initiated_by.trim().is_empty() {
            return Err(StrategyRolloutError::InvalidInput(
                "initiated_by must be non-empty".into(),
            ));
        }
        if input.reason.trim().is_empty() {
            return Err(StrategyRolloutError::InvalidInput(
                "reason must be non-empty".into(),
            ));
        }
        // Phase M5-C round 2 — rollback target validation runs
        // BEFORE we mutate any state so a bad target never
        // produces a partial write.
        if let Some(target) = input.target.as_ref() {
            self.validate_rollback_target(&input.strategy_id, target)
                .await?;
        }
        let mut candidate = self.load_required(&input.strategy_id).await?;
        if candidate.rollout_state != RolloutState::Active {
            return Err(StrategyRolloutError::IllegalTransition(format!(
                "candidate must be in Active to be rolled back; current = {:?}",
                candidate.rollout_state
            )));
        }
        let now = Utc::now();
        let audit = RollbackAudit {
            rolled_back_at: now,
            initiated_by: input.initiated_by,
            reason: input.reason,
            target: input.target,
        };
        candidate.rollback_audit = Some(audit.clone());
        candidate.rollback_history.push(audit);
        candidate.rollout_state = RolloutState::RolledBack;
        candidate.updated_at = now;
        self.registry
            .store()
            .save(&candidate)
            .await
            .map_err(|e| StrategyRolloutError::Registry(StrategyRegistryError::Persistence(e)))?;
        Ok(RolloutOutcome {
            candidate_after: candidate,
        })
    }

    /// Phase M5-C round 2 — validate a [`RollbackTarget`]
    /// before persistence.  Hard rules:
    ///
    /// - `Candidate { strategy_id }` must not equal the
    ///   strategy being rolled back (self-targeting).
    /// - `Candidate { strategy_id }` must resolve to an
    ///   existing record in the registry store.
    /// - `BaselinePolicy { policy_version }` must not be empty.
    /// - `Other { description }` must not be empty.
    ///
    /// Returns [`StrategyRolloutError::InvalidRollbackTarget`]
    /// on any violation.  Pure read against the registry store
    /// — no mutation.
    async fn validate_rollback_target(
        &self,
        rolling_back_strategy_id: &str,
        target: &RollbackTarget,
    ) -> Result<(), StrategyRolloutError> {
        match target {
            RollbackTarget::Candidate { strategy_id } => {
                if strategy_id.trim().is_empty() {
                    return Err(StrategyRolloutError::InvalidRollbackTarget(
                        "RollbackTarget::Candidate.strategy_id must be non-empty".into(),
                    ));
                }
                if strategy_id == rolling_back_strategy_id {
                    return Err(StrategyRolloutError::InvalidRollbackTarget(format!(
                        "RollbackTarget::Candidate cannot point to the strategy being rolled back ('{strategy_id}')"
                    )));
                }
                match self.registry.store().load(strategy_id).await {
                    Ok(Some(_)) => Ok(()),
                    Ok(None) => Err(StrategyRolloutError::InvalidRollbackTarget(format!(
                        "RollbackTarget::Candidate strategy_id '{strategy_id}' not present in registry"
                    ))),
                    Err(e) => Err(StrategyRolloutError::Registry(
                        StrategyRegistryError::Persistence(e),
                    )),
                }
            }
            RollbackTarget::BaselinePolicy { policy_version } => {
                if policy_version.trim().is_empty() {
                    return Err(StrategyRolloutError::InvalidRollbackTarget(
                        "RollbackTarget::BaselinePolicy.policy_version must be non-empty".into(),
                    ));
                }
                Ok(())
            }
            RollbackTarget::Other { description } => {
                if description.trim().is_empty() {
                    return Err(StrategyRolloutError::InvalidRollbackTarget(
                        "RollbackTarget::Other.description must be non-empty".into(),
                    ));
                }
                Ok(())
            }
        }
    }

    /// Phase M5-C round 1 — list every persisted record
    /// currently in [`RolloutState::Active`].  Singleton-active
    /// is **not** enforced in this round; the IPC returns a
    /// `Vec` so the caller can render the full set.
    ///
    /// The underlying store handles corrupt-file skipping.
    pub async fn list_active_strategies(
        &self,
    ) -> Result<Vec<CandidateStrategy>, StrategyRolloutError> {
        let entries =
            self.registry.store().list().await.map_err(|e| {
                StrategyRolloutError::Registry(StrategyRegistryError::Persistence(e))
            })?;
        let mut out: Vec<CandidateStrategy> = Vec::new();
        for entry in entries {
            if entry.rollout_state != RolloutState::Active.label() {
                continue;
            }
            match self.registry.store().load(&entry.strategy_id).await {
                Ok(Some(record)) => out.push(record),
                Ok(None) => {
                    // Race: file was deleted between list and
                    // load.  Skip silently — list is a snapshot.
                    continue;
                }
                Err(e) => {
                    tracing::warn!(
                        target: "learning.rollout",
                        strategy_id = %entry.strategy_id,
                        error = %e,
                        "[list_active_strategies] load failed; skipping"
                    );
                    continue;
                }
            }
        }
        Ok(out)
    }

    async fn load_required(
        &self,
        strategy_id: &str,
    ) -> Result<CandidateStrategy, StrategyRolloutError> {
        match self.registry.store().load(strategy_id).await {
            Ok(Some(c)) => Ok(c),
            Ok(None) => Err(StrategyRolloutError::Registry(
                StrategyRegistryError::NotFound(strategy_id.to_string()),
            )),
            Err(e) => Err(StrategyRolloutError::Registry(
                StrategyRegistryError::Persistence(e),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::harness::HARNESS_GATE_VERSION;
    use crate::modules::learning::promotion_gate::PromotionGateService;
    use crate::modules::learning::strategy_registry_service::{
        RegisterManualOpts, StrategyRegistryService,
    };
    use crate::modules::learning::strategy_registry_store::StrategyRegistryStore;
    use chrono::Duration;
    use tempfile::tempdir;

    /// Drive a fresh manual candidate up to `PromotedCandidate`
    /// using only the public service surface so the tests stay
    /// honest end-to-end.
    async fn promoted_candidate(svc: &StrategyRegistryService, label: &str) -> CandidateStrategy {
        use crate::modules::learning::strategy_registry::StrategyDefinition;
        let r = svc
            .register_manual(label.into(), RegisterManualOpts::default())
            .await
            .unwrap();
        // M5 closeout: activation refuses Noop definition, so
        // every test fixture upgrades to a non-Noop overlay.
        svc.set_definition(
            &r.identity.strategy_id,
            StrategyDefinition::PromptOverlay {
                text: format!("test overlay for {label}"),
            },
        )
        .await
        .unwrap();
        let t0 = Utc::now();
        svc.attach_compare_ref(
            &r.identity.strategy_id,
            "base".into(),
            "cand".into(),
            Some(t0),
        )
        .await
        .unwrap();
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
        // M5 closeout: default RequireBoth basis — attach a
        // matching suite recommendation so the gate returns
        // Ready end-to-end.
        let suite_report = crate::modules::harness::SuiteReport {
            suite_report_version: "harness-suite-report@m4.7".into(),
            suite_id: format!("suite-{label}"),
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
        let gate = PromotionGateService::new(svc);
        gate.apply_eligibility(&r.identity.strategy_id)
            .await
            .unwrap();
        gate.mark_promoted_candidate(&r.identity.strategy_id)
            .await
            .unwrap()
            .candidate_after
    }

    fn svc() -> (StrategyRegistryService, tempfile::TempDir) {
        let tmp = tempdir().unwrap();
        let svc = StrategyRegistryService::new(StrategyRegistryStore::new(tmp.path()));
        (svc, tmp)
    }

    #[tokio::test]
    async fn activate_promoted_candidate_writes_active_audit() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-1").await;
        let rs = StrategyRolloutService::new(&svc);
        let outcome = rs
            .activate_promoted_candidate(ActivateInput {
                strategy_id: promoted.identity.strategy_id.clone(),
                activated_by: "operator-alice".into(),
                note: Some("first rollout".into()),
            })
            .await
            .unwrap();
        assert_eq!(outcome.candidate_after.rollout_state, RolloutState::Active);
        let audit = outcome.candidate_after.activation_audit.unwrap();
        assert_eq!(audit.activated_by, "operator-alice");
        assert_eq!(audit.note.as_deref(), Some("first rollout"));
        assert_eq!(
            audit.source_policy_id.as_deref(),
            Some("default-conservative-m4.8")
        );
        assert_eq!(
            audit.source_gate_version.as_deref(),
            Some(HARNESS_GATE_VERSION)
        );
    }

    #[tokio::test]
    async fn activate_refuses_non_promoted_candidate_state() {
        let (svc, _tmp) = svc();
        let r = svc
            .register_manual("rollout-2".into(), RegisterManualOpts::default())
            .await
            .unwrap();
        let rs = StrategyRolloutService::new(&svc);
        let res = rs
            .activate_promoted_candidate(ActivateInput {
                strategy_id: r.identity.strategy_id,
                activated_by: "operator-bob".into(),
                note: None,
            })
            .await;
        assert!(matches!(
            res,
            Err(StrategyRolloutError::IllegalTransition(_))
        ));
    }

    #[tokio::test]
    async fn activate_refuses_empty_activated_by() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-3").await;
        let rs = StrategyRolloutService::new(&svc);
        let res = rs
            .activate_promoted_candidate(ActivateInput {
                strategy_id: promoted.identity.strategy_id,
                activated_by: "  ".into(),
                note: None,
            })
            .await;
        assert!(matches!(res, Err(StrategyRolloutError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn rollback_active_strategy_writes_full_audit() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-4").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: promoted.identity.strategy_id.clone(),
            activated_by: "operator-alice".into(),
            note: None,
        })
        .await
        .unwrap();
        let target = RollbackTarget::BaselinePolicy {
            policy_version: "default-conservative-m4.8".into(),
        };
        let outcome = rs
            .rollback_active_strategy(RollbackInput {
                strategy_id: promoted.identity.strategy_id.clone(),
                initiated_by: "operator-bob".into(),
                reason: "regression spotted in production".into(),
                target: Some(target.clone()),
            })
            .await
            .unwrap();
        assert_eq!(
            outcome.candidate_after.rollout_state,
            RolloutState::RolledBack
        );
        let audit = outcome.candidate_after.rollback_audit.unwrap();
        assert_eq!(audit.initiated_by, "operator-bob");
        assert_eq!(audit.reason, "regression spotted in production");
        assert_eq!(audit.target.unwrap(), target);
        // activation audit must be preserved so reviewers can
        // see "was active, then rolled back".
        assert!(outcome.candidate_after.activation_audit.is_some());
    }

    #[tokio::test]
    async fn rollback_refuses_empty_reason() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-5").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: promoted.identity.strategy_id.clone(),
            activated_by: "alice".into(),
            note: None,
        })
        .await
        .unwrap();
        let res = rs
            .rollback_active_strategy(RollbackInput {
                strategy_id: promoted.identity.strategy_id,
                initiated_by: "bob".into(),
                reason: "   ".into(),
                target: None,
            })
            .await;
        assert!(matches!(res, Err(StrategyRolloutError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn rollback_refuses_non_active_state() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-6").await;
        // Don't activate — try to rollback directly.
        let rs = StrategyRolloutService::new(&svc);
        let res = rs
            .rollback_active_strategy(RollbackInput {
                strategy_id: promoted.identity.strategy_id,
                initiated_by: "bob".into(),
                reason: "should be refused".into(),
                target: None,
            })
            .await;
        assert!(matches!(
            res,
            Err(StrategyRolloutError::IllegalTransition(_))
        ));
    }

    #[tokio::test]
    async fn double_rollback_is_refused() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-7").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: promoted.identity.strategy_id.clone(),
            activated_by: "alice".into(),
            note: None,
        })
        .await
        .unwrap();
        rs.rollback_active_strategy(RollbackInput {
            strategy_id: promoted.identity.strategy_id.clone(),
            initiated_by: "alice".into(),
            reason: "first rollback".into(),
            target: None,
        })
        .await
        .unwrap();
        // Second rollback must be refused — state is RolledBack now.
        let res = rs
            .rollback_active_strategy(RollbackInput {
                strategy_id: promoted.identity.strategy_id,
                initiated_by: "alice".into(),
                reason: "double rollback".into(),
                target: None,
            })
            .await;
        assert!(matches!(
            res,
            Err(StrategyRolloutError::IllegalTransition(_))
        ));
    }

    #[tokio::test]
    async fn singleton_active_supersedes_prior_active() {
        // M5 closeout: activating p2 while p1 is Active must
        // auto-supersede p1 (move it to RolledBack with a
        // typed audit chain) and leave only p2 in the active
        // list.
        let (svc, _tmp) = svc();
        let p1 = promoted_candidate(&svc, "list-1").await;
        let p2 = promoted_candidate(&svc, "list-2").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: p1.identity.strategy_id.clone(),
            activated_by: "alice".into(),
            note: None,
        })
        .await
        .unwrap();
        let activated_p2 = rs
            .activate_promoted_candidate(ActivateInput {
                strategy_id: p2.identity.strategy_id.clone(),
                activated_by: "alice".into(),
                note: None,
            })
            .await
            .unwrap();
        // p2 is the new sole Active; activation note records
        // the supersede.
        let act_audit = activated_p2.candidate_after.activation_audit.unwrap();
        let note = act_audit.note.unwrap();
        assert!(note.contains("superseded"), "expected supersede note, got {note}");

        // p1 must now be RolledBack with superseded_by + a
        // matching rollback_audit.
        let p1_after = svc
            .store()
            .load(&p1.identity.strategy_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p1_after.rollout_state, RolloutState::RolledBack);
        let sup = p1_after.superseded_by.unwrap();
        assert_eq!(sup.superseded_by, p2.identity.strategy_id);
        let rb = p1_after.rollback_audit.unwrap();
        assert_eq!(rb.initiated_by, "alice");
        assert!(rb.reason.contains("superseded by"));

        // list_active_strategies returns only p2 now.
        let actives = rs.list_active_strategies().await.unwrap();
        let ids: Vec<&str> = actives
            .iter()
            .map(|c| c.identity.strategy_id.as_str())
            .collect();
        assert_eq!(ids, vec![p2.identity.strategy_id.as_str()]);
    }

    // ───────────────── M5-C round 2 — rollback target validation ─────────────────

    #[tokio::test]
    async fn rollback_target_self_reference_is_refused() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-self").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: promoted.identity.strategy_id.clone(),
            activated_by: "alice".into(),
            note: None,
        })
        .await
        .unwrap();
        let res = rs
            .rollback_active_strategy(RollbackInput {
                strategy_id: promoted.identity.strategy_id.clone(),
                initiated_by: "alice".into(),
                reason: "self-target should be refused".into(),
                target: Some(RollbackTarget::Candidate {
                    strategy_id: promoted.identity.strategy_id.clone(),
                }),
            })
            .await;
        assert!(matches!(
            res,
            Err(StrategyRolloutError::InvalidRollbackTarget(_))
        ));
        // Candidate must remain Active — refusal happened before
        // any state mutation.
        let still = svc
            .store()
            .load(&promoted.identity.strategy_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(still.rollout_state, RolloutState::Active);
    }

    #[tokio::test]
    async fn rollback_target_missing_candidate_is_refused() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-missing").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: promoted.identity.strategy_id.clone(),
            activated_by: "alice".into(),
            note: None,
        })
        .await
        .unwrap();
        let res = rs
            .rollback_active_strategy(RollbackInput {
                strategy_id: promoted.identity.strategy_id,
                initiated_by: "alice".into(),
                reason: "missing target".into(),
                target: Some(RollbackTarget::Candidate {
                    strategy_id: "this-strategy-does-not-exist".into(),
                }),
            })
            .await;
        assert!(matches!(
            res,
            Err(StrategyRolloutError::InvalidRollbackTarget(_))
        ));
    }

    #[tokio::test]
    async fn rollback_target_existing_candidate_is_accepted() {
        let (svc, _tmp) = svc();
        // Two distinct candidates; rollback A targeting B.
        let promoted_a = promoted_candidate(&svc, "rollout-a").await;
        let promoted_b = promoted_candidate(&svc, "rollout-b").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: promoted_a.identity.strategy_id.clone(),
            activated_by: "alice".into(),
            note: None,
        })
        .await
        .unwrap();
        let outcome = rs
            .rollback_active_strategy(RollbackInput {
                strategy_id: promoted_a.identity.strategy_id.clone(),
                initiated_by: "alice".into(),
                reason: "switch to B".into(),
                target: Some(RollbackTarget::Candidate {
                    strategy_id: promoted_b.identity.strategy_id.clone(),
                }),
            })
            .await
            .unwrap();
        assert_eq!(
            outcome.candidate_after.rollout_state,
            RolloutState::RolledBack
        );
        let target = outcome
            .candidate_after
            .rollback_audit
            .unwrap()
            .target
            .unwrap();
        match target {
            RollbackTarget::Candidate { strategy_id } => {
                assert_eq!(strategy_id, promoted_b.identity.strategy_id);
            }
            other => panic!("unexpected target variant: {other:?}"),
        }
    }

    #[tokio::test]
    async fn rollback_target_baseline_policy_with_empty_version_is_refused() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "rollout-empty-policy").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: promoted.identity.strategy_id.clone(),
            activated_by: "alice".into(),
            note: None,
        })
        .await
        .unwrap();
        let res = rs
            .rollback_active_strategy(RollbackInput {
                strategy_id: promoted.identity.strategy_id,
                initiated_by: "alice".into(),
                reason: "empty policy version".into(),
                target: Some(RollbackTarget::BaselinePolicy {
                    policy_version: "  ".into(),
                }),
            })
            .await;
        assert!(matches!(
            res,
            Err(StrategyRolloutError::InvalidRollbackTarget(_))
        ));
    }

    #[tokio::test]
    async fn re_attach_compare_does_not_demote_active() {
        let (svc, _tmp) = svc();
        let promoted = promoted_candidate(&svc, "preserve-1").await;
        let rs = StrategyRolloutService::new(&svc);
        rs.activate_promoted_candidate(ActivateInput {
            strategy_id: promoted.identity.strategy_id.clone(),
            activated_by: "alice".into(),
            note: None,
        })
        .await
        .unwrap();
        // Attach a fresh compare ref via the registry seam —
        // the rollout-managed Active state must be preserved.
        let after = svc
            .attach_compare_ref(
                &promoted.identity.strategy_id,
                "base-2".into(),
                "cand-2".into(),
                None,
            )
            .await
            .unwrap();
        assert_eq!(after.rollout_state, RolloutState::Active);
    }
}
