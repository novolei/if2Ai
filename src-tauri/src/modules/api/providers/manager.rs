//! Provider Manager - Registry and routing for LLM providers
//!
//! Provides a unified interface for registering and accessing multiple LLM providers.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use super::ApiError;
use super::MessageRequest;
use super::MessageResponse;
use super::Provider;
use crate::modules::api::types::{OutputContentBlock, Usage};

/// ProviderManager manages multiple LLM providers and routes requests to them.
///
/// # Example
///
/// ```
/// use std::sync::Arc;
/// use if2ai_backend::modules::api::providers::manager::{MockProvider, ProviderManager};
///
/// let mut manager = ProviderManager::new("claude".to_string());
/// manager.register("claude", Arc::new(MockProvider::with_response("Hello!")));
/// ```
#[allow(dead_code)]
pub struct ProviderManager {
    providers: HashMap<String, Arc<dyn Provider<Stream = MockStream>>>,
    default_provider: String,
}

#[allow(dead_code)]
impl ProviderManager {
    /// Creates a new ProviderManager with the specified default provider.
    #[must_use]
    pub fn new(default_provider: String) -> Self {
        Self {
            providers: HashMap::new(),
            default_provider,
        }
    }

    /// Registers a provider with the given name.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        provider: Arc<dyn Provider<Stream = MockStream>>,
    ) {
        self.providers.insert(name.into(), provider);
    }

    /// Gets a provider by name, returning None if not found.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Provider<Stream = MockStream>> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// Gets the default provider.
    #[must_use]
    pub fn default_provider_name(&self) -> &str {
        &self.default_provider
    }

    /// Checks if a provider with the given name exists.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }

    /// Creates a message using the specified provider.
    pub async fn create_message(
        &self,
        provider_name: &str,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| ApiError::Auth(format!("provider not found: {provider_name}")))?;

        provider.send_message(request).await
    }

    /// Creates a message using the default provider.
    pub async fn create_message_default(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        self.create_message(&self.default_provider, request).await
    }
}

/// MockStream is a simple mock stream for testing.
#[allow(dead_code)]
pub struct MockStream {
    content: Option<Vec<OutputContentBlock>>,
}

impl MockStream {
    #[must_use]
    pub fn new(content: Vec<OutputContentBlock>) -> Self {
        Self {
            content: Some(content),
        }
    }
}

impl tokio::io::AsyncRead for MockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if let Some(content) = self.content.take() {
            // Serialize content to JSON for streaming
            if let Ok(json) = serde_json::to_string(&content) {
                buf.put_slice(json.as_bytes());
            }
            std::task::Poll::Ready(Ok(()))
        } else {
            std::task::Poll::Ready(Ok(()))
        }
    }
}

/// MockProvider is a test provider that returns fixed responses.
#[derive(Debug)]
#[allow(dead_code)]
pub struct MockProvider {
    responses: Vec<MessageResponse>,
    call_count: Cell<usize>,
}

#[allow(dead_code)]
impl MockProvider {
    /// Creates a MockProvider with a single repeated response containing text content.
    #[must_use]
    pub fn with_response(text: impl Into<String>) -> Self {
        let text_content = text.into();
        Self {
            responses: vec![MessageResponse {
                id: "mock-1".to_string(),
                kind: "message".to_string(),
                role: "assistant".to_string(),
                content: vec![OutputContentBlock::Text { text: text_content }],
                model: "mock".to_string(),
                stop_reason: Some("end_turn".to_string()),
                stop_sequence: None,
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
                request_id: None,
            }],
            call_count: Cell::new(0),
        }
    }

    /// Creates a MockProvider with a sequence of responses.
    #[must_use]
    pub fn with_responses(responses: Vec<MessageResponse>) -> Self {
        Self {
            responses,
            call_count: Cell::new(0),
        }
    }

    /// Returns the number of times send_message was called.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.call_count.get()
    }
}

impl Provider for MockProvider {
    type Stream = MockStream;

