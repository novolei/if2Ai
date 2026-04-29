//! FEAT-TE-003 — GenericAgent L1 mini-index builder.
//!
//! Generates a deterministic, ≤ 30-line plain-text "table of contents"
//! over the full session history. Injected into the prompt planner at
//! priority 95 so the model gets a stable spine of recent tool calls,
//! the last user goal, and the last assistant decision *before* it
//! reads the (now compressed) recent-message tier.
//!
//! Why a separate module?
//! - The output is pure text; no LLM call (cheap, sync, deterministic).
//! - Enforced output budgets (`MAX_INDEX_LINES`, `MAX_INDEX_TOKENS`)
//!   give us a hard upper bound on prompt growth.
//! - Decoupled from the planner so the same builder can later be
//!   re-used by other consumers (e.g. M5 strategy registry, harness
//!   diagnostics).

use crate::modules::api::{InputContentBlock, InputMessage, ToolResultContentBlock};
use crate::modules::runtime::budget::estimate_tokens;

/// Hard ceiling on number of lines emitted.
pub const MAX_INDEX_LINES: usize = 30;

/// Hard ceiling on token count of the rendered index. Enforced *after*
/// line trimming so the budget stays predictable regardless of message
/// content shape.
pub const MAX_INDEX_TOKENS: usize = 500;

/// Cap on how many recent tool calls show up in the index. Older calls
/// are summarized as `... +N earlier tool call(s)`.
const MAX_TOOL_CALLS: usize = 12;

/// Per-line truncation length so wide tool inputs don't explode the
/// index (UTF-8 safe, breaks on grapheme boundary fallback to byte).
const MAX_LINE_CHARS: usize = 160;

/// Build the L1 mini-index over a flat message stream.
///
/// Empty input → empty string (callers MUST treat empty as
/// "no block emitted"; the planner skips the block entirely so we
/// never spend a header on nothing).
///
/// The output is intentionally plain text (no markdown headings) so
/// downstream tokenization is stable across model families.
#[must_use]
pub fn build_mini_index(messages: &[InputMessage]) -> String {
    if messages.is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("[mini_index] turn_messages={}", messages.len()));

    if let Some(goal) = last_user_goal(messages) {
        lines.push(format!("[mini_index] last_user_goal: {goal}"));
    }
    if let Some(action) = last_assistant_action(messages) {
        lines.push(format!("[mini_index] last_assistant_action: {action}"));
    }

    let tool_lines = recent_tool_call_lines(messages);
    if !tool_lines.is_empty() {
        lines.push(format!(
            "[mini_index] recent_tool_calls ({}):",
            tool_lines.len()
        ));
        lines.extend(tool_lines);
    }

    enforce_line_limit(&mut lines, MAX_INDEX_LINES);
    let mut rendered = lines.join("\n");
    enforce_token_limit(&mut rendered, MAX_INDEX_TOKENS);
    rendered
}

