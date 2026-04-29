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
    parse_tool_input_json_candidate(raw_input, 0)
        .or_else(|| parse_tool_input_json_candidate(strip_json_fence(raw_input), 0))
        .or_else(|| extract_first_json_object(raw_input).and_then(parse_json_object))
        .unwrap_or_else(|| serde_json::json!({}))
}

fn parse_tool_input_json_candidate(raw_input: &str, depth: usize) -> Option<serde_json::Value> {
    if depth > 2 {
        return None;
    }
    let parsed = serde_json::from_str::<serde_json::Value>(raw_input.trim()).ok()?;
    match parsed {
        serde_json::Value::Object(_) => Some(parsed),
        serde_json::Value::String(value) => parse_tool_input_json_candidate(&value, depth + 1)
            .or_else(|| parse_tool_input_json_candidate(strip_json_fence(&value), depth + 1))
            .or_else(|| extract_first_json_object(&value).and_then(parse_json_object)),
        _ => None,
    }
}

fn parse_json_object(raw_input: &str) -> Option<serde_json::Value> {
    match serde_json::from_str::<serde_json::Value>(raw_input.trim()).ok()? {
        value if value.is_object() => Some(value),
        _ => None,
    }
}

fn strip_json_fence(raw_input: &str) -> &str {
    let trimmed = raw_input.trim();
    let Some(stripped) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let Some(first_newline) = stripped.find('\n') else {
        return trimmed;
    };
    let body = &stripped[first_newline + 1..];
    body.strip_suffix("```").map(str::trim).unwrap_or(trimmed)
}

fn extract_first_json_object(raw_input: &str) -> Option<&str> {
    let mut start: Option<usize> = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in raw_input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_string {
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let start_index = start?;
                    return Some(&raw_input[start_index..=index]);
                }
            }
            _ => {}
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tool_input_json_accepts_plain_object() {
        let parsed = parse_tool_input_json(r#"{"path":"index.html","content":"hello"}"#);

        assert_eq!(parsed["path"], "index.html");
        assert_eq!(parsed["content"], "hello");
    }

    #[test]
    fn parse_tool_input_json_accepts_double_encoded_object() {
        let encoded = serde_json::to_string(r#"{"path":"index.html","content":"hello"}"#)
            .expect("encode object string");

        let parsed = parse_tool_input_json(&encoded);

        assert_eq!(parsed["path"], "index.html");
        assert_eq!(parsed["content"], "hello");
    }

    #[test]
    fn parse_tool_input_json_accepts_fenced_object() {
        let parsed =
            parse_tool_input_json("```json\n{\"language\":\"shell\",\"code\":\"pwd\"}\n```");

        assert_eq!(parsed["language"], "shell");
        assert_eq!(parsed["code"], "pwd");
    }

    #[test]
    fn parse_tool_input_json_extracts_embedded_object() {
        let parsed = parse_tool_input_json(
            "tool args:\n{\"path\":\"/tmp/index.html\",\"content\":\"<html></html>\"}\nthanks",
        );

        assert_eq!(parsed["path"], "/tmp/index.html");
        assert_eq!(parsed["content"], "<html></html>");
    }

    #[test]
    fn parse_tool_input_json_rejects_non_object() {
        assert_eq!(
            parse_tool_input_json(r#"["path","index.html"]"#),
            serde_json::json!({})
        );
    }
}
