//! JSON Parse tool - parses and formats JSON strings
//!
//! Provides JSON validation and pretty-printing functionality.

use std::sync::Arc;

use serde_json::{self, Value};

use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the json_parse tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn json_parse_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        |args: serde_json::Value, _context: crate::modules::tools::context::SharedToolContext| {
            Box::pin(async move {
                let input = args
                    .get("input")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: input".to_string())
                    })?
                    .to_string();

                let pretty = args.get("pretty").and_then(|v| v.as_bool()).unwrap_or(true);

                parse_json_internal(&input, pretty)
            })
        },
    );

    ToolEntry {
        name: "json_parse".to_string(),
        toolset: "utility".to_string(),
        description: "Parses a JSON string and returns formatted output".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "JSON string to parse"
                },
                "pretty": {
                    "type": "boolean",
                    "description": "Whether to pretty-print the output (default: true)"
                }
            },
            "required": ["input"]
        }),
        max_result_size: Some(1024 * 1024), // 1MB
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

/// Internal JSON parsing function.
#[allow(dead_code)]
fn parse_json_internal(input: &str, pretty: bool) -> Result<String, ToolError> {
    // Parse the JSON
    let value: Value = serde_json::from_str(input)
        .map_err(|e| ToolError::Handler(format!("invalid JSON: {e}")))?;

    // Format the output
    let output = if pretty {
        serde_json::to_string_pretty(&value)
    } else {
        serde_json::to_string(&value)
    };

    output.map_err(|e| ToolError::Handler(format!("failed to serialize: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tools::context::{SharedToolContext, ToolContext};
    use serde_json::json;

    fn test_context() -> SharedToolContext {
        std::sync::Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            std::path::PathBuf::from("."),
        )))
    }

    #[allow(dead_code)]
    fn make_test_handler(output: &'static str) -> ToolHandler {
        Arc::new(
            move |_input: serde_json::Value, _context: SharedToolContext| {
                let output = output.to_string();
                Box::pin(async move { Ok(output) })
            },
        )
    }

    #[tokio::test]
    async fn json_parse_tool_entry_has_correct_structure() {
        let entry = json_parse_tool_entry();
        assert_eq!(entry.name, "json_parse");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
    }

    #[tokio::test]
    async fn parses_valid_json() {
        let entry = json_parse_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(json!({"input": r#"{"key": "value"}"#}), ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn pretty_print_works() {
        let entry = json_parse_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(json!({"input": r#"{"key":"value","nested":{"a":1}}"#}), ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should be pretty-printed with newlines and indentation
        assert!(output.contains('\n'));
    }

    #[tokio::test]
    async fn invalid_json_returns_error() {
        let entry = json_parse_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(json!({"input": "not valid json {"}), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn non_pretty_mode_works() {
        let entry = json_parse_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(
            json!({"input": r#"{"key": "value"}"#, "pretty": false}),
            ctx,
        )
        .await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should be compact without newlines
        assert!(!output.contains('\n'));
    }
}
