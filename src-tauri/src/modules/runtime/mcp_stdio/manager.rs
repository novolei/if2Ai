//! MCP server manager: tracks managed stdio servers + their tool index +
//! exposes async discover/call APIs to higher layers.
//!
//! Extracted from `runtime/mcp_stdio/mod.rs` in GFR-T1-C-3 (pure
//! structural move; struct/enum fields and function bodies
//! byte-identical).

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::sync::{Mutex, OnceLock};

use serde_json::json;
use serde_json::Value as JsonValue;

use super::super::config::{McpTransport, RuntimeConfig, ScopedMcpServerConfig};
use super::super::mcp::mcp_tool_name;
use super::super::mcp_client::McpClientBootstrap;
use super::rpc::{JsonRpcError, JsonRpcId, JsonRpcResponse};
use super::types::{
    ManagedMcpPrompt, ManagedMcpResource, ManagedMcpTool, McpGetPromptParams, McpGetPromptResult,
    McpListPromptsParams, McpListResourcesParams, McpListToolsParams, McpReadResourceParams,
    McpReadResourceResult, McpToolCallParams, McpToolCallResult, McpWorkbenchActivityEntry,
    McpWorkbenchActivityStatus, McpWorkbenchDiscoveryDto, McpWorkbenchPromptDto,
    McpWorkbenchResourceDto, McpWorkbenchServerDto, McpWorkbenchToolDto, UnsupportedMcpServer,
};
use super::{default_initialize_params, spawn_mcp_stdio_process, McpStdioProcess};

const WORKBENCH_ACTIVITY_LIMIT: usize = 200;
const WORKBENCH_STRING_LIMIT: usize = 800;

static WORKBENCH_ACTIVITY: OnceLock<Mutex<VecDeque<McpWorkbenchActivityEntry>>> = OnceLock::new();

#[derive(Debug)]
pub enum McpServerManagerError {
    Io(io::Error),
    JsonRpc {
        server_name: String,
        method: &'static str,
        error: JsonRpcError,
    },
    InvalidResponse {
        server_name: String,
        method: &'static str,
        details: String,
    },
    UnknownTool {
        qualified_name: String,
    },
    UnknownServer {
        server_name: String,
    },
}

impl std::fmt::Display for McpServerManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::JsonRpc {
                server_name,
                method,
                error,
            } => write!(
                f,
                "MCP server `{server_name}` returned JSON-RPC error for {method}: {} ({})",
                error.message, error.code
            ),
            Self::InvalidResponse {
                server_name,
                method,
                details,
            } => write!(
                f,
                "MCP server `{server_name}` returned invalid response for {method}: {details}"
            ),
            Self::UnknownTool { qualified_name } => {
                write!(f, "unknown MCP tool `{qualified_name}`")
            }
            Self::UnknownServer { server_name } => write!(f, "unknown MCP server `{server_name}`"),
        }
    }
}

impl std::error::Error for McpServerManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::JsonRpc { .. }
            | Self::InvalidResponse { .. }
            | Self::UnknownTool { .. }
            | Self::UnknownServer { .. } => None,
        }
    }
}

impl From<io::Error> for McpServerManagerError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolRoute {
    server_name: String,
    raw_name: String,
}

#[derive(Debug)]
struct ManagedMcpServer {
    bootstrap: McpClientBootstrap,
    process: Option<McpStdioProcess>,
    initialized: bool,
}

impl ManagedMcpServer {
    fn new(bootstrap: McpClientBootstrap) -> Self {
        Self {
            bootstrap,
            process: None,
            initialized: false,
        }
    }
}

#[must_use]
pub fn sanitize_mcp_workbench_value(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(object) => {
            let mut sanitized = serde_json::Map::new();
            for (key, item) in object {
                if is_secret_key(key) {
                    sanitized.insert(key.clone(), JsonValue::String("[redacted]".to_string()));
                } else {
                    sanitized.insert(key.clone(), sanitize_mcp_workbench_value(item));
                }
            }
            JsonValue::Object(sanitized)
        }
        JsonValue::Array(items) => JsonValue::Array(
            items
                .iter()
                .map(sanitize_mcp_workbench_value)
                .collect::<Vec<_>>(),
        ),
        JsonValue::String(text) if text.len() > WORKBENCH_STRING_LIMIT => {
            let mut truncated = text
                .chars()
                .take(WORKBENCH_STRING_LIMIT)
                .collect::<String>();
            truncated.push_str("...");
            JsonValue::String(truncated)
        }
        other => other.clone(),
    }
}

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "authorization",
        "api_key",
        "apikey",
        "auth",
        "bearer",
        "credential",
        "key",
        "password",
        "secret",
        "token",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn record_mcp_workbench_activity(entry: McpWorkbenchActivityEntry) {
    let activity = WORKBENCH_ACTIVITY.get_or_init(|| Mutex::new(VecDeque::new()));
    if let Ok(mut entries) = activity.lock() {
        entries.push_back(entry);
        while entries.len() > WORKBENCH_ACTIVITY_LIMIT {
            let _ = entries.pop_front();
        }
    }
}

