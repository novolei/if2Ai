//! File Read tool - safely reads file contents with size limits
//!
//! Provides a safe way to read file contents with configurable limits.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::modules::control_plane::BoundaryResolver;
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Maximum file size: 1MB
#[allow(dead_code)]
const MAX_FILE_SIZE: usize = 1024 * 1024;

/// Creates the file_read tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn file_read_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, context: SharedToolContext| {
        Box::pin(async move {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::Handler("missing required parameter: path".to_string()))?
                .to_string();

            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(MAX_FILE_SIZE as u64) as usize;

            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            // Extract workdir from context before async block (MutexGuard must not cross await)
            let workdir = {
                let ctx = context
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("failed to lock context: {}", e)))?;
                ctx.workdir.clone()
            };

            // Allowlist check: path must be within workdir
            let requested_path = PathBuf::from(&path);
            let resolved_path = BoundaryResolver::resolve_user_path(&workdir, &requested_path);
            let canonical_path = BoundaryResolver::canonicalize_existing(&resolved_path)?;
            let canonical_workdir = BoundaryResolver::canonicalize_workdir(&workdir)?;
            BoundaryResolver::assert_within_workdir(&canonical_workdir, &canonical_path)?;

            // Extra sensitive path check (denylist as additional protection)
            let sensitive_patterns = ["/etc/passwd", "/etc/shadow", "/.ssh/", "/.aws/"];
            let lower_path = path.to_lowercase();
            for pattern in &sensitive_patterns {
                if lower_path.contains(&pattern.to_lowercase()) {
                    return Err(ToolError::Handler(
                        "access denied: path contains sensitive pattern".to_string(),
                    ));
                }
            }

            read_file_internal(&canonical_path, offset, limit).await
        })
    });

    ToolEntry {
        name: "read_file".to_string(),
        toolset: "files".to_string(),
        description: "Reads file contents with optional offset and limit".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": "Byte offset to start reading from (default: 0)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum bytes to read (default: 1MB, max: 1MB)"
                }
            },
            "required": ["path"]
        }),
        max_result_size: Some(MAX_FILE_SIZE),
        timeout_secs: Some(30),
        disabled: false,
        handler,
    }
}

/// Internal file reading function.
#[allow(dead_code)]
async fn read_file_internal(
    path: &PathBuf,
    offset: usize,
    limit: usize,
) -> Result<String, ToolError> {
    // Check if file exists
    if !path.exists() {
        return Err(ToolError::Handler(format!(
            "file not found: {}",
            path.display()
        )));
    }

    // Check if it's a regular file
    if !path.is_file() {
        return Err(ToolError::Handler(format!(
            "not a regular file: {}",
            path.display()
        )));
    }

    // Get file metadata for size check
    let metadata = fs::metadata(path)
        .await
        .map_err(|e| ToolError::Handler(format!("failed to read metadata: {e}")))?;

    let file_size = metadata.len() as usize;

    // Check if offset is beyond file size
    if offset >= file_size {
        return Ok(String::new());
    }

    // Open file
    let mut file = fs::File::open(path)
        .await
        .map_err(|e| ToolError::Handler(format!("failed to open file: {e}")))?;

    // Seek to offset if specified
    if offset > 0 {
        tokio::io::AsyncSeekExt::seek(&mut file, tokio::io::SeekFrom::Start(offset as u64))
            .await
            .map_err(|e| ToolError::Handler(format!("failed to seek: {e}")))?;
    }

    // Limit read size
    let read_limit = limit
        .min(MAX_FILE_SIZE)
        .min(file_size.saturating_sub(offset));

    // Read file contents
    let mut buffer = vec![0u8; read_limit];
    let bytes_read = file
        .read(&mut buffer)
        .await
        .map_err(|e| ToolError::Handler(format!("failed to read file: {e}")))?;

    buffer.truncate(bytes_read);

    // Convert to string, handling potential encoding issues
    match String::from_utf8(buffer) {
        Ok(content) => Ok(content),
        Err(e) => Err(ToolError::Handler(format!("file is not valid UTF-8: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tools::context::{SharedToolContext, ToolContext};
    use std::fs;

    fn test_context() -> SharedToolContext {
        std::sync::Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
            std::path::PathBuf::from("."),
        )))
    }

    #[allow(dead_code)]
    fn make_test_handler(output: &'static str) -> ToolHandler {
        Arc::new(
            move |_input: serde_json::Value, _context: SharedToolContext| {
                let output = output.to_string();
                Box::pin(async move { Ok(output) })
            },
        )
    }

    #[tokio::test]
    async fn file_read_tool_entry_has_correct_structure() {
        let entry = file_read_tool_entry();
        assert_eq!(entry.name, "read_file");
        assert_eq!(entry.toolset, "files");
        assert!(!entry.disabled);
    }

    #[tokio::test]
    async fn sensitive_paths_are_blocked() {
        let entry = file_read_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(serde_json::json!({"path": "/etc/passwd"}), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_nonexistent_file_returns_error() {
        let entry = file_read_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(
            serde_json::json!({"path": "/nonexistent/file/path.txt"}),
            ctx,
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_actual_file_works() {
        // Create a temp file in current directory (workdir is ".")
        let temp_path = std::path::PathBuf::from(".")
            .canonicalize()
            .unwrap()
            .join("if2ai_test_read_file.txt");
        fs::write(&temp_path, "Hello, World!\nLine 2\n").unwrap();

        let entry = file_read_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(
            serde_json::json!({"path": temp_path.to_str().unwrap()}),
            ctx,
        )
        .await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Hello, World!"));

        // Cleanup
        let _ = fs::remove_file(&temp_path);
    }
}
