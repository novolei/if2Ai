//! RealApiClient — adapter that implements the runtime `ApiClient` trait
//! over the outbound `ProviderClient`.
//!
//! Extracted from `commands/agent.rs` in GFR-001 (pure structural move,
//! function bodies byte-identical).

use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use crate::modules::api::{
    InputContentBlock, InputMessage, MessageRequest, ProviderClient, ToolDefinition,
};
use crate::modules::runtime::block_conversion::runtime_block_to_input_block;
use crate::modules::runtime::conversation::{ApiClient, ApiRequest, AssistantEvent, RuntimeError};

/// Real API client that calls the Claw API (Claude/MiniMax).
///
/// This implements the `ApiClient` trait and makes real LLM API calls.
pub(crate) struct RealApiClient {
    provider: ProviderClient,
    model: String,
    request_timeout: Duration,
    tool_registry: Arc<crate::modules::tools::ToolRegistry>,
}

impl RealApiClient {
    pub(crate) fn new(
        provider: ProviderClient,
        model: String,
        request_timeout: Duration,
        tool_registry: Arc<crate::modules::tools::ToolRegistry>,
    ) -> Self {
        Self {
            provider,
            model,
            request_timeout,
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
                timeout(self.request_timeout, api_future).await
            })
        });

        let response = result
            .map_err(|_| RuntimeError::ApiError("API call timed out".to_string()))?
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
                    .map(runtime_block_to_input_block)
                    .collect();

                let role = match msg.role {
                    crate::modules::runtime::session::MessageRole::System => "user".to_string(),
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

        self.provider.send_message(&api_request).await
    }
}