#[must_use]
pub fn mcp_workbench_activity_snapshot() -> Vec<McpWorkbenchActivityEntry> {
    let activity = WORKBENCH_ACTIVITY.get_or_init(|| Mutex::new(VecDeque::new()));
    activity
        .lock()
        .map(|entries| entries.iter().cloned().collect())
        .unwrap_or_default()
}

pub fn clear_mcp_workbench_activity_for_tests() {
    let activity = WORKBENCH_ACTIVITY.get_or_init(|| Mutex::new(VecDeque::new()));
    if let Ok(mut entries) = activity.lock() {
        entries.clear();
    }
}

#[must_use]
pub fn mcp_workbench_activity_entry(
    server_id: impl Into<String>,
    operation: impl Into<String>,
    target: Option<String>,
    status: McpWorkbenchActivityStatus,
    duration_ms: u64,
    params: Option<JsonValue>,
    result_summary: Option<JsonValue>,
    error: Option<String>,
) -> McpWorkbenchActivityEntry {
    McpWorkbenchActivityEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        server_id: server_id.into(),
        operation: operation.into(),
        target,
        status,
        duration_ms,
        params: params.as_ref().map(sanitize_mcp_workbench_value),
        result_summary: result_summary.as_ref().map(sanitize_mcp_workbench_value),
        error,
    }
}

#[must_use]
pub fn summarize_mcp_counts(tools: usize, resources: usize, prompts: usize) -> JsonValue {
    json!({
        "tools": tools,
        "resources": resources,
        "prompts": prompts,
    })
}

#[must_use]
pub fn mcp_workbench_transport_label(transport: McpTransport) -> String {
    match transport {
        McpTransport::Stdio => "stdio",
        McpTransport::Sse => "sse",
        McpTransport::Http => "http",
        McpTransport::Ws => "ws",
        McpTransport::Sdk => "sdk",
        McpTransport::ManagedProxy => "claudeai-proxy",
    }
    .to_string()
}

#[must_use]
pub fn mcp_workbench_unsupported_server_dtos(
    unsupported: &[UnsupportedMcpServer],
) -> Vec<McpWorkbenchServerDto> {
    unsupported
        .iter()
        .map(|server| McpWorkbenchServerDto {
            name: server.server_name.clone(),
            transport: mcp_workbench_transport_label(server.transport),
            scope: "effective".to_string(),
            active: false,
            reason: Some(server.reason.clone()),
        })
        .collect()
}

#[must_use]
pub fn mcp_workbench_discovery_dto(
    tools: Vec<ManagedMcpTool>,
    resources: Vec<ManagedMcpResource>,
    prompts: Vec<ManagedMcpPrompt>,
    unsupported_servers: Vec<McpWorkbenchServerDto>,
) -> McpWorkbenchDiscoveryDto {
    McpWorkbenchDiscoveryDto {
        tools: tools
            .into_iter()
            .map(|tool| McpWorkbenchToolDto {
                server_name: tool.server_name,
                qualified_name: tool.qualified_name,
                name: tool.raw_name,
                description: tool.tool.description,
                input_schema: tool.tool.input_schema,
            })
            .collect(),
        resources: resources
            .into_iter()
            .map(|resource| McpWorkbenchResourceDto {
                server_name: resource.server_name,
                uri: resource.resource.uri,
                name: resource.resource.name,
                description: resource.resource.description,
                mime_type: resource.resource.mime_type,
            })
            .collect(),
        prompts: prompts
            .into_iter()
            .map(|prompt| McpWorkbenchPromptDto {
                server_name: prompt.server_name,
                name: prompt.prompt.name,
                description: prompt.prompt.description,
                arguments: prompt.prompt.arguments,
            })
            .collect(),
        unsupported_servers,
    }
}

#[derive(Debug)]
pub struct McpServerManager {
    servers: BTreeMap<String, ManagedMcpServer>,
    unsupported_servers: Vec<UnsupportedMcpServer>,
    tool_index: BTreeMap<String, ToolRoute>,
    next_request_id: u64,
}

impl McpServerManager {
    #[must_use]
    pub fn from_runtime_config(config: &RuntimeConfig) -> Self {
        Self::from_servers(config.mcp().servers())
    }

