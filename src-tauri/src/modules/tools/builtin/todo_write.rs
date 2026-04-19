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
/// Uses IF2AI_TODO_STORE env var, falling back to
/// `$IF2AI_HOME/todos/session-<session_id>.json` or
/// `$HOME/.if2ai/todos/session-<session_id>.json`.
fn todo_store_path(session_id: Option<&str>) -> std::path::PathBuf {
    let env_var = std::env::var("IF2AI_TODO_STORE").unwrap_or_default();
    if !env_var.is_empty() {
        return std::path::PathBuf::from(env_var);
    }

    let session_key = sanitize_session_key(session_id);
    let file_name = format!("session-{session_key}.json");

    if let Ok(if2ai_home) = std::env::var("IF2AI_HOME") {
        return std::path::PathBuf::from(if2ai_home)
            .join("todos")
            .join(file_name);
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".if2ai")
        .join("todos")
        .join(file_name)
}

fn sanitize_session_key(session_id: Option<&str>) -> String {
    let raw = session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("global");
    let mut normalized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if normalized.is_empty() {
        normalized = "global".to_string();
    }
    normalized
}

/// Creates the TodoWrite tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn todo_write_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        |args: serde_json::Value, context: crate::modules::tools::context::SharedToolContext| {
            Box::pin(async move {
                let input: TodoWriteInput = serde_json::from_value(args.clone())
                    .map_err(|e| ToolError::Handler(format!("Invalid TodoWrite input: {e}")))?;

                let session_id = {
                    let guard = context.lock().map_err(|e| {
                        ToolError::Handler(format!("Failed to lock tool context: {e}"))
                    })?;
                    guard.session_id.clone()
                };
                let result = execute_todo_write_internal(&input.todos, session_id.as_deref())
                    .map_err(ToolError::Handler)?;

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
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(30),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

/// Core todo persistence logic.
/// Reads old todos, writes new ones, returns both for comparison.
fn execute_todo_write_internal(
    todos: &Vec<TodoItem>,
    session_id: Option<&str>,
) -> Result<TodoWriteResult, String> {
    let store_path = todo_store_path(session_id);
    if let Some(parent) = store_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create todo store directory: {e}"))?;
    }

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
        let path = todo_store_path(None);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn todo_write_uses_session_scoped_store_path() {
        let entry = todo_write_tool_entry();
        let handler = entry.handler.clone();
        let ctx = Arc::new(std::sync::Mutex::new(ToolContext::new_with_session(
            Some("session:abc/123".to_string()),
            std::path::PathBuf::from("."),
            crate::modules::runtime::permissions::PermissionMode::DangerFullAccess,
        )));

        let todos = json!({
            "todos": [
                {
                    "content": "Session scoped todo",
                    "activeForm": "Persist per session",
                    "status": "pending"
                }
            ]
        });

        let result = handler(todos, ctx).await;
        assert!(result.is_ok());

        let path = todo_store_path(Some("session:abc/123"));
        assert!(path
            .to_string_lossy()
            .contains("session-session_abc_123.json"));
        let _ = std::fs::remove_file(&path);
    }
}
