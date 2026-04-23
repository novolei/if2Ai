//! P0-2 — `LlmProvider` trait sketch (Steward parity).
//!
//! The historical wiring is "[`ProviderClient`] called directly from
//! `turn_service`, with [`resilience`](super::resilience) helpers in the
//! middle". Steward instead exposes a single `LlmProvider` trait whose
//! implementations form an explicit decorator chain composed at
//! bootstrap time:
//!
//! ```text
//! RawClientProvider → CompletionCacheProvider → CircuitBreakerProvider
//!                  → RetryProvider → FailoverProvider
//! ```
//!
//! This file ships the trait + a no-op pass-through implementation so
//! call sites can be migrated module-by-module without a giant churn
//! diff. Subsequent packs will:
//!
//! 1. Wrap `ProviderClient` in `RawClientLlmProvider` (impl below).
//! 2. Convert `resilience::stream_message_with_resilience` into a
//!    `ResilienceLlmProvider` decorator that takes `Arc<dyn LlmProvider>`.
//! 3. Build the chain in [`crate::bootstrap`] so `compose()` reads
//!    top-down.
//!
//! Until then, the trait is `#[allow(dead_code)]`-style consumed only
//! by tests; runtime behaviour is unchanged.

use async_trait::async_trait;

use crate::modules::api::{
    ApiError, MessageRequest, MessageResponse, MessageStream, ProviderClient,
};

/// Single seam for LLM calls — implementations decorate this trait.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Non-streaming completion.
    async fn send_message(&self, req: &MessageRequest) -> Result<MessageResponse, ApiError>;

    /// Streaming completion. Implementations may add retries, circuit
    /// breaking, or failover before returning the final stream.
    async fn stream_message(&self, req: &MessageRequest) -> Result<MessageStream, ApiError>;
}

/// Pass-through implementation that simply forwards to a
/// [`ProviderClient`]. Use this as the innermost layer of the
/// decorator chain.
pub struct RawClientLlmProvider {
    client: ProviderClient,
}

impl RawClientLlmProvider {
    #[must_use]
    pub fn new(client: ProviderClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl LlmProvider for RawClientLlmProvider {
    async fn send_message(&self, req: &MessageRequest) -> Result<MessageResponse, ApiError> {
        self.client.send_message(req).await
    }

    async fn stream_message(&self, req: &MessageRequest) -> Result<MessageStream, ApiError> {
        self.client.stream_message(req).await
    }
}
