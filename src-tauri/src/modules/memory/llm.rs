// 8A.5 lands UtilityLlm trait + ProviderUtilityLlm + MockUtilityLlm.
// First production consumer is 8A.7 (RollingSummarizer); tests in this
// file already exercise every public item, so the bin target's "never
// used" warnings are expected until the next slice.
#![allow(dead_code)]

//! `UtilityLlm` — one-shot system+user → text shim used by every memory
//! subsystem (rolling summary, compile_today/week/longterm/facts,
//! deep-memory fact extraction, experience extractor, diary writer).
//!
//! Per `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §0.5 Δ-1 the memory modules MUST NOT depend on
//! `crate::modules::api::providers` directly — they only see the
//! [`UtilityLlm`] trait.  The default production implementation
//! [`ProviderUtilityLlm`] wraps an
//! [`api::providers::manager::ProviderManager`] and constructs a no-tool
//! [`MessageRequest`] from the supplied `system` + `user` strings.
//!
//! The `temperature` argument on [`UtilityLlm::complete`] is part of the
//! v2 design surface but the current Anthropic-shaped
//! [`MessageRequest`] type does not yet expose a `temperature` field;
//! callers should still pass an honest value (it is forwarded to
//! tracing for observability and will become a real wire field once the
//! provider API gains the parameter).

use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::modules::api::providers::manager::ProviderManager;
use crate::modules::api::{InputMessage, MessageRequest, OutputContentBlock};
use crate::modules::memory::MemoryError;

/// One-shot LLM completion seam used by every memory subsystem.
///
/// Implementations MUST:
/// - Be `Send + Sync` so they can sit on `AppState` behind `Arc`.
/// - Not panic on empty `system` / `user` — return `Ok(String::new())`
///   instead so callers can defensively skip empty-input calls without
///   special-casing the error type.
#[async_trait]
pub trait UtilityLlm: Send + Sync {
    /// Run a single non-streaming completion.  No tools, no images.
    ///
    /// `temperature` is currently advisory; see the module-level doc for
    /// why it is not yet forwarded to the wire request.
    async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<String, MemoryError>;
}

/// Production [`UtilityLlm`] backed by a [`ProviderManager`].
///
/// `model` selects which registered provider name to dispatch to; when
/// `None`, the manager's default provider is used.  The wire request
/// builds a single user message (`InputMessage::user_text(user)`) plus
/// the `system` string in [`MessageRequest::system`].
pub struct ProviderUtilityLlm {
    manager: Arc<ProviderManager>,
    model: Option<String>,
}

impl ProviderUtilityLlm {
    /// Construct a new shim.  `model = None` defers to
    /// [`ProviderManager::default_provider_name`].
    #[must_use]
    pub fn new(manager: Arc<ProviderManager>, model: Option<String>) -> Self {
        Self { manager, model }
    }
}

#[async_trait]
impl UtilityLlm for ProviderUtilityLlm {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<String, MemoryError> {
        if system.is_empty() && user.is_empty() {
            return Ok(String::new());
        }

        let provider_name = match self.model.as_deref() {
            Some(m) => m.to_string(),
            None => self.manager.default_provider_name().to_string(),
        };

        let request = MessageRequest {
            model: provider_name.clone(),
            max_tokens,
            messages: vec![InputMessage::user_text(user)],
            system: if system.is_empty() {
                None
            } else {
                Some(system.to_string())
            },
            tools: None,
            tool_choice: None,
            stream: false,
        };

        tracing::debug!(
            provider = %provider_name,
            max_tokens,
            temperature,
            system_len = system.len(),
            user_len = user.len(),
            "[utility-llm] dispatching one-shot completion"
        );

        let response = self
            .manager
            .create_message(&provider_name, &request)
            .await
            .map_err(|e| MemoryError::Generic(format!("UtilityLlm: {e}")))?;

