//! Tools command module - direct tool execution from frontend
//!
//! Provides Tauri commands for the frontend to directly invoke tools.

use crate::commands::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Tool definition in OpenAI format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for input validation
    pub input_schema: serde_json::Value,
}

/// Tool call result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ToolCallResult {
    /// Whether the call succeeded
    pub success: bool,
    /// Tool output if successful
    pub output: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Execute a tool by name with JSON arguments.
///
/// This command allows the frontend to directly invoke tools
/// through the tool registry.
#[tauri::command]
pub async fn execute_tool(
    state: State<'_, AppState>,
    name: String,
    args: String,
) -> Result<String, String> {
    let args: serde_json::Value =
        serde_json::from_str(&args).map_err(|e| format!("invalid JSON args: {}", e))?;

    state
        .tool_registry
        .dispatch(&name, args)
        .await
        .map_err(|e| e.to_string())
}

/// List all available tools.
///
/// Returns tool metadata for all registered tools.
#[tauri::command]
pub fn list_tools(state: State<'_, AppState>) -> Result<Vec<ToolDefinition>, String> {
    let entries = state.tool_registry.get_definitions(None);

    let tools: Vec<ToolDefinition> = entries
        .into_iter()
        .map(|def| {
            let obj = def.get("function").and_then(|f| f.as_object());
            ToolDefinition {
                name: obj
                    .and_then(|o| o.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: obj
                    .and_then(|o| o.get("description"))
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string(),
                input_schema: obj
                    .and_then(|o| o.get("parameters"))
                    .cloned()
                    .unwrap_or(serde_json::json!({})),
            }
        })
        .collect();

    Ok(tools)
}

/// Get tool definitions in OpenAI function calling format.
///
/// Optionally filter to only include tools in the `allowed` list.
#[tauri::command]
pub fn get_tool_definitions(
    state: State<'_, AppState>,
    allowed: Option<Vec<String>>,
) -> Result<Vec<serde_json::Value>, String> {
    let allowed_ref: Option<&[String]> = allowed.as_deref();
    Ok(state.tool_registry.get_definitions(allowed_ref))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_serialization() {
        let def = ToolDefinition {
            name: "test".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "arg": {"type": "string"}
                }
            }),
        };

        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("A test tool"));
    }

    #[test]
    fn tool_call_result_success() {
        let result = ToolCallResult {
            success: true,
            output: Some("hello".to_string()),
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("hello"));
    }

    #[test]
    fn tool_call_result_error() {
        let result = ToolCallResult {
            success: false,
            output: None,
            error: Some("something went wrong".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("something went wrong"));
    }
}
