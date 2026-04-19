//! Glob Search tool - searches for files matching glob patterns
//!
//! Provides file discovery using glob patterns within the workdir.

use std::path::PathBuf;
use std::sync::Arc;

use glob::glob;
use tokio::sync::Mutex;

use crate::modules::control_plane::BoundaryResolver;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Maximum results to return
#[allow(dead_code)]
const MAX_RESULTS: usize = 1000;

/// Creates the glob_search tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, context: SharedToolContext| {
        Box::pin(async move {
            let pattern = args
                .get("pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::Handler("missing required parameter: pattern".to_string())
                })?
                .to_string();

            let path = args.get("path").and_then(|v| v.as_str()).map(PathBuf::from);

            let max_results = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(MAX_RESULTS);

            // Extract workdir from context before async block
            let workdir = {
                let ctx = context
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("failed to lock context: {}", e)))?;
                ctx.workdir.clone()
            };

            // Use path as base or workdir, and enforce workspace boundary.
            let requested_base = path.unwrap_or_else(|| workdir.clone());
            let resolved_base = BoundaryResolver::resolve_user_path(&workdir, &requested_base);
            let canonical_workdir = BoundaryResolver::canonicalize_workdir(&workdir)?;
            let canonical_base = BoundaryResolver::canonicalize_existing(&resolved_base)?;
            BoundaryResolver::assert_within_workdir(&canonical_workdir, &canonical_base)?;

            // Build glob pattern
            let full_pattern = format!(
                "{}/{}",
                canonical_base.display(),
                pattern.trim_start_matches('/')
            );

            // Execute glob search
            let results: Mutex<Vec<String>> = Mutex::new(Vec::new());
            let count = Mutex::new(0);

            for entry in glob(&full_pattern).map_err(|e| {
                ToolError::Handler(format!("invalid glob pattern '{}': {}", pattern, e))
            })? {
                match entry {
                    Ok(path) => {
                        let canonical_match = match path.canonicalize() {
                            Ok(c) => c,
                            Err(_) => continue,
                        };
                        if BoundaryResolver::assert_within_workdir(
                            &canonical_workdir,
                            &canonical_match,
                        )
                        .is_err()
                        {
                            continue;
                        }
                        let mut cnt = count.lock().await;
                        if *cnt >= max_results {
                            break;
                        }
                        *cnt += 1;
                        let mut res = results.lock().await;
                        res.push(canonical_match.display().to_string());
                    }
                    Err(e) => {
                        // Skip entries that can't be accessed
                        tracing::warn!("glob entry error: {}", e);
                    }
                }
            }

            let res = results.lock().await;
            let output = res.join("\n");
            Ok(output)
        })
    });

    ToolEntry {
        name: "glob_search".to_string(),
        toolset: "files".to_string(),
        description: "Search for files matching a glob pattern".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g., **/*.rs)"
                },
                "path": {
                    "type": "string",
                    "description": "Base path to search from (default: workdir)"
                },
                "max_results": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 1000)"
                }
            },
            "required": ["pattern"]
        }),
        max_result_size: Some(1024 * 1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(30),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tools::context::ToolContext;
    use std::env::temp_dir;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[tokio::test]
    async fn glob_search_tool_entry_has_correct_structure() {
        let entry = entry();
        assert_eq!(entry.name, "glob_search");
        assert_eq!(entry.toolset, "files");
        assert!(!entry.disabled);
    }

    #[tokio::test]
    async fn glob_search_rejects_outside_workdir() {
        let workdir = temp_dir().join(format!("if2ai_glob_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&workdir).await.unwrap();
        let ctx = Arc::new(Mutex::new(ToolContext::new(
            workdir.clone(),
            crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
        )));

        let entry = entry();
        let result =
            (entry.handler)(serde_json::json!({ "pattern": "**/*", "path": "/" }), ctx).await;
        assert!(result.is_err());
        assert!(result
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
            .contains("outside allowed workdir"));
        let _ = tokio::fs::remove_dir_all(&workdir).await;
    }
}