        let text = response
            .content
            .iter()
            .filter_map(|block| match block {
                OutputContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(text)
    }
}

/// MEM-MOD-WIRE-FIX — Production `UtilityLlm` for the if2Ai chat
/// path.
///
/// Bridges the memory subsystem's `UtilityLlm` seam onto the same
/// `ProviderClient` resolver the live agent turn uses
/// ([`crate::modules::application::provider_service::resolve_chat_runtime_provider`]).
/// Lazy: every `complete()` re-resolves the provider so flipping the
/// configured chat model in Settings takes effect on the next utility
/// call without a restart (`ConfigLoader` itself is cached, so the
/// lookup is cheap).
///
/// Unlike [`ProviderUtilityLlm`], this adapter does NOT need a
/// pre-built [`ProviderManager`] — historically that path required
/// every memory consumer to register a `Box<dyn LlmProvider>` upfront,
/// which the if2Ai bootstrap has never done.  The result was the
/// production `MockUtilityLlm::empty()` placeholder turning every
/// rolling summary, every reflection pulse, every learned-trait
/// extraction into a no-op (=> empty `memory.md`, 0 traits, etc.).
/// This adapter is the fix.
pub struct ChatProviderUtilityLlm {
    /// Working directory used to load the per-project provider
    /// transport policy.  Bootstrap passes the user's home dir so we
    /// always pick up the global `settings.json`; per-project
    /// overrides will be honoured once a future slice threads the
    /// turn's `workdir` through the memory pipeline.
    workdir: std::path::PathBuf,
    /// Caller label written into the durable `usage` store every time
    /// `complete()` actually consumes provider tokens. Defaults to
    /// `"utility"`; override via [`with_caller`] to differentiate
    /// summarizer / compiler / utility_large work in the per-role
    /// dashboard.
    caller: &'static str,
}

impl ChatProviderUtilityLlm {
    /// Construct a new chat-provider-backed shim with the default
    /// `"utility"` caller label.
    #[must_use]
    pub fn new(workdir: std::path::PathBuf) -> Self {
        Self {
            workdir,
            caller: crate::modules::usage::CALLER_UTILITY,
        }
    }

    /// Override the usage-store caller label (e.g. `"summarizer"`,
    /// `"compiler"`, `"utility_large"`). Callers should use the
    /// `crate::modules::usage::CALLER_*` constants.
    #[must_use]
    pub fn with_caller(mut self, caller: &'static str) -> Self {
        self.caller = caller;
        self
    }
}

#[async_trait]
impl UtilityLlm for ChatProviderUtilityLlm {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<String, MemoryError> {
        if system.is_empty() && user.is_empty() {
            return Ok(String::new());
        }

        // Resolve provider lazily so config edits take effect on the
        // next utility call (no restart needed).
        let resolution =
            crate::modules::application::provider_service::resolve_chat_runtime_provider(
                &self.workdir,
            )
            .await
            .map_err(|e| MemoryError::Generic(format!("UtilityLlm.resolve: {e}")))?;

        let request = MessageRequest {
            model: resolution.model.clone(),
            max_tokens,
            messages: vec![InputMessage::user_text(user)],
            system: if system.is_empty() {
                None
            } else {
                Some(system.to_string())
            },
            tools: None,
            tool_choice: None,
            stream: false,
        };

        tracing::debug!(
            model = %resolution.model,
            max_tokens,
            temperature,
            system_len = system.len(),
            user_len = user.len(),
            "[utility-llm.chat] dispatching one-shot completion"
        );

        let cfg = crate::modules::provider::resilience::LlmResilienceConfig::from_env();
        let response = crate::modules::provider::resilience::send_message_resilient_cached(
            &resolution.provider_client,
            &request,
            &cfg,
        )
        .await
        .map_err(|e| MemoryError::Generic(format!("UtilityLlm.send: {e}")))?;

