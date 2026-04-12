//! Cron List tool - lists all scheduled cron jobs
//!
//! Provides functionality to list all registered cron jobs.

use std::sync::Arc;

use crate::modules::scheduler::{CronJob, SharedScheduler};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the cron_list tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry(scheduler: SharedScheduler) -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        move |_args: serde_json::Value, _context: SharedToolContext| {
            let scheduler = scheduler.clone();
            Box::pin(async move {
                let jobs = scheduler
                    .list()
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to list cron jobs: {}", e)))?;

                let output: Vec<String> = jobs
                    .iter()
                    .map(|j: &CronJob| {
                        format!(
                            "[{}] {} - {} ({})",
                            j.id,
                            j.schedule,
                            j.description,
                            status_string(&j.status)
                        )
                    })
                    .collect();

                if output.is_empty() {
                    Ok("No cron jobs registered".to_string())
                } else {
                    Ok(output.join("\n"))
                }
            })
        },
    );

    ToolEntry {
        name: "cron_list".to_string(),
        toolset: "scheduler".to_string(),
        description: "List all scheduled cron jobs".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        max_result_size: Some(1024 * 1024),
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

fn status_string(status: &crate::modules::scheduler::JobStatus) -> &str {
    match status {
        crate::modules::scheduler::JobStatus::Active => "active",
        crate::modules::scheduler::JobStatus::Paused => "paused",
        crate::modules::scheduler::JobStatus::Disabled => "disabled",
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
    async fn cron_list_tool_entry_has_correct_structure() {
        let entry = entry(test_scheduler());
        assert_eq!(entry.name, "cron_list");
        assert_eq!(entry.toolset, "scheduler");
        assert!(!entry.disabled);
    }
}
