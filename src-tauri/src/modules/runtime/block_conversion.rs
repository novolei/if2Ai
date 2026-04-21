//! Block conversion helpers — translate runtime `ContentBlock`s into
//! provider-facing `InputContentBlock`s and summarise tool results for
//! inclusion back into the next prompt.
//!
//! Extracted from `commands/agent.rs` in GFR-001 (pure structural move,
//! function bodies byte-identical). These helpers form the seam between
//! the `runtime` and `api` layers; placing them here keeps
//! `application::real_api_client` free of any reverse `commands::*` imports
//! while leaving `commands::agent` callers free to keep using them via
//! `use crate::modules::runtime::block_conversion::*`.

use std::hash::{Hash, Hasher};

use crate::modules::api::{InputContentBlock, ToolResultContentBlock};
use crate::modules::runtime::session::ContentBlock;

pub(crate) const MAX_TOOL_RESULT_FOR_MODEL_CHARS: usize = 8_000;
pub(crate) const TOOL_RESULT_PREVIEW_CHARS: usize = 320;

pub(crate) fn parse_tool_input_json(raw_input: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(raw_input) {
        Ok(value) if value.is_object() => value,
        _ => serde_json::json!({}),
    }
}

pub(crate) fn short_text_digest(text: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(crate) fn truncate_tool_result_for_model(result: &str) -> String {
    let total_chars = result.chars().count();
    if total_chars <= MAX_TOOL_RESULT_FOR_MODEL_CHARS {
        return result.to_string();
    }
    let kept: String = result
        .chars()
        .take(MAX_TOOL_RESULT_FOR_MODEL_CHARS)
        .collect();
    format!(
        "{kept}\n\n[tool_result_truncated_for_context: omitted {} chars]",
        total_chars - MAX_TOOL_RESULT_FOR_MODEL_CHARS
    )
}

pub(crate) fn summarize_tool_result_for_model(
    tool_name: &str,
    tool_use_id: &str,
    result: &str,
    is_error: bool,
) -> String {
    let preview: String = result.chars().take(TOOL_RESULT_PREVIEW_CHARS).collect();
    let digest = short_text_digest(result);
    let total_chars = result.chars().count();
    let compact = format!(
        "[tool_result_handle] tool={tool_name} id={tool_use_id} status={} chars={total_chars} digest={digest}\npreview:\n{}",
        if is_error { "error" } else { "ok" },
        preview
    );
    truncate_tool_result_for_model(&compact)
}

pub(crate) fn runtime_block_to_input_block(block: &ContentBlock) -> InputContentBlock {
    match block {
        ContentBlock::Text { text } => InputContentBlock::Text { text: text.clone() },
        ContentBlock::ToolUse { id, name, input } => {
            let input_value = parse_tool_input_json(input);
            InputContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input_value,
            }
        }
        ContentBlock::ToolResult {
            tool_use_id,
            output,
            is_error,
            ..
        } => InputContentBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: vec![ToolResultContentBlock::Text {
                text: summarize_tool_result_for_model("history", tool_use_id, output, *is_error),
            }],
            is_error: *is_error,
        },
    }
}
