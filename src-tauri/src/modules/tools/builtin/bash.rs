//! Bash tool - executes shell commands with sandboxing
//!
//! Provides a safe way to execute bash commands with timeout and危险命令黑名单.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Dangerous commands that are blocked for security reasons.
#[allow(dead_code)]
const DANGEROUS_COMMANDS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "sudo su",
    "mkfs",
    "dd if=",
    ":(){:|:&};:", // fork bomb
];

/// Checks if a command contains dangerous patterns.
#[allow(dead_code)]
fn is_dangerous(command: &str) -> bool {
    let lower = command.to_lowercase();
    for dangerous in DANGEROUS_COMMANDS {
        if lower.contains(&dangerous.to_lowercase()) {
            return true;
        }
    }
    false
}

/// Creates the bash tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn bash_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(
        |args: serde_json::Value, context: crate::modules::tools::context::SharedToolContext| {
            Box::pin(async move {
                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: command".to_string())
                    })?
                    .to_string();

                if is_dangerous(&command) {
                    return Err(ToolError::Handler(
                        "command blocked: dangerous pattern detected".to_string(),
                    ));
                }

                // Extract workdir from context before async block (MutexGuard must not cross await)
                let workdir = {
                    let ctx = context.lock().map_err(|e| {
                        ToolError::Handler(format!("failed to lock context: {}", e))
                    })?;
                    ctx.workdir.clone()
                };

                let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);

                tracing::info!(
                    "[bash_tool] workdir={}, command={}",
                    workdir.display(),
                    command
                );

                let result = timeout(
                    Duration::from_secs(timeout_secs),
                    execute_bash_internal(&command, &workdir),
                )
                .await;

                match result {
                    Ok(Ok(output)) => Ok(output),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(ToolError::Handler("command timed out".to_string())),
                }
            })
        },
    );

    ToolEntry {
        name: "bash".to_string(),
        toolset: "terminal".to_string(),
        description: "Executes a bash command with optional timeout and sandbox protection"
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Timeout in seconds (default: 30, max: 300)",
                    "default": 30
                }
            },
            "required": ["command"]
        }),
        max_result_size: Some(1024 * 1024), // 1MB limit
        timeout_secs: Some(300),            // 5 min absolute max
        disabled: false,
        handler,
    }
}

/// Internal bash execution function.
#[allow(dead_code)]
async fn execute_bash_internal(
    command: &str,
    workdir: &std::path::Path,
) -> Result<String, ToolError> {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ToolError::Handler(format!("failed to spawn bash: {e}")))?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    if let Some(mut out) = child.stdout.take() {
        out.read_to_string(&mut stdout)
            .await
            .map_err(|e| ToolError::Handler(format!("failed to read stdout: {e}")))?;
    }

    if let Some(mut err) = child.stderr.take() {
        err.read_to_string(&mut stderr)
            .await
            .map_err(|e| ToolError::Handler(format!("failed to read stderr: {e}")))?;
    }

    let status = child
        .wait()
        .await
        .map_err(|e| ToolError::Handler(format!("failed to wait for bash: {e}")))?;

    let result = serde_json::json!({
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": status.code().unwrap_or(-1)
    });

    Ok(result.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::tools::context::{SharedToolContext, ToolContext};
    use serde_json::json;
    use std::env::temp_dir;
    use uuid::Uuid;

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
    async fn bash_tool_entry_has_correct_structure() {
        let entry = bash_tool_entry();
        assert_eq!(entry.name, "bash");
        assert_eq!(entry.toolset, "terminal");
        assert!(!entry.disabled);
        assert!(entry.timeout_secs.is_some());
    }

    #[tokio::test]
    async fn dangerous_commands_are_blocked() {
        let entry = bash_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(json!({"command": "rm -rf /"}), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn fork_bomb_is_blocked() {
        let entry = bash_tool_entry();
        let handler = entry.handler.clone();
        let ctx = test_context();

        let result = handler(json!({"command": ":(){:|:&};:"}), ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bash_runs_in_context_workdir() {
        let workdir = temp_dir().join(format!("if2ai bash {}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&workdir).await.unwrap();
        let ctx = std::sync::Arc::new(std::sync::Mutex::new(ToolContext::new(
            workdir.clone(),
            crate::modules::runtime::permissions::PermissionMode::WorkspaceWrite,
        )));

        let entry = bash_tool_entry();
        let result = (entry.handler)(json!({"command":"pwd"}), ctx)
            .await
            .unwrap();
        assert!(result.contains(&workdir.to_string_lossy().to_string()));

        let _ = tokio::fs::remove_dir_all(workdir).await;
    }
}
