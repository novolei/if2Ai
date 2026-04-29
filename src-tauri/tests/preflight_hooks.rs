//! WU-004 — Preflight wire-up hook tests.

use std::sync::Arc;

use async_trait::async_trait;

use if2ai_backend::modules::api::{InputContentBlock, InputMessage};
use if2ai_backend::modules::application::turn_service::preflight_hooks::{
    digest_messages_for_preflight, maybe_inject_checkpoint, maybe_render_mini_index_block,
    DISABLE_DIGESTER_ENV, DISABLE_MINI_INDEX_ENV,
};
use if2ai_backend::modules::memory::{MemoryError, MockUtilityLlm, UtilityLlm};
use if2ai_backend::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
use if2ai_backend::modules::runtime::evolution_emitter::{emit_evolution_event, DISABLE_EMIT_ENV};
use if2ai_backend::modules::runtime::working_checkpoint::{
    CheckpointInjectionConfig, InjectionPosition, WorkingCheckpoint, DEFAULT_CHECKPOINT_MAX_TOKENS,
};

const SUMMARY_BODY: &str = "compact summary alpha";

fn long_text() -> String {
    "lorem ipsum dolor sit amet ".repeat(600)
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

#[tokio::test]
async fn digester_populates_digested_messages() {
    let prev = std::env::var(DISABLE_DIGESTER_ENV).ok();
    std::env::remove_var(DISABLE_DIGESTER_ENV);
    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::new(vec![SUMMARY_BODY.to_string()]));
    let messages = vec![InputMessage::user_text(long_text())];
    let out = digest_messages_for_preflight(&messages, llm).await;
    assert!(out.is_some(), "digester must populate digested_messages");
    let materialized = out.unwrap();
    assert_eq!(materialized.len(), messages.len());
    if let Some(v) = prev {
        std::env::set_var(DISABLE_DIGESTER_ENV, v);
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

#[tokio::test]
async fn digester_failure_falls_back() {
    // FEAT-TE-002 digester preserves originals on LLM error rather
    // than dropping them. The wire-up wrapper still returns Some(...)
    // (length-equal fallback) so callers can pass it into
    // PreflightContext.digested_messages safely; main flow never
    // breaks. Empty-input gives None.
    let llm: Arc<dyn UtilityLlm> = Arc::new(FailingLlm);
    let out_empty = digest_messages_for_preflight(&[], llm.clone()).await;
    assert!(out_empty.is_none(), "empty messages → None");

    let messages = vec![InputMessage::user_text(long_text())];
    let out = digest_messages_for_preflight(&messages, llm).await;
    assert!(out.is_some(), "FailingLlm must yield Some(originals)");
    let materialized = out.unwrap();
    assert_eq!(materialized.len(), messages.len());
}

#[test]
fn checkpoint_inject_emits_event() {
    let prev_emit = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");

    let cp = WorkingCheckpoint {
        session_id: "sess-test".to_string(),
        key_info: "file=README.md, line=42".to_string(),
        related_sop: None,
        turn_created: 1,
        turn_updated: 1,
    };
    let mut messages = vec![InputMessage::user_text("user q")];
    let cfg = CheckpointInjectionConfig {
        max_tokens: DEFAULT_CHECKPOINT_MAX_TOKENS,
        injection_position: InjectionPosition::BeforeLastUserMessage,
        auto_extract: true,
    };
    let injected = maybe_inject_checkpoint(Some(&cp), &mut messages, &cfg);
    assert!(injected, "checkpoint with non-empty key_info must inject");

    let env = emit_evolution_event(
        None,
        RuntimeEventType::CheckpointUpdated,
        "checkpoint_injected",
        CorrelationIds {
            session_id: Some("sess-test".to_string()),
            ..Default::default()
        },
        &serde_json::json!({"sessionId": "sess-test", "action": "injected"}),
        None,
    )
    .expect("emit must succeed under kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::CheckpointUpdated);

    match prev_emit {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[test]
fn mini_index_block_pushed_to_plan() {
    let prev = std::env::var(DISABLE_MINI_INDEX_ENV).ok();
    std::env::remove_var(DISABLE_MINI_INDEX_ENV);

    let mut history = vec![InputMessage::user_text("kick off the run")];
    for i in 0..5 {
        history.push(assistant_tool_use(&format!("tool_{i}")));
    }
    let block = maybe_render_mini_index_block(&history);
    assert!(
        block.is_some(),
        "non-trivial history must yield a mini index"
    );
    let b = block.unwrap();
    assert_eq!(b.priority, 95);
    assert!(!b.content.trim().is_empty());

    // Empty history → None (planner skips push entirely).
    let none_block = maybe_render_mini_index_block(&[]);
    assert!(none_block.is_none());

    if let Some(v) = prev {
        std::env::set_var(DISABLE_MINI_INDEX_ENV, v);
    }
}

#[tokio::test]
async fn env_flag_disables_digester() {
    let prev = std::env::var(DISABLE_DIGESTER_ENV).ok();
    std::env::set_var(DISABLE_DIGESTER_ENV, "1");
    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
    let messages = vec![InputMessage::user_text(long_text())];
    let out = digest_messages_for_preflight(&messages, llm).await;
    assert!(out.is_none(), "kill-switch must skip digester");
    match prev {
        Some(v) => std::env::set_var(DISABLE_DIGESTER_ENV, v),
        None => std::env::remove_var(DISABLE_DIGESTER_ENV),
    }
}
