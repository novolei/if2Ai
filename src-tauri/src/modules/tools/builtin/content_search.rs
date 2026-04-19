//! Content Search tool - searches file contents using regex
//!
//! Provides full-text content search across files within the workdir.

use std::path::PathBuf;
use std::sync::Arc;

use regex::Regex;
use tokio::sync::Mutex;
use walkdir::WalkDir;

use crate::modules::control_plane::BoundaryResolver;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Maximum results to return
#[allow(dead_code)]
const MAX_RESULTS: usize = 100;

/// Creates the content_search tool entry for the registry.
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

            let case_sensitive = args
                .get("case_sensitive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            // Extract workdir from context before async block
            let workdir = {
                let ctx = context
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("failed to lock context: {}", e)))?;
                ctx.workdir.clone()
            };

            let requested_base = path.unwrap_or_else(|| workdir.clone());
            let resolved_base = BoundaryResolver::resolve_user_path(&workdir, &requested_base);
            let canonical_workdir = BoundaryResolver::canonicalize_workdir(&workdir)?;
            let canonical_base = BoundaryResolver::canonicalize_existing(&resolved_base)?;
            BoundaryResolver::assert_within_workdir(&canonical_workdir, &canonical_base)?;

            // Compile regex
            let regex_pattern = if case_sensitive {
                format!("(?m){}", pattern)
            } else {
                format!("(?mi){}", pattern)
            };
            let re = Regex::new(&regex_pattern).map_err(|e| {
                ToolError::Handler(format!("invalid regex pattern '{}': {}", pattern, e))
            })?;

            let results: Mutex<Vec<String>> = Mutex::new(Vec::new());
            let count: Mutex<usize> = Mutex::new(0);

            for entry in WalkDir::new(&canonical_base)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let file_path = entry.path();

                // Only search files within workdir
                if let Ok(canonical_file) = file_path.canonicalize() {
                    if BoundaryResolver::assert_within_workdir(&canonical_workdir, &canonical_file)
                        .is_err()
                    {
                        continue;
                    }
                } else {
                    continue;
                }

                // Read and search file
                if let Ok(content) = tokio::fs::read_to_string(file_path).await {
                    for line in content.lines() {
                        if re.is_match(line) {
                            let mut cnt = count.lock().await;
                            if *cnt >= max_results {
                                break;
                            }
                            *cnt += 1;

                            let mut res = results.lock().await;
                            res.push(format!("{}:{}", file_path.display(), line));
                        }
                    }
                }
            }

            let res = results.lock().await;
            let output = res.join("\n");
            Ok(output)
        })
    });

    ToolEntry {
        name: "content_search".to_string(),
        toolset: "files".to_string(),
        description: "Search file contents using regex".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Base path to search in (default: workdir)"
                },
                "max_results": {
                    "type": "number",
                    "description": "Maximum number of results (default: 100)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Whether to match case sensitively (default: true)"
                }
            },
            "required": ["pattern"]
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
    use crate::modules::tools::context::ToolContext;
    use std::env::temp_dir;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[tokio::test]
    async fn content_search_tool_entry_has_correct_structure() {
        let entry = entry();
        assert_eq!(entry.name, "content_search");
        assert_eq!(entry.toolset, "files");
        assert!(!entry.disabled);
    }

    #[tokio::test]
    async fn content_search_rejects_outside_workdir() {
        let workdir = temp_dir().join(format!("if2ai_content_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&workdir).await.unwrap();
        let ctx = Arc::new(Mutex::new(ToolContext::new(
            workdir.clone(),
            crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
        )));

        let entry = entry();
        let result =
            (entry.handler)(serde_json::json!({ "pattern": "foo", "path": "/" }), ctx).await;
        assert!(result.is_err());
        assert!(result
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
            .contains("outside allowed workdir"));
        let _ = tokio::fs::remove_dir_all(&workdir).await;
    }
}
