//! P1-7 — Search prior turns in the current session via SQLite FTS5
//! (`conversation_recall_fts`), indexed at end of each chat turn.

use std::sync::Arc;

use crate::modules::memory::scope::MemoryScopeResolver;
use crate::modules::memory::SharedMemoryProvider;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

const DEFAULT_LIMIT: usize = 8;

/// Register the `conversation_search` tool.
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

                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(DEFAULT_LIMIT);

                let scope = context
                    .lock()
                    .ok()
                    .map(|ctx| MemoryScopeResolver::from_tool_context(&ctx))
                    .unwrap_or_else(crate::modules::memory::scope::MemoryExecutionScope::global);

                let session_id = scope.session_id.as_deref().ok_or_else(|| {
                    ToolError::Handler(
                        "conversation_search requires session_id in tool context".to_string(),
                    )
                })?;

                let hits = memory
                    .conversation_recall_search(
                        &query,
                        session_id,
                        scope.project_id.as_deref(),
                        limit,
                    )
                    .await
                    .map_err(|e| ToolError::Handler(e.to_string()))?;

                if hits.is_empty() {
                    return Ok("(no matching prior turns)".to_string());
                }
                Ok(hits.join("\n---\n"))
            })
        });

    ToolEntry {
        name: "conversation_search".to_string(),
        toolset: "memory".to_string(),
        description: "Search prior turns in this session (FTS over conversation recall index)."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Keywords to search; leave empty for most recent snippets"
                },
                "limit": {
                    "type": "number",
                    "description": "Max snippets (default 8, max 50)"
                }
            },
            "required": []
        }),
        max_result_size: Some(512 * 1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(15),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}
