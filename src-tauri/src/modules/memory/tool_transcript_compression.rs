//! Compress `tool_use` arguments and `tool_result` outputs in OLD history
//! messages while keeping the most recent N turns full-fidelity.
//!
//! Purpose: dramatically reduce token cost in long sessions where each turn
//! carries dense tool_use/tool_result transcripts. Old turns become
//! reference-only summaries; recent turns retain full args + outputs for
//! active reasoning.
//!
//! ## Pairing safety
//!
//! Compression preserves the `id` field on `ToolUse` blocks AND the
//! `tool_use_id` field on the matching `ToolResult` blocks (and `tool_name`
//! / `is_error`). This is critical because downstream
//! `sanitize_messages_for_provider` drops orphan tool_use/tool_result pairs
//! whose ids don't match. We only summarize the *content* of those blocks,
//! never the ids that hold the chain together.
//!
//! Phase 6 T3 — old-message tool transcript compression.
#![allow(dead_code)]

use crate::modules::runtime::session::{ContentBlock, ConversationMessage, MessageRole};

/// Maximum chars retained from a `ToolUse.input` string before truncating.
pub const COMPRESSED_ARGS_MAX_CHARS: usize = 80;
/// Maximum chars retained from a `ToolResult.output` string before truncating.
pub const COMPRESSED_RESULT_MAX_CHARS: usize = 120;

/// Compress old-history tool_use args + tool_result outputs.
///
/// Turns are anchored at `MessageRole::User` messages (matching
/// [`crate::modules::memory::working_memory::window_recent_turns`]). The cut
/// point is the (`keep_recent_turns`)th-from-end user message; messages AT
/// and AFTER the cut stay full-fidelity, messages BEFORE are compressed.
///
/// Special cases:
/// - `keep_recent_turns == 0` → compress everything (most aggressive).
/// - `keep_recent_turns >= number_of_user_turns` → passthrough (nothing old
///   enough to compress).
/// - empty input → empty output.
#[must_use]
pub fn compress_old_tool_transcripts(
    messages: Vec<ConversationMessage>,
    keep_recent_turns: usize,
) -> Vec<ConversationMessage> {
    if messages.is_empty() {
        return messages;
    }

    let user_indexes: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| matches!(m.role, MessageRole::User))
        .map(|(i, _)| i)
        .collect();

    if keep_recent_turns >= user_indexes.len() && keep_recent_turns != 0 {
        return messages;
    }

    let cut_at = if keep_recent_turns == 0 {
        messages.len()
    } else {
        user_indexes[user_indexes.len() - keep_recent_turns]
    };

    messages
        .into_iter()
        .enumerate()
        .map(|(idx, mut msg)| {
            if idx >= cut_at {
                return msg;
            }
            msg.blocks = msg.blocks.iter().map(compress_block).collect();
            msg
        })
        .collect()
}

