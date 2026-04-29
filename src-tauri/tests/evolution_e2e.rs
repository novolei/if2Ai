//! FEAT-INT-002 — Cross-Pack end-to-end harness.
//!
//! Five integration tests that wire one logical "evolution turn"
//! across every Pack family (TE / SE / SH / AE / DK / BR / INT).
//! Every external dependency is mocked in-process — zero real LLM,
//! Chromium, sqlite, or filesystem I/O. Each test must complete in
//! well under a second.

use std::collections::HashMap;

use async_trait::async_trait;

use if2ai_backend::modules::api::{InputContentBlock, InputMessage};
use if2ai_backend::modules::harness::run_report::Severity as HarnessSeverity;
use if2ai_backend::modules::learning::failure_clustering::{
    ClusteredFailureSet, FailureCluster, FailureSignature, FAILURE_CLUSTERING_VERSION,
};
use if2ai_backend::modules::learning::failure_taxonomy::FailureCategory;
use if2ai_backend::modules::learning::self_edit::{
    generate_proposals, next_stage, verify_proposals, PromotionStage, ProposalKind,
    SelfEditProposal, StageTransition, Verdict,
};
use if2ai_backend::modules::memory::{MemoryError, MockUtilityLlm, UtilityLlm};
use if2ai_backend::modules::runtime::budget::estimate_tokens;
use if2ai_backend::modules::runtime::context_compression::{
    compress_for_request, TierBudgetAllocation,
};
use if2ai_backend::modules::runtime::working_checkpoint::{
    inject_checkpoint, CheckpointInjectionConfig, InjectionPosition, WorkingCheckpoint,
    DEFAULT_CHECKPOINT_MAX_TOKENS,
};
use if2ai_backend::modules::skills::domain_knowledge::extract_domain_knowledge_candidates;
use if2ai_backend::modules::skills::sedimentation::{
    dedup_drafts, extract_skill_drafts, Embedder, MIN_REPEATS,
};
use if2ai_backend::modules::skills::vector_index::{
    index_skill, mock::MockVectorStore, search_skills,
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

const PROPOSAL_JSON: &str = r#"{"kind":"prompt_tweak","target":"system_prompt","before":"old","after":"add a graceful retry on tool_failure with backoff 250ms.","justification":"prevents recurring tool_failure"}"#;

const SKILL_BODY: &str = "---\nname: \"e2e-skill\"\ndescription: \"E2E skill for tests\"\n---\n# E2E\n\n## When to use\nDuring the e2e harness.\n\n## Steps\n1. Step one\n2. Step two\n";

const DK_JSON: &str = r#"{"kind":"website_domain","domain":"github.com","selector":"div.repo-content","purpose":"main panel","gotcha":"sticky header overlap"}"#;

fn cluster_with(category: FailureCategory, occurrences: usize) -> FailureCluster {
    FailureCluster {
        category,
        signatures: vec![FailureSignature {
            code: "tool_failure".to_string(),
            max_severity: HarnessSeverity::Blocking,
            run_id: "run-e2e".to_string(),
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
        input_run_ids: vec!["run-e2e".to_string()],
        total_failures: total,
    }
}

fn assistant_tool_use(name: &str) -> InputMessage {
    InputMessage {
        role: "assistant".to_string(),
        content: vec![InputContentBlock::ToolUse {
            id: format!("call-{name}"),
            name: name.to_string(),
            input: serde_json::json!({}),
        }],
        thinking: None,
    }
}

struct ConstEmbedder(Vec<f32>);

impl Embedder for ConstEmbedder {
    fn embed(&self, _text: &str) -> Vec<f32> {
        self.0.clone()
    }
}

struct LookupEmbedder {
    map: HashMap<String, Vec<f32>>,
    fallback: Vec<f32>,
}

impl Embedder for LookupEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        for (key, vec) in &self.map {
            if text.contains(key) {
                return vec.clone();
            }
        }
        self.fallback.clone()
    }
}

struct FailingLlm;

#[async_trait]
impl UtilityLlm for FailingLlm {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _max_tokens: u32,
        _temperature: f32,
    ) -> Result<String, MemoryError> {
        Err(MemoryError::Generic("simulated".into()))
    }
}

