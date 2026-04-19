//! StructuredOutput tool - return structured output in requested format.
//!
//! Migrated from GlobalToolRegistry mvp_tool_specs StructuredOutput.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

/// Creates the StructuredOutput tool entry.
#[must_use]
pub fn structured_output_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _ctx: SharedToolContext| {
        Box::pin(async move {
            serde_json::to_string_pretty(&args)
                .map_err(|e| ToolError::Handler(format!("Failed to serialize output: {e}")))
        })
    });

    ToolEntry {
        name: "StructuredOutput".to_string(),
        toolset: "utility".to_string(),
        description: "Return structured output in the requested format.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "output": {
                    "type": "object",
                    "description": "Arbitrary structured output in any JSON format."
                }
            },
            "additionalProperties": true
        }),
        max_result_size: Some(100 * 1024),
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
    async fn structured_output_tool_entry_has_correct_structure() {
        let entry = structured_output_tool_entry();
        assert_eq!(entry.name, "StructuredOutput");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(100 * 1024));
        assert_eq!(entry.timeout_secs, Some(10));
    }
}
