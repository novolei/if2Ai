//! Memory Recall tool - retrieves memories matching a query
//!
//! Provides memory retrieval with optional category filtering.

use std::sync::Arc;

use crate::modules::memory::{MemoryEntry, SharedMemoryProvider};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Default recall limit
#[allow(dead_code)]
const DEFAULT_LIMIT: usize = 10;

/// Creates the memory_recall tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        move |args: serde_json::Value, _context: SharedToolContext| {
            let memory = memory.clone();
            Box::pin(async move {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let category = args.get("category").and_then(|v| v.as_str());

                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(DEFAULT_LIMIT);

                let results = memory
                    .recall(&query, category, limit)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to recall memory: {}", e)))?;

                let output: Vec<String> = results
                    .iter()
                    .map(|e: &MemoryEntry| {
                        format!("[{}] {}: {}", e.category.as_str(), e.key, e.content)
                    })
                    .collect();

                Ok(output.join("\n"))
            })
        },
    );

    ToolEntry {
        name: "memory_recall".to_string(),
        toolset: "memory".to_string(),
        description: "Recall memories matching a query".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (matches key or content)"
                },
                "category": {
                    "type": "string",
                    "description": "Filter by category (optional)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum results to return (default: 10)"
                }
            },
            "required": []
        }),
        max_result_size: Some(1024 * 1024),
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
    async fn memory_recall_tool_entry_has_correct_structure() {
        let entry = entry(test_memory());
        assert_eq!(entry.name, "memory_recall");
        assert_eq!(entry.toolset, "memory");
        assert!(!entry.disabled);
    }
}
