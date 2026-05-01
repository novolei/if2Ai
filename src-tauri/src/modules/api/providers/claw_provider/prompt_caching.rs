//! Anthropic prompt caching — layered `cache_control` injection.
//!
//! Anthropic's prompt caching API allows marking specific content blocks
//! with `cache_control: {"type": "ephemeral"}` so that repeated prefixes
//! are served from cache at reduced cost.
//!
//! ## Strategy
//!
//! 1. **System message** — the last content block of the system array gets
//!    `cache_control: {"type": "ephemeral"}`.  Since the system prompt is
//!    almost always identical across turns, this maximises cache hits.
//! 2. **Last two user messages** — each message's last content block gets
//!    `cache_control: {"type": "ephemeral"}` to create a sliding-window
//!    ephemeral cache over the most recent context.
//! 3. **Tool definitions** — the last tool object in the `tools` array gets
//!    `cache_control: {"type": "ephemeral"}`, caching the entire tool schema
//!    prefix.
//!
//! This module operates on `serde_json::Value` so it is decoupled from
//! the shared `MessageRequest` type (which is also used by OpenAI-compat
//! providers that have no cache_control concept).

use serde_json::{json, Value};

/// Ephemeral cache control marker accepted by the Anthropic Messages API.
fn ephemeral_cache_control() -> Value {
    json!({"type": "ephemeral"})
}

/// Convert the serialized `MessageRequest` JSON into a form that carries
/// Anthropic `cache_control` annotations.
///
/// The input `body` is the JSON object produced by `serde_json::to_value`
/// on a `MessageRequest`. This function mutates it in place:
///
/// - Transforms `system` from a plain string into an array of content
///   blocks (required by Anthropic for `cache_control` placement).
/// - Annotates the last content block of `system`, the last two `user`
///   messages, and the last tool definition with ephemeral cache control.
pub fn inject_cache_control(body: &mut Value) {
    inject_system_cache_control(body);
    inject_user_message_cache_control(body);
    inject_tool_cache_control(body);
}

/// Convert `"system": "text"` → `"system": [{"type":"text","text":"...","cache_control":...}]`
/// and annotate the last block with ephemeral cache control.
fn inject_system_cache_control(body: &mut Value) {
    let Some(system_val) = body.get("system") else {
        return;
    };

    // If system is already an array (future-proofing), just annotate the last block.
    if let Some(arr) = system_val.as_array().cloned() {
        if arr.is_empty() {
            return;
        }
        let mut blocks = arr;
        if let Some(last) = blocks.last_mut() {
            last["cache_control"] = ephemeral_cache_control();
        }
        body["system"] = Value::Array(blocks);
        return;
    }

    // Standard path: system is a plain string → convert to content-block array.
    if let Some(text) = system_val.as_str().map(|s| s.to_owned()) {
        if text.is_empty() {
            return;
        }
        body["system"] = json!([{
            "type": "text",
            "text": text,
            "cache_control": ephemeral_cache_control()
        }]);
    }
}

/// Annotate the last content block of the most recent 2 user messages
/// with ephemeral cache control.
fn inject_user_message_cache_control(body: &mut Value) {
    let Some(messages) = body.get_mut("messages").and_then(|v| v.as_array_mut()) else {
        return;
    };

    // Collect indices of user messages (iterate in reverse to find last 2).
    let user_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, msg)| msg.get("role").and_then(|r| r.as_str()) == Some("user"))
        .take(2)
        .map(|(i, _)| i)
        .collect();

    for idx in user_indices {
        annotate_last_content_block(&mut messages[idx]);
    }
}

/// Annotate the last tool in the `tools` array with ephemeral cache control.
fn inject_tool_cache_control(body: &mut Value) {
    let Some(tools) = body.get_mut("tools").and_then(|v| v.as_array_mut()) else {
        return;
    };
    if let Some(last_tool) = tools.last_mut() {
        last_tool["cache_control"] = ephemeral_cache_control();
    }
}

