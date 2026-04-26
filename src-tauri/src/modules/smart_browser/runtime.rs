//! Smart Browser runtime orchestration.
//!
//! This is the thin bridge between the stable Smart Browser contract and the
//! existing tool/MCP runtimes. It keeps raw browser-use MCP tools behind the
//! first-party `browser` tool surface.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::modules::runtime::mcp::mcp_tool_name;
use crate::modules::runtime::mcp_stdio::{McpServerManager, McpServerManagerError};

use super::browser_use_mcp::{
    browser_use_arguments_for_command, browser_use_mcp_server_config, browser_use_tool_for_command,
    observation_from_mcp_result, projected_event_from_mcp_result, BROWSER_USE_MCP_SERVER_NAME,
};
use super::contract::{
    SmartBrowserBackend, SmartBrowserCommand, SmartBrowserCommandKind, SmartBrowserEvent,
    SmartBrowserSessionId,
};
use super::local_adapter::browser_tool_action_to_command_kind;
use super::policy::{resolve_backend_label, BackendPolicyError};

static BROWSER_USE_MCP_MANAGER: OnceLock<Arc<tokio::sync::Mutex<McpServerManager>>> =
    OnceLock::new();

/// Output from a browser-use MCP execution.
#[derive(Debug, Clone, PartialEq)]
pub struct SmartBrowserMcpExecution {
    pub event: SmartBrowserEvent,
    pub output: String,
}

/// Runtime failure while routing a Smart Browser command.
#[derive(Debug)]
pub enum SmartBrowserRuntimeError {
    Policy(BackendPolicyError),
    UnsupportedAction(String),
    UnsupportedMcpCommand(SmartBrowserCommandKind),
    MissingMcpArguments(SmartBrowserCommandKind),
    Mcp(McpServerManagerError),
    McpJsonRpc(String),
    McpToolError(String),
}

impl std::fmt::Display for SmartBrowserRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Policy(error) => write!(f, "{error}"),
            Self::UnsupportedAction(action) => {
                write!(f, "unsupported Smart Browser action: {action}")
            }
            Self::UnsupportedMcpCommand(kind) => {
                write!(
                    f,
                    "browser-use MCP does not support command '{}'",
                    kind.as_str()
                )
            }
            Self::MissingMcpArguments(kind) => {
                write!(
                    f,
                    "missing browser-use MCP arguments for '{}'",
                    kind.as_str()
                )
            }
            Self::Mcp(error) => write!(
                f,
                "browser-use MCP backend is unavailable or failed: {error}. \
                 Install browser-use/uvx or switch backend to local_rust_cdp."
            ),
            Self::McpJsonRpc(message) => write!(f, "browser-use MCP JSON-RPC error: {message}"),
            Self::McpToolError(message) => write!(f, "browser-use MCP tool failed: {message}"),
        }
    }
}

impl std::error::Error for SmartBrowserRuntimeError {}

impl From<BackendPolicyError> for SmartBrowserRuntimeError {
    fn from(value: BackendPolicyError) -> Self {
        Self::Policy(value)
    }
}

impl From<McpServerManagerError> for SmartBrowserRuntimeError {
    fn from(value: McpServerManagerError) -> Self {
        Self::Mcp(value)
    }
}

/// Resolve the requested backend from tool args and optional environment.
pub fn resolve_backend_for_args(
    args: &Value,
    env_backend: Option<&str>,
) -> Result<SmartBrowserBackend, SmartBrowserRuntimeError> {
    let requested = args
        .get("backend")
        .and_then(Value::as_str)
        .or(env_backend)
        .filter(|label| !label.trim().is_empty());
    Ok(resolve_backend_label(requested)?)
}

/// Return true when a tool invocation should go through browser-use MCP.
#[must_use]
pub fn should_route_to_browser_use_mcp(backend: SmartBrowserBackend) -> bool {
    backend == SmartBrowserBackend::BrowserUseMcp
}

/// Build the one-server map used by the managed MCP runtime.
#[must_use]
pub fn browser_use_mcp_servers(
    env: BTreeMap<String, String>,
) -> BTreeMap<String, crate::modules::runtime::config::ScopedMcpServerConfig> {
    let (name, config) = browser_use_mcp_server_config(env);
    [(name, config)].into()
}

