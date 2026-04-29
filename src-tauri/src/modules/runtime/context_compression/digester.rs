//! FEAT-TE-002 — Message-level LLM compression.
//!
//! Builds on the FEAT-TE-001 hard tier cap by giving each `RecentMessages`
//! tier message a chance to be **summarized** (instead of dropped) when
//! its individual token estimate exceeds [`DEFAULT_DIGEST_THRESHOLD_TOKENS`].
//!
//! Design notes (per Pack FEAT-TE-002):
//! - The digester is purely *additive*: messages under the threshold are
//!   wrapped in [`CompressedMessage::Full`] and pass through unchanged so
//!   no tokens are spent on them.
//! - Summarization goes through the existing [`UtilityLlm`] trait — no new
//!   provider configuration, no new Cargo dependency.
//! - On any LLM error the digester **falls back** to the original message
//!   (`CompressedMessage::Full`) rather than dropping it — losing context
//!   silently is worse than skipping a compression attempt.
//! - The downstream sync `stream_preflight` consumes the *materialized*
//!   `Vec<InputMessage>` produced by [`apply_digest`]; the actual `digest`
//!   call is async and must be performed by the orchestrator before
//!   building the preflight context.

use std::sync::Arc;

use crate::modules::api::{InputContentBlock, InputMessage};
use crate::modules::memory::UtilityLlm;
use crate::modules::runtime::budget::estimate_tokens;

/// Per-message compression threshold. Messages whose estimated token
/// count is `>` this value are candidates for LLM summarization; the
/// Pack mandates a default of 500.
pub const DEFAULT_DIGEST_THRESHOLD_TOKENS: usize = 500;

/// Per-summary `max_tokens` ceiling. Keeps the LLM honest about the
/// "compress" promise: we want the summary to be materially shorter
/// than the original, otherwise we're paying tokens for nothing.
const SUMMARY_MAX_TOKENS: u32 = 240;

/// LLM temperature for summarization. Lower → more deterministic
/// (we want stable, reproducible summaries that don't introduce
/// hallucinated facts).
const SUMMARY_TEMPERATURE: f32 = 0.1;

/// System prompt used for every summarization call. Kept short so it
/// does not itself blow the utility budget.
const SUMMARY_SYSTEM_PROMPT: &str = concat!(
    "You compress conversation history. Produce a single short paragraph ",
    "(≤ 200 tokens) that preserves: (1) the user intent, (2) any concrete ",
    "facts/decisions, (3) tool names referenced. Drop pleasantries, ",
    "rephrasings, and verbose tool transcripts. Reply with the summary text only.",
);

/// Result of digesting one message.
#[derive(Debug, Clone, PartialEq)]
pub enum CompressedMessage {
    /// Original message preserved verbatim — either it was below the
    /// threshold or summarization failed and we fell back.
    Full {
        original_index: usize,
        message: InputMessage,
    },
    /// Original message replaced with an LLM-generated summary stuffed
    /// back into a single `text` content block on the same role.
    Summarized {
        original_index: usize,
        summary: InputMessage,
        original_tokens: usize,
        summary_tokens: usize,
    },
}

impl CompressedMessage {
    /// Materialized message that should enter the request body.
    pub fn into_message(self) -> InputMessage {
        match self {
            CompressedMessage::Full { message, .. } => message,
            CompressedMessage::Summarized { summary, .. } => summary,
        }
    }

    /// Borrowing accessor used by callers that want to project
    /// statistics without consuming the enum.
    pub fn message(&self) -> &InputMessage {
        match self {
            CompressedMessage::Full { message, .. } => message,
            CompressedMessage::Summarized { summary, .. } => summary,
        }
    }

    /// `true` iff the LLM actually compressed this message.
    pub fn was_summarized(&self) -> bool {
        matches!(self, CompressedMessage::Summarized { .. })
    }
}

