//! Per-request governor: artifact gate, token budget gate, char/message
//! budget gate. Owns `RequestPreflightStats`.
//!
//! Extracted from `commands/agent.rs` in GFR-002b (pure structural move,
//! function bodies byte-identical).

use crate::modules::api::{InputContentBlock, InputMessage};
use crate::modules::application::prompt_planner::preflight::{
    estimate_messages_char_count, estimate_messages_token_count, summarize_message_for_budget,
};
use crate::modules::runtime::block_conversion::summarize_tool_result_for_model;

#[derive(Debug, Default)]
pub(crate) struct RequestPreflightStats {
    pub(crate) before_messages: usize,
    pub(crate) after_messages: usize,
    pub(crate) before_chars: usize,
    pub(crate) after_chars: usize,
    pub(crate) dropped_messages: usize,
    pub(crate) trimmed_chars: usize,
}

impl RequestPreflightStats {
    pub(crate) fn has_changes(&self) -> bool {
        self.dropped_messages > 0 || self.trimmed_chars > 0
    }
}

#[derive(Debug, Default)]
// harness symbol marker: ContextGovernor\|token_budget_gate\|message_char_budget_gate\|artifact_gate
pub(crate) struct ContextGovernor;

impl ContextGovernor {
    pub(crate) fn admit(
        &self,
        messages: &[InputMessage],
        max_messages: usize,
        max_chars: usize,
        max_tokens: usize,
    ) -> (Vec<InputMessage>, RequestPreflightStats) {
        let (artifact_trimmed, artifact_stats) = self.artifact_gate(messages);
        let (token_trimmed, token_stats) = self.token_budget_gate(&artifact_trimmed, max_tokens);
        let (char_trimmed, mut preflight_stats) =
            self.message_char_budget_gate(&token_trimmed, max_messages, max_chars);
        preflight_stats.dropped_messages +=
            artifact_stats.dropped_messages + token_stats.dropped_messages;
        preflight_stats.trimmed_chars += artifact_stats.trimmed_chars + token_stats.trimmed_chars;
        (char_trimmed, preflight_stats)
    }

    fn token_budget_gate(
        &self,
        messages: &[InputMessage],
        max_tokens: usize,
    ) -> (Vec<InputMessage>, RequestPreflightStats) {
        let before_tokens = estimate_messages_token_count(messages);
        let mut trimmed = messages.to_vec();
        while trimmed.len() > 1 && estimate_messages_token_count(&trimmed) > max_tokens {
            trimmed.remove(0);
        }
        if trimmed.len() == 1 && estimate_messages_token_count(&trimmed) > max_tokens {
            trimmed[0] = summarize_message_for_budget(&trimmed[0], max_tokens.saturating_mul(4));
        }
        let after_tokens = estimate_messages_token_count(&trimmed);
        let after_messages = trimmed.len();
        (
            trimmed,
            RequestPreflightStats {
                before_messages: messages.len(),
                after_messages,
                before_chars: before_tokens.saturating_mul(4),
                after_chars: after_tokens.saturating_mul(4),
                dropped_messages: messages.len().saturating_sub(after_messages),
                trimmed_chars: before_tokens.saturating_sub(after_tokens).saturating_mul(4),
            },
        )
    }

    fn message_char_budget_gate(
        &self,
        messages: &[InputMessage],
        max_messages: usize,
        max_chars: usize,
    ) -> (Vec<InputMessage>, RequestPreflightStats) {
        apply_request_preflight_limits(messages, max_messages, max_chars)
    }

    fn artifact_gate(
        &self,
        messages: &[InputMessage],
    ) -> (Vec<InputMessage>, RequestPreflightStats) {
        let before_chars = estimate_messages_char_count(messages);
        let transformed = messages
            .iter()
            .map(|message| InputMessage {
                role: message.role.clone(),
                content: message
                    .content
                    .iter()
                    .map(|block| match block {
                        InputContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            let summarized = content
                                .iter()
                                .map(|part| match part {
                                    crate::modules::api::ToolResultContentBlock::Text { text } => {
                                        if text
                                            .trim_start()
                                            .starts_with("[tool_result_handle] tool=")
                                        {
                                            text.clone()
                                        } else {
                                            summarize_tool_result_for_model(
                                                "artifact_gate",
                                                tool_use_id,
                                                text,
                                                *is_error,
                                            )
                                        }
                                    }
                                    crate::modules::api::ToolResultContentBlock::Json { value } => {
                                        summarize_tool_result_for_model(
                                            "artifact_gate",
                                            tool_use_id,
                                            &value.to_string(),
                                            *is_error,
                                        )
                                    }
                                    crate::modules::api::ToolResultContentBlock::Image {
                                        source,
                                        alt,
                                    } => {
                                        let bytes = source.data.len();
                                        let caption = alt.as_deref().unwrap_or("image");
                                        format!(
                                            "[image: {} {bytes}B — {caption}]",
                                            source.media_type
                                        )
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            InputContentBlock::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                content: vec![crate::modules::api::ToolResultContentBlock::Text {
                                    text: summarized,
                                }],
                                is_error: *is_error,
                            }
                        }
                        _ => block.clone(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let after_chars = estimate_messages_char_count(&transformed);
        (
            transformed,
            RequestPreflightStats {
                before_messages: messages.len(),
                after_messages: messages.len(),
                before_chars,
                after_chars,
                dropped_messages: 0,
                trimmed_chars: before_chars.saturating_sub(after_chars),
            },
        )
    }
}

fn apply_request_preflight_limits(
    messages: &[InputMessage],
    max_messages: usize,
    max_chars: usize,
) -> (Vec<InputMessage>, RequestPreflightStats) {
    let before_chars = estimate_messages_char_count(messages);
    let mut trimmed: Vec<InputMessage> = if messages.len() > max_messages {
        messages[messages.len() - max_messages..].to_vec()
    } else {
        messages.to_vec()
    };

    while trimmed.len() > 1 && estimate_messages_char_count(&trimmed) > max_chars {
        trimmed.remove(0);
    }
    if trimmed.len() == 1 && estimate_messages_char_count(&trimmed) > max_chars {
        trimmed[0] = summarize_message_for_budget(&trimmed[0], max_chars);
    }

    let after_chars = estimate_messages_char_count(&trimmed);
    let stats = RequestPreflightStats {
        before_messages: messages.len(),
        after_messages: trimmed.len(),
        before_chars,
        after_chars,
        dropped_messages: messages.len().saturating_sub(trimmed.len()),
        trimmed_chars: before_chars.saturating_sub(after_chars),
    };
    (trimmed, stats)
}