/// Return the process-wide browser-use MCP manager.
///
/// browser-use owns browser/page state inside the MCP server process, so Smart
/// Browser must reuse one manager instead of spawning a fresh server per action.
#[must_use]
pub fn browser_use_mcp_manager() -> Arc<tokio::sync::Mutex<McpServerManager>> {
    Arc::clone(BROWSER_USE_MCP_MANAGER.get_or_init(|| {
        Arc::new(tokio::sync::Mutex::new(McpServerManager::from_servers(
            &browser_use_mcp_servers(BTreeMap::new()),
        )))
    }))
}

/// Execute a first-party browser action through browser-use MCP.
pub async fn execute_browser_use_mcp_action(
    session_id: &str,
    action: &str,
    args: Value,
) -> Result<SmartBrowserMcpExecution, SmartBrowserRuntimeError> {
    let kind = browser_tool_action_to_command_kind(action)
        .ok_or_else(|| SmartBrowserRuntimeError::UnsupportedAction(action.to_string()))?;
    let raw_tool = browser_use_tool_for_command(kind)
        .ok_or(SmartBrowserRuntimeError::UnsupportedMcpCommand(kind))?;
    let command = SmartBrowserCommand {
        command_id: format!("smart-browser-{}-{kind}", now_ms(), kind = kind.as_str()),
        session_id: SmartBrowserSessionId::new(session_id),
        backend: SmartBrowserBackend::BrowserUseMcp,
        kind,
        input: args,
    };
    let arguments = browser_use_arguments_for_command(&command)
        .ok_or(SmartBrowserRuntimeError::MissingMcpArguments(kind))?;
    let qualified_tool_name = mcp_tool_name(BROWSER_USE_MCP_SERVER_NAME, raw_tool);

    let manager = browser_use_mcp_manager();
    let mut manager = manager.lock().await;
    let response = manager
        .call_tool_discovering(&qualified_tool_name, Some(arguments))
        .await?;
    if let Some(error) = response.error {
        return Err(SmartBrowserRuntimeError::McpJsonRpc(format!(
            "{} ({})",
            error.message, error.code
        )));
    }
    let result = response
        .result
        .ok_or_else(|| SmartBrowserRuntimeError::McpJsonRpc("missing result".to_string()))?;
    let event = projected_event_from_mcp_result(
        format!("smart-browser-event-{}", now_ms()),
        &command,
        &result,
        now_ms(),
    );
    let observation = observation_from_mcp_result(&command, &result);
    let output = observation
        .text_state
        .or_else(|| observation.raw_summary.map(|value| value.to_string()))
        .unwrap_or_else(|| "browser-use MCP completed without textual output".to_string());

    if result.is_error == Some(true) {
        return Err(SmartBrowserRuntimeError::McpToolError(output));
    }

    Ok(SmartBrowserMcpExecution { event, output })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        browser_use_mcp_manager, browser_use_mcp_servers, resolve_backend_for_args,
        should_route_to_browser_use_mcp,
    };
    use crate::modules::smart_browser::contract::SmartBrowserBackend;

    #[test]
    fn selects_backend_from_args_or_env() {
        assert_eq!(
            resolve_backend_for_args(&json!({"backend":"browser_use_mcp"}), None).expect("backend"),
            SmartBrowserBackend::BrowserUseMcp
        );
        assert_eq!(
            resolve_backend_for_args(&json!({}), Some("browser_use_cloud")).expect("env backend"),
            SmartBrowserBackend::BrowserUseCloud
        );
    }

    #[test]
    fn registers_browser_use_mcp_server_without_exposing_raw_agent_tool() {
        let servers = browser_use_mcp_servers(Default::default());
        assert!(servers.contains_key("browser-use"));
        assert!(should_route_to_browser_use_mcp(
            SmartBrowserBackend::BrowserUseMcp
        ));
        assert!(!should_route_to_browser_use_mcp(
            SmartBrowserBackend::BrowserUseCloud
        ));
    }

    #[test]
    fn browser_use_mcp_manager_is_process_wide() {
        let first = browser_use_mcp_manager();
        let second = browser_use_mcp_manager();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
    }
}
