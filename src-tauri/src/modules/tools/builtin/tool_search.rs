//! ToolSearch tool - search available tools by name or description.
//!
//! Provides a ToolSearch ToolHandler that allows the Agent to discover
//! registered tools through fuzzy name/description matching.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler, ToolRegistry},
};

/// Creates the ToolSearch tool entry for the registry.
///
/// The ToolSearch tool allows the Agent to search for available tools
/// by name or description, useful when the Agent needs to find a tool
/// it doesn't know by name.
#[must_use]
pub fn tool_search_tool_entry(registry: Arc<ToolRegistry>) -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        move |args: serde_json::Value, _context: SharedToolContext| {
            let registry_clone = registry.clone();
            Box::pin(async move {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();

                let definitions = registry_clone.get_definitions(None);
                let results: Vec<_> = definitions
                    .into_iter()
                    .filter_map(|def| {
                        let obj = def.as_object()?;
                        let func = obj.get("function")?.as_object()?;
                        let name = func.get("name")?.as_str()?.to_lowercase();
                        let desc = func
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if query.is_empty() || name.contains(&query) || desc.contains(&query) {
                            Some(def)
                        } else {
                            None
                        }
                    })
                    .collect();

                serde_json::to_string_pretty(&results)
                    .map_err(|e| ToolError::Handler(e.to_string()))
            })
        },
    );

    ToolEntry {
        name: "tool_search".to_string(),
        toolset: "utility".to_string(),
        description: "Search for available tools by name or description. \
                      Use this when you need to find a tool but don't know its exact name."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query for tool name or description"
                }
            },
            "required": ["query"]
        }),
        max_result_size: Some(5 * 1024),
        timeout_secs: Some(5),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tools::context::ToolContext;
    use serde_json::json;

    fn test_registry() -> Arc<ToolRegistry> {
        let ctx = ToolContext::default_for_workdir(std::path::PathBuf::from("."));
        let registry = Arc::new(ToolRegistry::new(Arc::new(std::sync::Mutex::new(ctx))));
        // Register a couple test tools
        registry
            .register(ToolEntry {
                name: "read_file".to_string(),
                toolset: "read".to_string(),
                description: "Read the contents of a file".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                timeout_secs: None,
                disabled: false,
                handler: Arc::new(|_, _| Box::pin(async move { Ok("".to_string()) })),
            })
            .unwrap();
        registry
            .register(ToolEntry {
                name: "write_file".to_string(),
                toolset: "write".to_string(),
                description: "Write content to a file".to_string(),
                input_schema: json!({"type": "object"}),
                max_result_size: None,
                timeout_secs: None,
                disabled: false,
                handler: Arc::new(|_, _| Box::pin(async move { Ok("".to_string()) })),
            })
            .unwrap();
        registry
    }

    #[tokio::test]
    async fn tool_search_tool_entry_has_correct_structure() {
        let registry = test_registry();
        let entry = tool_search_tool_entry(registry);
        assert_eq!(entry.name, "tool_search");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(5 * 1024));
        assert_eq!(entry.timeout_secs, Some(5));
    }

    #[tokio::test]
    async fn tool_search_finds_by_name() {
        let registry = test_registry();
        let entry = tool_search_tool_entry(registry);
        let handler = entry.handler.clone();
        let ctx = Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            std::path::PathBuf::from("."),
        )));

        let result = handler(json!({"query": "read"}), ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("read_file"));
    }

    #[tokio::test]
    async fn tool_search_finds_by_description() {
        let registry = test_registry();
        let entry = tool_search_tool_entry(registry);
        let handler = entry.handler.clone();
        let ctx = Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            std::path::PathBuf::from("."),
        )));

        let result = handler(json!({"query": "write content"}), ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("write_file"));
    }

    #[tokio::test]
    async fn tool_search_empty_query_returns_all() {
        let registry = test_registry();
        let entry = tool_search_tool_entry(registry);
        let handler = entry.handler.clone();
        let ctx = Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            std::path::PathBuf::from("."),
        )));

        let result = handler(json!({"query": ""}), ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("read_file"));
        assert!(output.contains("write_file"));
    }
}