    #[must_use]
    pub fn from_servers(servers: &BTreeMap<String, ScopedMcpServerConfig>) -> Self {
        let mut managed_servers = BTreeMap::new();
        let mut unsupported_servers = Vec::new();

        for (server_name, server_config) in servers {
            if server_config.transport() == McpTransport::Stdio {
                let bootstrap = McpClientBootstrap::from_scoped_config(server_name, server_config);
                managed_servers.insert(server_name.clone(), ManagedMcpServer::new(bootstrap));
            } else {
                unsupported_servers.push(UnsupportedMcpServer {
                    server_name: server_name.clone(),
                    transport: server_config.transport(),
                    reason: format!(
                        "transport {:?} is not supported by McpServerManager",
                        server_config.transport()
                    ),
                });
            }
        }

        Self {
            servers: managed_servers,
            unsupported_servers,
            tool_index: BTreeMap::new(),
            next_request_id: 1,
        }
    }

    #[must_use]
    pub fn unsupported_servers(&self) -> &[UnsupportedMcpServer] {
        &self.unsupported_servers
    }

    /// FEAT-SH-002 — liveness query for the daemon's MCP health check.
    ///
    /// Returns `true` iff the named server has been initialized AND its
    /// stdio child process has not exited. Returns `false` for unknown
    /// server names, never-initialized servers, and dead/exited
    /// processes.
    ///
    /// "Read-only" per Pack contract (I6 spirit): may reap a zombie
    /// child as OS-level housekeeping, but never signals/kills the
    /// process and never mutates MCP protocol state (initialized
    /// flag, request id counter, tool routes, server bootstrap). The
    /// `&mut self` is required only because [`tokio::process::Child::
    /// try_wait`] borrows mutably; semantically this is a query.
    #[must_use]
    pub fn is_server_process_alive(&mut self, server_name: &str) -> bool {
        let Some(server) = self.servers.get_mut(server_name) else {
            return false;
        };
        let Some(process) = server.process.as_mut() else {
            return false;
        };
        process.is_child_alive()
    }

    pub async fn discover_tools(&mut self) -> Result<Vec<ManagedMcpTool>, McpServerManagerError> {
        let server_names = self.servers.keys().cloned().collect::<Vec<_>>();
        let mut discovered_tools = Vec::new();

        for server_name in server_names {
            self.ensure_server_ready(&server_name).await?;
            self.clear_routes_for_server(&server_name);

            let mut cursor = None;
            loop {
                let request_id = self.take_request_id();
                let response = {
                    let server = self.server_mut(&server_name)?;
                    let process = server.process.as_mut().ok_or_else(|| {
                        McpServerManagerError::InvalidResponse {
                            server_name: server_name.clone(),
                            method: "tools/list",
                            details: "server process missing after initialization".to_string(),
                        }
                    })?;
                    process
                        .list_tools(
                            request_id,
                            Some(McpListToolsParams {
                                cursor: cursor.clone(),
                            }),
                        )
                        .await?
                };

                if let Some(error) = response.error {
                    return Err(McpServerManagerError::JsonRpc {
                        server_name: server_name.clone(),
                        method: "tools/list",
                        error,
                    });
                }

                let result =
                    response
                        .result
                        .ok_or_else(|| McpServerManagerError::InvalidResponse {
                            server_name: server_name.clone(),
                            method: "tools/list",
                            details: "missing result payload".to_string(),
                        })?;

                for tool in result.tools {
                    let qualified_name = mcp_tool_name(&server_name, &tool.name);
                    self.tool_index.insert(
                        qualified_name.clone(),
                        ToolRoute {
                            server_name: server_name.clone(),
                            raw_name: tool.name.clone(),
                        },
                    );
                    discovered_tools.push(ManagedMcpTool {
                        server_name: server_name.clone(),
                        qualified_name,
                        raw_name: tool.name.clone(),
                        tool,
                    });
                }

                match result.next_cursor {
                    Some(next_cursor) => cursor = Some(next_cursor),
                    None => break,
                }
            }
        }

        Ok(discovered_tools)
    }

