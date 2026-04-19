//! Memory Recall tool - retrieves memories matching a query
//!
//! Provides memory retrieval with optional category filtering, scoped to the
//! current session when a `session_id` is present in the tool context.

use std::sync::Arc;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::scope::MemoryScopeResolver;
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
    let handler: ToolHandler =
        Arc::new(move |args: serde_json::Value, context: SharedToolContext| {
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

                // Resolve scope from the tool execution context.  When a session_id
                // is present, recall is filtered to session-scoped + global entries.
                let scope = context
                    .lock()
                    .ok()
                    .map(|ctx| MemoryScopeResolver::from_tool_context(&ctx))
                    .unwrap_or_else(crate::modules::memory::scope::MemoryExecutionScope::global);

                let results = memory
                    .recall_scoped(&query, category, limit, &scope)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to recall memory: {}", e)))?;

                // Emit audit event for observability and frontend evidence chain.
                let audit_ctx = AuditContext::from_scope(&scope);
                MemoryAuditEmitter::memory_recall_served(
                    &audit_ctx,
                    &query,
                    category,
                    results.len(),
                );

                let output: Vec<String> = results
                    .iter()
                    .map(|e: &MemoryEntry| {
                        format!("[{}] {}: {}", e.category.as_str(), e.key, e.content)
                    })
                    .collect();

                Ok(output.join("\n"))
            })
        });

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
    async fn memory_recall_tool_entry_has_correct_structure() {
        let entry = entry(test_memory());
        assert_eq!(entry.name, "memory_recall");
        assert_eq!(entry.toolset, "memory");
        assert!(!entry.disabled);
    }
}
