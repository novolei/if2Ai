//! WU-003 — Stream-finalize sedimentation hook tests.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

/// Serialize every test in this file that mutates env vars — cargo
/// runs `#[tokio::test]` cases on parallel threads, and shared env
/// state is fundamentally racy. The lock costs a few microseconds
/// per test and removes a real flake.
static ENV_LOCK: Mutex<()> = Mutex::new(());

use if2ai_backend::modules::api::{InputContentBlock, InputMessage};
use if2ai_backend::modules::application::turn_service::finalize_hooks::{
    extract_turn_checkpoint, run_domain_knowledge_contributor, run_sedimentation_pipeline,
    DISABLE_AUTO_SKILL_ENV,
};
use if2ai_backend::modules::memory::{MemoryError, MockUtilityLlm, UtilityLlm};
use if2ai_backend::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
use if2ai_backend::modules::runtime::evolution_emitter::{emit_evolution_event, DISABLE_EMIT_ENV};

const SKILL_BODY: &str =
    "---\nname: \"finalize-skill\"\ndescription: \"WU-003 fixture\"\n---\n# Body\n";
const DK_JSON: &str = r#"{"kind":"website_domain","domain":"github.com","selector":"div.main","purpose":"main panel","gotcha":"sticky header"}"#;

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

fn build_history() -> Vec<InputMessage> {
    let mut h: Vec<InputMessage> = Vec::new();
    for _ in 0..4 {
        h.push(assistant_tool_use("file_read"));
        h.push(assistant_tool_use("file_write"));
    }
    h.insert(0, InputMessage::user_text("kick off the run"));
    h
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn sedimentation_called_on_success() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var(DISABLE_AUTO_SKILL_ENV).ok();
    std::env::remove_var(DISABLE_AUTO_SKILL_ENV);
    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::new(vec![SKILL_BODY.to_string(); 4]));
    let drafts = run_sedimentation_pipeline(&build_history(), llm).await;
    assert!(
        !drafts.is_empty(),
        "MIN_REPEATS+ history must yield ≥ 1 draft"
    );
    if let Some(v) = prev {
        std::env::set_var(DISABLE_AUTO_SKILL_ENV, v);
    }
}

struct PanickingLlm;

#[async_trait]
impl UtilityLlm for PanickingLlm {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _max_tokens: u32,
        _temperature: f32,
    ) -> Result<String, MemoryError> {
        Err(MemoryError::Generic("simulated outage (graceful)".into()))
    }
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn sedimentation_panic_does_not_propagate() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var(DISABLE_AUTO_SKILL_ENV).ok();
    std::env::remove_var(DISABLE_AUTO_SKILL_ENV);
    let llm: Arc<dyn UtilityLlm> = Arc::new(PanickingLlm);
    let drafts = run_sedimentation_pipeline(&build_history(), llm).await;
    assert!(
        drafts.is_empty(),
        "broken LLM must yield empty drafts (no propagation)"
    );
    if let Some(v) = prev {
        std::env::set_var(DISABLE_AUTO_SKILL_ENV, v);
    }
}

#[test]
fn sedimented_emits_skill_sedimented_event() {
    let prev = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    let env = emit_evolution_event(
        None,
        RuntimeEventType::SkillSedimented,
        "draft",
        CorrelationIds::default(),
        &serde_json::json!({
            "name": "finalize-skill",
            "description": "WU-003 fixture",
            "toolSequence": ["file_read", "file_write"],
            "sourceTurns": [1, 2, 3],
        }),
        None,
    )
    .expect("WU-001 emit must succeed under kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::SkillSedimented);
    match prev {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[test]
fn checkpoint_extracted_emits_event() {
    let prev = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    let extraction =
        extract_turn_checkpoint("done <key_info>file=README.md, line=42</key_info> ok");
    assert_eq!(
        extraction.key_info,
        Some("file=README.md, line=42".to_string())
    );

    let env = emit_evolution_event(
        None,
        RuntimeEventType::CheckpointUpdated,
        "extracted",
        CorrelationIds::default(),
        &serde_json::json!({
            "sessionId": "sess-test",
            "action": "extracted",
        }),
        None,
    )
    .expect("emit must succeed under kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::CheckpointUpdated);
    match prev {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn env_flag_disables_sedimentation() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var(DISABLE_AUTO_SKILL_ENV).ok();
    std::env::set_var(DISABLE_AUTO_SKILL_ENV, "1");
    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::new(vec![SKILL_BODY.to_string(); 4]));
    let drafts = run_sedimentation_pipeline(&build_history(), llm.clone()).await;
    assert!(drafts.is_empty(), "kill-switch must skip sedimentation");
    let dk = run_domain_knowledge_contributor(&build_history(), llm).await;
    assert!(dk.is_empty(), "kill-switch must also skip DK contributor");
    match prev {
        Some(v) => std::env::set_var(DISABLE_AUTO_SKILL_ENV, v),
        None => std::env::remove_var(DISABLE_AUTO_SKILL_ENV),
    }
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn domain_knowledge_event_emitted() {
    let _guard = ENV_LOCK.lock().unwrap();
    let prev = std::env::var(DISABLE_AUTO_SKILL_ENV).ok();
    std::env::remove_var(DISABLE_AUTO_SKILL_ENV);
    let prev_emit = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");

    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::new(vec![DK_JSON.to_string()]));
    let history = vec![
        InputMessage::user_text("scrape github.com"),
        InputMessage {
            role: "assistant".to_string(),
            content: vec![InputContentBlock::Text {
                text: "use selector div.main".to_string(),
            }],
            thinking: None,
        },
    ];
    let candidates = run_domain_knowledge_contributor(&history, llm).await;
    assert_eq!(candidates.len(), 1);

    let env = emit_evolution_event(
        None,
        RuntimeEventType::DomainKnowledge,
        "contribution",
        CorrelationIds::default(),
        &serde_json::json!({
            "entryId": &candidates[0].id,
            "kind": "website_domain",
            "source": "contribution",
        }),
        None,
    )
    .expect("emit must succeed under kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::DomainKnowledge);

    match prev_emit {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
    if let Some(v) = prev {
        std::env::set_var(DISABLE_AUTO_SKILL_ENV, v);
    }
}