/// Errors surfaced by [`MessageDigester::digest`]. Today this is opaque
/// because the trait only ever yields `MemoryError` — but a dedicated
/// type lets future Packs add structured variants without breaking
/// callers.
#[derive(Debug)]
pub enum DigestError {
    /// Underlying LLM call failed; the digester *already* fell back to
    /// `Full` for the affected messages, so callers can usually treat
    /// this as a soft warning.
    LlmFallback(String),
}

impl std::fmt::Display for DigestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DigestError::LlmFallback(msg) => write!(f, "digest LLM fallback: {msg}"),
        }
    }
}

impl std::error::Error for DigestError {}

/// Async LLM-backed compression of a flat message stream.
///
/// Holds an `Arc<dyn UtilityLlm>` so it can be cheaply cloned into
/// `tokio::spawn` futures by the orchestrator without taking ownership
/// of the underlying provider client.
pub struct MessageDigester {
    llm: Arc<dyn UtilityLlm>,
    threshold_tokens: usize,
}

impl MessageDigester {
    /// Construct a digester with the default 500-token threshold.
    #[must_use]
    pub fn new(llm: Arc<dyn UtilityLlm>) -> Self {
        Self {
            llm,
            threshold_tokens: DEFAULT_DIGEST_THRESHOLD_TOKENS,
        }
    }

    /// Override the threshold (for tests + future per-tier tuning).
    #[must_use]
    pub fn with_threshold(mut self, threshold_tokens: usize) -> Self {
        self.threshold_tokens = threshold_tokens;
        self
    }

    /// Threshold currently in effect.
    pub fn threshold_tokens(&self) -> usize {
        self.threshold_tokens
    }

    /// Digest every message, returning a parallel `Vec<CompressedMessage>`
    /// of the same length and order. Each long message that fails to
    /// compress is preserved as `CompressedMessage::Full` (the contract
    /// is *never silently drop*).
    ///
    /// This method does N sequential LLM calls — one per long message.
    /// We deliberately avoid `JoinSet` parallelism here because the
    /// utility provider in production is rate-limited and the typical
    /// caller only ever has a handful of long messages per turn.
    pub async fn digest(&self, messages: &[InputMessage]) -> Vec<CompressedMessage> {
        let mut out = Vec::with_capacity(messages.len());
        for (idx, msg) in messages.iter().enumerate() {
            let tokens = estimate_message_tokens_for_digest(msg);
            if tokens <= self.threshold_tokens {
                out.push(CompressedMessage::Full {
                    original_index: idx,
                    message: msg.clone(),
                });
                continue;
            }
            match self.summarize_one(msg).await {
                Ok(summary_text) if !summary_text.trim().is_empty() => {
                    let summary_msg = build_summary_message(msg, &summary_text);
                    let summary_tokens = estimate_message_tokens_for_digest(&summary_msg);
                    if summary_tokens >= tokens {
                        // LLM "compressed" to something larger than the
                        // original — refuse and fall back so we never
                        // make things worse.
                        tracing::warn!(
                            original_tokens = tokens,
                            summary_tokens,
                            "[digester] summary not shorter than original; falling back to full"
                        );
                        out.push(CompressedMessage::Full {
                            original_index: idx,
                            message: msg.clone(),
                        });
                    } else {
                        out.push(CompressedMessage::Summarized {
                            original_index: idx,
                            summary: summary_msg,
                            original_tokens: tokens,
                            summary_tokens,
                        });
                    }
                }
                Ok(_) => {
                    tracing::warn!(
                        original_tokens = tokens,
                        "[digester] LLM returned empty summary; falling back to full"
                    );
                    out.push(CompressedMessage::Full {
                        original_index: idx,
                        message: msg.clone(),
                    });
                }
                Err(err) => {
                    tracing::warn!(
                        original_tokens = tokens,
                        error = %err,
                        "[digester] LLM error; falling back to full"
                    );
                    out.push(CompressedMessage::Full {
                        original_index: idx,
                        message: msg.clone(),
                    });
                }
            }
        }
        out
    }

    async fn summarize_one(&self, msg: &InputMessage) -> Result<String, DigestError> {
        let user_payload = render_message_for_summarization(msg);
        self.llm
            .complete(
                SUMMARY_SYSTEM_PROMPT,
                &user_payload,
                SUMMARY_MAX_TOKENS,
                SUMMARY_TEMPERATURE,
            )
            .await
            .map_err(|e| DigestError::LlmFallback(e.to_string()))
    }
}

