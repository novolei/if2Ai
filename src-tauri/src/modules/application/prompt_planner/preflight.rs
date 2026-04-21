//! Per-request preflight estimators (char/token budgeting helpers).
//!
//! Extracted from `commands/agent.rs` in GFR-002c (pure structural move,
//! function bodies byte-identical).

use crate::modules::api::{InputContentBlock, InputMessage};
use crate::modules::runtime::block_conversion::TOOL_RESULT_PREVIEW_CHARS;
use crate::modules::runtime::compact::estimate_token_count_from_chars;

pub(crate) fn estimate_messages_char_count(messages: &[InputMessage]) -> usize {
    messages
        .iter()
        .map(|message| {
            let role_chars = message.role.chars().count();
            let content_chars: usize = message
                .content
                .iter()
                .map(|block| match block {
                    InputContentBlock::Text { text } => text.chars().count(),
                    InputContentBlock::ToolUse { id, name, input } => {
                        id.chars().count()
                            + name.chars().count()
                            + input.to_string().chars().count()
                    }
                    InputContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => {
                        tool_use_id.chars().count()
                            + content
                                .iter()
                                .map(|part| match part {
                                    crate::modules::api::ToolResultContentBlock::Text { text } => {
                                        text.chars().count()
                                    }
                                    crate::modules::api::ToolResultContentBlock::Json { value } => {
                                        value.to_string().chars().count()
                                    }
                                    // Phase 7C, slice 7C.2 — base64 image bytes
                                    // do not contribute to char-budgets used by
                                    // the textual context summariser.
                                    crate::modules::api::ToolResultContentBlock::Image {
                                        alt,
                                        ..
                                    } => alt.as_deref().map(str::len).unwrap_or(0),
                                })
                                .sum::<usize>()
                    }
                })
                .sum();
            role_chars + content_chars
        })
        .sum()
}

pub(crate) fn estimate_messages_token_count(messages: &[InputMessage]) -> usize {
    estimate_token_count_from_chars(estimate_messages_char_count(messages)) + messages.len()
}

pub(crate) fn summarize_message_for_budget(
    message: &InputMessage,
    max_chars: usize,
) -> InputMessage {
    let mut parts: Vec<String> = Vec::new();
    for block in &message.content {
        match block {
            InputContentBlock::Text { text } => {
                parts.push(format!(
                    "text:{}",
                    truncate_middle_chars(text, TOOL_RESULT_PREVIEW_CHARS)
                ));
            }
            InputContentBlock::ToolUse { id, name, input } => {
                parts.push(format!(
                    "tool_use id={id} name={name} input={}",
                    truncate_middle_chars(&input.to_string(), TOOL_RESULT_PREVIEW_CHARS)
                ));
            }
            InputContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let content_preview = content
                    .iter()
                    .map(|part| match part {
                        crate::modules::api::ToolResultContentBlock::Text { text } => {
                            truncate_middle_chars(text, TOOL_RESULT_PREVIEW_CHARS)
                        }
                        crate::modules::api::ToolResultContentBlock::Json { value } => {
                            truncate_middle_chars(&value.to_string(), TOOL_RESULT_PREVIEW_CHARS)
                        }
                        // Phase 7C, slice 7C.2 — image parts collapse to a
                        // short placeholder so the budget summariser stays
                        // text-only.
                        crate::modules::api::ToolResultContentBlock::Image { source, alt } => {
                            let alt_part = alt.as_deref().unwrap_or("image");
                            format!(
                                "[image:{} {}B {}]",
                                source.media_type,
                                source.data.len(),
                                truncate_middle_chars(alt_part, 32)
                            )
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                parts.push(format!(
                    "tool_result id={tool_use_id} is_error={is_error} content={content_preview}"
                ));
            }
        }
    }
    let joined = parts.join("\n");
    let summary = format!(
        "[context_trim_notice] latest message compacted for request budget. role={} compacted_content=\n{}",
        message.role,
        truncate_middle_chars(&joined, max_chars.saturating_sub(96))
    );
    InputMessage::user_text(summary)
}

fn truncate_middle_chars(input: &str, max_chars: usize) -> String {
    let total = input.chars().count();
    if total <= max_chars || max_chars < 16 {
        return input.to_string();
    }
    let keep_head = max_chars / 2;
    let keep_tail = max_chars.saturating_sub(keep_head + 14);
    let head: String = input.chars().take(keep_head).collect();
    let tail: String = input
        .chars()
        .rev()
        .take(keep_tail)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}[...omitted...]{tail}")
}
