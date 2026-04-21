//! Phase M5 closeout — cross-module integration test.
//!
//! Walks the full self-evolution lifecycle end-to-end against
//! tempdir-backed registry + harness report stores:
//!
//! 1. Register a manual candidate.
//! 2. Set an executable [`StrategyDefinition`] (PromptOverlay).
//! 3. Run [`CandidateEvaluator::evaluate_candidate`] against
//!    two synthetic [`HarnessRunReport`]s (writes
//!    `last_compare_ref` + `last_recommendation_ref`).
//! 4. Run [`CandidateEvaluator::evaluate_candidate_against_suite`]
//!    against a one-task corpus (writes
//!    `last_suite_evaluation_ref` + `last_suite_recommendation_ref`).
//! 5. Run [`PromotionGateService::apply_eligibility`] and
//!    confirm `Ready` under default `RequireBoth` basis.
//! 6. Run `mark_promoted_candidate` then
//!    `activate_promoted_candidate` and confirm typed audit
//!    chain.
//! 7. Resolve the active overlay and confirm the active
//!    strategy projects into the prompt overlay.
//! 8. Activate a second candidate; confirm singleton-active
//!    auto-supersede + typed rollback audit on the prior
//!    Active.
//! 9. Roll back the new Active explicitly with a
//!    `RollbackTarget::Candidate` pointing at the prior;
//!    confirm rollback target validation accepts existing
//!    candidates and refuses self-targeting.
//! 10. Confirm reflection-generation pipeline emits typed
//!     `ReflectionNote`s for a low-quality run.
//!
//! Mirrors the M5.8 "safety / regression suite" requirement —
//! exercises every cross-module seam (registry / evaluator /
//! gate / rollout / overlay / generator) in one test so a
//! regression in any layer surfaces here first.

use std::collections::BTreeMap;

use chrono::Utc;
use if2ai_backend::modules::harness::{
    AggregateMetrics, BlockingFailure, CorpusTask, CorpusTier, EvidenceBundle, HarnessReportStore,
    HarnessRunReport, RegressionCorpus, Severity, TaskOutcome, TaskRunResult,
    HARNESS_CORPUS_VERSION, HARNESS_RUN_REPORT_VERSION,
};
use if2ai_backend::modules::learning::{
    generate_reflection_notes, ActivateInput, ActiveStrategyOverlayResolver, CandidateEvaluator,
    PromotionDecision, PromotionGateService, RegisterManualOpts, RollbackInput, RollbackTarget,
    RolloutState, StrategyDefinition, StrategyRegistryError, StrategyRegistryService,
    StrategyRegistryStore, StrategyRolloutError, StrategyRolloutService,
};
use tempfile::tempdir;

fn synthetic_report(run_id: &str, outcome: TaskOutcome) -> HarnessRunReport {
    let now = Utc::now();
    HarnessRunReport {
        report_version: HARNESS_RUN_REPORT_VERSION.to_string(),
        run_id: run_id.into(),
        label: None,
        session_id: None,
        project_id: None,
        started_at: now,
        ended_at: now,
        task: TaskRunResult {
            task_id: run_id.into(),
            outcome,
            turn_count: 1,
            last_turn_succeeded: matches!(outcome, TaskOutcome::Success),
            error_summary: None,
        },
        aggregate: AggregateMetrics::default(),
        blocking_failures: Vec::new(),
        evidence: EvidenceBundle::default(),
    }
}

fn synthetic_corpus() -> RegressionCorpus {
    RegressionCorpus {
        corpus_version: HARNESS_CORPUS_VERSION.to_string(),
        name: "integration-corpus".into(),
        description: None,
        loaded_at: Utc::now(),
        source_path: None,
        tasks: vec![CorpusTask {
            id: "task-1".into(),
            tier: CorpusTier::Smoke,
            prompt: "noop".into(),
            expected_outcome: Some(TaskOutcome::Success),
            expected_blockers: Vec::new(),
            weight: 1.0,
            description: None,
            tags: Vec::new(),
        }],
    }
}

