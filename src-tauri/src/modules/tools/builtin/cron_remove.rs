//! Cron Remove tool - removes a scheduled cron job
//!
//! Provides functionality to remove existing cron jobs.

use std::sync::Arc;

use crate::modules::scheduler::SharedScheduler;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the cron_remove tool entry for the registry.
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

                scheduler
                    .remove(&id)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to remove cron job: {}", e)))?;

                Ok(format!("Removed cron job: {}", id))
            })
        },
    );

    ToolEntry {
        name: "cron_remove".to_string(),
        toolset: "scheduler".to_string(),
        description: "Remove a scheduled cron job".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "ID of the job to remove"
                }
            },
            "required": ["id"]
        }),
        max_result_size: Some(256),
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
    async fn cron_remove_tool_entry_has_correct_structure() {
        let entry = entry(test_scheduler());
        assert_eq!(entry.name, "cron_remove");
        assert_eq!(entry.toolset, "scheduler");
        assert!(!entry.disabled);
    }
}
