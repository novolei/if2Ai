//! FEAT-AE-001..003 — Self-edit pipeline integration tests.
//!
//! Each Pack appends its own marked section.

use if2ai_backend::modules::api::{InputContentBlock, InputMessage};
use if2ai_backend::modules::harness::run_report::Severity as HarnessSeverity;
use if2ai_backend::modules::learning::failure_clustering::{
    ClusteredFailureSet, FailureCluster, FailureSignature, FAILURE_CLUSTERING_VERSION,
};
use if2ai_backend::modules::learning::failure_taxonomy::FailureCategory;
use if2ai_backend::modules::learning::self_edit::{
    generate_proposals, ProposalKind, SelfEditProposal,
};
use if2ai_backend::modules::memory::{MemoryError, MockUtilityLlm, UtilityLlm};

fn assistant_text(t: &str) -> InputMessage {
    InputMessage {
        role: "assistant".to_string(),
        content: vec![InputContentBlock::Text {
            text: t.to_string(),
        }],
        thinking: None,
    }
}

fn user_text(t: &str) -> InputMessage {
    InputMessage::user_text(t)
}

fn cluster_with_occurrences(category: FailureCategory, occurrences: usize) -> FailureCluster {
    FailureCluster {
        category,
        signatures: vec![FailureSignature {
            code: "tool_failure".to_string(),
            max_severity: HarnessSeverity::Blocking,
            run_id: "run-test".to_string(),
            occurrences,
        }],
        total_failures: occurrences,
        max_severity: HarnessSeverity::Blocking,
    }
}

fn build_failures(clusters: Vec<FailureCluster>) -> ClusteredFailureSet {
    let total: usize = clusters.iter().map(|c| c.total_failures).sum();
    ClusteredFailureSet {
        clustering_version: FAILURE_CLUSTERING_VERSION.to_string(),
        clusters,
        input_run_ids: vec!["run-test".to_string()],
        total_failures: total,
    }
}

const PROPOSAL_JSON: &str = r#"{"kind":"prompt_tweak","target":"system_prompt","before":"old","after":"new improved guidance for tool errors","justification":"prevents recurring tool_failure"}"#;

// ---------------------------------------------------------------------------
// FEAT-AE-001: Self-edit Proposal Generator
// ---------------------------------------------------------------------------

#[tokio::test]
async fn proposal_empty_cluster_returns_empty() {
    let mock = MockUtilityLlm::empty();
    let failures = ClusteredFailureSet::empty();
    let history = vec![user_text("hi")];
    let out = generate_proposals(&failures, &history, &mock).await;
    assert!(out.is_empty(), "empty failure set must yield no proposals");
}

#[tokio::test]
async fn proposal_high_freq_cluster_generates() {
    let responses = vec![PROPOSAL_JSON.to_string(); 4];
    let mock = MockUtilityLlm::new(responses);
    let failures = build_failures(vec![cluster_with_occurrences(
        FailureCategory::ToolMisuse,
        5,
    )]);
    let history = vec![user_text("kick off"), assistant_text("running tool")];
    let out = generate_proposals(&failures, &history, &mock).await;
    assert!(
        !out.is_empty(),
        "≥3-occurrence cluster must yield ≥ 1 proposal, got {}",
        out.len()
    );
    let p = &out[0];
    assert_eq!(p.kind, ProposalKind::PromptTweak);
    assert_eq!(p.target, "system_prompt");
    assert!(!p.after.is_empty());
}

struct FailingLlm;

#[async_trait::async_trait]
impl UtilityLlm for FailingLlm {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _max_tokens: u32,
        _temperature: f32,
    ) -> Result<String, MemoryError> {
        Err(MemoryError::Generic("simulated LLM outage".into()))
    }
}

#[tokio::test]
async fn proposal_llm_error_graceful_degradation() {
    let llm = FailingLlm;
    let failures = build_failures(vec![cluster_with_occurrences(
        FailureCategory::ToolMisuse,
        4,
    )]);
    let history = vec![user_text("anything")];
    let out = generate_proposals(&failures, &history, &llm).await;
    assert!(
        out.is_empty(),
        "LLM Err must degrade to empty proposals, got {}",
        out.len()
    );
}

#[tokio::test]
async fn proposal_fields_non_empty_and_unique_ids() {
    let responses = vec![PROPOSAL_JSON.to_string(); 4];
    let mock = MockUtilityLlm::new(responses);
    let failures = build_failures(vec![
        cluster_with_occurrences(FailureCategory::ToolMisuse, 4),
        cluster_with_occurrences(FailureCategory::PolicyIssue, 3),
    ]);
    let out = generate_proposals(&failures, &[], &mock).await;
    assert!(!out.is_empty());
    let mut seen_ids = std::collections::HashSet::new();
    for p in &out {
        assert!(!p.id.is_empty(), "id must be non-empty");
        assert!(!p.after.is_empty(), "after must be non-empty");
        assert!(seen_ids.insert(p.id.clone()), "id must be unique: {}", p.id);
    }
}

// Silence unused warnings for tests that future Pack sections will use.
#[allow(dead_code)]
fn _silence_unused() {
    let _ = SelfEditProposal {
        id: String::new(),
        kind: ProposalKind::SkillDraft,
        target: String::new(),
        before: None,
        after: String::new(),
        justification: String::new(),
        source_cluster_id: None,
    };
}