    pub async fn call_tool(
        &mut self,
        qualified_tool_name: &str,
        arguments: Option<JsonValue>,
    ) -> Result<JsonRpcResponse<McpToolCallResult>, McpServerManagerError> {
        let route = self
            .tool_index
            .get(qualified_tool_name)
            .cloned()
            .ok_or_else(|| McpServerManagerError::UnknownTool {
                qualified_name: qualified_tool_name.to_string(),
            })?;

        self.ensure_server_ready(&route.server_name).await?;
        let request_id = self.take_request_id();
        let response =
            {
                let server = self.server_mut(&route.server_name)?;
                let process = server.process.as_mut().ok_or_else(|| {
                    McpServerManagerError::InvalidResponse {
                        server_name: route.server_name.clone(),
                        method: "tools/call",
                        details: "server process missing after initialization".to_string(),
                    }
                })?;
                process
                    .call_tool(
                        request_id,
                        McpToolCallParams {
                            name: route.raw_name,
                            arguments,
                            meta: None,
                        },
                    )
                    .await?
            };
        Ok(response)
    }

    pub async fn list_resources(
        &mut self,
    ) -> Result<Vec<ManagedMcpResource>, McpServerManagerError> {
        let server_names = self.servers.keys().cloned().collect::<Vec<_>>();
        let mut discovered_resources = Vec::new();

        for server_name in server_names {
            self.ensure_server_ready(&server_name).await?;
            let mut cursor = None;
            loop {
                let request_id = self.take_request_id();
                let response = {
                    let server = self.server_mut(&server_name)?;
                    let process = server.process.as_mut().ok_or_else(|| {
                        McpServerManagerError::InvalidResponse {
                            server_name: server_name.clone(),
                            method: "resources/list",
                            details: "server process missing after initialization".to_string(),
                        }
                    })?;
                    process
                        .list_resources(
                            request_id,
                            Some(McpListResourcesParams {
                                cursor: cursor.clone(),
                            }),
                        )
                        .await?
                };

                if let Some(error) = response.error {
                    return Err(McpServerManagerError::JsonRpc {
                        server_name: server_name.clone(),
                        method: "resources/list",
                        error,
                    });
                }
                let result =
                    response
                        .result
                        .ok_or_else(|| McpServerManagerError::InvalidResponse {
                            server_name: server_name.clone(),
                            method: "resources/list",
                            details: "missing result payload".to_string(),
                        })?;
                discovered_resources.extend(result.resources.into_iter().map(|resource| {
                    ManagedMcpResource {
                        server_name: server_name.clone(),
                        resource,
                    }
                }));
                match result.next_cursor {
                    Some(next_cursor) => cursor = Some(next_cursor),
                    None => break,
                }
            }
        }

        Ok(discovered_resources)
    }

    pub async fn read_resource(
        &mut self,
        server_name: &str,
        uri: &str,
    ) -> Result<JsonRpcResponse<McpReadResourceResult>, McpServerManagerError> {
        self.ensure_server_ready(server_name).await?;
        let request_id = self.take_request_id();
        let response =
            {
                let server = self.server_mut(server_name)?;
                let process = server.process.as_mut().ok_or_else(|| {
                    McpServerManagerError::InvalidResponse {
                        server_name: server_name.to_string(),
                        method: "resources/read",
                        details: "server process missing after initialization".to_string(),
                    }
                })?;
                process
                    .read_resource(
                        request_id,
                        McpReadResourceParams {
                            uri: uri.to_string(),
                        },
                    )
                    .await?
            };
        Ok(response)
    }

    pub async fn list_prompts(&mut self) -> Result<Vec<ManagedMcpPrompt>, McpServerManagerError> {
        let server_names = self.servers.keys().cloned().collect::<Vec<_>>();
        let mut discovered_prompts = Vec::new();

        for server_name in server_names {
            self.ensure_server_ready(&server_name).await?;
            let mut cursor = None;
            loop {
                let request_id = self.take_request_id();
                let response = {
                    let server = self.server_mut(&server_name)?;
                    let process = server.process.as_mut().ok_or_else(|| {
                        McpServerManagerError::InvalidResponse {
                            server_name: server_name.clone(),
                            method: "prompts/list",
                            details: "server process missing after initialization".to_string(),
                        }
                    })?;
                    process
                        .list_prompts(
                            request_id,
                            Some(McpListPromptsParams {
                                cursor: cursor.clone(),
                            }),
                        )
                        .await?
                };

                if let Some(error) = response.error {
                    return Err(McpServerManagerError::JsonRpc {
                        server_name: server_name.clone(),
                        method: "prompts/list",
                        error,
                    });
                }
                let result =
                    response
                        .result
                        .ok_or_else(|| McpServerManagerError::InvalidResponse {
                            server_name: server_name.clone(),
                            method: "prompts/list",
                            details: "missing result payload".to_string(),
                        })?;
                discovered_prompts.extend(result.prompts.into_iter().map(|prompt| {
                    ManagedMcpPrompt {
                        server_name: server_name.clone(),
                        prompt,
                    }
                }));
                match result.next_cursor {
                    Some(next_cursor) => cursor = Some(next_cursor),
                    None => break,
                }
            }
        }

        Ok(discovered_prompts)
    }

