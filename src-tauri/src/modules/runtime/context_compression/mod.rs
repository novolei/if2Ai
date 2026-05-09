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

/// Default character threshold for tag content truncation.
/// Content blocks inside `<thinking>` / `<tool_result>` tags longer than this
/// will be truncated to `max_chars / 2` head + tail with a `[Truncated N chars]`
/// placeholder in between.
const DEFAULT_TAG_TRUNCATE_THRESHOLD: usize = 800;

/// Context pressure level derived from actual-vs-budget token usage.
///
/// Drives how aggressively the compression pipeline operates:
/// - **Normal** — token usage < 80 % of budget → message-level compression only.
/// - **Warning** — 80–95 % → message-level + tag-level truncation.
/// - **Critical** — > 95 % → tag-level truncation + drop earliest messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPressure {
    /// Actual usage < 80 % of budget. Only standard message-level compression.
    Normal,
    /// 80–95 % of budget used. Tag-level truncation kicks in.
    Warning,
    /// > 95 % of budget used. Tag-level + earliest-message drop.
    Critical,
}

impl ContextPressure {
    /// Compute pressure from actual token usage vs. total budget.
    pub fn from_usage(used_tokens: usize, budget: usize) -> Self {
        if budget == 0 {
            return Self::Critical;
        }
        let ratio = used_tokens as f64 / budget as f64;
        if ratio > 0.95 {
            Self::Critical
        } else if ratio > 0.80 {
            Self::Warning
        } else {
            Self::Normal
        }
    }
}

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

/// Find the largest valid UTF-8 char boundary at or before `byte_pos`.
///
/// Returns `s.len()` when `byte_pos >= s.len()`, and walks backwards
/// from `byte_pos` until `is_char_boundary` holds so that multi-byte
/// characters (CJK, emoji, etc.) are never split mid-sequence.
fn safe_byte_boundary(s: &str, byte_pos: usize) -> usize {
    if byte_pos >= s.len() {
        return s.len();
    }
    let mut pos = byte_pos;
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Truncate the content of known verbose XML tags (`<thinking>`, `<tool_result>`).
///
/// For each matched tag pair found in `text`, if the inner body exceeds
/// `max_chars` characters, keep the first and last `max_chars / 2` characters
/// and replace the middle with `[Truncated N chars]`.
///
/// This is a lightweight string-scan approach — no full XML parser required.
pub fn compress_tag_content(text: &str, max_chars: usize) -> String {
    const TAGS: &[(&str, &str)] = &[
        ("<thinking>", "</thinking>"),
        ("<tool_result>", "</tool_result>"),
    ];

    let mut result = text.to_string();
    for &(open, close) in TAGS {
        let mut output = String::with_capacity(result.len());
        let mut search_from = 0;
        while let Some(start) = result[search_from..].find(open) {
            let abs_start = search_from + start;
            let body_start = abs_start + open.len();
            if let Some(end_offset) = result[body_start..].find(close) {
                let body_end = body_start + end_offset;
                let body = &result[body_start..body_end];
                output.push_str(&result[search_from..body_start]);
                if body.len() > max_chars {
                    let half = max_chars / 2;
                    let truncated = body.len() - max_chars;
                    let head_end = safe_byte_boundary(body, half);
                    let tail_start = safe_byte_boundary(body, body.len().saturating_sub(half));
                    output.push_str(&body[..head_end]);
                    output.push_str(&format!("\n[Truncated {} chars]\n", truncated));
                    output.push_str(&body[tail_start..]);
                } else {
                    output.push_str(body);
                }
                output.push_str(close);
                search_from = body_end + close.len();
            } else {
                // No matching close tag — copy the rest verbatim.
                break;
            }
        }
        output.push_str(&result[search_from..]);
        result = output;
    }
    result
}

/// Apply tag-level truncation to a single [`InputMessage`], returning a
/// compressed clone. Only text content blocks and the `thinking` field are
/// scanned; binary / image blocks are left untouched.
fn compress_message_tags(msg: &InputMessage, max_chars: usize) -> InputMessage {
    use crate::modules::api::{InputContentBlock, ToolResultContentBlock};

    let thinking = msg
        .thinking
        .as_deref()
        .map(|t| compress_tag_content(t, max_chars));

    let content = msg
        .content
        .iter()
        .map(|block| match block {
            InputContentBlock::Text { text } => InputContentBlock::Text {
                text: compress_tag_content(text, max_chars),
            },
            InputContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let compressed: Vec<ToolResultContentBlock> = content
                    .iter()
                    .map(|tr| match tr {
                        ToolResultContentBlock::Text { text } => ToolResultContentBlock::Text {
                            text: compress_tag_content(text, max_chars),
                        },
                        other => other.clone(),
                    })
                    .collect();
                InputContentBlock::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: compressed,
                    is_error: *is_error,
                }
            }
            other => other.clone(),
        })
        .collect();

    InputMessage {
        role: msg.role.clone(),
        content,
        thinking,
    }
}

