//! Cron Run tool - runs a cron job immediately
//!
//! Provides functionality to execute a cron job on demand.

use std::sync::Arc;

use crate::modules::scheduler::SharedScheduler;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the cron_run tool entry for the registry.
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

                let output = scheduler
                    .run_now(&id)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to run cron job: {}", e)))?;

                Ok(output)
            })
        },
    );

    ToolEntry {
        name: "cron_run".to_string(),
        toolset: "scheduler".to_string(),
        description: "Run a cron job immediately".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "ID of the job to run"
                }
            },
            "required": ["id"]
        }),
        max_result_size: Some(1024 * 1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(60),
        disabled: false,
        handler,
        multimodal_handler: None,
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
    async fn cron_run_tool_entry_has_correct_structure() {
        let entry = entry(test_scheduler());
        assert_eq!(entry.name, "cron_run");
        assert_eq!(entry.toolset, "scheduler");
        assert!(!entry.disabled);
    }
}
