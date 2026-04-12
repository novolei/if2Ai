//! File Edit tool - applies unified diff patches to files
//!
//! Provides a safe way to edit files using unified diff format patches.

use std::sync::Arc;

use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the file_edit tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        |_args: serde_json::Value, _context: SharedToolContext| {
            Box::pin(async move {
                // Stub implementation - file edit using diff patches is complex
                // This would require parsing unified diff format and applying patches
                Err(ToolError::Handler(
                    "file_edit tool is not yet implemented".to_string(),
                ))
            })
        },
    );

    ToolEntry {
        name: "file_edit".to_string(),
        toolset: "files".to_string(),
        description: "Apply a unified diff patch to a file".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "patch": {
                    "type": "string",
                    "description": "Unified diff patch to apply"
                }
            },
            "required": ["path", "patch"]
        }),
        max_result_size: Some(1024 * 1024),
        timeout_secs: Some(30),
        disabled: true,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn file_edit_tool_entry_is_disabled() {
        let entry = entry();
        assert_eq!(entry.name, "file_edit");
        assert_eq!(entry.toolset, "files");
        assert!(entry.disabled);
    }
}