/// Apply the hard tier budget to a flat message stream.
///
/// Strategy (TE-001): keep the **most recent** messages whose cumulative
/// token estimate fits inside `allocation.total()`, dropping older ones.
/// This is the floor every later FEAT-TE-* Pack builds on; per-tier
/// digesters will replace the dropped slice with a compressed summary
/// rather than discarding it outright.
///
/// **Tag-level compression** (pressure-aware):
/// After the initial message-level pass, [`ContextPressure`] is computed
/// from the kept tokens vs. the budget. When pressure reaches `Warning`
/// or above, older messages (outside the `keep_recent` window) have their
/// `<thinking>` / `<tool_result>` tag bodies truncated via
/// [`compress_tag_content`], further reducing token usage by 30–40 %.
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

    // ── Phase 1: message-level drop (oldest first) ──────────────
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

    let mut kept: Vec<InputMessage> = messages[keep_from..].to_vec();
    let total_messages = messages.len();

    // ── Phase 2: tag-level compression (pressure-aware) ─────────
    let pressure = ContextPressure::from_usage(acc, budget);
    if pressure == ContextPressure::Warning || pressure == ContextPressure::Critical {
        // Number of recent messages to protect from tag truncation.
        let keep_recent = crate::modules::runtime::budget::stream_tool_keep_recent();
        let safe_boundary = kept.len().saturating_sub(keep_recent);

        let pre_tag_tokens = acc;
        for msg in kept[..safe_boundary].iter_mut() {
            *msg = compress_message_tags(msg, DEFAULT_TAG_TRUNCATE_THRESHOLD);
        }

        // Re-estimate after tag compression.
        acc = kept.iter().map(estimate_message_tokens).sum();

        tracing::info!(
            "[compress_for_request] tag-level compression: pressure={:?}, \
             pre_tokens={}, post_tokens={}, saved={}",
            pressure,
            pre_tag_tokens,
            acc,
            pre_tag_tokens.saturating_sub(acc),
        );
    }

    // ── Phase 3: critical — drop earliest kept messages ─────────
    if pressure == ContextPressure::Critical && acc > budget && kept.len() > 1 {
        while acc > budget && kept.len() > 1 {
            let front_tokens = estimate_message_tokens(&kept[0]);
            kept.remove(0);
            acc = acc.saturating_sub(front_tokens);
        }
        tracing::info!(
            "[compress_for_request] critical drop: final kept={}, tokens={}",
            kept.len(),
            acc,
        );
    }

    let dropped = total_messages - kept.len();
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

    // ── Tag-level compression tests ─────────────────────────────

    #[test]
    fn compress_tag_content_short_body_unchanged() {
        let input = "<thinking>short</thinking>";
        assert_eq!(compress_tag_content(input, 800), input);
    }

    #[test]
    fn compress_tag_content_long_body_truncated() {
        let body = "a".repeat(2000);
        let input = format!("<thinking>{}</thinking>", body);
        let result = compress_tag_content(&input, 800);
        assert!(result.contains("[Truncated"));
        assert!(result.contains("</thinking>"));
        assert!(result.len() < input.len());
    }

    #[test]
    fn compress_tag_content_multiple_tags() {
        let body = "b".repeat(1500);
        let input = format!(
            "prefix <thinking>{}</thinking> middle <tool_result>{}</tool_result> suffix",
            body, body
        );
        let result = compress_tag_content(&input, 800);
        assert!(result.contains("prefix"));
        assert!(result.contains("middle"));
        assert!(result.contains("suffix"));
        // Both tags should be truncated.
        let truncated_count = result.matches("[Truncated").count();
        assert_eq!(truncated_count, 2);
    }

    #[test]
    fn compress_tag_content_no_tags_unchanged() {
        let input = "plain text with no XML tags";
        assert_eq!(compress_tag_content(input, 800), input);
    }

    #[test]
    fn compress_tag_content_unclosed_tag_unchanged() {
        let input = "<thinking>no close tag here";
        assert_eq!(compress_tag_content(input, 800), input);
    }

    // ── ContextPressure tests ───────────────────────────────────

    #[test]
    fn context_pressure_normal() {
        assert_eq!(
            ContextPressure::from_usage(700, 1000),
            ContextPressure::Normal
        );
    }

    #[test]
    fn context_pressure_warning() {
        assert_eq!(
            ContextPressure::from_usage(850, 1000),
            ContextPressure::Warning
        );
    }

    #[test]
    fn context_pressure_critical() {
        assert_eq!(
            ContextPressure::from_usage(960, 1000),
            ContextPressure::Critical
        );
    }

    #[test]
    fn context_pressure_zero_budget_is_critical() {
        assert_eq!(ContextPressure::from_usage(0, 0), ContextPressure::Critical);
    }
}
