//! Memory Export tool - exports memories in various formats
//!
//! Provides memory export as JSON or Markdown.

use std::sync::Arc;

use crate::modules::memory::{MemoryEntry, SharedMemoryProvider};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the memory_export tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        move |args: serde_json::Value, _context: SharedToolContext| {
            let memory = memory.clone();
            Box::pin(async move {
                let category = args.get("category").and_then(|v| v.as_str());

                let format = args
                    .get("format")
                    .and_then(|v| v.as_str())
                    .unwrap_or("json")
                    .to_lowercase();

                let entries = memory
                    .export(category)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to export memory: {}", e)))?;

                let output = match format.as_str() {
                    "markdown" | "md" => format_as_markdown(&entries),
                    "json" => format_as_json(&entries),
                    _ => format_as_json(&entries),
                };

                Ok(output)
            })
        },
    );

    ToolEntry {
        name: "memory_export".to_string(),
        toolset: "memory".to_string(),
        description: "Export memories in JSON or Markdown format".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "description": "Filter by category (optional)"
                },
                "format": {
                    "type": "string",
                    "description": "Output format: json or markdown (default: json)"
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

fn format_as_json(entries: &[MemoryEntry]) -> String {
    serde_json::to_string_pretty(entries).unwrap_or_else(|_| "[]".to_string())
}

fn format_as_markdown(entries: &[MemoryEntry]) -> String {
    let mut output = String::from("# Memory Export\n\n");
    output.push_str("| Category | Key | Content | Created |\n");
    output.push_str("|----------|-----|---------|--------|\n");

    for entry in entries {
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            entry.category.as_str(),
            entry.key,
            entry.content.replace('|', "\\|"),
            entry.created_at.format("%Y-%m-%d %H:%M")
        ));
    }

    output
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
    async fn memory_export_tool_entry_has_correct_structure() {
        let entry = entry(test_memory());
        assert_eq!(entry.name, "memory_export");
        assert_eq!(entry.toolset, "memory");
        assert!(!entry.disabled);
    }
}