    fn send_message<'a>(
        &'a self,
        _request: &'a MessageRequest,
    ) -> super::ProviderFuture<'a, MessageResponse> {
        let call_idx = self.call_count.get();
        self.call_count.set(call_idx + 1);
        let response = if call_idx < self.responses.len() {
            self.responses[call_idx].clone()
        } else {
            self.responses
                .last()
                .cloned()
                .unwrap_or_else(|| MessageResponse {
                    id: "mock-fallback".to_string(),
                    kind: "message".to_string(),
                    role: "assistant".to_string(),
                    content: vec![OutputContentBlock::Text {
                        text: "fallback response".to_string(),
                    }],
                    model: "mock".to_string(),
                    stop_reason: Some("end_turn".to_string()),
                    stop_sequence: None,
                    usage: Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    },
                    request_id: None,
                })
        };

        Box::pin(async move { Ok(response) })
    }

    fn stream_message<'a>(
        &'a self,
        _request: &'a MessageRequest,
    ) -> super::ProviderFuture<'a, Self::Stream> {
        let content = if let Some(first) = self.responses.first() {
            first.content.clone()
        } else {
            vec![OutputContentBlock::Text {
                text: "fallback response".to_string(),
            }]
        };

        Box::pin(async move { Ok(MockStream::new(content)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::api::providers::Provider;
    use crate::modules::api::types::InputMessage;

    fn make_text_response(text: &str) -> MessageResponse {
        MessageResponse {
            id: "mock-1".to_string(),
            kind: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![OutputContentBlock::Text {
                text: text.to_string(),
            }],
            model: "mock".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            request_id: None,
        }
    }

    #[tokio::test]
    #[allow(clippy::arc_with_non_send_sync)]
    async fn registers_and_retrieves_provider() {
        let mut manager = ProviderManager::new("test".to_string());
        let provider = Arc::new(MockProvider::with_response("test response"));
        manager.register("test", provider.clone());

        assert!(manager.has("test"));
        assert_eq!(
            manager.get("test").map(|p| p as *const _),
            Some(provider.as_ref() as *const _)
        );
    }

    #[tokio::test]
    #[allow(clippy::arc_with_non_send_sync)]
    async fn create_message_uses_correct_provider() {
        let mut manager = ProviderManager::new("mock".to_string());
        manager.register("mock", Arc::new(MockProvider::with_response("hello")));

        let request = MessageRequest {
            model: "mock".to_string(),
            max_tokens: 100,
            messages: vec![InputMessage::user_text("hi")],
            system: None,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let response = manager.create_message("mock", &request).await.unwrap();
        assert!(
            matches!(response.content.first(), Some(OutputContentBlock::Text { text }) if text == "hello")
        );
    }

    #[tokio::test]
    #[allow(clippy::arc_with_non_send_sync)]
    async fn create_message_default_uses_default_provider() {
        let mut manager = ProviderManager::new("default".to_string());
        manager.register(
            "default",
            Arc::new(MockProvider::with_response("default response")),
        );

        let request = MessageRequest {
            model: "default".to_string(),
            max_tokens: 100,
            messages: vec![InputMessage::user_text("hi")],
            system: None,
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let response = manager.create_message_default(&request).await.unwrap();
        assert!(
            matches!(response.content.first(), Some(OutputContentBlock::Text { text }) if text == "default response")
        );
    }

    #[test]
    fn mock_provider_returns_sequential_responses() {
        let responses = vec![make_text_response("first"), make_text_response("second")];

        let provider = MockProvider::with_responses(responses);
        assert_eq!(provider.call_count(), 0);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let request = MessageRequest {
                model: "test".to_string(),
                max_tokens: 100,
                messages: vec![InputMessage::user_text("hi")],
                system: None,
                tools: None,
                tool_choice: None,
                stream: false,
            };

            let response1 = provider.send_message(&request).await.unwrap();
            assert!(matches!(response1.content.first(), Some(OutputContentBlock::Text { text }) if text == "first"));

            let response2 = provider.send_message(&request).await.unwrap();
            assert!(matches!(response2.content.first(), Some(OutputContentBlock::Text { text }) if text == "second"));

            let response3 = provider.send_message(&request).await.unwrap();
            assert!(matches!(response3.content.first(), Some(OutputContentBlock::Text { text }) if text == "second")); // Falls back to last
        });
    }
}