/// Materialize a digest result back into a flat `Vec<InputMessage>`
/// suitable for the synchronous preflight stage.
pub fn apply_digest(digest: &[CompressedMessage]) -> Vec<InputMessage> {
    digest.iter().map(|d| d.message().clone()).collect()
}

/// Build the replacement message that a summary lives inside. We
/// preserve the original message's `role` so the conversation order
/// stays valid; the body becomes a single text block prefixed with
/// `[summary]` so downstream consumers (and humans tailing logs) can
/// tell at a glance that the content was compressed.
fn build_summary_message(original: &InputMessage, summary: &str) -> InputMessage {
    InputMessage {
        role: original.role.clone(),
        content: vec![InputContentBlock::Text {
            text: format!("[summary] {}", summary.trim()),
        }],
        thinking: None,
    }
}

/// Flatten a message into a single string the LLM can ingest. Tool
/// results are wrapped in lightweight markers so the LLM understands
/// the shape but doesn't need to parse JSON internals.
fn render_message_for_summarization(msg: &InputMessage) -> String {
    use crate::modules::api::ToolResultContentBlock;

    let mut buf = String::new();
    buf.push_str("ROLE: ");
    buf.push_str(&msg.role);
    buf.push('\n');
    if let Some(thinking) = msg.thinking.as_deref() {
        if !thinking.is_empty() {
            buf.push_str("THINKING: ");
            buf.push_str(thinking);
            buf.push('\n');
        }
    }
    for block in &msg.content {
        match block {
            InputContentBlock::Text { text } => {
                buf.push_str("TEXT: ");
                buf.push_str(text);
                buf.push('\n');
            }
            InputContentBlock::ToolUse { name, input, .. } => {
                buf.push_str("TOOL_USE: ");
                buf.push_str(name);
                buf.push(' ');
                buf.push_str(&input.to_string());
                buf.push('\n');
            }
            InputContentBlock::ToolResult { content, .. } => {
                buf.push_str("TOOL_RESULT: ");
                for tr in content {
                    match tr {
                        ToolResultContentBlock::Text { text } => buf.push_str(text),
                        ToolResultContentBlock::Json { value } => buf.push_str(&value.to_string()),
                        ToolResultContentBlock::Image { .. } => buf.push_str("[image]"),
                    }
                    buf.push('\n');
                }
            }
        }
    }
    buf
}

/// Token-estimate for digester decisions. Mirrors the algorithm used by
/// [`super::estimate_message_tokens`] but is duplicated here to avoid
/// exporting that helper publicly (it remains private to the parent
/// module on purpose — TE-001 internal heuristic).
fn estimate_message_tokens_for_digest(msg: &InputMessage) -> usize {
    use crate::modules::api::ToolResultContentBlock;

    let role_overhead = estimate_tokens(&msg.role).saturating_add(4);
    let mut total = role_overhead;
    if let Some(thinking) = msg.thinking.as_deref() {
        total = total.saturating_add(estimate_tokens(thinking));
    }
    for block in &msg.content {
        match block {
            InputContentBlock::Text { text } => {
                total = total.saturating_add(estimate_tokens(text));
            }
            InputContentBlock::ToolUse { name, input, .. } => {
                total = total.saturating_add(estimate_tokens(name));
                total = total.saturating_add(estimate_tokens(&input.to_string()));
            }
            InputContentBlock::ToolResult { content, .. } => {
                for tr in content {
                    match tr {
                        ToolResultContentBlock::Text { text } => {
                            total = total.saturating_add(estimate_tokens(text));
                        }
                        ToolResultContentBlock::Json { value } => {
                            total = total.saturating_add(estimate_tokens(&value.to_string()));
                        }
                        ToolResultContentBlock::Image { .. } => {
                            total = total.saturating_add(256);
                        }
                    }
                }
            }
        }
    }
    total
}
