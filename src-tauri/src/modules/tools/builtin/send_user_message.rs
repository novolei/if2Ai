//! SendUserMessage tool - send a message to the user.
//!
//! Migrated from GlobalToolRegistry mvp_tool_specs SendUserMessage.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

#[derive(serde::Deserialize)]
struct SendUserMessageInput {
    message: String,
    status: String,
    #[allow(dead_code)]
    attachments: Option<Vec<String>>,
}

/// Creates the SendUserMessage tool entry.
#[must_use]
pub fn send_user_message_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _ctx: SharedToolContext| {
        Box::pin(async move {
            let input: SendUserMessageInput = serde_json::from_value(args.clone())
                .map_err(|e| ToolError::Handler(format!("Invalid input: {e}")))?;

            Ok(format!(
                "[Message to user] ({}): {}",
                input.status, input.message
            ))
        })
    });

    ToolEntry {
        name: "SendUserMessage".to_string(),
        toolset: "utility".to_string(),
        description: "Send a message to the user with optional attachments.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": { "type": "string", "description": "Message content" },
                "attachments": { "type": "array", "items": { "type": "string" }, "description": "Optional file attachments" },
                "status": { "type": "string", "enum": ["normal", "proactive"], "description": "Message priority" }
            },
            "required": ["message", "status"]
        }),
        max_result_size: Some(1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(10),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn send_user_message_tool_entry_has_correct_structure() {
        let entry = send_user_message_tool_entry();
        assert_eq!(entry.name, "SendUserMessage");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(1024));
        assert_eq!(entry.timeout_secs, Some(10));
    }
}