// ---------------------------------------------------------------------------
// FEAT-AE-002: Verification Gate (4-dim)
// ---------------------------------------------------------------------------

use if2ai_backend::modules::learning::self_edit::{
    verify_proposals, Verdict, GATE_CONSTITUTION, GATE_DEDUP, GATE_MALFORMED,
};
use if2ai_backend::modules::skills::sedimentation::Embedder;
use std::collections::HashMap;

struct ProposalEmbedder {
    map: HashMap<String, Vec<f32>>,
    fallback: Vec<f32>,
}

impl Embedder for ProposalEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        for (key, vec) in &self.map {
            if text.contains(key) {
                return vec.clone();
            }
        }
        self.fallback.clone()
    }
}

fn proposal(id: &str, after: &str) -> SelfEditProposal {
    SelfEditProposal {
        id: id.to_string(),
        kind: ProposalKind::PromptTweak,
        target: format!("target-{id}"),
        before: None,
        after: after.to_string(),
        justification: "test justification".to_string(),
        source_cluster_id: Some("tool_misuse".to_string()),
    }
}

#[test]
fn verify_clean_proposal_passes() {
    let embedder = ProposalEmbedder {
        map: HashMap::new(),
        fallback: vec![1.0, 0.0, 0.0],
    };
    let p = proposal(
        "p1",
        "Add a graceful retry on tool_failure with backoff 250ms.",
    );
    let out = verify_proposals(vec![p], &[], &embedder);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1.verdict, Verdict::Pass);
    assert!(out[0].1.failed_gates.is_empty());
}

#[test]
fn verify_malicious_after_fails_constitution() {
    let embedder = ProposalEmbedder {
        map: HashMap::new(),
        fallback: vec![1.0, 0.0, 0.0],
    };
    let p = proposal("p2", "Run `rm -rf /` to clear stale files");
    let out = verify_proposals(vec![p], &[], &embedder);
    assert_eq!(out[0].1.verdict, Verdict::Fail);
    assert!(
        out[0].1.failed_gates.iter().any(|g| g == GATE_CONSTITUTION),
        "expected constitution gate in {:?}",
        out[0].1.failed_gates
    );
}

#[test]
fn verify_duplicate_proposal_fails_dedup() {
    let mut map = HashMap::new();
    map.insert("ALPHA".to_string(), vec![1.0, 0.0, 0.0, 0.0]);
    map.insert("ALPHA_DUP".to_string(), vec![0.99, 0.01, 0.0, 0.0]);
    let embedder = ProposalEmbedder {
        map,
        fallback: vec![0.0, 0.0, 0.0, 1.0],
    };
    let prior = vec![proposal("prior", "ALPHA original after content")];
    let new_one = proposal("new", "ALPHA_DUP almost identical content");
    let out = verify_proposals(vec![new_one], &prior, &embedder);
    assert_eq!(out[0].1.verdict, Verdict::Fail);
    assert!(
        out[0].1.failed_gates.iter().any(|g| g == GATE_DEDUP),
        "expected dedup gate in {:?}",
        out[0].1.failed_gates
    );
}

#[test]
fn verify_empty_after_fails_malformed() {
    let embedder = ProposalEmbedder {
        map: HashMap::new(),
        fallback: vec![1.0, 0.0],
    };
    let p = proposal("p_empty", "   ");
    let out = verify_proposals(vec![p], &[], &embedder);
    assert_eq!(out[0].1.verdict, Verdict::Fail);
    assert!(
        out[0].1.failed_gates.iter().any(|g| g == GATE_MALFORMED),
        "expected malformed gate in {:?}",
        out[0].1.failed_gates
    );
}

// ---------------------------------------------------------------------------
// FEAT-AE-003: Promotion State Machine
// ---------------------------------------------------------------------------

use if2ai_backend::modules::learning::self_edit::{
    next_stage, PromotionStage, StageTransition, MIN_SAMPLE_FOR_DECISION,
};

#[test]
fn promotion_shadow_healthy_promotes() {
    let t = next_stage(PromotionStage::Shadow, 0.02, MIN_SAMPLE_FOR_DECISION);
    assert_eq!(t, StageTransition::Promote(PromotionStage::Canary1Pct));
}

#[test]
fn promotion_canary1_high_failure_demotes() {
    let t = next_stage(PromotionStage::Canary1Pct, 0.15, 200);
    assert_eq!(t, StageTransition::Demote(PromotionStage::Shadow));
}

#[test]
fn promotion_production_healthy_holds() {
    let t = next_stage(PromotionStage::Production, 0.01, 1000);
    assert_eq!(t, StageTransition::Hold);
}

#[test]
fn promotion_insufficient_sample_holds() {
    let t = next_stage(PromotionStage::Shadow, 0.0, MIN_SAMPLE_FOR_DECISION - 1);
    assert_eq!(t, StageTransition::Hold);
}

#[test]
fn promotion_shadow_demote_stays_shadow() {
    let t = next_stage(PromotionStage::Shadow, 0.5, 100);
    assert_eq!(
        t,
        StageTransition::Hold,
        "Shadow is the floor — must Hold on demote"
    );
}
