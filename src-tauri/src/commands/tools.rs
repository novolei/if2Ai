//! Tools command module - direct tool execution from frontend
//!
//! Provides Tauri commands for the frontend to directly invoke tools.

use crate::commands::AppState;
use crate::modules::control_plane::{
    AuditEmitter, SessionContextResolver, SessionExecutionContext, ToolExecutionBroker,
};
use crate::modules::runtime::permissions::PermissionOutcome;
use crate::modules::tools::{ToolSet, ToolSetRegistry};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Tool definition in OpenAI format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for input validation
    pub input_schema: serde_json::Value,
}

/// Tool call result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ToolCallResult {
    /// Whether the call succeeded
    pub success: bool,
    /// Tool output if successful
    pub output: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Execute a tool by name with JSON arguments.
///
/// This command allows the frontend to directly invoke tools
/// through the tool registry.
#[tauri::command]
pub async fn execute_tool(
    state: State<'_, AppState>,
    name: String,
    args: String,
    permission_mode: Option<String>,
    session_id: Option<String>,
) -> Result<String, String> {
    let args: serde_json::Value =
        serde_json::from_str(&args).map_err(|e| format!("invalid JSON args: {}", e))?;

    // Apply the same permission policy used by agent streaming path,
    // so direct execute_tool cannot bypass selected sandbox mode.
    let mode = super::agent::parse_permission_mode(permission_mode.as_deref());
    let permission_policy = super::agent::build_permission_policy(mode);
    let args_for_auth = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
    if session_id.is_none()
        && crate::modules::tools::registry::requires_explicit_context(name.as_str())
    {
        return Err(format!(
            "execute_tool for '{}' requires session_id to bind project workdir",
            name
        ));
    }

    let resolver =
        SessionContextResolver::new(state.session_manager.clone(), state.project_manager.clone());
    let broker = ToolExecutionBroker::new(state.tool_registry.clone());

    let execution_context = if let Some(session_id) = session_id {
        resolver
            .resolve(&session_id, mode, "execute_tool")
            .await
            .map_err(|e| e.to_string())?
    } else {
        let workdir = state
            .tool_registry
            .context()
            .lock()
            .map(|ctx| ctx.workdir.clone())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        SessionExecutionContext::stateless(workdir, mode)
    };
    let trace_id = AuditEmitter::new_trace_id();
    let request_id = format!("non_stream:{}", trace_id);
    match permission_policy.authorize(&name, &args_for_auth, None) {
        PermissionOutcome::Allow => AuditEmitter::policy_decision_made(
            &trace_id,
            &execution_context.session_id,
            &name,
            &execution_context.workdir,
            mode,
            "allow",
            Some(request_id.as_str()),
        ),
        PermissionOutcome::Deny { reason } => {
            AuditEmitter::policy_decision_made(
                &trace_id,
                &execution_context.session_id,
                &name,
                &execution_context.workdir,
                mode,
                &format!("deny:{reason}"),
                Some(request_id.as_str()),
            );
            return Err(format!("Permission denied: {reason}"));
        }
    }
    let context_fingerprint = crate::modules::tools::context::context_fingerprint(
        &execution_context.session_id,
        &execution_context.workdir,
    );
    let control_plane =
        crate::modules::runtime::config::ConfigLoader::default_for(&execution_context.workdir)
            .load()
            .map(|loaded| loaded.control_plane().clone())
            .unwrap_or_default();
    let control_plane_v2_enabled = std::env::var("IF2AI_CONTROL_PLANE_V2_ENABLED")
        .map(|value| value != "0")
        .unwrap_or_else(|_| control_plane.control_plane_v2_enabled());
    let boundary_enforce_mode = std::env::var("IF2AI_BOUNDARY_ENFORCE_MODE")
        .map(|value| {
            if value.eq_ignore_ascii_case("shadow") {
                "shadow"
            } else {
                "enforce"
            }
        })
        .unwrap_or(control_plane.boundary_enforce_mode().as_str());
    let sandbox_strict_mode = std::env::var("IF2AI_SANDBOX_STRICT_MODE")
        .map(|value| value != "0")
        .unwrap_or_else(|_| control_plane.sandbox_strict_mode());
    tracing::info!(
        "[execute_tool] context fingerprint='{}', session_id='{}', workdir='{}', tool='{}', control_plane_v2_enabled={}, boundary_enforce_mode={}, sandbox_strict_mode={}",
        context_fingerprint,
        execution_context.session_id,
        execution_context.workdir.display(),
        name,
        control_plane_v2_enabled,
        boundary_enforce_mode,
        sandbox_strict_mode
    );

    if control_plane_v2_enabled {
        broker
            .execute_with_trace(
                &execution_context,
                &name,
                args,
                &trace_id,
                Some(request_id.as_str()),
            )
            .await
            .map_err(|e| e.to_string())
    } else {
        tracing::warn!(
            "[execute_tool] controlPlaneV2Enabled=false, fallback to direct dispatch_with_context"
        );
        state
            .tool_registry
            .dispatch_with_context(&name, args, broker.to_tool_context(&execution_context))
            .await
            .map_err(|e| e.to_string())
    }
}

/// List all available tools.
///
/// Returns tool metadata for all registered tools.
#[tauri::command]
pub fn list_tools(state: State<'_, AppState>) -> Result<Vec<ToolDefinition>, String> {
    let entries = state.tool_registry.get_definitions(None);

    let tools: Vec<ToolDefinition> = entries
        .into_iter()
        .map(|def| {
            let obj = def.get("function").and_then(|f| f.as_object());
            ToolDefinition {
                name: obj
                    .and_then(|o| o.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: obj
                    .and_then(|o| o.get("description"))
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string(),
                input_schema: obj
                    .and_then(|o| o.get("parameters"))
                    .cloned()
                    .unwrap_or(serde_json::json!({})),
            }
        })
        .collect();

    Ok(tools)
}

/// Get tool definitions in OpenAI function calling format.
///
/// Optionally filter to only include tools in the `allowed` list.
#[tauri::command]
pub fn get_tool_definitions(
    state: State<'_, AppState>,
    allowed: Option<Vec<String>>,
) -> Result<Vec<serde_json::Value>, String> {
    let allowed_ref: Option<&[String]> = allowed.as_deref();
    Ok(state.tool_registry.get_definitions(allowed_ref))
}

/// List all available toolsets.
#[tauri::command]
pub fn list_toolsets(_state: State<'_, AppState>) -> Result<Vec<ToolSet>, String> {
    Ok(ToolSetRegistry::new().all_toolsets())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_serialization() {
        let def = ToolDefinition {
            name: "test".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "arg": {"type": "string"}
                }
            }),
        };

        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("A test tool"));
    }

    #[test]
    fn tool_call_result_success() {
        let result = ToolCallResult {
            success: true,
            output: Some("hello".to_string()),
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("hello"));
    }

    #[test]
    fn tool_call_result_error() {
        let result = ToolCallResult {
            success: false,
            output: None,
            error: Some("something went wrong".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("something went wrong"));
    }
}