// ---------------------------------------------------------------------------
// Spec #1 — self-edit pipeline (AE-001 → AE-002 → AE-003)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evolution_e2e_self_edit_pipeline() {
    let llm = MockUtilityLlm::new(vec![PROPOSAL_JSON.to_string(); 4]);
    let failures = build_failures(vec![cluster_with(FailureCategory::ToolMisuse, 5)]);
    let history = vec![InputMessage::user_text("kick off")];

    let proposals = generate_proposals(&failures, &history, &llm).await;
    assert!(!proposals.is_empty(), "AE-001 must yield ≥ 1 proposal");

    let embedder = ConstEmbedder(vec![1.0, 0.0]);
    let verified = verify_proposals(proposals.clone(), &[], &embedder);
    assert_eq!(verified.len(), proposals.len());
    assert!(
        verified.iter().any(|(_, v)| v.verdict == Verdict::Pass),
        "AE-002 must let at least one proposal through"
    );

    // AE-003: a healthy Shadow stage with 50+ samples and 0% failure
    // → Promote(Canary1Pct).
    let transition = next_stage(PromotionStage::Shadow, 0.0, 50);
    assert_eq!(
        transition,
        StageTransition::Promote(PromotionStage::Canary1Pct)
    );
}

// ---------------------------------------------------------------------------
// Spec #2 — skill pipeline (SE-001 → SE-002 → SE-004)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evolution_e2e_skill_pipeline() {
    let llm = MockUtilityLlm::new(vec![SKILL_BODY.to_string(); 4]);
    let mut history: Vec<InputMessage> = Vec::new();
    for _ in 0..MIN_REPEATS + 1 {
        history.push(assistant_tool_use("file_read"));
        history.push(assistant_tool_use("file_write"));
    }

    let drafts = extract_skill_drafts(&history, &llm).await;
    assert!(!drafts.is_empty(), "SE-001 must produce ≥ 1 draft");

    let mut emb_map: HashMap<String, Vec<f32>> = HashMap::new();
    emb_map.insert("e2e-skill".to_string(), vec![1.0, 0.0, 0.0, 0.0]);
    emb_map.insert("Read file".to_string(), vec![0.95, 0.05, 0.0, 0.0]);
    let embedder = LookupEmbedder {
        map: emb_map,
        fallback: vec![0.0, 0.0, 0.0, 1.0],
    };

    let deduped = dedup_drafts(drafts.clone(), &embedder);
    assert!(!deduped.is_empty(), "SE-002 must produce ≥ 1 dedup group");

    let store = MockVectorStore::new();
    for d in &deduped {
        index_skill(
            &d.representative.name,
            &d.representative.description,
            &d.representative.body,
            &embedder,
            &store,
        )
        .await
        .expect("SE-004 index must succeed");
    }
    let hits = search_skills("Read file please", 5, &embedder, &store).await;
    assert!(!hits.is_empty(), "SE-004 search must hit the indexed skill");
}

// ---------------------------------------------------------------------------
// Spec #3 — self-healing → proposal (cluster → AE → promotion)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evolution_e2e_self_healing_to_proposal() {
    // Daemon probe failures aggregate into a `RecoveryIssue` cluster
    // (one signature, occurrences = 4) — above the AE-001 threshold.
    let failures = build_failures(vec![cluster_with(FailureCategory::RecoveryIssue, 4)]);

    let llm = MockUtilityLlm::new(vec![PROPOSAL_JSON.to_string(); 2]);
    let proposals = generate_proposals(&failures, &[], &llm).await;
    assert!(!proposals.is_empty());

    let embedder = ConstEmbedder(vec![1.0, 0.0]);
    let verified = verify_proposals(proposals, &[], &embedder);
    let pass_count = verified
        .iter()
        .filter(|(_, v)| v.verdict == Verdict::Pass)
        .count();
    assert!(pass_count >= 1, "at least one proposal must pass AE-002");

    // Healthy promotion afterwards.
    let t = next_stage(PromotionStage::Shadow, 0.02, 60);
    assert_eq!(t, StageTransition::Promote(PromotionStage::Canary1Pct));
}

