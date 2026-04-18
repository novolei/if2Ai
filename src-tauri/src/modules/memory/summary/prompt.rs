//! Rolling-summary prompt builder + token budget computation.
//!
//! Phase 8A.6 / Sprint 1 T-B2.  Given a slice of new conversation
//! messages (and an optional prior summary), produce the system + user
//! prompts that the rolling summarizer (8A.7) feeds to UtilityLlm.
//!
//! Budget formula (mirrors openhanako session-summary.js
//! `_callRollingLLM`):
//!
//! ```text
//!   total_budget   = clamp(turn_count * 40, 40, 400) chars
//!   facts_budget   = round(total_budget * 0.3)
//!   events_budget  = total_budget - facts_budget
//!   max_tokens     = clamp(round(total_budget * 1.5), 150, 750) as u32
//! ```
//!
//! For English, the char budgets above are interpreted as approximate
//! word budgets via `total_budget * 0.6` — same multiplier openhanako
//! uses.  Both languages produce a fixed two-section output:
//!     `## 重要事实` / `## Key facts`
//!     `## 事情经过` / `## What happened`
//!
//! See `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-B2 + §0.5 Δ-13.

#![allow(dead_code)] // first production consumer lands in 8A.7 RollingSummarizer

use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};

/// Maximum chars retained per assistant turn before truncating with the
/// "（长回复已截断）" marker.  Mirrors openhanako's `ASSISTANT_CAP`.
pub const ASSISTANT_CAP: usize = 300;

/// Per-turn budget multiplier (chars / turn) before clamping.
pub const PER_TURN_BUDGET_CHARS: usize = 40;

/// Lower bound on total summary budget (chars).
pub const MIN_TOTAL_BUDGET: usize = 40;

/// Upper bound on total summary budget (chars).
pub const MAX_TOTAL_BUDGET: usize = 400;

/// Lower bound on `max_tokens` requested from the LLM.
pub const MIN_MAX_TOKENS: u32 = 150;

/// Upper bound on `max_tokens` requested from the LLM.
pub const MAX_MAX_TOKENS: u32 = 750;

/// Char-to-token-budget multiplier (1.5×).  Approximates: a 400-char
/// Chinese summary needs ~600 LLM tokens of headroom.
pub const TOKENS_PER_CHAR_MULTIPLIER: f32 = 1.5;

/// English approximation: 1 word ≈ 0.6 chars-equivalent for the same
/// information density (= openhanako's `WORD_TO_CHAR_RATIO`).
pub const ENGLISH_WORD_TO_CHAR_RATIO: f32 = 0.6;

/// Output of [`build_rolling_summary_prompt`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollingSummaryPrompt {
    /// System message — instructions + locale-aware section headers.
    pub system: String,
    /// User message — either pure new conversation or a
    /// `## 已有摘要 … ## 新增对话 …` composite when `has_prev=true`.
    pub user: String,
    /// `max_tokens` to pass to the LLM call (already clamped).
    pub max_tokens: u32,
}

/// Computed budgets for one rolling summary call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryBudget {
    /// Total chars (Chinese) or word-equivalents (English) the summary may use.
    pub total_chars: usize,
    /// Subset spent on the `## 重要事实` / `## Key facts` section (~30%).
    pub facts_chars: usize,
    /// Subset spent on the `## 事情经过` / `## What happened` section.
    pub events_chars: usize,
    /// Final clamped `max_tokens` value sent to the LLM.
    pub max_tokens: u32,
}

/// Compute the per-call summary budget from `turn_count`.
#[must_use]
pub fn compute_budget(turn_count: usize) -> SummaryBudget {
    let total_chars = turn_count
        .saturating_mul(PER_TURN_BUDGET_CHARS)
        .clamp(MIN_TOTAL_BUDGET, MAX_TOTAL_BUDGET);
    let facts_chars = ((total_chars as f32) * 0.3).round() as usize;
    let events_chars = total_chars.saturating_sub(facts_chars);
    let raw_max_tokens = ((total_chars as f32) * TOKENS_PER_CHAR_MULTIPLIER).round() as u32;
    let max_tokens = raw_max_tokens.clamp(MIN_MAX_TOKENS, MAX_MAX_TOKENS);
    SummaryBudget {
        total_chars,
        facts_chars,
        events_chars,
        max_tokens,
    }
}

