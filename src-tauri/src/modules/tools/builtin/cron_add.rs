//! Cron Add tool - creates a scheduled cron job
//!
//! Provides functionality to add new scheduled tasks.

use std::sync::Arc;

use crate::modules::scheduler::SharedScheduler;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the cron_add tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry(scheduler: SharedScheduler) -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        move |args: serde_json::Value, _context: SharedToolContext| {
            let scheduler = scheduler.clone();
            Box::pin(async move {
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: id".to_string())
                    })?
                    .to_string();

                let schedule = args
                    .get("schedule")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: schedule".to_string())
                    })?
                    .to_string();

                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: command".to_string())
                    })?
                    .to_string();

                let description = args
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                scheduler
                    .add(&id, &schedule, &command, &description)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to add cron job: {}", e)))?;

                Ok(format!("Added cron job: {}", id))
            })
        },
    );

    ToolEntry {
        name: "cron_add".to_string(),
        toolset: "scheduler".to_string(),
        description: "Create a scheduled cron job".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Unique identifier for the job"
                },
                "schedule": {
                    "type": "string",
                    "description": "Cron expression (min hour day month weekday)"
                },
                "command": {
                    "type": "string",
                    "description": "Command to execute"
                },
                "description": {
                    "type": "string",
                    "description": "Description of the job"
                }
            },
            "required": ["id", "schedule", "command"]
        }),
        max_result_size: Some(1024),
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::scheduler::InMemoryScheduler;

    fn test_scheduler() -> SharedScheduler {
        Arc::new(InMemoryScheduler::new())
    }

    #[tokio::test]
    async fn cron_add_tool_entry_has_correct_structure() {
        let entry = entry(test_scheduler());
        assert_eq!(entry.name, "cron_add");
        assert_eq!(entry.toolset, "scheduler");
        assert!(!entry.disabled);
    }
}
