//! Provider runtime resolution service (Phase M1.2).
//!
//! Owns the responsibility of turning the user's runtime config into
//! a concrete provider `ProviderClient` + model id + request timeout
//! ready for [`crate::modules::runtime::conversation::ConversationRuntime`].
//!
//! Migration source: previously
//! [`crate::commands::agent::create_runtime_provider_client_from_config`]
//! (now removed) and the inline call sites in `run_agent_turn` /
//! `start_agent_stream`.
//!
//! Hard rules:
//! 1. This service MUST NOT import from `crate::commands::*`.
//! 2. Provider fallback / transport policy semantics are preserved
//!    bit-for-bit during the M1.2 cut-over; behavioural changes are
//!    deferred to dedicated slices.
//! 3. Provider configuration lookup continues to flow through
//!    [`crate::modules::config::model_resolver::ModelResolver`] —
//!    this service is a thin orchestrator, not a replacement for the
//!    config layer.

use std::path::Path;
use std::time::Duration;

use crate::modules::api::providers::claw_provider::{AuthSource, ClawApiClient};
use crate::modules::api::providers::openai_compat::{OpenAiCompatClient, OpenAiCompatConfig};
use crate::modules::api::ProviderClient;
use crate::modules::config::model_resolver::ModelResolver;
use crate::modules::runtime::config::{ConfigLoader, ProviderTransportConfig};

/// Resolved provider runtime triple consumed by the conversation
/// runtime layer.
///
/// Held as a typed value (instead of returning a tuple) so future M1
/// slices can attach diagnostics (e.g. resolved policy version,
/// chosen role) without breaking call sites.
#[derive(Debug)]
pub struct RuntimeProviderResolution {
    /// Concrete provider client, ready to issue requests.
    pub provider_client: ProviderClient,
    /// Resolved model id (e.g. `"claude-3-5-sonnet"`, `"gpt-4o"`).
    pub model: String,
    /// Per-request overall timeout derived from the loaded
    /// [`ProviderTransportConfig`].
    pub request_timeout: Duration,
}

/// Load the provider transport policy for the given working
/// directory.
///
/// Falls back to [`ProviderTransportConfig::default`] when the
/// runtime config cannot be loaded — this preserves the legacy
/// behaviour of `commands/agent.rs::load_provider_transport_policy`.
pub fn load_provider_transport_policy(workdir: &Path) -> ProviderTransportConfig {
    match ConfigLoader::default_for(workdir).load() {
        Ok(config) => config.control_plane().provider_transport().clone(),
        Err(error) => {
            tracing::warn!(
                "[provider_service] failed to load transport policy from runtime config, fallback to defaults: workdir={}, error={}",
                workdir.display(),
                error
            );
            ProviderTransportConfig::default()
        }
    }
}

/// Resolve the chat-role provider runtime from local configuration.
///
/// Equivalent to the legacy
/// `commands::agent::create_runtime_provider_client_from_config` but
/// returns a typed [`RuntimeProviderResolution`] instead of a 3-tuple.
///
/// Behaviour preserved:
/// - Reads `chat` role via [`ModelResolver::resolve_role_model`].
/// - Anthropic-style providers require an API key; missing key is a
///   user-facing error.
/// - OpenAI-compatible providers tolerate an empty `api_key` (used by
///   local providers like Ollama).
/// - Unknown protocols return a user-facing error.
pub async fn resolve_chat_runtime_provider(
    workdir: &Path,
) -> Result<RuntimeProviderResolution, String> {
    let resolved = ModelResolver::resolve_role_model("chat")
        .await
        .map_err(|e| format!("Failed to resolve configured chat model: {e}"))?;

    let policy = load_provider_transport_policy(workdir);
    let request_timeout = Duration::from_millis(policy.overall_timeout_ms());

    let provider_client = match resolved.api.as_str() {
        "anthropic-messages" => {
            let api_key = resolved.api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                "Anthropic provider is missing API key. Please complete provider setup first."
                    .to_string()
            })?;
            let client = ClawApiClient::from_auth(AuthSource::ApiKey(api_key))
                .with_base_url(resolved.base_url)
                .with_transport_policy(&policy);
            ProviderClient::ClawApi(client)
        }
        "openai-completions" => {
            let api_key = resolved.api_key.unwrap_or_default();
            let client = OpenAiCompatClient::new(api_key, OpenAiCompatConfig::openai())
                .with_base_url(resolved.base_url)
                .with_retry_policy(
                    policy.max_retries(),
                    Duration::from_millis(policy.initial_backoff_ms()),
                    Duration::from_millis(policy.max_backoff_ms()),
                );
            ProviderClient::OpenAi(client)
        }
        other => {
            return Err(format!(
                "Configured chat model protocol '{other}' is not supported."
            ));
        }
    };

    Ok(RuntimeProviderResolution {
        provider_client,
        model: resolved.model_id,
        request_timeout,
    })
}

