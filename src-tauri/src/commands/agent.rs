//! Agent commands - run_agent_turn and start_agent_stream
//!
//! Provides the main agent execution commands for Tauri.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, State};
use tokio::time::timeout;

use crate::commands::AppState;
use crate::modules::api::providers::claw_provider::AuthSource;
use crate::modules::api::providers::claw_provider::ClawApiClient;
use crate::modules::api::{InputContentBlock, InputMessage, MessageRequest, ToolDefinition};
use crate::modules::runtime::conversation::{
    ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError, ToolExecutor,
};
use crate::modules::runtime::permissions::{
    PermissionMode, PermissionPolicy, PermissionPromptDecision, PermissionPrompter,
    PermissionRequest,
};
use crate::modules::runtime::session::{ContentBlock, Session as RuntimeSession};
use crate::modules::session::Session as AppSession;

/// Event payload for streaming token updates
#[derive(serde::Serialize, Clone)]
struct StreamTokenPayload {
    stream_id: String,
    text: Option<String>,
    thinking: Option<String>,
    event_type: String,
    // tool_call_update fields
    tool_call_id: Option<String>,
    tool_name: Option<String>,
    tool_status: Option<String>, // "queued" | "running" | "completed" | "error"
    tool_args: Option<serde_json::Value>,
    tool_result: Option<String>,
    tool_duration_ms: Option<u64>,
}

/// Response from a run_agent_turn command.
#[derive(serde::Serialize)]
pub struct RunAgentTurnResponse {
    /// The generated message/response.
    pub message: String,
    /// The session ID.
    pub session_id: String,
    /// Thinking content from the model (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

/// Load LLM settings from ~/.claude/settings.json
fn load_llm_settings() -> Result<(String, String, String), String> {
    let settings_path =
        PathBuf::from(std::env::var("HOME").map_err(|_| "无法获取 HOME 目录".to_string())?)
            .join(".claude/settings.json");

    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("无法读取配置文件 ~/.claude/settings.json: {}", e))?;

    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("配置文件格式错误: {}", e))?;

    let env = json.get("env").ok_or("配置文件缺少 env 字段")?;

    let base_url = env
        .get("ANTHROPIC_BASE_URL")
        .and_then(|v| v.as_str())
        .ok_or("配置文件缺少 ANTHROPIC_BASE_URL")?
        .to_string();

    let auth_token = env
        .get("ANTHROPIC_AUTH_TOKEN")
        .and_then(|v| v.as_str())
        .ok_or("配置文件缺少 ANTHROPIC_AUTH_TOKEN")?
        .to_string();

    let model = env
        .get("ANTHROPIC_MODEL")
        .and_then(|v| v.as_str())
        .ok_or("配置文件缺少 ANTHROPIC_MODEL")?
        .to_string();

    if auth_token.is_empty() {
        return Err("ANTHROPIC_AUTH_TOKEN 为空，请检查配置文件".to_string());
    }

    Ok((base_url, auth_token, model))
}

/// Create a ClawApiClient using settings from ~/.claude/settings.json
fn create_claw_client_from_settings() -> Result<(ClawApiClient, String), String> {
    let (base_url, auth_token, model) = load_llm_settings()?;

    let auth = AuthSource::BearerToken(auth_token);
    let client = ClawApiClient::from_auth(auth).with_base_url(base_url);

    Ok((client, model))
}

/// Convert application session to runtime session.
///
/// The application session has extra metadata (id, title, etc.) that we don't need
/// for the runtime. We only need the messages.
fn app_session_to_runtime(app_session: &AppSession) -> RuntimeSession {
    RuntimeSession {
        version: 1,
        messages: app_session.messages.clone(),
    }
}

/// Real API client that calls the Claw API (Claude/MiniMax).
///
/// This implements the `ApiClient` trait and makes real LLM API calls.
struct RealApiClient {
    provider: ClawApiClient,
    model: String,
    tool_registry: Arc<crate::modules::tools::ToolRegistry>,
}

impl RealApiClient {
    fn new(
        provider: ClawApiClient,
        model: String,
        tool_registry: Arc<crate::modules::tools::ToolRegistry>,
    ) -> Self {
        Self {
            provider,
            model,
            tool_registry,
        }
    }
}

