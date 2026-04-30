//! Context compression — tier budget hard constraint enforcement.
//!
//! FEAT-TE-001: introduces the 5-tier budget model from
//! `.qoder/specs/if2ai-agent-evolution-report.md` §Module A and turns the
//! existing `MAX_REQUEST_TOKEN_BUDGET_ESTIMATE` "advisory" cap into a
//! *hard* constraint applied before [`crate::modules::application::
//! prompt_planner::ContextGovernor`] sees the message stream.
//!
//! Subsequent Packs (FEAT-TE-002 message digester, FEAT-TE-003 mini index,
//! FEAT-TE-004 tool result summary) layer real per-tier compression on top
//! of this hard-cap floor. For TE-001 the goal is *just*: never let a
//! single request exceed `TierBudgetAllocation::total()` worth of tokens,
//! regardless of how many messages the caller hands us.

#![allow(dead_code)]

pub mod digester;
pub mod mini_index;

#[allow(unused_imports)]
pub use digester::{
    apply_digest, CompressedMessage, DigestError, MessageDigester, DEFAULT_DIGEST_THRESHOLD_TOKENS,
};
#[allow(unused_imports)]
pub use mini_index::{build_mini_index, MAX_INDEX_LINES, MAX_INDEX_TOKENS};

use serde::{Deserialize, Serialize};

use crate::modules::api::InputMessage;
use crate::modules::runtime::budget::{
    estimate_tokens, TIER_COMPRESSED_HISTORY_TOKENS, TIER_RECENT_MESSAGES_TOKENS,
    TIER_SYSTEM_TOKENS, TIER_TOOL_BUFFER_TOKENS, TIER_WORKING_CHECKPOINT_TOKENS,
};

/// One of the 5 context tiers defined in Module A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextTier {
    /// Frozen identity / constitution / tool guides. Always injected.
    System,
    /// LLM-digested summary of older turns (populated by FEAT-TE-002).
    CompressedHistory,
    /// `<key_info>` checkpoint extracted by FEAT-DK-002.
    WorkingCheckpoint,
    /// Uncompressed last-N user/assistant/tool messages.
    RecentMessages,
    /// Current-turn tool results (largest variance, often dominant).
    ToolBuffer,
}

impl ContextTier {
    /// Human-readable label for diagnostics / events.
    pub fn as_str(self) -> &'static str {
        match self {
            ContextTier::System => "system",
            ContextTier::CompressedHistory => "compressed_history",
            ContextTier::WorkingCheckpoint => "working_checkpoint",
            ContextTier::RecentMessages => "recent_messages",
            ContextTier::ToolBuffer => "tool_buffer",
        }
    }
}

/// Per-tier token budget allocation. The sum of all tiers is the *hard*
/// per-request cap; downstream compression Packs MUST respect this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierBudgetAllocation {
    pub system: usize,
    pub compressed_history: usize,
    pub working_checkpoint: usize,
    pub recent_messages: usize,
    pub tool_buffer: usize,
}

impl TierBudgetAllocation {
    /// Default 5-tier split anchored on
    /// [`crate::modules::runtime::budget::MAX_REQUEST_TOKEN_BUDGET_ESTIMATE`].
    /// Defaults are tuned to land at ≤ 30K tokens (≈ 120K chars) total
    /// while leaving the lion's share to recent messages + tool buffer.
    pub const fn default_const() -> Self {
        Self {
            system: TIER_SYSTEM_TOKENS,
            compressed_history: TIER_COMPRESSED_HISTORY_TOKENS,
            working_checkpoint: TIER_WORKING_CHECKPOINT_TOKENS,
            recent_messages: TIER_RECENT_MESSAGES_TOKENS,
            tool_buffer: TIER_TOOL_BUFFER_TOKENS,
        }
    }

    /// Construct an allocation whose tiers are scaled proportionally
    /// from the historical 30K default so a different `total` retains
    /// the same recent-vs-system-vs-tool shape.
    ///
    /// Used by the streaming preflight in concert with
    /// [`crate::modules::runtime::budget::streaming_tier_budget`] so the
    /// runtime knob can be lowered (Phase 4 T1) without zero-ing any
    /// individual tier. Rounding error (≤ 4 tokens) is absorbed into
    /// `recent_messages` so the returned `total()` matches the requested
    /// value exactly.
    pub fn with_total(total: usize) -> Self {
        const DEFAULT_TOTAL: usize = TIER_SYSTEM_TOKENS
            + TIER_COMPRESSED_HISTORY_TOKENS
            + TIER_WORKING_CHECKPOINT_TOKENS
            + TIER_RECENT_MESSAGES_TOKENS
            + TIER_TOOL_BUFFER_TOKENS;

        let scale = |numer: usize| -> usize {
            ((numer as u128 * total as u128) / DEFAULT_TOTAL as u128) as usize
        };

        let system = scale(TIER_SYSTEM_TOKENS);
        let compressed_history = scale(TIER_COMPRESSED_HISTORY_TOKENS);
        let working_checkpoint = scale(TIER_WORKING_CHECKPOINT_TOKENS);
        let tool_buffer = scale(TIER_TOOL_BUFFER_TOKENS);
        let assigned = system + compressed_history + working_checkpoint + tool_buffer;
        let recent_messages = total.saturating_sub(assigned);

        Self {
            system,
            compressed_history,
            working_checkpoint,
            recent_messages,
            tool_buffer,
        }
    }

