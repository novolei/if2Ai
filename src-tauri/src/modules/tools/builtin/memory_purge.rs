//! Memory Purge tool - removes all memories in a category
//!
//! Provides bulk deletion of memory entries by category.

use std::sync::Arc;

use crate::modules::memory::SharedMemoryProvider;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the memory_purge tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        move |args: serde_json::Value, _context: SharedToolContext| {
            let memory = memory.clone();
            Box::pin(async move {
                let category = args
                    .get("category")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: category".to_string())
                    })?
                    .to_string();

                memory
                    .purge_category(&category)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to purge category: {}", e)))?;

                Ok(format!("Purged all memories in category: {}", category))
            })
        },
    );

    ToolEntry {
        name: "memory_purge".to_string(),
        toolset: "memory".to_string(),
        description: "Remove all memories in a category".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "description": "Category to purge (core, daily, conversation, or custom)"
                }
            },
            "required": ["category"]
        }),
        max_result_size: Some(256),
        timeout_secs: Some(10),
        disabled: false,
        handler,
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
    async fn memory_purge_tool_entry_has_correct_structure() {
        let entry = entry(test_memory());
        assert_eq!(entry.name, "memory_purge");
        assert_eq!(entry.toolset, "memory");
        assert!(!entry.disabled);
    }
}
