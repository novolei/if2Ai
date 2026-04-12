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
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _ctx: SharedToolContext| {
        Box::pin(async move {
            let input: ReplInput = serde_json::from_value(args.clone())
                .map_err(|e| ToolError::Handler(format!("Invalid input: {e}")))?;

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

    #[tokio::test]
    async fn repl_tool_entry_has_correct_structure() {
        let entry = repl_tool_entry();
        assert_eq!(entry.name, "REPL");
        assert_eq!(entry.toolset, "admin");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(50 * 1024));
        assert_eq!(entry.timeout_secs, Some(60));
    }
}