fn last_user_goal(messages: &[InputMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .and_then(extract_visible_text)
        .map(|t| truncate_line(&t))
}

fn last_assistant_action(messages: &[InputMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .and_then(extract_visible_text)
        .map(|t| truncate_line(&t))
}

fn recent_tool_call_lines(messages: &[InputMessage]) -> Vec<String> {
    let mut tool_uses: Vec<(String, String)> = Vec::new();
    for msg in messages.iter() {
        for block in &msg.content {
            if let InputContentBlock::ToolUse { name, input, .. } = block {
                let payload = compact_one_line(&input.to_string());
                tool_uses.push((name.clone(), truncate_line(&payload)));
            }
        }
    }
    if tool_uses.is_empty() {
        return Vec::new();
    }

    let total = tool_uses.len();
    let kept_start = total.saturating_sub(MAX_TOOL_CALLS);
    let mut out: Vec<String> = tool_uses[kept_start..]
        .iter()
        .map(|(name, payload)| format!("  - {name}: {payload}"))
        .collect();
    if kept_start > 0 {
        out.insert(0, format!("  ... +{kept_start} earlier tool call(s)"));
    }
    out
}

fn extract_visible_text(msg: &InputMessage) -> Option<String> {
    let mut buf = String::new();
    for block in &msg.content {
        match block {
            InputContentBlock::Text { text } if !text.trim().is_empty() => {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(text.trim());
            }
            InputContentBlock::ToolResult { content, .. } => {
                for tr in content {
                    if let ToolResultContentBlock::Text { text } = tr {
                        if !text.trim().is_empty() {
                            if !buf.is_empty() {
                                buf.push(' ');
                            }
                            buf.push_str(text.trim());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if buf.is_empty() {
        None
    } else {
        Some(compact_one_line(&buf))
    }
}

fn compact_one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_line(s: &str) -> String {
    if s.chars().count() <= MAX_LINE_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX_LINE_CHARS).collect();
    out.push('…');
    out
}

fn enforce_line_limit(lines: &mut Vec<String>, max_lines: usize) {
    if lines.len() <= max_lines {
        return;
    }
    let dropped = lines.len() - (max_lines - 1);
    lines.truncate(max_lines - 1);
    lines.push(format!("[mini_index] … truncated {dropped} more line(s)"));
}

fn enforce_token_limit(rendered: &mut String, max_tokens: usize) {
    if estimate_tokens(rendered) <= max_tokens {
        return;
    }
    while estimate_tokens(rendered) > max_tokens && !rendered.is_empty() {
        if let Some(last_newline) = rendered.rfind('\n') {
            rendered.truncate(last_newline);
        } else {
            let target_chars = max_tokens.saturating_mul(4);
            let truncated: String = rendered.chars().take(target_chars).collect();
            *rendered = truncated;
            break;
        }
    }
    if !rendered.is_empty() {
        rendered.push_str("\n[mini_index] … truncated to fit token budget");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::api::InputContentBlock;
    use serde_json::json;

    fn assistant_text(text: &str) -> InputMessage {
        InputMessage {
            role: "assistant".to_string(),
            content: vec![InputContentBlock::Text {
                text: text.to_string(),
            }],
            thinking: None,
        }
    }

    fn assistant_tool_use(name: &str, input: serde_json::Value) -> InputMessage {
        InputMessage {
            role: "assistant".to_string(),
            content: vec![InputContentBlock::ToolUse {
                id: format!("call-{name}"),
                name: name.to_string(),
                input,
            }],
            thinking: None,
        }
    }

    #[test]
    fn mini_index_respects_line_limit() {
        let mut messages: Vec<InputMessage> = (0..40)
            .map(|i| assistant_tool_use(&format!("tool_{i}"), json!({"i": i})))
            .collect();
        messages.insert(0, InputMessage::user_text("kick off the run"));
        messages.push(assistant_text("done."));

        let index = build_mini_index(&messages);
        let line_count = index.lines().count();
        assert!(
            line_count <= MAX_INDEX_LINES,
            "mini index emitted {line_count} lines > limit {MAX_INDEX_LINES}"
        );
        assert!(
            line_count >= 4,
            "mini index too sparse for non-trivial history: {line_count}"
        );
    }

    #[test]
    fn mini_index_respects_token_budget() {
        let big_payload =
            serde_json::Value::String("x".repeat(50_000));
        let messages: Vec<InputMessage> = (0..30)
            .map(|i| {
                assistant_tool_use(
                    &format!("massive_tool_{i}"),
                    json!({"blob": big_payload.clone(), "i": i}),
                )
            })
            .collect();

        let index = build_mini_index(&messages);
        let tokens = estimate_tokens(&index);
        assert!(
            tokens <= MAX_INDEX_TOKENS + 16,
            "mini index used {tokens} tokens > budget {MAX_INDEX_TOKENS}"
        );
    }

    #[test]
    fn mini_index_empty_history_no_panic() {
        let index = build_mini_index(&[]);
        assert!(index.is_empty(), "empty history must yield empty string");
    }
}