/// Build the LLM prompt for one rolling summary call.
///
/// `is_zh` toggles between Chinese (default) and English templates;
/// callers should source it from [`crate::modules::runtime::locale::is_zh`].
///
/// `has_prev = true` triggers the composite user prompt that asks the
/// model to merge `prev_summary` with the new turns; `false` (first
/// call) sends the conversation alone.
#[must_use]
pub fn build_rolling_summary_prompt(
    is_zh: bool,
    has_prev: bool,
    prev_summary: &str,
    new_conversation: &str,
    turn_count: usize,
) -> RollingSummaryPrompt {
    let budget = compute_budget(turn_count);
    let system = render_system_prompt(is_zh, &budget);
    let user = render_user_prompt(is_zh, has_prev, prev_summary, new_conversation);
    RollingSummaryPrompt {
        system,
        user,
        max_tokens: budget.max_tokens,
    }
}

fn render_system_prompt(is_zh: bool, budget: &SummaryBudget) -> String {
    if is_zh {
        format!(
            "你是会话摘要助手。请根据以下对话生成结构化摘要，输出固定两节：\n\
             \n\
             ## 重要事实\n\
             - 列出本轮对话中明确陈述的关键事实、决策、用户偏好（约 {facts} 字以内）。\n\
             - 一行一个条目，避免重复，避免猜测。\n\
             \n\
             ## 事情经过\n\
             - 时间顺序简述本轮对话发生了什么（约 {events} 字以内）。\n\
             - 仅记录事件本身，不做评价。\n\
             \n\
             总长度严格 ≤ {total} 字。直接输出 markdown，不要寒暄、不要解释。",
            facts = budget.facts_chars,
            events = budget.events_chars,
            total = budget.total_chars,
        )
    } else {
        let facts_words =
            ((budget.facts_chars as f32) * ENGLISH_WORD_TO_CHAR_RATIO).round() as usize;
        let events_words =
            ((budget.events_chars as f32) * ENGLISH_WORD_TO_CHAR_RATIO).round() as usize;
        let total_words =
            ((budget.total_chars as f32) * ENGLISH_WORD_TO_CHAR_RATIO).round() as usize;
        format!(
            "You are a conversation summarizer. Produce a structured summary in two sections:\n\
             \n\
             ## Key facts\n\
             - List the explicit facts, decisions, and user preferences from this turn (≤ {facts_words} words).\n\
             - One item per line; no duplicates, no speculation.\n\
             \n\
             ## What happened\n\
             - Chronological recap of what occurred (≤ {events_words} words).\n\
             - Events only, no commentary.\n\
             \n\
             Total length must stay ≤ {total_words} words. Output markdown directly; no greetings, no explanation."
        )
    }
}

fn render_user_prompt(
    is_zh: bool,
    has_prev: bool,
    prev_summary: &str,
    new_conversation: &str,
) -> String {
    if has_prev {
        if is_zh {
            format!(
                "## 已有摘要\n\n{}\n\n## 新增对话\n\n{}",
                prev_summary.trim(),
                new_conversation.trim()
            )
        } else {
            format!(
                "## Previous summary\n\n{}\n\n## New conversation\n\n{}",
                prev_summary.trim(),
                new_conversation.trim()
            )
        }
    } else {
        new_conversation.trim().to_string()
    }
}

