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

/// Cap on the previous-content snapshot embedded in the structured tool
/// result.  Files larger than this are diff'd as "all additions" on the
/// frontend so we don't blow up the event log with megabytes of base
/// content per write.  Tuned to comfortably cover typical source files
/// while still being two orders of magnitude smaller than `MAX_FILE_SIZE`.
const PREVIOUS_CONTENT_PREVIEW_BYTES: usize = 64 * 1024;

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

            // Snapshot existing content BEFORE the write (overwrite mode only)
            // so the frontend diff card can render an accurate `+N / -N`.
            // Append mode is by definition additive, so we skip the read to
            // avoid the extra IO and to keep the result payload small.
            //
            // Cap at PREVIOUS_CONTENT_PREVIEW_BYTES — anything larger gets
            // dropped from the result (frontend then falls back to "all
            // additions").  This mirrors `MAX_FILE_SIZE` constraints and
            // protects the event log from oversized JSON blobs.
            let mut previous_content: Option<String> = None;
            let mut previous_truncated = false;
            if !append && resolved_path.is_file() {
                if let Ok(text) = fs::read_to_string(&resolved_path).await {
                    if text.len() <= PREVIOUS_CONTENT_PREVIEW_BYTES {
                        previous_content = Some(text);
                    } else {
                        previous_truncated = true;
                    }
                }
            }

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

            // Structured JSON result so the frontend can render the diff
            // card with accurate `+N / -N`.  `kind: "file_write"` is the
            // discriminator the chat-ui summarizer keys on; legacy text-only
            // consumers can still read `message` for a human-readable line.
            let result = serde_json::json!({
                "kind": "file_write",
                "ok": true,
                "path": path,
                "appended": append,
                "wrote_bytes": content.len(),
                "previous_content": previous_content,
                "previous_truncated": previous_truncated,
                "message": format!("Successfully wrote to file: {}", path),
            });
            Ok(result.to_string())
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
        // Result is now a structured JSON payload that can embed up to
        // PREVIOUS_CONTENT_PREVIEW_BYTES (64 KiB) of pre-write content for
        // the diff card.  Cap doubled to comfortably hold the snapshot
        // plus metadata; oversize previews are dropped at the read site
        // (`previous_truncated: true`) so this ceiling is not load-bearing.
        max_result_size: Some(128 * 1024),
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
    async fn file_write_overwrite_includes_previous_content_snapshot() {
        let workdir = temp_dir().join(format!("if2ai_file_write_prev_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&workdir)
            .await
            .expect("create temp workdir");
        let target = workdir.join("notes.md");
        tokio::fs::write(&target, "old line 1\nold line 2\n")
            .await
            .expect("seed target");

        let ctx = std::sync::Arc::new(std::sync::Mutex::new(
            crate::modules::tools::context::ToolContext::new(
                workdir.clone(),
                crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
            ),
        ));

        let entry = entry();
        let args = serde_json::json!({
            "path": "notes.md",
            "content": "new line 1\nnew line 2\nnew line 3\n",
        });
        let raw = (entry.handler)(args, ctx).await.expect("write ok");
        let payload: serde_json::Value =
            serde_json::from_str(&raw).expect("result is structured JSON");

        assert_eq!(payload["kind"], "file_write");
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["appended"], false);
        assert_eq!(payload["previous_truncated"], false);
        assert_eq!(payload["previous_content"], "old line 1\nold line 2\n");
        assert_eq!(payload["path"], "notes.md");

        let _ = tokio::fs::remove_dir_all(&workdir).await;
    }

    #[tokio::test]
    async fn file_write_new_file_has_null_previous_content() {
        let workdir = temp_dir().join(format!("if2ai_file_write_new_{}", Uuid::new_v4()));
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
        let args = serde_json::json!({ "path": "fresh.txt", "content": "hi\n" });
        let raw = (entry.handler)(args, ctx).await.expect("write ok");
        let payload: serde_json::Value = serde_json::from_str(&raw).expect("structured JSON");

        assert!(
            payload["previous_content"].is_null(),
            "fresh file should have null previous_content, got {payload}"
        );
        assert_eq!(payload["previous_truncated"], false);

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
