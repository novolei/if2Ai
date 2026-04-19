//! Sleep tool - async wait without holding a shell process.
//!
//! Migrated from GlobalToolRegistry mvp_tool_specs Sleep.

use std::sync::Arc;
use std::time::Duration;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

#[derive(serde::Deserialize)]
struct SleepInput {
    duration_ms: u64,
}

/// Creates the Sleep tool entry.
#[must_use]
pub fn sleep_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _ctx: SharedToolContext| {
        Box::pin(async move {
            let input: SleepInput = serde_json::from_value(args.clone())
                .map_err(|e| ToolError::Handler(format!("Invalid input: {e}")))?;

            if input.duration_ms > 300_000 {
                return Err(ToolError::Handler(
                    "Sleep duration exceeds maximum of 300000ms (5 minutes)".into(),
                ));
            }

            tokio::time::sleep(Duration::from_millis(input.duration_ms)).await;
            Ok(format!("Slept for {}ms", input.duration_ms))
        })
    });

    ToolEntry {
        name: "Sleep".to_string(),
        toolset: "utility".to_string(),
        description: "Wait for a specified duration without holding a shell process.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "duration_ms": { "type": "integer", "minimum": 0, "description": "Duration to sleep in milliseconds" }
            },
            "required": ["duration_ms"]
        }),
        max_result_size: Some(1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(300),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sleep_tool_entry_has_correct_structure() {
        let entry = sleep_tool_entry();
        assert_eq!(entry.name, "Sleep");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(1024));
        assert_eq!(entry.timeout_secs, Some(300));
    }

    #[tokio::test]
    async fn sleep_tool_short_sleep() {
        let entry = sleep_tool_entry();
        let handler = entry.handler.clone();
        let ctx = Arc::new(std::sync::Mutex::new(
            crate::modules::tools::context::ToolContext::default_for_workdir(
                std::path::PathBuf::from("."),
            ),
        ));

        let result = handler(serde_json::json!({"duration_ms": 10}), ctx).await;
        assert!(result.is_ok());
    }
}