// ---------------------------------------------------------------------------
// Spec #4 — domain knowledge extraction co-existing with sedimentation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evolution_e2e_domain_knowledge_to_skill() {
    // Two LLMs: one for DK candidates, one for skill drafts. We share
    // the same sync `Embedder` between dedup + (future) DK
    // verification — proving the trait is portable across pipelines.
    let dk_llm = MockUtilityLlm::new(vec![DK_JSON.to_string()]);
    let skill_llm = MockUtilityLlm::new(vec![SKILL_BODY.to_string(); 4]);

    let history: Vec<InputMessage> = vec![
        InputMessage::user_text("scrape github.com main panel"),
        InputMessage {
            role: "assistant".to_string(),
            content: vec![InputContentBlock::Text {
                text: "use selector div.repo-content".to_string(),
            }],
            thinking: None,
        },
    ];

    let dk_candidates = extract_domain_knowledge_candidates(&history, &dk_llm).await;
    assert_eq!(dk_candidates.len(), 1);
    assert_eq!(dk_candidates[0].kind.label(), "website_domain");

    let mut tool_history: Vec<InputMessage> = history.clone();
    for _ in 0..MIN_REPEATS + 1 {
        tool_history.push(assistant_tool_use("http_get"));
        tool_history.push(assistant_tool_use("html_parse"));
    }
    let drafts = extract_skill_drafts(&tool_history, &skill_llm).await;
    assert!(!drafts.is_empty(), "SE-001 must produce drafts in parallel");

    let embedder = ConstEmbedder(vec![1.0, 0.0, 0.0]);
    let deduped = dedup_drafts(drafts, &embedder);
    assert!(!deduped.is_empty(), "SE-002 dedup must use shared Embedder");
}

// ---------------------------------------------------------------------------
// Spec #5 — checkpoint injection inside compressed request stream
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evolution_e2e_checkpoint_inject_into_compressed_request() {
    // 1. Build a session that exceeds the tier budget so TE-001 trims.
    let bloat = "x".repeat(2_000);
    let mut messages: Vec<InputMessage> = (0..20)
        .map(|i| InputMessage::user_text(format!("msg-{i}-{bloat}")))
        .collect();
    messages.push(InputMessage::user_text("recent"));

    let tier = TierBudgetAllocation {
        system: 100,
        compressed_history: 100,
        working_checkpoint: 50,
        recent_messages: 200,
        tool_buffer: 50,
    };
    let outcome = compress_for_request(&messages, &tier);
    assert!(!outcome.passthrough);
    assert!(outcome.dropped > 0);

    // 2. Inject a checkpoint after compression.
    let mut next_messages = outcome.kept;
    let cp = WorkingCheckpoint {
        session_id: "sess-e2e".to_string(),
        key_info: "important fact: file=README.md, line=42".to_string(),
        related_sop: None,
        turn_created: 1,
        turn_updated: 1,
    };
    let cfg = CheckpointInjectionConfig {
        max_tokens: DEFAULT_CHECKPOINT_MAX_TOKENS,
        injection_position: InjectionPosition::BeforeLastUserMessage,
        auto_extract: true,
    };
    inject_checkpoint(&cp, &mut next_messages, &cfg);

    // 3. The injected payload must respect the per-block 200-token cap.
    let injected = next_messages
        .iter()
        .find_map(|m| {
            m.content.iter().find_map(|b| match b {
                InputContentBlock::Text { text } if text.contains("[checkpoint]") => {
                    Some(text.clone())
                }
                _ => None,
            })
        })
        .expect("checkpoint payload must be present after inject");
    assert!(estimate_tokens(&injected) <= DEFAULT_CHECKPOINT_MAX_TOKENS);

    // 4. Last user message survives unchanged.
    let last_text = match next_messages.last().and_then(|m| m.content.first()) {
        Some(InputContentBlock::Text { text }) => text.clone(),
        _ => String::new(),
    };
    assert!(last_text.contains("recent"));
}

// Silence unused-import warnings when test selection skips a path.
#[allow(dead_code)]
fn _silence_unused() {
    let _ = ProposalKind::PromptTweak;
    let _: SelfEditProposal = SelfEditProposal {
        id: String::new(),
        kind: ProposalKind::PromptTweak,
        target: String::new(),
        before: None,
        after: String::new(),
        justification: String::new(),
        source_cluster_id: None,
    };
    let _ = FailingLlm;
}
