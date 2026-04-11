//! Agent commands - run_agent_turn
//!
//! Provides the main agent execution command for Tauri.

use std::sync::Arc;

use tauri::State;
use tokio::runtime::Handle;

use crate::commands::AppState;
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

/// Mock API client that provides simple responses for testing.
///
/// This implements the `ApiClient` trait directly without needing the async ProviderManager.
/// In Phase 2, this will be replaced with a real ProviderManager integration.
struct MockApiClient;

impl MockApiClient {
    fn new() -> Self {
        Self
    }
}

impl ApiClient for MockApiClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        // Extract the user's last message to generate a contextual response
        let user_message = request
            .messages
            .iter()
            .rev()
            .find(|m| {
                m.role == crate::modules::runtime::session::MessageRole::User
            })
            .and_then(|m| {
                m.blocks.iter().find_map(|block| {
                    if let ContentBlock::Text { text } = block {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();

        // Generate a contextual response based on the input
        let response_text = if user_message.to_lowercase().contains("hello")
            || user_message.to_lowercase().contains("hi") {
            "Hello! I'm If2Ai, your AI assistant. How can I help you today?".to_string()
        } else if user_message.to_lowercase().contains("help") {
            "I'm here to help! I can assist you with various tasks including:\n\
             - Writing and editing code\n\
             - Reading and analyzing files\n\
             - Running commands\n\
             - Answering questions\n\n\
             What would you like me to help with?".to_string()
        } else if user_message.to_lowercase().contains("bye")
            || user_message.to_lowercase().contains("goodbye") {
            "Goodbye! Feel free to come back if you need any help. Have a great day!".to_string()
        } else if !user_message.is_empty() {
            format!(
                "I received your message: '{}'. This is a demonstration of the If2Ai \
                 agent system. In Phase 2, I will be connected to real LLM providers \
                 (Claude, GPT, etc.) to provide actual intelligent responses. \
                 Stay tuned for the full implementation!",
                user_message
            )
        } else {
            "I'm ready to help! What would you like me to do?".to_string()
        };

        Ok(vec![
            AssistantEvent::TextDelta(response_text),
            AssistantEvent::Usage(crate::modules::runtime::usage::TokenUsage {
                input_tokens: (user_message.len() / 4) as u32,
                output_tokens: 50,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }),
            AssistantEvent::MessageStop,
        ])
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

    // Create API client
    let api_client = MockApiClient::new();

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
