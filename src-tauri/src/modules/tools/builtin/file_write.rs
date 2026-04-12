//! File Write tool - safely writes content to files
//!
//! Provides a safe way to write content to files with optional append mode.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Maximum file size: 1MB
#[allow(dead_code)]
const MAX_FILE_SIZE: usize = 1024 * 1024;

/// Creates the file_write tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, context: SharedToolContext| {
        Box::pin(async move {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("missing required parameter: path".to_string()))?
                .to_string();

            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::Handler("missing required parameter: content".to_string())
                })?
                .to_string();

            let append = args
                .get("append")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Extract workdir from context before async block
            let workdir = {
                let ctx = context
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("failed to lock context: {}", e)))?;
                ctx.workdir.clone()
            };

            // Allowlist check: path must be within workdir
            let requested_path = PathBuf::from(&path);
            let canonical_path = requested_path
                .canonicalize()
                .map_err(|e| ToolError::Handler(format!("invalid path '{}': {}", path, e)))?;

            let canonical_workdir = workdir.canonicalize().map_err(|e| {
                ToolError::Handler(format!("invalid workdir '{}': {}", workdir.display(), e))
            })?;

            if !canonical_path.starts_with(&canonical_workdir) {
                return Err(ToolError::Handler(format!(
                    "path '{}' is outside allowed workdir '{}'",
                    path,
                    workdir.display()
                )));
            }

            // Check content size
            if content.len() > MAX_FILE_SIZE {
                return Err(ToolError::Handler(format!(
                    "content size {} exceeds maximum {}",
                    content.len(),
                    MAX_FILE_SIZE
                )));
            }

            // Write file
            if append {
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&canonical_path)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to open file: {}", e)))?;
                file.write_all(content.as_bytes())
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to write file: {}", e)))?;
            } else {
                let mut file = fs::File::create(&canonical_path)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to create file: {}", e)))?;
                file.write_all(content.as_bytes())
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to write file: {}", e)))?;
            }

            Ok(format!("Successfully wrote to file: {}", path))
        })
    });

    ToolEntry {
        name: "file_write".to_string(),
        toolset: "files".to_string(),
        description: "Write content to a file".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                },
                "append": {
                    "type": "boolean",
                    "description": "Append to file instead of overwriting (default: false)"
                }
            },
            "required": ["path", "content"]
        }),
        max_result_size: Some(1024),
        timeout_secs: Some(30),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn test_context() -> SharedToolContext {
        std::sync::Arc::new(std::sync::Mutex::new(
            crate::modules::tools::context::ToolContext::default_for_workdir(
                std::path::PathBuf::from("."),
            ),
        ))
    }

    #[tokio::test]
    async fn file_write_tool_entry_has_correct_structure() {
        let entry = entry();
        assert_eq!(entry.name, "file_write");
        assert_eq!(entry.toolset, "files");
        assert!(!entry.disabled);
    }
}
