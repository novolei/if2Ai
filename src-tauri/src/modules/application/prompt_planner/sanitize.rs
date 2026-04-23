//! Sanitize input messages before sending to the provider.
//!
//! Extracted from `commands/agent.rs` in GFR-002a (pure structural move,
//! function bodies byte-identical).

use std::collections::HashSet;

use crate::modules::api::{InputContentBlock, InputMessage};

#[derive(Debug, Default)]
pub(crate) struct SanitizationStats {
    pub(crate) dropped_empty_messages: usize,
    pub(crate) dropped_orphan_tool_results: usize,
    pub(crate) dropped_unmatched_tool_uses: usize,
    pub(crate) dropped_invalid_tool_use_inputs: usize,
    pub(crate) orphan_tool_result_ids: Vec<String>,
    pub(crate) unmatched_tool_use_ids: Vec<String>,
    pub(crate) invalid_tool_use_input_ids: Vec<String>,
}

impl SanitizationStats {
    pub(crate) fn has_changes(&self) -> bool {
        self.dropped_empty_messages > 0
            || self.dropped_orphan_tool_results > 0
            || self.dropped_unmatched_tool_uses > 0
            || self.dropped_invalid_tool_use_inputs > 0
    }

    fn push_orphan_tool_result_id(&mut self, tool_use_id: &str) {
        if self.orphan_tool_result_ids.len() < 8 {
            self.orphan_tool_result_ids.push(tool_use_id.to_string());
        }
    }

    fn push_unmatched_tool_use_id(&mut self, tool_use_id: &str) {
        if self.unmatched_tool_use_ids.len() < 8 {
            self.unmatched_tool_use_ids.push(tool_use_id.to_string());
        }
    }

    fn push_invalid_tool_use_input_id(&mut self, tool_use_id: &str) {
        if self.invalid_tool_use_input_ids.len() < 8 {
            self.invalid_tool_use_input_ids
                .push(tool_use_id.to_string());
        }
    }
}

pub(crate) fn sanitize_messages_for_provider(
    messages: &[InputMessage],
) -> (Vec<InputMessage>, SanitizationStats) {
    let mut sanitized: Vec<InputMessage> = Vec::with_capacity(messages.len());
    let mut expected_tool_results: HashSet<String> = HashSet::new();
    let mut pending_assistant_index: Option<usize> = None;
    let mut stats = SanitizationStats::default();

    for message in messages {
        if let Some(idx) = pending_assistant_index {
            let matched = message.role == "user"
                && message.content.iter().any(|block| {
                    matches!(
                        block,
                        InputContentBlock::ToolResult { tool_use_id, .. } if expected_tool_results.contains(tool_use_id)
                    )
                });
            if !matched {
                let removed_ids = remove_tool_use_blocks(&mut sanitized[idx]);
                stats.dropped_unmatched_tool_uses += removed_ids.len();
                for tool_use_id in removed_ids {
                    stats.push_unmatched_tool_use_id(&tool_use_id);
                }
                pending_assistant_index = None;
                expected_tool_results.clear();
            }
        }

        let mut next_content: Vec<InputContentBlock> = Vec::new();
        for block in &message.content {
            match block {
                InputContentBlock::ToolResult { tool_use_id, .. } => {
                    if message.role == "user" && expected_tool_results.contains(tool_use_id) {
                        next_content.push(block.clone());
                        expected_tool_results.remove(tool_use_id);
                        if expected_tool_results.is_empty() {
                            pending_assistant_index = None;
                        }
                    } else {
                        stats.dropped_orphan_tool_results += 1;
                        stats.push_orphan_tool_result_id(tool_use_id);
                    }
                }
                InputContentBlock::ToolUse { id, input, .. } => {
                    if input.is_object() {
                        next_content.push(block.clone());
                    } else {
                        stats.dropped_invalid_tool_use_inputs += 1;
                        stats.push_invalid_tool_use_input_id(id);
                    }
                }
                _ => next_content.push(block.clone()),
            }
        }

        if next_content.is_empty() {
            stats.dropped_empty_messages += 1;
            continue;
        }

        let tool_use_ids: HashSet<String> = next_content
            .iter()
            .filter_map(|block| match block {
                InputContentBlock::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();

        let sanitized_message = InputMessage {
            role: match message.role.as_str() {
                "assistant" => "assistant".to_string(),
                _ => "user".to_string(),
            },
            content: next_content,
            // P-MULTI-API: preserve captured thinking through sanitize so
            // downstream `translate_message` can still emit `reasoning_content`.
            thinking: message.thinking.clone(),
        };
        if !tool_use_ids.is_empty() && sanitized_message.role == "assistant" {
            expected_tool_results = tool_use_ids;
            pending_assistant_index = Some(sanitized.len());
        }

        if sanitized_message.content.is_empty() {
            continue;
        }
        sanitized.push(sanitized_message);
    }

    if let Some(idx) = pending_assistant_index {
        let removed_ids = remove_tool_use_blocks(&mut sanitized[idx]);
        stats.dropped_unmatched_tool_uses += removed_ids.len();
        for tool_use_id in removed_ids {
            stats.push_unmatched_tool_use_id(&tool_use_id);
        }
    }

    let result: Vec<InputMessage> = sanitized
        .into_iter()
        .filter(|message| !message.content.is_empty())
        .collect();
    (result, stats)
}

pub(crate) fn remove_tool_use_blocks(message: &mut InputMessage) -> Vec<String> {
    let mut removed_ids: Vec<String> = Vec::new();
    message.content.retain(|block| {
        if let InputContentBlock::ToolUse { id, .. } = block {
            removed_ids.push(id.clone());
            return false;
        }
        true
    });
    removed_ids
}

pub(crate) fn extend_sample_ids(target: &mut Vec<String>, incoming: &[String], max_samples: usize) {
    for sample in incoming {
        if target.len() >= max_samples {
            break;
        }
        if !target.iter().any(|existing| existing == sample) {
            target.push(sample.clone());
        }
    }
}
