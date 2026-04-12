//! TodoWrite tool - update structured task list for the current session.
//!
//! Provides a TodoWrite ToolHandler migrated from GlobalToolRegistry
//! to the ToolRegistry pattern (sync → async adaptation).
//! See docs/bs_gap/08-critical-fix-priority.md §N3 for migration plan.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// A single todo item.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct TodoItem {
    content: String,
    #[serde(rename = "activeForm", default)]
    active_form: String,
    status: String, // "pending" | "in_progress" | "completed"
}

/// Input schema for TodoWrite tool.
#[derive(Debug, Deserialize)]
struct TodoWriteInput {
    todos: Vec<TodoItem>,
}

/// Result returned by TodoWrite tool.
#[derive(Debug, Serialize)]
struct TodoWriteResult {
    old_todos: Option<Vec<TodoItem>>,
    new_todos: Vec<TodoItem>,
    verification_nudge_needed: Option<String>,
}

/// Determines the file path for todo persistence.
/// Uses IF2AI_TODO_STORE env var, falling back to .if2ai-todos.json in cwd.
fn todo_store_path() -> std::path::PathBuf {
    let env_var = std::env::var("IF2AI_TODO_STORE").unwrap_or_default();
    if env_var.is_empty() {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".if2ai-todos.json")
    } else {
        std::path::PathBuf::from(env_var)
    }
}

/// Creates the TodoWrite tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn todo_write_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        |args: serde_json::Value, _context: crate::modules::tools::context::SharedToolContext| {
            Box::pin(async move {
                let input: TodoWriteInput = serde_json::from_value(args.clone())
                    .map_err(|e| ToolError::Handler(format!("Invalid TodoWrite input: {e}")))?;

                let result =
                    execute_todo_write_internal(&input.todos).map_err(ToolError::Handler)?;

                serde_json::to_string_pretty(&result).map_err(|e| ToolError::Handler(e.to_string()))
            })
        },
    );

    ToolEntry {
        name: "TodoWrite".to_string(),
        toolset: "utility".to_string(),
        description: "Update the structured task list for the current session. \
                      Use this to track multi-step tasks with status \
                      (pending/in_progress/completed)."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "Task description"
                            },
                            "activeForm": {
                                "type": "string",
                                "description": "Present-tense action description"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"]
                            }
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["todos"]
        }),
        max_result_size: Some(10_240),
        timeout_secs: Some(30),
        disabled: false,
        handler,
    }
}

/// Core todo persistence logic.
/// Reads old todos, writes new ones, returns both for comparison.
fn execute_todo_write_internal(todos: &Vec<TodoItem>) -> Result<TodoWriteResult, String> {
    let store_path = todo_store_path();

    let old_todos = if store_path.exists() {
        let content = std::fs::read_to_string(&store_path)
            .map_err(|e| format!("Failed to read todo store: {e}"))?;
        serde_json::from_str(&content).ok()
    } else {
        None
    };

    let json = serde_json::to_string_pretty(todos)
        .map_err(|e| format!("Failed to serialize todos: {e}"))?;
    std::fs::write(&store_path, json).map_err(|e| format!("Failed to write todo store: {e}"))?;

    Ok(TodoWriteResult {
        old_todos,
        new_todos: todos.clone(),
        verification_nudge_needed: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tools::context::{SharedToolContext, ToolContext};
    use serde_json::json;

    fn test_context() -> SharedToolContext {
        Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            std::path::PathBuf::from("."),
        )))
    }

    #[tokio::test]
    async fn todo_write_tool_entry_has_correct_structure() {
        let entry = todo_write_tool_entry();
        assert_eq!(entry.name, "TodoWrite");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(10_240));
        assert_eq!(entry.timeout_secs, Some(30));
    }

    #[tokio::test]
    async fn todo_write_validates_input() {
        let entry = todo_write_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        // Missing "todos" field
        let result = handler(json!({}), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn todo_write_writes_and_returns_result() {
        let entry = todo_write_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let todos = json!({
            "todos": [
                {
                    "content": "Test todo",
                    "activeForm": "Testing todo",
                    "status": "in_progress"
                }
            ]
        });

        let result = handler(todos, ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Test todo"));
        assert!(output.contains("in_progress"));

        // Clean up test file
        let path = todo_store_path();
        let _ = std::fs::remove_file(&path);
    }
}
