//! Agent commands - run_agent_turn
//!
//! Provides the main agent execution command for Tauri.

use std::sync::Arc;

use tauri::State;
use tokio::runtime::Handle;

use crate::commands::AppState;
use crate::modules::api::providers::claw_provider::ClawApiClient;
use crate::modules::api::{InputContentBlock, InputMessage, MessageRequest};
use crate::modules::runtime::conversation::{
    ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError, ToolExecutor,
};
use crate::modules::runtime::permissions::{PermissionMode, PermissionPolicy};
use crate::modules::runtime::session::{ContentBlock, Session as RuntimeSession};
use crate::modules::session::Session as AppSession;

/// Response from a run_agent_turn command.
#[derive(serde::Serialize)]
pub struct RunAgentTurnResponse {
    /// The generated message/response.
    pub message: String,
    /// The session ID.
    pub session_id: String,
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

/// Real API client that calls the Claw API (Claude).
///
/// This implements the `ApiClient` trait and makes real LLM API calls.
struct RealApiClient {
    provider: ClawApiClient,
    model: String,
}

impl RealApiClient {
    fn new(provider: ClawApiClient, model: String) -> Self {
        Self { provider, model }
    }
}

impl ApiClient for RealApiClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        // Convert ApiRequest to MessageRequest
        let messages: Vec<InputMessage> = request
            .messages
            .iter()
            .map(|msg| {
                let content: Vec<InputContentBlock> = msg
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(InputContentBlock::Text {
                            text: text.clone(),
                        }),
                        ContentBlock::ToolUse { id, name, input } => {
                            // Parse the input JSON string into a Value
                            let input_value: serde_json::Value = serde_json::from_str(input)
                                .unwrap_or(serde_json::Value::Null);
                            Some(InputContentBlock::ToolUse {
                                id: id.clone(),
                                name: name.clone(),
                                input: input_value,
                            })
                        }
                        ContentBlock::ToolResult { .. } => {
                            // Skip tool results in the input conversion
                            // They should be converted to user tool results in the request
                            None
                        }
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

        let system_prompt = if request.system_prompt.is_empty() {
            None
        } else {
            Some(request.system_prompt.join("\n"))
        };

        let api_request = MessageRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages,
            system: system_prompt,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        // Use block_on to call async Provider from sync trait method
        let response = Handle::current()
            .block_on(self.provider.send_message(&api_request))
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
                crate::modules::api::OutputContentBlock::Thinking { .. } => {
                    // Skip thinking blocks for now
                }
                crate::modules::api::OutputContentBlock::RedactedThinking { .. } => {
                    // Skip redacted thinking blocks
                }
            }
        }

        events.push(AssistantEvent::Usage(crate::modules::runtime::usage::TokenUsage {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            cache_creation_input_tokens: response.usage.cache_creation_input_tokens,
            cache_read_input_tokens: response.usage.cache_read_input_tokens,
        }));

        events.push(AssistantEvent::MessageStop);

        Ok(events)
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
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, crate::modules::runtime::conversation::ToolError> {
        let args: serde_json::Value = serde_json::from_str(input)
            .unwrap_or(serde_json::Value::Null);

        // Use block_on to call async ToolRegistry from sync context
        let result = Handle::current()
            .block_on(self.tool_registry.dispatch(tool_name, args))
            .map_err(|e: crate::modules::tools::ToolError| {
                crate::modules::runtime::conversation::ToolError::new(e.to_string())
            })?;

        Ok(result)
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
) -> Result<RunAgentTurnResponse, String> {
    // Restore the session
    let app_session = state
        .session_manager
        .restore_session(&session_id)
        .await
        .map_err(|e| e.to_string())?;

    // Convert application session to runtime session
    let runtime_session = app_session_to_runtime(&app_session);

    // Create real API client (Claude)
    let api_client = match ClawApiClient::from_env() {
        Ok(client) => RealApiClient::new(client, "claude-sonnet-4-6".to_string()),
        Err(e) => {
            return Err(format!(
                "Failed to initialize AI client: {}. Please check your ANTHROPIC_API_KEY environment variable.",
                e
            ));
        }
    };

    // Create tool executor bridge
    let tool_executor = ToolRegistryExecutor::new(state.tool_registry.clone());

    // Create permission policy (allow all in this implementation)
    let permission_policy = PermissionPolicy::new(PermissionMode::DangerFullAccess);

    // System prompt
    let system_prompt = vec![
        "You are If2Ai, a helpful AI assistant.".to_string(),
        "You have access to various tools to help the user.".to_string(),
        "Always be helpful, harmless, and honest.".to_string(),
    ];

    // Create runtime
    let mut runtime = ConversationRuntime::new(
        runtime_session,
        api_client,
        tool_executor,
        permission_policy,
        system_prompt,
    );

    // Run the conversation turn
    let result = runtime.run_turn(user_message.clone(), None);

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

            Ok(RunAgentTurnResponse {
                message: final_text,
                session_id,
            })
        }
        Err(e) => {
            // Return friendly error message
            let error_message = match e {
                RuntimeError::MaxIterationsExceeded => {
                    "对话达到最大迭代次数限制，请尝试简化您的问题。".to_string()
                }
                RuntimeError::ApiError(msg) => {
                    format!("AI 服务调用失败: {}. 请稍后重试。", msg)
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