impl ApiClient for RealApiClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        // Use block_in_place to run async code in a blocking context
        // This allows us to call async functions from the sync stream method
        let result = tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(async move {
                let api_future = self.call_api(request);
                timeout(Duration::from_secs(30), api_future).await
            })
        });

        let response = result
            .map_err(|_| RuntimeError::ApiError("API call timed out after 30 seconds".to_string()))?
            .map_err(|e| RuntimeError::ApiError(e.to_string()))?;

        // Convert MessageResponse to Vec<AssistantEvent>
        let mut events = Vec::new();

        for block in &response.content {
            match block {
                crate::modules::api::OutputContentBlock::Text { text } => {
                    events.push(AssistantEvent::TextDelta(text.clone()));
                }
                crate::modules::api::OutputContentBlock::ToolUse { id, name, input } => {
                    events.push(AssistantEvent::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: serde_json::to_string(input).unwrap_or_default(),
                    });
                }
                crate::modules::api::OutputContentBlock::Thinking { thinking, .. } => {
                    events.push(AssistantEvent::Thinking(thinking.clone()));
                }
                crate::modules::api::OutputContentBlock::RedactedThinking { .. } => {
                    // Skip redacted thinking blocks - don't expose internal data
                }
            }
        }

        events.push(AssistantEvent::Usage(
            crate::modules::runtime::usage::TokenUsage {
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                cache_creation_input_tokens: response.usage.cache_creation_input_tokens,
                cache_read_input_tokens: response.usage.cache_read_input_tokens,
            },
        ));

        events.push(AssistantEvent::MessageStop);

        Ok(events)
    }
}

impl RealApiClient {
    /// Call the API asynchronously
    async fn call_api(
        &self,
        request: ApiRequest,
    ) -> Result<crate::modules::api::MessageResponse, crate::modules::api::ApiError> {
        // Convert ApiRequest to MessageRequest
        let messages: Vec<InputMessage> = request
            .messages
            .iter()
            .map(|msg| {
                let content: Vec<InputContentBlock> = msg
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => {
                            Some(InputContentBlock::Text { text: text.clone() })
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            let input_value: serde_json::Value =
                                serde_json::from_str(input).unwrap_or(serde_json::Value::Null);
                            Some(InputContentBlock::ToolUse {
                                id: id.clone(),
                                name: name.clone(),
                                input: input_value,
                            })
                        }
                        ContentBlock::ToolResult { .. } => None,
                    })
                    .collect();

                let role = match msg.role {
                    crate::modules::runtime::session::MessageRole::System => "system".to_string(),
                    crate::modules::runtime::session::MessageRole::User => "user".to_string(),
                    crate::modules::runtime::session::MessageRole::Assistant => {
                        "assistant".to_string()
                    }
                    crate::modules::runtime::session::MessageRole::Tool => "user".to_string(),
                };

                InputMessage { role, content }
            })
            .collect();

        let system_prompt = if request.system_prompt.is_empty() {
            None
        } else {
            Some(request.system_prompt.join("\n"))
        };

        // Prefer tool definitions from request.tools; fall back to registry
        let tool_defs: Option<Vec<ToolDefinition>> = request.tools.clone().or_else(|| {
            let definitions = self.tool_registry.get_definitions(None);
            Some(
                definitions
                    .into_iter()
                    .filter_map(|def| {
                        let obj = def.as_object()?;
                        let func = obj.get("function")?.as_object()?;
                        Some(ToolDefinition {
                            name: func.get("name")?.as_str()?.to_string(),
                            description: func
                                .get("description")
                                .and_then(|d| d.as_str())
                                .map(String::from),
                            input_schema: func.get("parameters")?.clone(),
                        })
                    })
                    .collect(),
            )
        });
        let tools = tool_defs.filter(|t| !t.is_empty());

        let api_request = MessageRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages,
            system: system_prompt,
            tools,
            tool_choice: None,
            stream: false,
        };

        // Call the provider's async send_message method
        self.provider.send_message(&api_request).await
    }
}

/// Bridge from async ToolRegistry to sync ToolExecutor trait.
///
/// This allows ConversationRuntime to use the ToolRegistry for tool calls.
struct ToolRegistryExecutor {
    tool_registry: Arc<crate::modules::tools::ToolRegistry>,
}

impl ToolRegistryExecutor {
    fn new(tool_registry: Arc<crate::modules::tools::ToolRegistry>) -> Self {
        Self { tool_registry }
    }
}