// ── P1-8 smart routing (cheap model for low-complexity turns) ─────────────

fn smart_routing_enabled() -> bool {
    std::env::var("IF2AI_SMART_ROUTING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn smart_route_threshold() -> f32 {
    std::env::var("IF2AI_SMART_ROUTE_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.42f32)
        .clamp(0.0, 1.0)
}

fn cheap_model_id_from_env() -> Option<String> {
    let v = std::env::var("IF2AI_CHEAP_MODEL_ID").ok()?;
    let t = v.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// When `IF2AI_SMART_ROUTING=1` and classifier `complexity_score` ≤ threshold,
/// replace [`RuntimeProviderResolution::model`] with `IF2AI_CHEAP_MODEL_ID`.
#[must_use]
pub fn apply_complexity_model_routing(
    mut resolution: RuntimeProviderResolution,
    complexity_score: f32,
) -> RuntimeProviderResolution {
    if !smart_routing_enabled() {
        return resolution;
    }
    let Some(cheap) = cheap_model_id_from_env() else {
        tracing::warn!(
            "[smart_routing] IF2AI_SMART_ROUTING=1 but IF2AI_CHEAP_MODEL_ID is empty; skipping"
        );
        return resolution;
    };
    let t = smart_route_threshold();
    if complexity_score <= t {
        tracing::info!(
            score = complexity_score,
            threshold = t,
            cheap_model = %cheap,
            "[smart_routing] low complexity → cheap model"
        );
        resolution.model = cheap;
    } else {
        tracing::debug!(
            score = complexity_score,
            threshold = t,
            "[smart_routing] above threshold → primary model"
        );
    }
    resolution
}

/// Optional OpenAI-compatible failover `ProviderClient` when
/// `IF2AI_FAILOVER_BASE_URL` is set (e.g. backup endpoint or local Ollama).
///
/// `IF2AI_FAILOVER_API_KEY` is optional (empty for keyless local servers).
pub async fn resolve_optional_failover_openai_client(
    workdir: &Path,
) -> Result<Option<ProviderClient>, String> {
    let base = match std::env::var("IF2AI_FAILOVER_BASE_URL") {
        Ok(b) if !b.trim().is_empty() => b,
        _ => return Ok(None),
    };
    let api_key = std::env::var("IF2AI_FAILOVER_API_KEY").unwrap_or_default();
    let policy = load_provider_transport_policy(workdir);
    let client = OpenAiCompatClient::new(api_key, OpenAiCompatConfig::openai())
        .with_base_url(base)
        .with_retry_policy(
            policy.max_retries(),
            Duration::from_millis(policy.initial_backoff_ms()),
            Duration::from_millis(policy.max_backoff_ms()),
        );
    Ok(Some(ProviderClient::OpenAi(client)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Loading transport policy from a non-existent workdir must fall
    /// back silently to defaults — preserves legacy behaviour.
    #[test]
    fn transport_policy_falls_back_to_defaults_on_missing_workdir() {
        let workdir = PathBuf::from("/nonexistent/path/for/m1-tests");
        let policy = load_provider_transport_policy(&workdir);
        let default_policy = ProviderTransportConfig::default();
        assert_eq!(
            policy.overall_timeout_ms(),
            default_policy.overall_timeout_ms()
        );
        assert_eq!(policy.max_retries(), default_policy.max_retries());
    }
}
