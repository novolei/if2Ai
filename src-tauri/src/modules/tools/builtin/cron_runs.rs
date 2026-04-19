//! Cron Runs tool - retrieves run history for a cron job
//!
//! Provides functionality to view past executions of a cron job.

use std::sync::Arc;

use crate::modules::scheduler::{CronRun, SharedScheduler};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Default limit for run history
#[allow(dead_code)]
const DEFAULT_LIMIT: usize = 10;

/// Creates the cron_runs tool entry for the registry.
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

                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(DEFAULT_LIMIT);

                let runs = scheduler
                    .get_runs(&id, limit)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to get cron runs: {}", e)))?;

                let output: Vec<String> = runs
                    .iter()
                    .map(|r: &CronRun| {
                        let finished = r
                            .finished_at
                            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "running".to_string());
                        let status = if r.error.is_some() {
                            "ERROR"
                        } else if r.output.is_some() {
                            "OK"
                        } else {
                            "PENDING"
                        };
                        format!(
                            "[{}] {} - {}: {}",
                            r.job_id,
                            r.started_at.format("%Y-%m-%d %H:%M:%S"),
                            finished,
                            status
                        )
                    })
                    .collect();

                if output.is_empty() {
                    Ok("No runs recorded".to_string())
                } else {
                    Ok(output.join("\n"))
                }
            })
        },
    );

    ToolEntry {
        name: "cron_runs".to_string(),
        toolset: "scheduler".to_string(),
        description: "Get run history for a cron job".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "ID of the job"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of runs to return (default: 10)"
                }
            },
            "required": ["id"]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::scheduler::InMemoryScheduler;

    fn test_scheduler() -> SharedScheduler {
        Arc::new(InMemoryScheduler::new())
    }

    #[tokio::test]
    async fn cron_runs_tool_entry_has_correct_structure() {
        let entry = entry(test_scheduler());
        assert_eq!(entry.name, "cron_runs");
        assert_eq!(entry.toolset, "scheduler");
        assert!(!entry.disabled);
    }
}