impl ToolExecutor for ToolRegistryExecutor {
    fn execute(
        &mut self,
        tool_name: &str,
        input: &str,
    ) -> Result<String, crate::modules::runtime::conversation::ToolError> {
        let args: serde_json::Value =
            serde_json::from_str(input).unwrap_or(serde_json::Value::Null);

        // Use block_in_place to run async code in a blocking context
        let result = tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(self.tool_registry.dispatch(tool_name, args))
        })
        .map_err(|e: crate::modules::tools::ToolError| {
            crate::modules::runtime::conversation::ToolError::new(e.to_string())
        })?;

        Ok(result)
    }

    fn get_definitions(&self) -> Vec<crate::modules::api::ToolDefinition> {
        let definitions = self.tool_registry.get_definitions(None);
        definitions
            .into_iter()
            .filter_map(|def| {
                let obj = def.as_object()?;
                let func = obj.get("function")?.as_object()?;
                Some(crate::modules::api::ToolDefinition {
                    name: func.get("name")?.as_str()?.to_string(),
                    description: func
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(String::from),
                    input_schema: func.get("parameters")?.clone(),
                })
            })
            .collect()
    }
}

/// Parse a permission_mode string into PermissionMode enum.
fn parse_permission_mode(mode: Option<&str>) -> PermissionMode {
    match mode {
        Some("readOnly") => PermissionMode::ReadOnly,
        Some("workspaceWrite") => PermissionMode::WorkspaceWrite,
        Some("prompt") => PermissionMode::Prompt,
        Some("dangerFullAccess") | None => PermissionMode::DangerFullAccess,
        _ => PermissionMode::DangerFullAccess,
    }
}