/// Compress a single content block.
///
/// - `Text` passes through unchanged.
/// - `ToolUse.input` is truncated to [`COMPRESSED_ARGS_MAX_CHARS`] chars
///   with a "[args truncated, original N chars]" suffix.  `id` and `name`
///   are preserved verbatim — `id` is the pairing key.
/// - `ToolResult.output` is truncated to [`COMPRESSED_RESULT_MAX_CHARS`]
///   chars with a similar suffix. `tool_use_id`, `tool_name`, and
///   `is_error` are preserved verbatim — `tool_use_id` is the pairing key.
fn compress_block(block: &ContentBlock) -> ContentBlock {
    match block {
        ContentBlock::ToolUse { id, name, input } => {
            let input_chars = input.chars().count();
            let summary = if input_chars > COMPRESSED_ARGS_MAX_CHARS {
                let truncated: String = input.chars().take(COMPRESSED_ARGS_MAX_CHARS).collect();
                format!("{truncated} … [args truncated, original {input_chars} chars]")
            } else {
                input.clone()
            };
            ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: summary,
            }
        }
        ContentBlock::ToolResult {
            tool_use_id,
            tool_name,
            output,
            is_error,
        } => {
            let output_chars = output.chars().count();
            let summary = if output_chars > COMPRESSED_RESULT_MAX_CHARS {
                let truncated: String = output.chars().take(COMPRESSED_RESULT_MAX_CHARS).collect();
                format!("{truncated} … [result truncated, original {output_chars} chars]")
            } else {
                output.clone()
            };
            ContentBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
                tool_name: tool_name.clone(),
                output: summary,
                is_error: *is_error,
            }
        }
        ContentBlock::Text { .. } => block.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_msg(role: MessageRole, blocks: Vec<ContentBlock>) -> ConversationMessage {
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
            finish_reason: None,
        }
    }

    fn text_msg(role: MessageRole, text: &str) -> ConversationMessage {
        build_msg(
            role,
            vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        )
    }

    #[test]
    fn empty_input_returns_empty() {
        let out = compress_old_tool_transcripts(vec![], 3);
        assert!(out.is_empty());
    }

    #[test]
    fn passthrough_when_keep_recent_exceeds_user_count() {
        let msgs = vec![
            text_msg(MessageRole::User, "u1"),
            text_msg(MessageRole::Assistant, "a1"),
            text_msg(MessageRole::User, "u2"),
            text_msg(MessageRole::Assistant, "a2"),
        ];
        let out = compress_old_tool_transcripts(msgs.clone(), 8);
        assert_eq!(out, msgs);
    }

    #[test]
    fn compresses_old_tool_use_blocks_keeps_recent_full() {
        let long_input = "X".repeat(500);
        let long_output = "very long output ".repeat(20);

        let msgs = vec![
            text_msg(MessageRole::User, "u1"),
            build_msg(
                MessageRole::Assistant,
                vec![ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "bash".into(),
                    input: long_input.clone(),
                }],
            ),
            build_msg(
                MessageRole::Tool,
                vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    tool_name: "bash".into(),
                    output: long_output.clone(),
                    is_error: false,
                }],
            ),
            text_msg(MessageRole::Assistant, "ok"),
            text_msg(MessageRole::User, "u2"),
            build_msg(
                MessageRole::Assistant,
                vec![ContentBlock::ToolUse {
                    id: "call_2".into(),
                    name: "bash".into(),
                    input: "{\"command\":\"echo hello\"}".into(),
                }],
            ),
            build_msg(
                MessageRole::Tool,
                vec![ContentBlock::ToolResult {
                    tool_use_id: "call_2".into(),
                    tool_name: "bash".into(),
                    output: "hello".into(),
                    is_error: false,
                }],
            ),
        ];
        let out = compress_old_tool_transcripts(msgs, 1);

        match &out[1].blocks[0] {
            ContentBlock::ToolUse { id, input, name } => {
                assert_eq!(id, "call_1", "id MUST be preserved");
                assert_eq!(name, "bash");
                assert!(
                    input.contains("truncated"),
                    "old args should be compressed: {input}"
                );
                assert!(input.chars().count() < long_input.chars().count());
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }

        match &out[2].blocks[0] {
            ContentBlock::ToolResult {
                tool_use_id,
                output,
                tool_name,
                is_error,
            } => {
                assert_eq!(tool_use_id, "call_1", "tool_use_id MUST be preserved");
                assert_eq!(tool_name, "bash");
                assert!(!*is_error);
                assert!(output.contains("truncated"));
                assert!(output.chars().count() < long_output.chars().count());
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }

        match &out[5].blocks[0] {
            ContentBlock::ToolUse { id, input, .. } => {
                assert_eq!(id, "call_2");
                assert!(
                    !input.contains("truncated"),
                    "recent should NOT be compressed: {input}"
                );
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
        match &out[6].blocks[0] {
            ContentBlock::ToolResult {
                tool_use_id,
                output,
                ..
            } => {
                assert_eq!(tool_use_id, "call_2");
                assert_eq!(output, "hello");
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn preserves_tool_use_to_tool_result_pairing() {
        let msgs = vec![
            text_msg(MessageRole::User, "u1"),
            build_msg(
                MessageRole::Assistant,
                vec![ContentBlock::ToolUse {
                    id: "abc-123".into(),
                    name: "x".into(),
                    input: "long".repeat(40),
                }],
            ),
            build_msg(
                MessageRole::Tool,
                vec![ContentBlock::ToolResult {
                    tool_use_id: "abc-123".into(),
                    tool_name: "x".into(),
                    output: "long output".repeat(20),
                    is_error: false,
                }],
            ),
            text_msg(MessageRole::User, "u2"),
        ];
        let out = compress_old_tool_transcripts(msgs, 1);
        let use_id = match &out[1].blocks[0] {
            ContentBlock::ToolUse { id, .. } => Some(id.as_str()),
            _ => None,
        };
        let result_id = match &out[2].blocks[0] {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        };
        assert_eq!(use_id, Some("abc-123"));
        assert_eq!(result_id, Some("abc-123"));
    }

    #[test]
    fn keep_zero_compresses_all_tool_blocks() {
        let msgs = vec![
            text_msg(MessageRole::User, "u1"),
            build_msg(
                MessageRole::Assistant,
                vec![ContentBlock::ToolUse {
                    id: "k0".into(),
                    name: "x".into(),
                    input: "X".repeat(200),
                }],
            ),
        ];
        let out = compress_old_tool_transcripts(msgs, 0);
        assert_eq!(out.len(), 2);
        match &out[1].blocks[0] {
            ContentBlock::ToolUse { id, input, .. } => {
                assert_eq!(id, "k0");
                assert!(input.contains("truncated"), "expected truncation: {input}");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn short_inputs_pass_through_unchanged() {
        let msgs = vec![
            text_msg(MessageRole::User, "u1"),
            build_msg(
                MessageRole::Assistant,
                vec![ContentBlock::ToolUse {
                    id: "s1".into(),
                    name: "echo".into(),
                    input: "{\"x\":1}".into(),
                }],
            ),
            build_msg(
                MessageRole::Tool,
                vec![ContentBlock::ToolResult {
                    tool_use_id: "s1".into(),
                    tool_name: "echo".into(),
                    output: "ok".into(),
                    is_error: false,
                }],
            ),
            text_msg(MessageRole::User, "u2"),
            text_msg(MessageRole::User, "u3"),
        ];
        let out = compress_old_tool_transcripts(msgs.clone(), 1);
        assert_eq!(out[1], msgs[1]);
        assert_eq!(out[2], msgs[2]);
    }

    #[test]
    fn text_blocks_in_old_messages_pass_through() {
        let msgs = vec![
            text_msg(MessageRole::User, "old user text"),
            text_msg(MessageRole::Assistant, "old assistant text"),
            text_msg(MessageRole::User, "recent"),
        ];
        let out = compress_old_tool_transcripts(msgs.clone(), 1);
        assert_eq!(out, msgs);
    }
}
