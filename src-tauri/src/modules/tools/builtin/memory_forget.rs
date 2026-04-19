//! Memory Forget tool - deletes a specific memory entry
//!
//! Provides deletion of individual memory entries by key.

use std::sync::Arc;

use crate::modules::memory::SharedMemoryProvider;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the memory_forget tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        move |args: serde_json::Value, _context: SharedToolContext| {
            let memory = memory.clone();
            Box::pin(async move {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: key".to_string())
                    })?
                    .to_string();

                memory
                    .delete(&key)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to delete memory: {}", e)))?;

                Ok(format!("Deleted memory: {}", key))
            })
        },
    );

    ToolEntry {
        name: "memory_forget".to_string(),
        toolset: "memory".to_string(),
        description: "Delete a specific memory entry".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "Key of the memory to delete"
                }
            },
            "required": ["key"]
        }),
        max_result_size: Some(256),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(10),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::modules::memory::InMemoryMemoryProvider;

    fn test_memory() -> SharedMemoryProvider {
        Arc::new(InMemoryMemoryProvider::new())
    }

    #[tokio::test]
    async fn memory_forget_tool_entry_has_correct_structure() {
        let entry = entry(test_memory());
        assert_eq!(entry.name, "memory_forget");
        assert_eq!(entry.toolset, "memory");
        assert!(!entry.disabled);
    }
}