/// Run a single agent turn with the given user message.
///
/// This is the main entry point for the frontend to interact with the agent.
/// It calls the ConversationRuntime with the session and returns the result.
#[tauri::command]
#[allow(dead_code)]
pub async fn run_agent_turn(
    state: State<'_, AppState>,
    session_id: String,
    user_message: String,
    permission_mode: Option<String>,
) -> Result<RunAgentTurnResponse, String> {
    eprintln!(
        "[DEBUG] run_agent_turn called with session_id: {}, message: {}",
        session_id, user_message
    );
    tracing::info!(
        "[run_agent_turn] Starting - session_id: {}, message: {}",
        session_id,
        user_message
    );

    // Restore the session
    let app_session = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "[run_agent_turn] Session restored, {} messages",
        app_session.messages.len()
    );

    // Convert application session to runtime session
    let runtime_session = app_session_to_runtime(&app_session);

    // Create real API client using settings from ~/.claude/settings.json
    let (claw_client, model) = match create_claw_client_from_settings() {
        Ok((client, model)) => {
            tracing::info!("[run_agent_turn] API client created, model: {}", model);
            (client, model)
        }
        Err(e) => {
            tracing::error!("[run_agent_turn] Failed to create API client: {}", e);
            return Err(format!(
                "无法连接 AI 服务: {}. 请检查 ~/.claude/settings.json 配置是否正确。",
                e
            ));
        }
    };
    let api_client = RealApiClient::new(claw_client, model, state.tool_registry.clone());

    // Create tool executor bridge
    let tool_executor = ToolRegistryExecutor::new(state.tool_registry.clone());

    // Create permission policy from parameter (defaults to DangerFullAccess)
    let mode = parse_permission_mode(permission_mode.as_deref());
    let permission_policy = PermissionPolicy::new(mode);

    // System prompt — use SystemPromptBuilder for dynamic prompt
    let system_prompt_str = crate::modules::runtime::prompt::SystemPromptBuilder::new().render();
    let system_prompt = vec![system_prompt_str];

    // Create runtime
    let mut runtime = ConversationRuntime::new(
        runtime_session,
        api_client,
        tool_executor,
        permission_policy,
        system_prompt,
    );

    tracing::info!(
        "[run_agent_turn] Runtime created, calling run_turn with message: {}",
        user_message
    );

    // Run the conversation turn
    let result = runtime.run_turn(user_message.clone(), None);

    tracing::info!(
        "[run_agent_turn] run_turn completed, result: {:?}",
        result.is_ok()
    );

    match result {
        Ok(summary) => {
            // Extract text from assistant messages
            let response_text = summary
                .assistant_messages
                .iter()
                .filter_map(|msg| {
                    msg.blocks.iter().find_map(|block| {
                        if let ContentBlock::Text { text } = block {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                })
                .collect::<Vec<_>>()
                .join("\n");

            // Extract thinking content from assistant messages
            let thinking_content: Option<String> = {
                let collected: Vec<String> = summary
                    .assistant_messages
                    .iter()
                    .filter_map(|msg| msg.thinking.clone())
                    .collect();
                let joined = collected.join("\n\n");
                if joined.is_empty() {
                    None
                } else {
                    Some(joined)
                }
            };

            let final_text = if response_text.is_empty() {
                "Agent completed the request.".to_string()
            } else {
                response_text
            };

            // Get the updated session from the runtime
            let updated_runtime_session = runtime.into_session();

            // Update the application session with the new messages
            let mut updated_app_session = app_session;
            updated_app_session.messages = updated_runtime_session.messages;

            // Save the updated session
            state
                .session_manager
                .save_session(&updated_app_session)
                .await
                .map_err(|e| e.to_string())?;

            tracing::info!("[run_agent_turn] Returning response with message length: {}, thinking length: {:?}, session_id: {}", final_text.len(), thinking_content.as_ref().map(|s| s.len()), session_id);
            Ok(RunAgentTurnResponse {
                message: final_text,
                session_id,
                thinking: thinking_content,
            })
        }
        Err(e) => {
            // Return friendly error message
            let error_message = match e {
                RuntimeError::MaxIterationsExceeded => {
                    "对话达到最大迭代次数限制，请尝试简化您的问题。".to_string()
                }
                RuntimeError::ApiError(msg) => {
                    // Check for common network errors and provide friendly messages
                    if msg.contains("connection refused") {
                        "无法连接到 AI 服务服务器，请检查网络连接。".to_string()
                    } else if msg.contains("timeout") || msg.contains("timed out") {
                        "AI 服务响应超时，请稍后重试。".to_string()
                    } else if msg.contains("dns") || msg.contains("Name or service not known") {
                        "无法解析 AI 服务地址，请检查网络配置。".to_string()
                    } else if msg.contains("401")
                        || msg.contains("403")
                        || msg.contains("invalid signature")
                    {
                        "AI 服务认证失败，请检查 API 配置是否正确。".to_string()
                    } else if msg.contains("429") {
                        "AI 服务请求过于频繁，请稍后重试。".to_string()
                    } else if msg.contains("500") || msg.contains("502") || msg.contains("503") {
                        "AI 服务暂时不可用，请稍后重试。".to_string()
                    } else {
                        format!("AI 服务调用失败: {}. 请稍后重试。", msg)
                    }
                }
                RuntimeError::ToolError(msg) => {
                    format!("工具执行失败: {}. 请稍后重试。", msg)
                }
                RuntimeError::PermissionDenied(msg) => {
                    format!("权限被拒绝: {}. 请检查设置。", msg)
                }
                RuntimeError::SessionError(msg) => {
                    format!("会话错误: {}. 请刷新页面后重试。", msg)
                }
                RuntimeError::ConfigError(msg) => {
                    format!("配置错误: {}. 请检查设置。", msg)
                }
            };
            Err(error_message)
        }
    }
}

/// Start a streaming agent turn.
///
/// This command initiates a streaming response from the AI. It emits token events
/// via Tauri's event system which the frontend can listen to for progressive updates.
///
/// # Arguments
/// * `state` - Application state with session manager and tool registry
/// * `app_handle` - Tauri app handle for emitting events
/// * `session_id` - The session ID to continue
/// * `user_message` - The user's message
///
/// # Returns
/// A stream ID that the frontend uses to correlate events
#[tauri::command]
pub async fn start_agent_stream(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    session_id: String,
    user_message: String,
    permission_mode: Option<String>,
) -> Result<String, String> {
    use crate::modules::api::StreamEvent as ApiStreamEvent;
    use tauri::Manager;

    let stream_id = uuid::Uuid::new_v4().to_string();
    eprintln!(
        "[DEBUG] start_agent_stream called - stream_id: {}, session_id: {}, message: {}",
        stream_id, session_id, user_message
    );
    tracing::info!(
        "[start_agent_stream] Starting - stream_id: {}, session_id: {}",
        stream_id,
        session_id
    );

    // Get the main window for emitting events
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "Failed to get main window".to_string())?;

    // Restore the session
    let app_session = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "[start_agent_stream] Session restored, {} messages",
        app_session.messages.len()
    );

    // Create API client
    let (claw_client, model) = create_claw_client_from_settings().map_err(|e| {
        tracing::error!("[start_agent_stream] Failed to create API client: {}", e);
        e
    })?;

    // Convert session messages to API format
    let runtime_session = app_session_to_runtime(&app_session);
    let messages: Vec<InputMessage> = runtime_session
        .messages
        .iter()
        .map(|msg| {
            let content: Vec<InputContentBlock> = msg
                .blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => {
                        Some(InputContentBlock::Text { text: text.clone() })
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        let input_value: serde_json::Value =
                            serde_json::from_str(input).unwrap_or(serde_json::Value::Null);
                        Some(InputContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input_value,
                        })
                    }
                    ContentBlock::ToolResult { .. } => None,
                })
                .collect();

            let role = match msg.role {
                crate::modules::runtime::session::MessageRole::System => "system".to_string(),
                crate::modules::runtime::session::MessageRole::User => "user".to_string(),
                crate::modules::runtime::session::MessageRole::Assistant => "assistant".to_string(),
                crate::modules::runtime::session::MessageRole::Tool => "user".to_string(),
            };

            InputMessage { role, content }
        })
        .collect();

    // Add the user's new message
    let mut all_messages = messages;
    all_messages.push(InputMessage::user_text(&user_message));

    // Get tool definitions from registry and convert to ToolDefinition format
    let definitions = state.tool_registry.get_definitions(None);
    let tool_defs: Vec<crate::modules::api::ToolDefinition> = definitions
        .into_iter()
        .filter_map(|def| {
            let obj = def.as_object()?;
            let func = obj.get("function")?.as_object()?;
            Some(crate::modules::api::ToolDefinition {
                name: func.get("name")?.as_str()?.to_string(),
                description: func
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from),
                input_schema: func.get("parameters")?.clone(),
            })
        })
        .collect();

    // Build system prompt using SystemPromptBuilder
    let system_prompt = crate::modules::runtime::prompt::SystemPromptBuilder::new().render();

    // Clone everything needed for the background task
    let session_manager = state.session_manager.clone();
    let app_session_clone = app_session.clone();
    let user_message_clone = user_message.clone();
    let tool_registry_clone = state.tool_registry.clone();
    let model_for_stream = model.clone();
    let messages_for_stream = all_messages.clone();
    let tool_defs_for_stream = tool_defs.clone();
    let system_prompt_for_stream = system_prompt.clone();
    let permission_mode_for_stream = permission_mode.clone();
    let permission_senders = state.permission_senders.clone();

    // Spawn a background task to process the stream
    let stream_id_for_task = stream_id.clone();
    let stream_id_return = stream_id.clone();
    tokio::spawn(async move {
        tracing::info!(
            "[start_agent_stream] Spawned background task for stream_id: {}",
            stream_id_for_task
        );

        let max_iterations: usize = 10;
        let mut tool_loop_iter: usize = 0;
        let mut session_messages = messages_for_stream.clone();
        let mut accumulated_text = String::new();
        let mut accumulated_thinking = String::new();
        // Session-format tool result messages (for persistence)
        let mut tool_result_session_messages: Vec<
            crate::modules::runtime::session::ConversationMessage,
        > = Vec::new();

        loop {
            if tool_loop_iter >= max_iterations {
                tracing::warn!(
                    "[start_agent_stream] Tool loop exceeded max_iterations={}",
                    max_iterations
                );
                break;
            }
            tool_loop_iter += 1;

            // Build API request for this iteration
            let iter_api_request = MessageRequest {
                model: model_for_stream.clone(),
                max_tokens: 4096,
                messages: session_messages.clone(),
                system: if system_prompt_for_stream.is_empty() {
                    None
                } else {
                    Some(system_prompt_for_stream.clone())
                },
                tools: if tool_defs_for_stream.is_empty() {
                    None
                } else {
                    Some(tool_defs_for_stream.clone())
                },
                tool_choice: None,
                stream: true,
            };

            let mut stream = match claw_client.stream_message(&iter_api_request).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(
                        "[start_agent_stream] Background task failed to start stream: {}",
                        e
                    );
                    let payload = StreamTokenPayload {
                        stream_id: stream_id_for_task.clone(),
                        text: None,
                        thinking: None,
                        event_type: "stream_error".to_string(),
                        tool_call_id: None,
                        tool_name: None,
                        tool_status: None,
                        tool_args: None,
                        tool_result: None,
                        tool_duration_ms: None,
                    };
                    let _ = window.emit("agent-token", payload);
                    return;
                }
            };

            // Tool call tracking for this iteration — uses block index to support
            // parallel tool calls (each tool_call has its own index in the stream).
            let mut tool_arguments: HashMap<String, String> = HashMap::new();
            let mut index_to_tool_id: HashMap<u32, String> = HashMap::new();
            let mut index_to_tool_name: HashMap<u32, String> = HashMap::new();
            let mut pending_tool_uses: Vec<(String, String, String)> = Vec::new();

            loop {
                match stream.next_event().await {
                    Ok(Some(event)) => match event {
                        ApiStreamEvent::ContentBlockDelta(delta_event) => match delta_event.delta {
                            crate::modules::api::ContentBlockDelta::TextDelta { text } => {
                                accumulated_text.push_str(&text);
                                let payload = StreamTokenPayload {
                                    stream_id: stream_id_for_task.clone(),
                                    text: Some(text),
                                    thinking: None,
                                    event_type: "text_delta".to_string(),
                                    tool_call_id: None,
                                    tool_name: None,
                                    tool_status: None,
                                    tool_args: None,
                                    tool_result: None,
                                    tool_duration_ms: None,
                                };
                                let _ = window.emit("agent-token", payload);
                            }
                            crate::modules::api::ContentBlockDelta::ThinkingDelta { thinking } => {
                                accumulated_thinking.push_str(&thinking);
                                let payload = StreamTokenPayload {
                                    stream_id: stream_id_for_task.clone(),
                                    text: None,
                                    thinking: Some(thinking),
                                    event_type: "thinking_delta".to_string(),
                                    tool_call_id: None,
                                    tool_name: None,
                                    tool_status: None,
                                    tool_args: None,
                                    tool_result: None,
                                    tool_duration_ms: None,
                                };
                                let _ = window.emit("agent-token", payload);
                            }
                            crate::modules::api::ContentBlockDelta::SignatureDelta { .. } => {}
                            crate::modules::api::ContentBlockDelta::InputJsonDelta {
                                partial_json,
                            } => {
                                // Route delta to the correct tool_call via block index.
                                if let Some(tool_id) =
                                    index_to_tool_id.get(&delta_event.index).cloned()
                                {
                                    tool_arguments
                                        .entry(tool_id)
                                        .or_default()
                                        .push_str(&partial_json);
                                }
                            }
                        },
                        ApiStreamEvent::ContentBlockStop(stop_event) => {
                            // Extract completed tool_call as its block ends
                            if let Some(tool_id) = index_to_tool_id.remove(&stop_event.index) {
                                let tool_name = index_to_tool_name
                                    .remove(&stop_event.index)
                                    .unwrap_or_default();
                                if let Some(input_json) = tool_arguments.remove(&tool_id) {
                                    pending_tool_uses.push((tool_id, tool_name, input_json));
                                }
                            }
                        }
                        ApiStreamEvent::MessageStop(_) => {
                            // Extract any remaining tools (fallback — should already
                            // have been caught by ContentBlockStop above)
                            for (index, tool_id) in index_to_tool_id.drain() {
                                let tool_name =
                                    index_to_tool_name.remove(&index).unwrap_or_default();
                                if let Some(input_json) = tool_arguments.remove(&tool_id) {
                                    pending_tool_uses.push((tool_id, tool_name, input_json));
                                }
                            }

                            let payload = StreamTokenPayload {
                                stream_id: stream_id_for_task.clone(),
                                text: None,
                                thinking: None,
                                event_type: "stream_complete".to_string(),
                                tool_call_id: None,
                                tool_name: None,
                                tool_status: None,
                                tool_args: None,
                                tool_result: None,
                                tool_duration_ms: None,
                            };
                            let _ = window.emit("agent-token", payload);
                            break;
                        }
                        ApiStreamEvent::ContentBlockStart(start_event) => {
                            match start_event.content_block {
                                crate::modules::api::OutputContentBlock::Thinking { .. } => {
                                    let payload = StreamTokenPayload {
                                        stream_id: stream_id_for_task.clone(),
                                        text: None,
                                        thinking: None,
                                        event_type: "thinking_start".to_string(),
                                        tool_call_id: None,
                                        tool_name: None,
                                        tool_status: None,
                                        tool_args: None,
                                        tool_result: None,
                                        tool_duration_ms: None,
                                    };
                                    let _ = window.emit("agent-token", payload);
                                }
                                crate::modules::api::OutputContentBlock::ToolUse {
                                    id,
                                    name,
                                    ..
                                } => {
                                    // Track by block index to support parallel tool calls
                                    index_to_tool_id.insert(start_event.index, id.clone());
                                    index_to_tool_name.insert(start_event.index, name.clone());
                                    let payload = StreamTokenPayload {
                                        stream_id: stream_id_for_task.clone(),
                                        text: None,
                                        thinking: None,
                                        event_type: "tool_call_update".to_string(),
                                        tool_call_id: Some(id.clone()),
                                        tool_name: Some(name.clone()),
                                        tool_status: Some("queued".to_string()),
                                        tool_args: None,
                                        tool_result: None,
                                        tool_duration_ms: None,
                                    };
                                    let _ = window.emit("agent-token", payload);
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    },
                    Ok(None) => {
                        // Extract any remaining tools (same as MessageStop fallback)
                        for (index, tool_id) in index_to_tool_id.drain() {
                            let tool_name = index_to_tool_name.remove(&index).unwrap_or_default();
                            if let Some(input_json) = tool_arguments.remove(&tool_id) {
                                pending_tool_uses.push((tool_id, tool_name, input_json));
                            }
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::error!("[start_agent_stream] Background task stream error: {}", e);
                        let payload = StreamTokenPayload {
                            stream_id: stream_id_for_task.clone(),
                            text: None,
                            thinking: None,
                            event_type: "stream_error".to_string(),
                            tool_call_id: None,
                            tool_name: None,
                            tool_status: None,
                            tool_args: None,
                            tool_result: None,
                            tool_duration_ms: None,
                        };
                        let _ = window.emit("agent-token", payload);
                        break;
                    }
                }
            }

            // If no tool calls, exit the outer loop
            if pending_tool_uses.is_empty() {
                break;
            }

            // Execute each tool and append results to session_messages
            let mut tool_executor =
                crate::commands::agent::ToolRegistryExecutor::new(tool_registry_clone.clone());

            // Permission policy from stream parameter
            let mode = parse_permission_mode(permission_mode_for_stream.as_deref());
            let permission_policy = PermissionPolicy::new(mode);

            // Set up TauriPermissionPrompter for interactive permission requests
            let (perm_tx, perm_rx): (
                std::sync::mpsc::Sender<PermissionPromptDecision>,
                std::sync::mpsc::Receiver<PermissionPromptDecision>,
            ) = std::sync::mpsc::channel();
            {
                let mut senders = permission_senders.lock().unwrap();
                senders.insert(session_id.clone(), perm_tx);
            }
            let mut prompter = TauriPermissionPrompter::new(window.clone(), perm_rx);

            for (tool_id, tool_name, input_json) in pending_tool_uses.drain(..) {
                // Emit running event
                let _ = window.emit(
                    "agent-token",
                    StreamTokenPayload {
                        stream_id: stream_id_for_task.clone(),
                        text: None,
                        thinking: None,
                        event_type: "tool_call_update".to_string(),
                        tool_call_id: Some(tool_id.clone()),
                        tool_name: Some(tool_name.clone()),
                        tool_status: Some("running".to_string()),
                        tool_args: None,
                        tool_result: None,
                        tool_duration_ms: None,
                    },
                );

                // Permission check
                let permission_outcome = permission_policy.authorize(
                    &tool_name,
                    &input_json,
                    Some(&mut prompter),
                );

                let start_time = std::time::Instant::now();
                let (result_text, is_error) = match permission_outcome {
                    crate::modules::runtime::permissions::PermissionOutcome::Allow => {
                        match tool_executor.execute(&tool_name, &input_json) {
                            Ok(output) => (output, false),
                            Err(e) => (e.to_string(), true),
                        }
                    }
                    crate::modules::runtime::permissions::PermissionOutcome::Deny { reason } => {
                        (reason, true)
                    }
                };
                let duration_ms = start_time.elapsed().as_millis() as u64;

                // Emit completed/error event
                let _ = window.emit(
                    "agent-token",
                    StreamTokenPayload {
                        stream_id: stream_id_for_task.clone(),
                        text: None,
                        thinking: None,
                        event_type: "tool_call_update".to_string(),
                        tool_call_id: Some(tool_id.clone()),
                        tool_name: Some(tool_name.clone()),
                        tool_status: Some(if is_error { "error" } else { "completed" }.to_string()),
                        tool_args: None,
                        tool_result: Some(result_text.clone()),
                        tool_duration_ms: Some(duration_ms),
                    },
                );

                // Append tool_result to session_messages (API format)
                session_messages.push(crate::modules::api::InputMessage {
                    role: "tool".to_string(),
                    content: vec![crate::modules::api::InputContentBlock::ToolResult {
                        tool_use_id: tool_id.clone(),
                        content: vec![crate::modules::api::ToolResultContentBlock::Text {
                            text: result_text.clone(),
                        }],
                        is_error,
                    }],
                });

                // Also collect session-format message for persistence
                tool_result_session_messages.push(
                    crate::modules::runtime::session::ConversationMessage::tool_result(
                        tool_id,
                        tool_name,
                        result_text,
                        is_error,
                    ),
                );
            }
            // Continue outer loop → send next LLM request with tool results
        }

        // Save session with all accumulated messages
        let mut updated_app_session = app_session_clone;
        let user_msg = crate::modules::runtime::session::ConversationMessage {
            role: crate::modules::runtime::session::MessageRole::User,
            blocks: vec![ContentBlock::Text {
                text: user_message_clone.clone(),
            }],
            usage: None,
            thinking: None,
        };
        let assistant_msg = crate::modules::runtime::session::ConversationMessage {
            role: crate::modules::runtime::session::MessageRole::Assistant,
            blocks: vec![ContentBlock::Text {
                text: accumulated_text.clone(),
            }],
            usage: None,
            thinking: if accumulated_thinking.is_empty() {
                None
            } else {
                Some(accumulated_thinking)
            },
        };
        updated_app_session.messages.push(user_msg);
        updated_app_session.messages.push(assistant_msg);
        // Append tool_result messages from the tool loop
        updated_app_session
            .messages
            .extend(tool_result_session_messages);

        if let Err(e) = session_manager.save_session(&updated_app_session).await {
            tracing::error!("[start_agent_stream] Failed to save session: {}", e);
        }
    });

    // Return immediately with stream_id
    tracing::info!(
        "[start_agent_stream] Returning stream_id: {}",
        stream_id_return
    );
    Ok(stream_id_return)
}

/// TauriPermissionPrompter — bridges the sync PermissionPrompter trait
/// with async Tauri IPC. Emits a `permission-request` event to the
/// frontend and blocks on an mpsc channel until the user responds.
///
/// Usage: register `respond_permission` on the frontend side and have it
/// invoke with `{ sessionId, decision: "allow" | "deny" }`.
pub struct TauriPermissionPrompter {
    window: tauri::WebviewWindow,
    receiver: std::sync::mpsc::Receiver<PermissionPromptDecision>,
}

#[allow(dead_code)]
impl TauriPermissionPrompter {
    /// Create a new TauriPermissionPrompter.
    pub fn new(
        window: tauri::WebviewWindow,
        receiver: std::sync::mpsc::Receiver<PermissionPromptDecision>,
    ) -> Self {
        Self { window, receiver }
    }
}

impl PermissionPrompter for TauriPermissionPrompter {
    fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision {
        // 1. emit confirmation event to the frontend
        let _ = self.window.emit(
            "permission-request",
            serde_json::json!({
                "tool_name": request.tool_name,
                "permission_mode": request.required_mode.as_str(),
                "current_mode": request.current_mode.as_str(),
                "message": format!(
                    "Tool '{}' requires {} permission (current: {})",
                    request.tool_name,
                    request.required_mode.as_str(),
                    request.current_mode.as_str()
                ),
            }),
        );

        // 2. block waiting for frontend response (mpsc blocks — acceptable in sync context)
        match self
            .receiver
            .recv_timeout(std::time::Duration::from_secs(60))
        {
            Ok(decision) => decision,
            Err(_) => PermissionPromptDecision::Deny {
                reason: "Permission request timed out".to_string(),
            },
        }
    }
}

/// Respond to a permission request from the frontend.
/// The decision is sent to the waiting TauriPermissionPrompter via mpsc channel.
#[tauri::command]
#[allow(dead_code)]
pub fn respond_permission(
    state: State<'_, AppState>,
    session_id: String,
    decision: String,
) -> Result<(), String> {
    let decision_enum = match decision.as_str() {
        "allow" => PermissionPromptDecision::Allow,
        _ => PermissionPromptDecision::Deny {
            reason: "User denied permission".to_string(),
        },
    };

    let senders = state
        .permission_senders
        .lock()
        .map_err(|e| format!("Failed to lock permission senders: {e}"))?;

    let sender = senders
        .get(&session_id)
        .ok_or_else(|| "No pending permission request for this session".to_string())?;

    sender
        .send(decision_enum)
        .map_err(|_| "Failed to send permission decision".to_string())?;

    Ok(())
}
