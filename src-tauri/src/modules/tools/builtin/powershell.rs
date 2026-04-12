//! PowerShell tool - execute PowerShell commands.
//!
//! Migrated from GlobalToolRegistry mvp_tool_specs PowerShell.

use std::sync::Arc;

use crate::modules::tools::{
    context::SharedToolContext,
    registry::{ToolEntry, ToolError, ToolHandler},
};

#[derive(serde::Deserialize)]
// Input schema declares optional timeout/description/background; handler currently only reads command.
#[allow(dead_code)]
struct PowerShellInput {
    command: String,
    timeout: Option<u32>,
    description: Option<String>,
    run_in_background: Option<bool>,
}

/// Creates the PowerShell tool entry.
#[must_use]
pub fn powershell_tool_entry() -> ToolEntry {
    let handler: ToolHandler = Arc::new(|args: serde_json::Value, _ctx: SharedToolContext| {
        Box::pin(async move {
            let input: PowerShellInput = serde_json::from_value(args.clone())
                .map_err(|e| ToolError::Handler(format!("Invalid input: {e}")))?;

            // PowerShell is primarily a Windows tool; on macOS/Linux try pwsh
            let cmd = if cfg!(windows) { "powershell" } else { "pwsh" };

            let output = std::process::Command::new(cmd)
                .arg("-Command")
                .arg(&input.command)
                .output();

            match output {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        Ok(stdout.to_string())
                    } else {
                        Err(ToolError::Handler(format!("PowerShell failed: {stderr}")))
                    }
                }
                Err(e) => Err(ToolError::Handler(format!(
                    "PowerShell not available: {e}. On macOS install pwsh via brew."
                ))),
            }
        })
    });

    ToolEntry {
        name: "PowerShell".to_string(),
        toolset: "admin".to_string(),
        description: "Execute a PowerShell command with optional timeout.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "PowerShell command to execute" },
                "timeout": { "type": "integer", "minimum": 1, "description": "Timeout in seconds" },
                "description": { "type": "string", "description": "Description of the command" },
                "run_in_background": { "type": "boolean", "description": "Run in background" }
            },
            "required": ["command"]
        }),
        max_result_size: Some(50 * 1024),
        timeout_secs: Some(30),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn powershell_tool_entry_has_correct_structure() {
        let entry = powershell_tool_entry();
        assert_eq!(entry.name, "PowerShell");
        assert_eq!(entry.toolset, "admin");
        assert!(!entry.disabled);
        assert_eq!(entry.max_result_size, Some(50 * 1024));
        assert_eq!(entry.timeout_secs, Some(30));
    }
}
