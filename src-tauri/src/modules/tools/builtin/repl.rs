//! REPL tool - execute code in a REPL-like subprocess.
//!
//! Migrated from GlobalToolRegistry mvp_tool_specs REPL.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

#[derive(serde::Deserialize)]
// Input schema declares optional timeout_ms; handler currently uses a fixed default.
#[allow(dead_code)]
struct ReplInput {
    code: String,
    language: String,
    timeout_ms: Option<u64>,
}

/// Creates the REPL tool entry.
#[must_use]
pub fn repl_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, ctx: SharedToolContext| {
        Box::pin(async move {
            let input: ReplInput = serde_json::from_value(args.clone())
                .map_err(|e| ToolError::Handler(format!("Invalid input: {e}")))?;

            let workdir = {
                let guard = ctx
                    .lock()
                    .map_err(|e| ToolError::Handler(format!("failed to lock context: {e}")))?;
                guard.workdir.clone()
            };

            let (cmd, args_vec) = match input.language.as_str() {
                "python" | "py" => ("python3", vec!["-c", &input.code]),
                "javascript" | "js" | "node" => ("node", vec!["-e", &input.code]),
                "shell" | "sh" => ("sh", vec!["-c", &input.code]),
                _ => {
                    return Err(ToolError::Handler(format!(
                        "Unsupported language: {}. Supported: python, javascript, shell",
                        input.language
                    )))
                }
            };

            let _timeout_ms = input.timeout_ms.unwrap_or(60_000);
            let output = std::process::Command::new(cmd)
                .args(&args_vec)
                .current_dir(&workdir)
                .output()
                .map_err(|e| ToolError::Handler(format!("Failed to execute: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if output.status.success() {
                Ok(format!("{stdout}{stderr}"))
            } else {
                Err(ToolError::Handler(format!(
                    "REPL execution failed (exit {}):\n{stderr}",
                    output.status.code().unwrap_or(-1)
                )))
            }
        })
    });

    ToolEntry {
        name: "REPL".to_string(),
        toolset: "admin".to_string(),
        description: "Execute code in a REPL-like subprocess (Python, JavaScript, or shell)."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "Code to execute" },
                "language": { "type": "string", "description": "Language: python, javascript, or shell" },
                "timeout_ms": { "type": "integer", "minimum": 1, "description": "Execution timeout in ms" }
            },
            "required": ["code", "language"]
        }),
        max_result_size: Some(50 * 1024),
        timeout_secs: Some(60),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use uuid::Uuid;

    #[tokio::test]
    async fn repl_tool_entry_has_correct_structure() {
        let entry = repl_tool_entry();
        assert_eq!(entry.name, "REPL");
        assert_eq!(entry.toolset, "admin");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(50 * 1024));
        assert_eq!(entry.timeout_secs, Some(60));
    }

    #[tokio::test]
    async fn repl_runs_in_context_workdir() {
        let workdir = temp_dir().join(format!("if2ai_repl_{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&workdir)
            .await
            .expect("create temp workdir");

        let ctx = std::sync::Arc::new(std::sync::Mutex::new(
            crate::modules::tools::context::ToolContext::new(
                workdir.clone(),
                crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
            ),
        ));

        let entry = repl_tool_entry();
        let args = serde_json::json!({
            "language": "shell",
            "code": "pwd"
        });

        let result = (entry.handler)(args, ctx)
            .await
            .expect("repl should execute");

        assert!(
            result.contains(&workdir.to_string_lossy().to_string()),
            "expected pwd output to contain workdir, output: {}",
            result
        );

        let _ = tokio::fs::remove_dir_all(&workdir).await;
    }
}