#[tokio::test]
async fn m5_full_lifecycle_register_through_supersede_and_rollback() {
    let registry_dir = tempdir().unwrap();
    let harness_dir = tempdir().unwrap();
    let registry = StrategyRegistryService::new(StrategyRegistryStore::new(registry_dir.path()));
    let harness_store = HarnessReportStore::new(harness_dir.path());

    // Seed harness reports the evaluator will consume.
    harness_store
        .save(&synthetic_report("base", TaskOutcome::Success))
        .await
        .unwrap();
    harness_store
        .save(&synthetic_report("cand", TaskOutcome::Success))
        .await
        .unwrap();
    let mut suite_run = synthetic_report("suite-task-1", TaskOutcome::Success);
    suite_run.task.task_id = "task-1".into();
    harness_store.save(&suite_run).await.unwrap();

    // Register two candidates (we'll need two for the
    // singleton-supersede step).
    let primary = registry
        .register_manual("primary".into(), RegisterManualOpts::default())
        .await
        .unwrap();
    let secondary = registry
        .register_manual("secondary".into(), RegisterManualOpts::default())
        .await
        .unwrap();

    // Set executable definitions (Noop is refused at activation).
    registry
        .set_definition(
            &primary.identity.strategy_id,
            StrategyDefinition::PromptOverlay {
                text: "Always cite sources verbatim.".into(),
            },
        )
        .await
        .unwrap();
    registry
        .set_definition(
            &secondary.identity.strategy_id,
            StrategyDefinition::DiscourageTool {
                tool_name: "rm_rf".into(),
            },
        )
        .await
        .unwrap();

    let evaluator = CandidateEvaluator::new(registry.clone(), harness_store.clone());

    // Compare-pair eval for both candidates.
    for id in [
        &primary.identity.strategy_id,
        &secondary.identity.strategy_id,
    ] {
        let outcome = evaluator
            .evaluate_candidate(id, "base", "cand", None)
            .await
            .unwrap();
        assert_eq!(
            outcome.recommendation.decision,
            if2ai_backend::modules::harness::GateDecision::Promote
        );
    }

    // Suite eval for both candidates (RequireBoth basis needs it).
    let mut t2r = BTreeMap::new();
    t2r.insert("task-1".to_string(), "suite-task-1".to_string());
    let corpus = synthetic_corpus();
    for id in [
        &primary.identity.strategy_id,
        &secondary.identity.strategy_id,
    ] {
        let outcome = evaluator
            .evaluate_candidate_against_suite(id, "suite-1", &corpus, t2r.clone(), None, true)
            .await
            .unwrap();
        assert!(outcome.recommendation.is_some());
        assert!(outcome.candidate_after.last_suite_evaluation_ref.is_some());
    }

    // Promotion gate: default RequireBoth must say Ready.
    let gate = PromotionGateService::new(&registry);
    let primary_eligibility = gate
        .apply_eligibility(&primary.identity.strategy_id)
        .await
        .unwrap();
    assert_eq!(
        primary_eligibility.eligibility.decision,
        PromotionDecision::Ready
    );
    gate.mark_promoted_candidate(&primary.identity.strategy_id)
        .await
        .unwrap();

    // Activate primary.
    let rs = StrategyRolloutService::new(&registry);
    let activated = rs
        .activate_promoted_candidate(ActivateInput {
            strategy_id: primary.identity.strategy_id.clone(),
            activated_by: "integration-operator".into(),
            note: None,
        })
        .await
        .unwrap();
    assert_eq!(
        activated.candidate_after.rollout_state,
        RolloutState::Active
    );
    assert!(activated.candidate_after.activation_audit.is_some());
    assert_eq!(activated.candidate_after.activation_history.len(), 1);

    // Resolve overlay — must include the active prompt overlay.
    let resolver = ActiveStrategyOverlayResolver::new(registry.clone());
    let overlay = resolver.resolve().await;
    assert_eq!(overlay.effects.len(), 1);
    assert!(overlay
        .render_prompt_block()
        .contains("Always cite sources verbatim."));

    // Drive secondary through gate + activate → must
    // auto-supersede primary under singleton-active policy.
    gate.apply_eligibility(&secondary.identity.strategy_id)
        .await
        .unwrap();
    gate.mark_promoted_candidate(&secondary.identity.strategy_id)
        .await
        .unwrap();
    let activated_secondary = rs
        .activate_promoted_candidate(ActivateInput {
            strategy_id: secondary.identity.strategy_id.clone(),
            activated_by: "integration-operator".into(),
            note: Some("switching strategies".into()),
        })
        .await
        .unwrap();
    let act_note = activated_secondary
        .candidate_after
        .activation_audit
        .as_ref()
        .unwrap()
        .note
        .as_deref()
        .unwrap();
    assert!(
        act_note.contains("superseded"),
        "activation note must record supersede: {act_note}"
    );

    // Primary is now RolledBack with superseded_by + rollback_audit.
    let primary_after = registry
        .store()
        .load(&primary.identity.strategy_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(primary_after.rollout_state, RolloutState::RolledBack);
    let sup = primary_after.superseded_by.as_ref().unwrap();
    assert_eq!(sup.superseded_by, secondary.identity.strategy_id);
    let rb = primary_after.rollback_audit.as_ref().unwrap();
    assert!(rb.reason.contains("superseded by"));
    assert_eq!(primary_after.rollback_history.len(), 1);

    // Rollback target validation — refuse self-target.
    let self_target = rs
        .rollback_active_strategy(RollbackInput {
            strategy_id: secondary.identity.strategy_id.clone(),
            initiated_by: "operator".into(),
            reason: "test self-target".into(),
            target: Some(RollbackTarget::Candidate {
                strategy_id: secondary.identity.strategy_id.clone(),
            }),
        })
        .await;
    assert!(matches!(
        self_target,
        Err(StrategyRolloutError::InvalidRollbackTarget(_))
    ));

    // Rollback target validation — refuse non-existent target.
    let missing_target = rs
        .rollback_active_strategy(RollbackInput {
            strategy_id: secondary.identity.strategy_id.clone(),
            initiated_by: "operator".into(),
            reason: "test missing target".into(),
            target: Some(RollbackTarget::Candidate {
                strategy_id: "no-such-id".into(),
            }),
        })
        .await;
    assert!(matches!(
        missing_target,
        Err(StrategyRolloutError::InvalidRollbackTarget(_))
    ));

    // Real rollback — point back at the previously-superseded
    // primary.  Acceptable target.
    let rolled_back = rs
        .rollback_active_strategy(RollbackInput {
            strategy_id: secondary.identity.strategy_id.clone(),
            initiated_by: "operator".into(),
            reason: "rolling back to previous strategy".into(),
            target: Some(RollbackTarget::Candidate {
                strategy_id: primary.identity.strategy_id.clone(),
            }),
        })
        .await
        .unwrap();
    assert_eq!(
        rolled_back.candidate_after.rollout_state,
        RolloutState::RolledBack
    );

    // After explicit rollback, registry has no Active record.
    let actives = rs.list_active_strategies().await.unwrap();
    assert!(actives.is_empty());

    // ── Reflection generation: low-quality run produces
    //    typed ReflectionNotes the registry could ingest. ──
    let bad_run = HarnessRunReport {
        report_version: HARNESS_RUN_REPORT_VERSION.to_string(),
        run_id: "bad-run".into(),
        label: None,
        session_id: None,
        project_id: None,
        started_at: Utc::now(),
        ended_at: Utc::now(),
        task: TaskRunResult {
            task_id: "bad".into(),
            outcome: TaskOutcome::Failed,
            turn_count: 1,
            last_turn_succeeded: false,
            error_summary: None,
        },
        aggregate: AggregateMetrics::default(),
        blocking_failures: vec![BlockingFailure {
            code: "memory_rejected".into(),
            severity: Severity::Blocking,
            message: String::new(),
            evidence_ref: None,
            observed_at: Utc::now(),
        }],
        evidence: EvidenceBundle::default(),
    };
    let g = generate_reflection_notes(&bad_run);
    assert!(!g.notes.is_empty(), "expected at least one reflection note");

    // ── "Wrong promotion blocked by gate" — make a candidate
    //    with only compare promote (no suite eval) and confirm
    //    RequireBoth blocks. ──
    let bad_candidate = registry
        .register_manual("wrong-promote".into(), RegisterManualOpts::default())
        .await
        .unwrap();
    registry
        .set_definition(
            &bad_candidate.identity.strategy_id,
            StrategyDefinition::PromptOverlay {
                text: "should not promote".into(),
            },
        )
        .await
        .unwrap();
    evaluator
        .evaluate_candidate(&bad_candidate.identity.strategy_id, "base", "cand", None)
        .await
        .unwrap();
    // Skip suite eval intentionally.
    let blocked = gate
        .apply_eligibility(&bad_candidate.identity.strategy_id)
        .await
        .unwrap();
    assert_eq!(blocked.eligibility.decision, PromotionDecision::Blocked);

    // mark_promoted_candidate must refuse the bad candidate.
    let mark_res = gate
        .mark_promoted_candidate(&bad_candidate.identity.strategy_id)
        .await;
    assert!(mark_res.is_err());

    // Cleanup confirmation: trying to load a non-existent
    // candidate via registry returns NotFound.
    let res = registry.set_notes("never-registered", None).await;
    assert!(matches!(res, Err(StrategyRegistryError::NotFound(_))));
}
