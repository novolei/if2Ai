//! FEAT-TE-001 + FEAT-TE-002 integration tests.
//!
//! TE-001: proves the 5-tier hard cap is wired into the preflight
//! pipeline path consumers see in production.
//!
//! TE-002: proves message-level LLM compression replaces long messages
//! with their summaries before tier compression sees them, and falls
//! back gracefully on LLM errors.

use std::sync::Arc;

use async_trait::async_trait;
use if2ai_backend::modules::api::{InputContentBlock, InputMessage};
use if2ai_backend::modules::memory::{MemoryError, MockUtilityLlm, UtilityLlm};
use if2ai_backend::modules::runtime::context_compression::{
    apply_digest, compress_for_request, CompressedMessage, MessageDigester, TierBudgetAllocation,
    DEFAULT_DIGEST_THRESHOLD_TOKENS,
};

/// Spec #3: when the caller hands in *more* than the tier hard cap,
/// the overflow is dropped from the head before any downstream
/// preflight / sanitize step gets to see it.
#[test]
fn preflight_integration_hard_drops_overflow() {
    let allocation = TierBudgetAllocation {
        system: 50,
        compressed_history: 50,
        working_checkpoint: 25,
        recent_messages: 100,
        tool_buffer: 25,
    };

    let bloat = "y".repeat(2_000);
    let mut messages: Vec<InputMessage> = (0..40)
        .map(|i| InputMessage::user_text(format!("msg-{i}-{bloat}")))
        .collect();
    messages.push(InputMessage::user_text("most-recent-marker"));

    let outcome = compress_for_request(&messages, &allocation);

    assert!(
        !outcome.passthrough,
        "overflow input must not pass through unchanged"
    );
    assert!(
        outcome.dropped > 0,
        "expected at least one head message dropped (got {})",
        outcome.dropped
    );
    assert!(
        outcome.kept_tokens <= allocation.total() + 4,
        "kept_tokens {} must respect hard cap {}",
        outcome.kept_tokens,
        allocation.total()
    );
    let last_text = match outcome.kept.last().and_then(|m| m.content.first()).cloned() {
        Some(InputContentBlock::Text { text }) => text,
        _ => String::new(),
    };
    assert!(
        last_text.contains("most-recent-marker"),
        "tail message must always survive the hard cap; got: {}",
        last_text
    );
}

// ---------------------------------------------------------------------------
// FEAT-TE-002: message digester tests
// ---------------------------------------------------------------------------

/// A long message body that comfortably exceeds the 500-token default
/// threshold (≈ 4 chars / token → ~3.5K tokens at 14K chars).
fn long_text() -> String {
    "lorem ipsum dolor sit amet ".repeat(600)
}

/// Spec #1: long messages get LLM-summarized; the resulting message has
/// strictly fewer tokens than the original (Pack: < 50 %).
#[tokio::test]
async fn digester_compresses_long_message() {
    let mock = Arc::new(MockUtilityLlm::new(vec!["short summary".to_string()]));
    let digester = MessageDigester::new(mock.clone());
    let messages = vec![InputMessage::user_text(long_text())];

    let digest = digester.digest(&messages).await;

    assert_eq!(digest.len(), 1, "digest length must mirror input length");
    let entry = &digest[0];
    assert!(
        entry.was_summarized(),
        "long message must be summarized, got Full"
    );
    if let CompressedMessage::Summarized {
        original_tokens,
        summary_tokens,
        ..
    } = entry
    {
        assert!(
            *summary_tokens * 2 < *original_tokens,
            "summary tokens {} must be < 50% of original {}",
            summary_tokens,
            original_tokens
        );
    } else {
        panic!("expected Summarized variant");
    }
    assert_eq!(mock.call_count(), 1, "exactly one LLM call expected");
}

/// Spec #2: short messages skip the LLM entirely (no calls made).
#[tokio::test]
async fn digester_skips_short_message() {
    let mock = Arc::new(MockUtilityLlm::empty());
    let digester = MessageDigester::new(mock.clone());
    let messages = vec![
        InputMessage::user_text("hi"),
        InputMessage::user_text("how are you?"),
    ];

    let digest = digester.digest(&messages).await;

    assert_eq!(digest.len(), 2);
    assert!(digest
        .iter()
        .all(|d| matches!(d, CompressedMessage::Full { .. })));
    assert_eq!(
        mock.call_count(),
        0,
        "short messages must NOT trigger any LLM call (got {})",
        mock.call_count()
    );
    assert!(DEFAULT_DIGEST_THRESHOLD_TOKENS >= 500);
}

/// Spec #3: when the digest is fed into the preflight pipeline (via the
/// `digested_messages` field of `PreflightContext`), the compressed
/// summary replaces the original message in the request body.
#[tokio::test]
async fn preflight_uses_digested_messages() {
    let mock = Arc::new(MockUtilityLlm::new(vec![
        "compact summary alpha".to_string()
    ]));
    let digester = MessageDigester::new(mock);

    let original = vec![
        InputMessage::user_text(long_text()),
        InputMessage::user_text("recent short"),
    ];
    let digest = digester.digest(&original).await;
    let materialized = apply_digest(&digest);

    assert_eq!(materialized.len(), original.len());
    let first_text = match materialized[0].content.first() {
        Some(InputContentBlock::Text { text }) => text.clone(),
        _ => panic!("first message must be text after digest"),
    };
    assert!(
        first_text.contains("compact summary alpha"),
        "preflight input must carry the LLM summary, got: {first_text}"
    );

    let allocation = TierBudgetAllocation::default();
    let outcome = compress_for_request(&materialized, &allocation);
    assert!(outcome.passthrough, "summarized payload must fit budget");
    assert_eq!(outcome.kept.len(), 2);
    let kept_first = match outcome.kept[0].content.first() {
        Some(InputContentBlock::Text { text }) => text.clone(),
        _ => String::new(),
    };
    assert!(
        kept_first.contains("compact summary alpha"),
        "tier compressor must preserve digested summary, got: {kept_first}"
    );
}

struct FailingUtilityLlm;

#[async_trait]
impl UtilityLlm for FailingUtilityLlm {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _max_tokens: u32,
        _temperature: f32,
    ) -> Result<String, MemoryError> {
        Err(MemoryError::Generic("simulated provider outage".into()))
    }
}

/// Spec #4: LLM error → digester preserves the original message instead
/// of dropping it; never panics.
#[tokio::test]
async fn digester_fallback_on_llm_error() {
    let digester = MessageDigester::new(Arc::new(FailingUtilityLlm));
    let original = long_text();
    let messages = vec![InputMessage::user_text(original.clone())];

    let digest = digester.digest(&messages).await;

    assert_eq!(digest.len(), 1);
    assert!(
        matches!(&digest[0], CompressedMessage::Full { .. }),
        "LLM error must fall back to Full, got Summarized"
    );
    let materialized = apply_digest(&digest);
    let kept_text = match materialized[0].content.first() {
        Some(InputContentBlock::Text { text }) => text.clone(),
        _ => String::new(),
    };
    assert_eq!(
        kept_text, original,
        "fallback must preserve the original message verbatim"
    );
}