        // Best-effort per-role usage logging.  `response.usage` is the
        // provider-billable counts; cost falls back to the same
        // pricing table the chat path uses. The store itself
        // short-circuits zero-token records so non-emitting providers
        // don't spam empty rows.
        let api_usage = &response.usage;
        let usage = crate::modules::runtime::usage::TokenUsage {
            input_tokens: api_usage.input_tokens,
            output_tokens: api_usage.output_tokens,
            cache_creation_input_tokens: api_usage.cache_creation_input_tokens,
            cache_read_input_tokens: api_usage.cache_read_input_tokens,
        };
        if usage.total_tokens() > 0 {
            let cost = crate::modules::runtime::usage::cost_for_usage(usage, &resolution.model);
            crate::modules::usage::record_turn_usage(crate::modules::usage::TurnUsageRecord {
                caller: self.caller.to_string(),
                provider_id: resolution.provider_id.clone(),
                model_id: resolution.model.clone(),
                usage,
                cost_usd: cost,
                session_id: None,
            });
        }

        let text = response
            .content
            .iter()
            .filter_map(|block| match block {
                OutputContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(text)
    }
}

/// In-memory test double for [`UtilityLlm`].
///
/// Returns canned responses in order; once `responses` is exhausted the
/// mock returns `Ok(String::new())` so well-behaved callers gracefully
/// degrade rather than panic.  `call_count` is incremented atomically
/// before the response lookup so concurrent callers can be asserted on.
pub struct MockUtilityLlm {
    responses: tokio::sync::Mutex<Vec<String>>,
    call_count: AtomicUsize,
}

impl MockUtilityLlm {
    /// Build a mock that yields `responses[i]` on the i-th call.
    #[must_use]
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: tokio::sync::Mutex::new(responses),
            call_count: AtomicUsize::new(0),
        }
    }

    /// Build a mock that always returns `Ok(String::new())` — useful as
    /// a placeholder when the production provider is not yet wired up.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Total number of [`UtilityLlm::complete`] invocations seen so far.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl UtilityLlm for MockUtilityLlm {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _max_tokens: u32,
        _temperature: f32,
    ) -> Result<String, MemoryError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let mut responses = self.responses.lock().await;
        if responses.is_empty() {
            Ok(String::new())
        } else {
            Ok(responses.remove(0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_returns_responses_in_order() {
        let mock = MockUtilityLlm::new(vec!["first".into(), "second".into()]);
        assert_eq!(mock.complete("s", "u", 100, 0.0).await.unwrap(), "first");
        assert_eq!(mock.complete("s", "u", 100, 0.0).await.unwrap(), "second");
        assert_eq!(mock.call_count(), 2);
    }

    #[tokio::test]
    async fn mock_returns_empty_when_exhausted() {
        let mock = MockUtilityLlm::new(vec!["only".into()]);
        assert_eq!(mock.complete("s", "u", 100, 0.0).await.unwrap(), "only");
        assert_eq!(mock.complete("s", "u", 100, 0.0).await.unwrap(), "");
        assert_eq!(mock.complete("s", "u", 100, 0.0).await.unwrap(), "");
    }

    #[tokio::test]
    async fn mock_empty_constructor_yields_empty_string() {
        let mock = MockUtilityLlm::empty();
        assert_eq!(mock.complete("s", "u", 1, 0.0).await.unwrap(), "");
        assert_eq!(mock.call_count(), 1);
    }

    #[tokio::test]
    async fn provider_utility_constructor_does_not_panic() {
        let mgr = Arc::new(ProviderManager::new("default".to_string()));
        let _shim = ProviderUtilityLlm::new(mgr, None);
        let mgr2 = Arc::new(ProviderManager::new("default".to_string()));
        let _shim2 = ProviderUtilityLlm::new(mgr2, Some("haiku".into()));
    }

    #[tokio::test]
    async fn provider_utility_returns_empty_when_both_inputs_empty() {
        let mgr = Arc::new(ProviderManager::new("default".to_string()));
        let shim = ProviderUtilityLlm::new(mgr, None);
        // Empty system + empty user must short-circuit to Ok("") without
        // even consulting the provider (no provider registered here).
        let out = shim.complete("", "", 100, 0.0).await.unwrap();
        assert_eq!(out, "");
    }
}