    /// Hard ceiling — sum of all tier budgets.
    pub const fn total(&self) -> usize {
        self.system
            + self.compressed_history
            + self.working_checkpoint
            + self.recent_messages
            + self.tool_buffer
    }

    /// Look up budget for a single tier (kept as a method so future
    /// per-tier compression Packs have one stable API).
    pub fn budget_for(&self, tier: ContextTier) -> usize {
        match tier {
            ContextTier::System => self.system,
            ContextTier::CompressedHistory => self.compressed_history,
            ContextTier::WorkingCheckpoint => self.working_checkpoint,
            ContextTier::RecentMessages => self.recent_messages,
            ContextTier::ToolBuffer => self.tool_buffer,
        }
    }
}

impl Default for TierBudgetAllocation {
    fn default() -> Self {
        Self::default_const()
    }
}

/// Outcome of one [`compress_for_request`] call. Statistics are surfaced
/// so the streaming preflight can attribute kept-tokens / dropped-msg
/// counts into its existing diagnostic projection.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompressionOutcome {
    /// Messages that survived the hard cap, oldest-first.
    pub kept: Vec<InputMessage>,
    /// Number of messages dropped from the head.
    pub dropped: usize,
    /// Total estimated tokens of `kept`.
    pub kept_tokens: usize,
    /// True when the input was returned verbatim (already under budget).
    pub passthrough: bool,
}

/// Apply the hard tier budget to a flat message stream.
///
/// Strategy (TE-001): keep the **most recent** messages whose cumulative
/// token estimate fits inside `allocation.total()`, dropping older ones.
/// This is the floor every later FEAT-TE-* Pack builds on; per-tier
/// digesters will replace the dropped slice with a compressed summary
/// rather than discarding it outright.
///
/// Edge cases:
/// - Empty input → returns an empty outcome with `passthrough = true`.
/// - All messages individually larger than budget → keeps the most recent
///   one anyway (so the model has *something* to react to) and reports
///   the overflow via [`CompressionOutcome::dropped`].
pub fn compress_for_request(
    messages: &[InputMessage],
    allocation: &TierBudgetAllocation,
) -> CompressionOutcome {
    if messages.is_empty() {
        return CompressionOutcome {
            kept: Vec::new(),
            dropped: 0,
            kept_tokens: 0,
            passthrough: true,
        };
    }

    let budget = allocation.total().max(1);

    let token_estimates: Vec<usize> = messages.iter().map(estimate_message_tokens).collect();
    let total_tokens: usize = token_estimates.iter().sum();

    if total_tokens <= budget {
        return CompressionOutcome {
            kept: messages.to_vec(),
            dropped: 0,
            kept_tokens: total_tokens,
            passthrough: true,
        };
    }

    let mut keep_from = messages.len();
    let mut acc = 0usize;
    for (idx, tokens) in token_estimates.iter().enumerate().rev() {
        let new_total = acc.saturating_add(*tokens);
        if new_total > budget {
            break;
        }
        acc = new_total;
        keep_from = idx;
    }

    if keep_from == messages.len() {
        keep_from = messages.len().saturating_sub(1);
        acc = token_estimates[keep_from];
    }

    let kept: Vec<InputMessage> = messages[keep_from..].to_vec();
    let dropped = keep_from;
    CompressionOutcome {
        kept,
        dropped,
        kept_tokens: acc,
        passthrough: false,
    }
}

fn estimate_message_tokens(msg: &InputMessage) -> usize {
    use crate::modules::api::{InputContentBlock, ToolResultContentBlock};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_budget_allocation_default_sums_to_total() {
        let a = TierBudgetAllocation::default();
        assert_eq!(
            a.total(),
            a.system
                + a.compressed_history
                + a.working_checkpoint
                + a.recent_messages
                + a.tool_buffer
        );
        assert!(
            a.total() <= 35_000,
            "default tier total should be near the 30K floor, got {}",
            a.total()
        );
        assert!(a.total() >= 25_000, "tier total too small: {}", a.total());
    }

    #[test]
    fn compress_for_request_respects_hard_limit() {
        let allocation = TierBudgetAllocation {
            system: 100,
            compressed_history: 100,
            working_checkpoint: 50,
            recent_messages: 200,
            tool_buffer: 50,
        };
        let big_text = "x".repeat(8_000);
        let messages = vec![
            InputMessage::user_text(big_text.clone()),
            InputMessage::user_text(big_text.clone()),
            InputMessage::user_text("recent short message"),
        ];
        let outcome = compress_for_request(&messages, &allocation);
        assert!(!outcome.passthrough);
        assert!(outcome.dropped >= 1);
        assert!(
            outcome.kept_tokens <= allocation.total() + 4,
            "kept_tokens {} must be ≤ budget {}",
            outcome.kept_tokens,
            allocation.total()
        );
        assert!(!outcome.kept.is_empty(), "must keep at least one message");
    }

    #[test]
    fn compress_graceful_fallback_on_empty() {
        let allocation = TierBudgetAllocation::default();
        let outcome = compress_for_request(&[], &allocation);
        assert!(outcome.passthrough);
        assert!(outcome.kept.is_empty());
        assert_eq!(outcome.dropped, 0);
        assert_eq!(outcome.kept_tokens, 0);
    }
}