    pub async fn get_prompt(
        &mut self,
        server_name: &str,
        name: &str,
        arguments: Option<JsonValue>,
    ) -> Result<JsonRpcResponse<McpGetPromptResult>, McpServerManagerError> {
        self.ensure_server_ready(server_name).await?;
        let request_id = self.take_request_id();
        let response =
            {
                let server = self.server_mut(server_name)?;
                let process = server.process.as_mut().ok_or_else(|| {
                    McpServerManagerError::InvalidResponse {
                        server_name: server_name.to_string(),
                        method: "prompts/get",
                        details: "server process missing after initialization".to_string(),
                    }
                })?;
                process
                    .get_prompt(
                        request_id,
                        McpGetPromptParams {
                            name: name.to_string(),
                            arguments,
                        },
                    )
                    .await?
            };
        Ok(response)
    }

    /// Call a tool, discovering MCP tools first when the route index is empty.
    ///
    /// This keeps higher-level runtimes from needing to remember an ordering
    /// contract (`discover_tools` before `call_tool`) while still preserving
    /// the explicit `call_tool` API for callers that want strict routing.
    pub async fn call_tool_discovering(
        &mut self,
        qualified_tool_name: &str,
        arguments: Option<JsonValue>,
    ) -> Result<JsonRpcResponse<McpToolCallResult>, McpServerManagerError> {
        if !self.tool_index.contains_key(qualified_tool_name) {
            self.discover_tools().await?;
        }
        self.call_tool(qualified_tool_name, arguments).await
    }

    /// Return true when the manager has a route for `qualified_tool_name`.
    #[must_use]
    pub fn has_tool_route(&self, qualified_tool_name: &str) -> bool {
        self.tool_index.contains_key(qualified_tool_name)
    }

    pub async fn shutdown(&mut self) -> Result<(), McpServerManagerError> {
        let server_names = self.servers.keys().cloned().collect::<Vec<_>>();
        for server_name in server_names {
            let server = self.server_mut(&server_name)?;
            if let Some(process) = server.process.as_mut() {
                process.shutdown().await?;
            }
            server.process = None;
            server.initialized = false;
        }
        Ok(())
    }

    fn clear_routes_for_server(&mut self, server_name: &str) {
        self.tool_index
            .retain(|_, route| route.server_name != server_name);
    }

    fn server_mut(
        &mut self,
        server_name: &str,
    ) -> Result<&mut ManagedMcpServer, McpServerManagerError> {
        self.servers
            .get_mut(server_name)
            .ok_or_else(|| McpServerManagerError::UnknownServer {
                server_name: server_name.to_string(),
            })
    }

    fn take_request_id(&mut self) -> JsonRpcId {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        JsonRpcId::Number(id)
    }

    async fn ensure_server_ready(
        &mut self,
        server_name: &str,
    ) -> Result<(), McpServerManagerError> {
        let needs_spawn = self
            .servers
            .get(server_name)
            .map(|server| server.process.is_none())
            .ok_or_else(|| McpServerManagerError::UnknownServer {
                server_name: server_name.to_string(),
            })?;

        if needs_spawn {
            let server = self.server_mut(server_name)?;
            server.process = Some(spawn_mcp_stdio_process(&server.bootstrap)?);
            server.initialized = false;
        }

        let needs_initialize = self
            .servers
            .get(server_name)
            .map(|server| !server.initialized)
            .ok_or_else(|| McpServerManagerError::UnknownServer {
                server_name: server_name.to_string(),
            })?;

        if needs_initialize {
            let request_id = self.take_request_id();
            let response = {
                let server = self.server_mut(server_name)?;
                let process = server.process.as_mut().ok_or_else(|| {
                    McpServerManagerError::InvalidResponse {
                        server_name: server_name.to_string(),
                        method: "initialize",
                        details: "server process missing before initialize".to_string(),
                    }
                })?;
                process
                    .initialize(request_id, default_initialize_params())
                    .await?
            };

            if let Some(error) = response.error {
                return Err(McpServerManagerError::JsonRpc {
                    server_name: server_name.to_string(),
                    method: "initialize",
                    error,
                });
            }

            if response.result.is_none() {
                return Err(McpServerManagerError::InvalidResponse {
                    server_name: server_name.to_string(),
                    method: "initialize",
                    details: "missing result payload".to_string(),
                });
            }

            let server = self.server_mut(server_name)?;
            server.initialized = true;
        }

        Ok(())
    }
}
