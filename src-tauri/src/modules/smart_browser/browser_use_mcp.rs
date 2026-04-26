//! browser-use MCP backend adapter.
//!
//! This module intentionally stays at the contract/mapping layer. The live
//! process lifecycle is still owned by `runtime::mcp_stdio::McpServerManager`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::modules::runtime::config::{
    ConfigSource, McpServerConfig, McpStdioServerConfig, ScopedMcpServerConfig,
};
use crate::modules::runtime::mcp_stdio::McpToolCallResult;

use super::contract::{
    SmartBrowserBackend, SmartBrowserCommand, SmartBrowserCommandKind, SmartBrowserEscalationState,
    SmartBrowserEvent, SmartBrowserEventKind, SmartBrowserObservation,
};

/// Canonical MCP server name used by If2Ai for browser-use.
pub const BROWSER_USE_MCP_SERVER_NAME: &str = "browser-use";

/// Core browser-use MCP tool names supported by FEAT-SB-003.
pub const CORE_BROWSER_USE_TOOLS: &[&str] = &[
    "browser_navigate",
    "browser_get_state",
    "browser_screenshot",
    "browser_click",
    "browser_type",
];

/// Build the managed stdio MCP config for browser-use.
#[must_use]
pub fn browser_use_mcp_server_config(
    env: BTreeMap<String, String>,
) -> (String, ScopedMcpServerConfig) {
    (
        BROWSER_USE_MCP_SERVER_NAME.to_string(),
        ScopedMcpServerConfig {
            scope: ConfigSource::User,
            config: McpServerConfig::Stdio(McpStdioServerConfig {
                command: "uvx".to_string(),
                args: vec!["browser-use[cli]".to_string(), "--mcp".to_string()],
                env,
            }),
        },
    )
}

/// Return the browser-use MCP tool name for a Smart Browser command.
#[must_use]
pub fn browser_use_tool_for_command(kind: SmartBrowserCommandKind) -> Option<&'static str> {
    Some(match kind {
        SmartBrowserCommandKind::Navigate => "browser_navigate",
        SmartBrowserCommandKind::State => "browser_get_state",
        SmartBrowserCommandKind::Screenshot => "browser_screenshot",
        SmartBrowserCommandKind::Click => "browser_click",
        SmartBrowserCommandKind::TypeText => "browser_type",
        _ => return None,
    })
}

/// Convert a Smart Browser command into browser-use MCP arguments.
#[must_use]
pub fn browser_use_arguments_for_command(command: &SmartBrowserCommand) -> Option<Value> {
    let input = &command.input;
    Some(match command.kind {
        SmartBrowserCommandKind::Navigate => json!({
            "url": input.get("url").cloned().unwrap_or(Value::Null),
            "new_tab": input.get("new_tab").cloned().unwrap_or(Value::Bool(false)),
        }),
        SmartBrowserCommandKind::State => json!({
            "include_screenshot": input
                .get("include_screenshot")
                .cloned()
                .unwrap_or(Value::Bool(false)),
        }),
        SmartBrowserCommandKind::Screenshot => json!({
            "full_page": input.get("full_page").cloned().unwrap_or(Value::Bool(false)),
        }),
        SmartBrowserCommandKind::Click => {
            if input.get("coordinate_x").is_some() || input.get("coordinate_y").is_some() {
                json!({
                    "coordinate_x": input.get("coordinate_x").cloned().unwrap_or(Value::Null),
                    "coordinate_y": input.get("coordinate_y").cloned().unwrap_or(Value::Null),
                })
            } else {
                json!({
                    "index": input
                        .get("index")
                        .or_else(|| input.get("ref"))
                        .cloned()
                        .unwrap_or(Value::Null),
                })
            }
        }
        SmartBrowserCommandKind::TypeText => json!({
            "index": input
                .get("index")
                .or_else(|| input.get("ref"))
                .cloned()
                .unwrap_or(Value::Null),
            "text": input.get("text").cloned().unwrap_or(Value::Null),
        }),
        _ => return None,
    })
}