/// Add `cache_control` to the last element of a message's `content` array.
///
/// Handles both array content (`[{type: "text", ...}]`) and string content.
/// When content is a string, it is converted to an array of one text block.
fn annotate_last_content_block(message: &mut Value) {
    let Some(content) = message.get_mut("content") else {
        return;
    };

    if let Some(arr) = content.as_array_mut() {
        if let Some(last) = arr.last_mut() {
            last["cache_control"] = ephemeral_cache_control();
        }
    } else if let Some(text) = content.as_str().map(|s| s.to_owned()) {
        // Content is a plain string — convert to block array.
        message["content"] = json!([{
            "type": "text",
            "text": text,
            "cache_control": ephemeral_cache_control()
        }]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn system_string_converted_to_block_with_cache_control() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [],
            "system": "You are helpful."
        });
        inject_cache_control(&mut body);
        let system = body["system"].as_array().expect("system should be array");
        assert_eq!(system.len(), 1);
        assert_eq!(system[0]["type"], "text");
        assert_eq!(system[0]["text"], "You are helpful.");
        assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn system_array_annotates_last_block() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [],
            "system": [
                {"type": "text", "text": "Part 1"},
                {"type": "text", "text": "Part 2"}
            ]
        });
        inject_cache_control(&mut body);
        let system = body["system"].as_array().unwrap();
        assert!(system[0].get("cache_control").is_none());
        assert_eq!(system[1]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn no_system_field_is_noop() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}]
        });
        inject_cache_control(&mut body);
        assert!(body.get("system").is_none());
    }

    #[test]
    fn last_two_user_messages_annotated() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "msg1"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "resp1"}]},
                {"role": "user", "content": [{"type": "text", "text": "msg2"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "resp2"}]},
                {"role": "user", "content": [{"type": "text", "text": "msg3"}]}
            ]
        });
        inject_cache_control(&mut body);
        let msgs = body["messages"].as_array().unwrap();
        // msg1 (index 0) — not in the last 2 user messages
        assert!(msgs[0]["content"][0].get("cache_control").is_none());
        // msg2 (index 2) — 2nd-to-last user message
        assert_eq!(msgs[2]["content"][0]["cache_control"]["type"], "ephemeral");
        // msg3 (index 4) — last user message
        assert_eq!(msgs[4]["content"][0]["cache_control"]["type"], "ephemeral");
        // assistant messages untouched
        assert!(msgs[1]["content"][0].get("cache_control").is_none());
        assert!(msgs[3]["content"][0].get("cache_control").is_none());
    }

    #[test]
    fn single_user_message_annotated() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "only one"}]}
            ]
        });
        inject_cache_control(&mut body);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["content"][0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn last_tool_annotated() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [],
            "tools": [
                {"name": "bash", "description": "Run bash", "input_schema": {}},
                {"name": "grep", "description": "Search", "input_schema": {}}
            ]
        });
        inject_cache_control(&mut body);
        let tools = body["tools"].as_array().unwrap();
        assert!(tools[0].get("cache_control").is_none());
        assert_eq!(tools[1]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn no_tools_is_noop() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [{"role": "user", "content": "hi"}]
        });
        inject_cache_control(&mut body);
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn empty_system_skipped() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [],
            "system": ""
        });
        inject_cache_control(&mut body);
        // Empty string system should remain as-is (not converted)
        assert_eq!(body["system"], "");
    }

    #[test]
    fn user_message_string_content_converted() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {"role": "user", "content": "plain string content"}
            ]
        });
        inject_cache_control(&mut body);
        let msgs = body["messages"].as_array().unwrap();
        let content = msgs[0]["content"].as_array().expect("converted to array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "plain string content");
        assert_eq!(content[0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn multi_block_user_message_annotates_last_only() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "first"},
                    {"type": "tool_result", "tool_use_id": "t1", "content": [{"type": "text", "text": "result"}]}
                ]}
            ]
        });
        inject_cache_control(&mut body);
        let content = body["messages"][0]["content"].as_array().unwrap();
        assert!(content[0].get("cache_control").is_none());
        assert_eq!(content[1]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn full_layered_cache_control() {
        let mut body = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 4096,
            "system": "System instructions",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q1"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "a1"}]},
                {"role": "user", "content": [{"type": "text", "text": "q2"}]}
            ],
            "tools": [
                {"name": "bash", "description": "shell", "input_schema": {}},
                {"name": "read", "description": "read file", "input_schema": {}}
            ],
            "stream": true
        });
        inject_cache_control(&mut body);

        // System: converted to array with cache_control
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");

        // User messages: last 2 annotated
        assert_eq!(body["messages"][0]["content"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["messages"][2]["content"][0]["cache_control"]["type"], "ephemeral");
        // Assistant untouched
        assert!(body["messages"][1]["content"][0].get("cache_control").is_none());

        // Tools: last annotated
        assert!(body["tools"][0].get("cache_control").is_none());
        assert_eq!(body["tools"][1]["cache_control"]["type"], "ephemeral");
    }
}
