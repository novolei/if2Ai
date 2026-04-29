//! WU-005 — Self-edit background scanner tests.

use std::sync::{Arc, Mutex};

use if2ai_backend::modules::api::InputMessage;

/// Serialize env-var mutating tests (cargo runs `#[tokio::test]` in
/// parallel; shared env state is fundamentally racy).
static ENV_LOCK: Mutex<()> = Mutex::new(());
use if2ai_backend::modules::harness::run_report::HarnessRunReport;
use if2ai_backend::modules::learning::self_edit::scanner::{
    run_scanner_once, self_edit_scanner_disabled, ScannerOutcome, DISABLE_SELF_EDIT_ENV,
};
use if2ai_backend::modules::learning::self_edit::{PromotionStage, StageTransition};
use if2ai_backend::modules::memory::{MockUtilityLlm, UtilityLlm};
use if2ai_backend::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
use if2ai_backend::modules::runtime::evolution_emitter::{emit_evolution_event, DISABLE_EMIT_ENV};
use if2ai_backend::modules::skills::sedimentation::Embedder;

const PROPOSAL_JSON: &str = r#"{"kind":"prompt_tweak","target":"system_prompt","before":"old","after":"add a graceful retry","justification":"prevents recurring failure"}"#;

struct ConstEmbedder(Vec<f32>);

impl Embedder for ConstEmbedder {
    fn embed(&self, _text: &str) -> Vec<f32> {
        self.0.clone()
    }
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn scanner_not_spawned_when_disabled() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var(DISABLE_SELF_EDIT_ENV).ok();
    std::env::set_var(DISABLE_SELF_EDIT_ENV, "1");
    assert!(self_edit_scanner_disabled());
    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
    let outcome: ScannerOutcome = run_scanner_once(
        &[],
        &[],
        llm,
        &ConstEmbedder(vec![1.0, 0.0]),
        PromotionStage::Shadow,
        0.0,
        0,
    )
    .await;
    assert!(outcome.proposals.is_empty());
    assert_eq!(outcome.skipped_reason.as_deref(), Some("kill-switch set"));
    match prev {
        Some(v) => std::env::set_var(DISABLE_SELF_EDIT_ENV, v),
        None => std::env::remove_var(DISABLE_SELF_EDIT_ENV),
    }
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn empty_cluster_skips_generation() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var(DISABLE_SELF_EDIT_ENV).ok();
    std::env::remove_var(DISABLE_SELF_EDIT_ENV);
    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
    let reports: Vec<&HarnessRunReport> = Vec::new();
    let outcome = run_scanner_once(
        &reports,
        &[],
        llm,
        &ConstEmbedder(vec![1.0, 0.0]),
        PromotionStage::Shadow,
        0.0,
        0,
    )
    .await;
    assert!(outcome.proposals.is_empty(), "no clusters → no proposals");
    assert_eq!(outcome.skipped_reason.as_deref(), Some("no clusters yet"));
    if let Some(v) = prev {
        std::env::set_var(DISABLE_SELF_EDIT_ENV, v);
    }
}

#[test]
fn proposal_emits_self_edit_event() {
    let prev = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    let env = emit_evolution_event(
        None,
        RuntimeEventType::SelfEditProposal,
        "draft",
        CorrelationIds::default(),
        &serde_json::json!({
            "id": "p1",
            "kind": "prompt_tweak",
            "target": "system_prompt",
            "justification": "fixture",
        }),
        None,
    )
    .expect("emit must succeed under kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::SelfEditProposal);
    let _ = PROPOSAL_JSON;
    match prev {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[test]
fn rejected_proposal_emits_verification_decision() {
    let prev = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    let env = emit_evolution_event(
        None,
        RuntimeEventType::VerificationDecision,
        "rejected",
        CorrelationIds::default(),
        &serde_json::json!({
            "proposalId": "p1",
            "verdict": "fail",
            "failedGates": ["constitution"],
        }),
        None,
    )
    .expect("emit must succeed under kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::VerificationDecision);
    match prev {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn scanner_panic_does_not_crash_setup() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var(DISABLE_SELF_EDIT_ENV).ok();
    std::env::remove_var(DISABLE_SELF_EDIT_ENV);
    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
    let reports: Vec<&HarnessRunReport> = Vec::new();
    let outcome = run_scanner_once(
        &reports,
        &[InputMessage::user_text("anything")],
        llm,
        &ConstEmbedder(vec![1.0]),
        PromotionStage::Shadow,
        0.5,
        100,
    )
    .await;
    // transition reflects upstream input; never panics.
    assert!(matches!(
        outcome.transition,
        StageTransition::Hold | StageTransition::Demote(_)
    ));
    if let Some(v) = prev {
        std::env::set_var(DISABLE_SELF_EDIT_ENV, v);
    }
}
