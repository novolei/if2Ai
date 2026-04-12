//! GrepSearch tool - search file contents with regex pattern.
//!
//! Migrated from GlobalToolRegistry mvp_tool_specs grep_search.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

#[derive(serde::Deserialize)]
// Input schema declares these fields but the handler only reads a subset;
// remaining fields are reserved for future implementation.
#[allow(dead_code)]
struct GrepSearchInput {
    pattern: String,
    path: Option<String>,
    glob: Option<String>,
    output_mode: Option<String>,
    context: Option<usize>,
    head_limit: Option<usize>,
}

/// Creates the GrepSearch tool entry.
#[must_use]
pub fn grep_search_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let input: GrepSearchInput = serde_json::from_value(args.clone())
                .map_err(|e| ToolError::Handler(format!("Invalid input: {e}")))?;

            let search_path = match input.path {
                Some(p) => p,
                None => {
                    let guard = ctx
                        .lock()
                        .map_err(|e| ToolError::Handler(format!("Context lock poisoned: {e}")))?;
                    guard.workdir.to_string_lossy().to_string()
                }
            };

            let mut cmd = std::process::Command::new("rg");
            cmd.arg("--no-heading")
                .arg("--color=never")
                .arg("--line-number");

            if let Some(limit) = input.head_limit {
                cmd.arg("--max-count").arg(limit.to_string());
            }
            if let Some(ctx_lines) = input.context {
                cmd.arg("-C").arg(ctx_lines.to_string());
            }
            if let Some(glob) = &input.glob {
                cmd.arg("--glob").arg(glob);
            }
            cmd.arg(&input.pattern).arg(&search_path);

            let output = cmd
                .output()
                .map_err(|e| ToolError::Handler(e.to_string()))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.is_empty() {
                Ok("No matches found.".to_string())
            } else {
                Ok(stdout.to_string())
            }
        })
    });

    ToolEntry {
        name: "grep_search".to_string(),
        toolset: "read".to_string(),
        description: "Search file contents with a regex pattern.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern to search for" },
                "path": { "type": "string", "description": "Directory to search in" },
                "glob": { "type": "string", "description": "Glob pattern to filter files" },
                "output_mode": { "type": "string", "enum": ["content", "files_with_matches", "count"] },
                "context": { "type": "integer", "minimum": 0, "description": "Context lines around match" },
                "head_limit": { "type": "integer", "minimum": 1, "description": "Max results to return" }
            },
            "required": ["pattern"]
        }),
        max_result_size: Some(50 * 1024),
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn grep_search_tool_entry_has_correct_structure() {
        let entry = grep_search_tool_entry();
        assert_eq!(entry.name, "grep_search");
        assert_eq!(entry.toolset, "read");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(50 * 1024));
        assert_eq!(entry.timeout_secs, Some(10));
    }
}