/// Format a slice of [`ConversationMessage`] into the plain-text block
/// fed to the rolling summarizer.  Output shape mirrors openhanako's
/// `_buildConversationText`:
///
/// ```text
/// 【用户】<text>
///
/// 【助手】<text or "…（长回复已截断）">
/// ```
///
/// - Skips [`MessageRole::System`] and [`MessageRole::Tool`] messages
///   (only user/assistant turns belong in a *conversation* summary).
/// - Concatenates [`ContentBlock::Text`] blocks; ignores
///   [`ContentBlock::ToolUse`] / [`ContentBlock::ToolResult`] blocks
///   (they are tool-loop noise, not conversation).
/// - Truncates assistant turns longer than [`ASSISTANT_CAP`] *Unicode
///   chars* (NOT bytes — Chinese summaries would mis-count otherwise)
///   and appends `…（长回复已截断）` / `… (long reply truncated)`.
/// - Inter-message separator: `"\n\n"`.
///
/// `ConversationMessage` does NOT carry a wall-clock timestamp in this
/// codebase, so the openhanako `[HH:MM]` time prefix is intentionally
/// dropped (recorded as a known divergence in the slice review notes).
#[must_use]
pub fn build_conversation_text(messages: &[ConversationMessage]) -> String {
    let is_zh = crate::modules::runtime::locale::is_zh();
    messages
        .iter()
        .filter_map(|m| format_one(m, is_zh))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn format_one(message: &ConversationMessage, is_zh: bool) -> Option<String> {
    let speaker = match message.role {
        MessageRole::User => {
            if is_zh {
                "用户"
            } else {
                "User"
            }
        }
        MessageRole::Assistant => {
            if is_zh {
                "助手"
            } else {
                "Assistant"
            }
        }
        MessageRole::System | MessageRole::Tool => return None,
    };
    let body = extract_text_blocks(&message.blocks);
    if body.trim().is_empty() {
        return None;
    }
    let body = if matches!(message.role, MessageRole::Assistant) {
        truncate_chars(&body, ASSISTANT_CAP, is_zh)
    } else {
        body
    };
    Some(format!("【{speaker}】{body}"))
}

fn extract_text_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_chars(text: &str, cap: usize, is_zh: bool) -> String {
    let count = text.chars().count();
    if count <= cap {
        return text.to_string();
    }
    let head: String = text.chars().take(cap).collect();
    if is_zh {
        format!("{head}…（长回复已截断）")
    } else {
        format!("{head}… (long reply truncated)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};

    fn msg(role: MessageRole, text: &str) -> ConversationMessage {
        ConversationMessage {
            role,
            blocks: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            usage: None,
            thinking: None,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            request_id: None,
        }
    }

    fn msg_with_blocks(role: MessageRole, blocks: Vec<ContentBlock>) -> ConversationMessage {
        ConversationMessage {
            role,
            blocks,
            usage: None,
            thinking: None,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            request_id: None,
        }
    }

    // ─── compute_budget ────────────────────────────────────────────

    #[test]
    fn compute_budget_1_turn() {
        // turn=1 → raw=40 → clamp lower bound (still 40)
        // facts = round(40 * 0.3) = 12
        // events = 40 - 12 = 28
        // raw_max_tokens = round(40 * 1.5) = 60 → clamp to 150
        let b = compute_budget(1);
        assert_eq!(
            b,
            SummaryBudget {
                total_chars: 40,
                facts_chars: 12,
                events_chars: 28,
                max_tokens: 150,
            }
        );
    }

    #[test]
    fn compute_budget_10_turns() {
        // turn=10 → raw=400 → 400 (no clamp)
        // facts = round(400 * 0.3) = 120
        // events = 400 - 120 = 280
        // max_tokens = round(400 * 1.5) = 600
        let b = compute_budget(10);
        assert_eq!(
            b,
            SummaryBudget {
                total_chars: 400,
                facts_chars: 120,
                events_chars: 280,
                max_tokens: 600,
            }
        );
    }

    #[test]
    fn compute_budget_clamps_zero_turn() {
        let b = compute_budget(0);
        assert_eq!(b.total_chars, MIN_TOTAL_BUDGET);
        assert_eq!(b.max_tokens, MIN_MAX_TOKENS);
    }

    #[test]
    fn compute_budget_clamps_huge_turn() {
        let b = compute_budget(100);
        assert_eq!(b.total_chars, MAX_TOTAL_BUDGET);
    }

    #[test]
    fn compute_budget_max_tokens_clamp_upper() {
        // total=400 → tokens=600 (still under 750 upper clamp)
        // verify the upper clamp triggers: synthetic via clamp invariant
        let b = compute_budget(usize::MAX);
        assert!(b.max_tokens <= MAX_MAX_TOKENS);
        assert_eq!(b.total_chars, MAX_TOTAL_BUDGET);
    }

    // ─── system prompts ────────────────────────────────────────────

    #[test]
    fn system_prompt_zh_has_required_sections() {
        let p = build_rolling_summary_prompt(true, false, "", "【用户】hi", 5);
        assert!(p.system.contains("## 重要事实"));
        assert!(p.system.contains("## 事情经过"));
    }

    #[test]
    fn system_prompt_en_has_required_sections() {
        let p = build_rolling_summary_prompt(false, false, "", "User: hi", 5);
        assert!(p.system.contains("## Key facts"));
        assert!(p.system.contains("## What happened"));
    }

    #[test]
    fn system_prompt_embeds_budget() {
        let p = build_rolling_summary_prompt(true, false, "", "x", 10);
        // 10 turns → total=400
        assert!(
            p.system.contains("400 字"),
            "expected '400 字' in: {}",
            p.system
        );
    }

    // ─── user prompts ──────────────────────────────────────────────

    #[test]
    fn user_prompt_with_prev_combines_both_zh() {
        let p = build_rolling_summary_prompt(true, true, "old summary", "new conv", 5);
        assert!(p.user.contains("## 已有摘要"));
        assert!(p.user.contains("## 新增对话"));
        assert!(p.user.contains("old summary"));
        assert!(p.user.contains("new conv"));
    }

    #[test]
    fn user_prompt_with_prev_combines_both_en() {
        let p = build_rolling_summary_prompt(false, true, "old summary", "new conv", 5);
        assert!(p.user.contains("## Previous summary"));
        assert!(p.user.contains("## New conversation"));
    }

    #[test]
    fn user_prompt_without_prev_returns_conversation_only() {
        let p = build_rolling_summary_prompt(true, false, "ignored", "  conv body  ", 5);
        assert_eq!(p.user, "conv body");
    }

    // ─── build_conversation_text ───────────────────────────────────

    #[test]
    fn build_conversation_text_skips_system_and_tool() {
        let msgs = vec![
            msg(MessageRole::System, "system instr"),
            msg(MessageRole::User, "hello"),
            msg(MessageRole::Tool, "tool result"),
            msg(MessageRole::Assistant, "hi back"),
        ];
        let out = build_conversation_text(&msgs);
        assert!(!out.contains("system instr"));
        assert!(!out.contains("tool result"));
        assert!(out.contains("hello"));
        assert!(out.contains("hi back"));
    }

    #[test]
    fn build_conversation_text_truncates_long_assistant() {
        let long: String = "中".repeat(500);
        let msgs = vec![msg(MessageRole::Assistant, &long)];
        let out = build_conversation_text(&msgs);
        // truncation marker present (zh or en — depends on locale env)
        let truncated = out.contains("（长回复已截断）") || out.contains("(long reply truncated)");
        assert!(truncated, "expected truncation marker in: {out}");
        // body length ≤ ASSISTANT_CAP + marker
        let body_chars = out.chars().count();
        assert!(body_chars < 500 + 30);
    }

    #[test]
    fn build_conversation_text_does_not_truncate_short_user() {
        let long: String = "x".repeat(500);
        let msgs = vec![msg(MessageRole::User, &long)];
        let out = build_conversation_text(&msgs);
        assert!(!out.contains("长回复已截断"));
        assert!(!out.contains("long reply truncated"));
    }

    #[test]
    fn build_conversation_text_skips_tool_use_blocks() {
        let blocks = vec![
            ContentBlock::Text {
                text: "spoken".to_string(),
            },
            ContentBlock::ToolUse {
                id: "t1".to_string(),
                name: "search".to_string(),
                input: "{}".to_string(),
            },
            ContentBlock::ToolResult {
                tool_use_id: "t1".to_string(),
                tool_name: "search".to_string(),
                output: "result".to_string(),
                is_error: false,
            },
        ];
        let msgs = vec![msg_with_blocks(MessageRole::Assistant, blocks)];
        let out = build_conversation_text(&msgs);
        assert!(out.contains("spoken"));
        assert!(!out.contains("search"));
        assert!(!out.contains("result"));
    }

    #[test]
    fn build_conversation_text_empty_message_skipped() {
        let msgs = vec![
            msg(MessageRole::User, "   "),
            msg(MessageRole::Assistant, "real reply"),
        ];
        let out = build_conversation_text(&msgs);
        assert!(out.contains("real reply"));
        // Only one segment → no separator
        assert!(!out.contains("\n\n"));
    }

    #[test]
    fn build_conversation_text_separator_is_double_newline() {
        let msgs = vec![
            msg(MessageRole::User, "a"),
            msg(MessageRole::Assistant, "b"),
            msg(MessageRole::User, "c"),
        ];
        let out = build_conversation_text(&msgs);
        assert_eq!(out.matches("\n\n").count(), 2);
    }
}