/// Convert browser-use MCP output into a Smart Browser observation.
#[must_use]
pub fn observation_from_mcp_result(
    command: &SmartBrowserCommand,
    result: &McpToolCallResult,
) -> SmartBrowserObservation {
    let text = result
        .content
        .iter()
        .filter(|part| part.kind == "text")
        .filter_map(|part| part.data.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let screenshot = result
        .content
        .iter()
        .find(|part| part.kind == "image")
        .and_then(|part| part.data.get("data"))
        .and_then(Value::as_str)
        .map(str::to_string);

    SmartBrowserObservation {
        session_id: command.session_id.clone(),
        backend: SmartBrowserBackend::BrowserUseMcp,
        url: None,
        title: None,
        text_state: if text.is_empty() { None } else { Some(text) },
        screenshot_mime: screenshot.as_ref().map(|_| "image/png".to_string()),
        screenshot_base64: screenshot,
        risk_flags: Vec::new(),
        escalation_state: SmartBrowserEscalationState::None,
        raw_summary: result.structured_content.clone(),
    }
}

/// Build the projection event produced after a browser-use MCP result.
#[must_use]
pub fn projected_event_from_mcp_result(
    event_id: impl Into<String>,
    command: &SmartBrowserCommand,
    result: &McpToolCallResult,
    occurred_at_ms: u64,
) -> SmartBrowserEvent {
    SmartBrowserEvent {
        event_id: event_id.into(),
        session_id: command.session_id.clone(),
        backend: SmartBrowserBackend::BrowserUseMcp,
        kind: if result.is_error == Some(true) {
            SmartBrowserEventKind::Failed
        } else {
            SmartBrowserEventKind::Observed
        },
        command_kind: command.kind,
        occurred_at_ms,
        observation: Some(observation_from_mcp_result(command, result)),
        message: None,
    }
}

#[cfg(test)]
pub mod tests {
    use serde_json::json;

    use crate::modules::runtime::config::{ConfigSource, McpServerConfig};
    use crate::modules::runtime::mcp_stdio::{McpToolCallContent, McpToolCallResult};
    use crate::modules::smart_browser::contract::{
        SmartBrowserBackend, SmartBrowserCommand, SmartBrowserCommandKind, SmartBrowserSessionId,
    };

    use super::{
        browser_use_arguments_for_command, browser_use_mcp_server_config,
        browser_use_tool_for_command, projected_event_from_mcp_result,
    };

    #[test]
    fn builds_stdio_config() {
        let (name, config) = browser_use_mcp_server_config(Default::default());

        assert_eq!(name, "browser-use");
        assert_eq!(config.scope, ConfigSource::User);
        match config.config {
            McpServerConfig::Stdio(stdio) => {
                assert_eq!(stdio.command, "uvx");
                assert_eq!(
                    stdio.args,
                    vec!["browser-use[cli]".to_string(), "--mcp".to_string()]
                );
            }
            _ => panic!("browser-use must use stdio MCP"),
        }
    }

    #[test]
    fn maps_core_tools() {
        let command = SmartBrowserCommand {
            command_id: "cmd-1".to_string(),
            session_id: SmartBrowserSessionId::new("session-1"),
            backend: SmartBrowserBackend::BrowserUseMcp,
            kind: SmartBrowserCommandKind::Click,
            input: json!({"ref": 5}),
        };

        assert_eq!(
            browser_use_tool_for_command(SmartBrowserCommandKind::Navigate),
            Some("browser_navigate")
        );
        assert_eq!(
            browser_use_tool_for_command(SmartBrowserCommandKind::Evaluate),
            None
        );
        assert_eq!(
            browser_use_arguments_for_command(&command),
            Some(json!({"index": 5}))
        );
    }

    #[test]
    fn emits_projected_observations() {
        let command = SmartBrowserCommand {
            command_id: "cmd-2".to_string(),
            session_id: SmartBrowserSessionId::new("session-2"),
            backend: SmartBrowserBackend::BrowserUseMcp,
            kind: SmartBrowserCommandKind::State,
            input: json!({}),
        };
        let result = McpToolCallResult {
            content: vec![McpToolCallContent {
                kind: "text".to_string(),
                data: [("text".to_string(), json!("state text"))].into(),
            }],
            structured_content: Some(json!({"url":"https://example.com"})),
            is_error: Some(false),
            meta: None,
        };

        let event = projected_event_from_mcp_result("event-1", &command, &result, 99);
        let observation = event.observation.expect("observation");

        assert_eq!(event.backend, SmartBrowserBackend::BrowserUseMcp);
        assert_eq!(observation.text_state, Some("state text".to_string()));
        assert_eq!(
            observation.raw_summary,
            Some(json!({"url":"https://example.com"}))
        );
    }
}
