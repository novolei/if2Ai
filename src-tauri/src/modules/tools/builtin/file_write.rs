//! File Write tool - safely writes content to files
//!
//! Provides a safe way to write content to files with optional append mode.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::modules::control_plane::BoundaryResolver;
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

            // Resolve relative paths against workdir and enforce workspace boundary.
            let requested_path = PathBuf::from(&path);
            let resolved_path = if requested_path.is_absolute() {
                requested_path
            } else {
                workdir.join(requested_path)
            };

            let canonical_workdir = BoundaryResolver::canonicalize_workdir(&workdir)?;
            let canonical_target =
                BoundaryResolver::canonicalize_with_missing_leaf_support(&resolved_path)?;
            BoundaryResolver::assert_within_workdir(&canonical_workdir, &canonical_target)?;

            // Check content size
            if content.len() > MAX_FILE_SIZE {
                return Err(ToolError::Handler(format!(
                    "content size {} exceeds maximum {}",
                    content.len(),
                    MAX_FILE_SIZE
                )));
            }

            if let Some(parent_dir) = resolved_path.parent() {
                fs::create_dir_all(parent_dir).await.map_err(|e| {
                    ToolError::Handler(format!(
                        "failed to create parent directory '{}': {}",
                        parent_dir.display(),
                        e
                    ))
                })?;
            }

            // Write file
            if append {
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&resolved_path)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to open file: {}", e)))?;
                file.write_all(content.as_bytes())
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to write file: {}", e)))?;
            } else {
                let mut file = fs::File::create(&resolved_path)
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
    use std::env::temp_dir;
    use uuid::Uuid;

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

    #[tokio::test]
    async fn file_write_creates_missing_parent_dirs_within_workdir() {
        let workdir = temp_dir().join(format!("if2ai_file_write_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&workdir)
            .await
            .expect("create temp workdir");

        let ctx = std::sync::Arc::new(std::sync::Mutex::new(
            crate::modules::tools::context::ToolContext::new(
                workdir.clone(),
                crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
            ),
        ));

        let entry = entry();
        let args = serde_json::json!({
            "path": "ai/hello.md",
            "content": "hello"
        });

        let result = (entry.handler)(args, ctx).await;
        assert!(result.is_ok(), "expected write success, got: {:?}", result);

        let written = tokio::fs::read_to_string(workdir.join("ai/hello.md"))
            .await
            .expect("read written file");
        assert_eq!(written, "hello");

        let _ = tokio::fs::remove_dir_all(&workdir).await;
    }

    #[tokio::test]
    async fn file_write_rejects_parent_dir_escape() {
        let workdir = temp_dir().join(format!("if2ai_file_write_escape_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&workdir)
            .await
            .expect("create temp workdir");

        let ctx = std::sync::Arc::new(std::sync::Mutex::new(
            crate::modules::tools::context::ToolContext::new(
                workdir.clone(),
                crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
            ),
        ));

        let entry = entry();
        let args = serde_json::json!({
            "path": "../escape.txt",
            "content": "blocked"
        });

        let result = (entry.handler)(args, ctx).await;
        assert!(result.is_err(), "expected path escape to fail");
        assert!(result
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
            .contains("outside allowed workdir"));

        let _ = tokio::fs::remove_dir_all(&workdir).await;
    }
}
