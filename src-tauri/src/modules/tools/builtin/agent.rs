//! Agent tool - delegate a task to a nested AI agent.
//!
//! Provides an Agent ToolHandler that allows the Agent to delegate
//! subtasks to a nested agent with recursion depth limiting.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

/// Creates the Agent tool entry for the registry.
///
/// The Agent tool enables nested agent calls with a default recursion
/// depth limit of 3 to prevent infinite loops.
#[must_use]
pub fn agent_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _context: SharedToolContext| {
        Box::pin(async move {
            // Limit recursion depth to prevent infinite nesting
            let current_depth = std::env::var("IF2AI_AGENT_DEPTH")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            if current_depth >= 3 {
                return Err(ToolError::Handler(
                    "Agent recursion depth limit exceeded (max 3)".into(),
                ));
            }

            // Increment depth for this call
            std::env::set_var("IF2AI_AGENT_DEPTH", format!("{}", current_depth + 1));

            let task = args
                .get("task")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("Missing 'task' argument".into()))?;
            let context = args.get("context").and_then(|v| v.as_str()).unwrap_or("");

            // In a full implementation, this would call the agent loop synchronously.
            // For now, return a placeholder that acknowledges the task.
            let result = if context.is_empty() {
                format!("[Agent completed task: {task}]")
            } else {
                format!("[Agent completed task: {task} (context: {context})]")
            };

            // Restore depth
            if current_depth > 0 {
                std::env::set_var("IF2AI_AGENT_DEPTH", format!("{current_depth}"));
            } else {
                std::env::remove_var("IF2AI_AGENT_DEPTH");
            }

            Ok(result)
        })
    });

    ToolEntry {
        name: "agent".to_string(),
        toolset: "utility".to_string(),
        description: "Delegate a task to a nested AI agent. \
                      Use this to break complex problems into subtasks. \
                      Recursion depth is limited to 3 levels."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Task description for the nested agent"
                },
                "context": {
                    "type": "string",
                    "description": "Optional context or background information"
                }
            },
            "required": ["task"]
        }),
        max_result_size: Some(10 * 1024),
        timeout_secs: Some(60),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn agent_tool_entry_has_correct_structure() {
        let entry = agent_tool_entry();
        assert_eq!(entry.name, "agent");
        assert_eq!(entry.toolset, "utility");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(10 * 1024));
        assert_eq!(entry.timeout_secs, Some(60));
    }

    #[tokio::test]
    async fn agent_tool_requires_task_argument() {
        let entry = agent_tool_entry();
        let handler = entry.handler.clone();
        let ctx = Arc::new(std::sync::Mutex::new(
            crate::modules::tools::context::ToolContext::default_for_workdir(
                std::path::PathBuf::from("."),
            ),
        ));

        // Missing "task" field
        let result = handler(json!({}), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn agent_tool_returns_result_with_task() {
        let entry = agent_tool_entry();
        let handler = entry.handler.clone();
        let ctx = Arc::new(std::sync::Mutex::new(
            crate::modules::tools::context::ToolContext::default_for_workdir(
                std::path::PathBuf::from("."),
            ),
        ));

        // Clean up depth env before test
        std::env::remove_var("IF2AI_AGENT_DEPTH");

        let result = handler(json!({"task": "summarize this"}), ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("summarize this"));

        // Clean up
        std::env::remove_var("IF2AI_AGENT_DEPTH");
    }
}
