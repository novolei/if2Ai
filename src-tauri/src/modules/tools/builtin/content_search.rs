//! Content Search tool - searches file contents using regex
//!
//! Provides full-text content search across files within the workdir.

use std::path::PathBuf;
use std::sync::Arc;

use regex::Regex;
use tokio::sync::Mutex;
use walkdir::WalkDir;

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

            let base_path = path.unwrap_or_else(|| workdir.clone());

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

            for entry in WalkDir::new(&base_path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let file_path = entry.path();
                let canonical_base = base_path
                    .canonicalize()
                    .map_err(|e| ToolError::Handler(format!("invalid base path: {}", e)))?;

                // Only search files within workdir
                if let Ok(canonical_file) = file_path.canonicalize() {
                    if !canonical_file.starts_with(&canonical_base) {
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
        timeout_secs: Some(60),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn content_search_tool_entry_has_correct_structure() {
        let entry = entry();
        assert_eq!(entry.name, "content_search");
        assert_eq!(entry.toolset, "files");
        assert!(!entry.disabled);
    }
}
