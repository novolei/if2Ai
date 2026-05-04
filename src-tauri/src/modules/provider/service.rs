//! ProviderService — provider listing, model resolution, and selection.
//!
//! Provides the service layer for provider onboarding (Step 4):
//! - `list_providers()`: Returns all 14 builtin providers
//! - `list_models(provider_id, base_url, api_key)`: Fetches available models
//!   - Ollama: `GET {base_url}/api/tags`
//!   - Anthropic: Uses builtin registry
//!   - OpenAI-compatible: `GET {base_url}/models` with Bearer token
//! - `configure_provider()`: Saves provider config via ConfigService
//! - `select_model()`: Saves model selection via ConfigService

use crate::modules::api::providers::resolve_model_alias;
use crate::modules::config::store::{models_json_path, read_json, write_json};
use crate::modules::config::{ConfigService, ModelsJson, ProviderConfig};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::client::PROVIDER_HTTP_CLIENT;
use super::registry::builtin_providers;
use super::types::{Model, ModelModality, Provider};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilitySelection {
    pub id: String,
    #[serde(rename = "supportsThinking")]
    pub supports_thinking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingProbeResult {
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(rename = "supportsThinking")]
    pub supports_thinking: bool,
    #[serde(rename = "chunksRead")]
    pub chunks_read: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// List all 14 builtin providers.
#[must_use]
pub fn list_providers() -> Vec<Provider> {
    builtin_providers()
}

/// List available models for a given provider.
///
/// Uses three different protocols:
/// - **Ollama**: `GET {base_url}/api/tags` — returns locally available models
/// - **Anthropic**: Returns known Claude models from builtin registry
/// - **OpenAI-compatible**: `GET {base_url}/models` — returns API model list
///
/// # Arguments
///
/// * `provider_id` - Provider identifier (e.g. "openai", "anthropic", "ollama")
/// * `base_url` - API base URL
/// * `api_key` - API key (may be `None` for local providers)
pub async fn list_models(
    provider_id: &str,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<Model>, String> {
    match provider_id {
        "ollama" => list_ollama_models(base_url).await,
        "anthropic" => Ok(list_anthropic_models()),
        _ => list_openai_compat_models(base_url, api_key).await,
    }
}

/// Fetch models from a local Ollama instance via `/api/tags`.
async fn list_ollama_models(base_url: &str) -> Result<Vec<Model>, String> {
    // Ollama's /api/tags is on the native API root, not under /v1
    let base = base_url.trim_end_matches('/').trim_end_matches("/v1");
    let url = format!("{base}/api/tags");
    let response = PROVIDER_HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    let models = body
        .get("models")
        .and_then(|m| m.as_array())
        .ok_or_else(|| "Ollama response missing 'models' field".to_string())?;

    Ok(models
        .iter()
        .filter_map(|m| {
            let name = m.get("name").and_then(|v| v.as_str())?;
            let canonical = resolve_model_alias(name);
            Some(enrich_model_capability(
                "ollama",
                Model {
                    id: name.to_string(),
                    name: canonical,
                    context_window: None,
                    max_tokens: None,
                    modality: ModelModality::Text,
                    reasoning: false,
                    reasoning_required_in_tool_calls: false,
                    supports_reasoning_effort: false,
                },
            ))
        })
        .collect())
}

/// Return known Anthropic/Claude models from builtin registry.
///
/// Anthropic doesn't expose a public model listing endpoint,
/// so we use a hardcoded list of known models.
fn list_anthropic_models() -> Vec<Model> {
    [
        ("claude-opus-4-6", "Claude Opus 4.6", 200_000, 32_000),
        ("claude-sonnet-4-6", "Claude Sonnet 4.6", 200_000, 64_000),
        (
            "claude-sonnet-4-5-20250514",
            "Claude Sonnet 4.5",
            200_000,
            64_000,
        ),
        (
            "claude-haiku-4-5-20251213",
            "Claude Haiku 4.5",
            200_000,
            8_000,
        ),
    ]
    .into_iter()
    .map(|(id, name, ctx, max)| {
        enrich_model_capability(
            "anthropic",
            Model {
                id: id.to_string(),
                name: name.to_string(),
                context_window: Some(ctx),
                max_tokens: Some(max),
                modality: ModelModality::Text,
                reasoning: false,
                reasoning_required_in_tool_calls: false,
                supports_reasoning_effort: false,
            },
        )
    })
    .collect()
}

/// P-MULTI-API — Fill the reasoning-related fields on a `Model` from the
/// `(provider_id, model.id)` lookup against the built-in capability
/// dictionary. Idempotent — re-running on an already-enriched model is
/// safe.
fn enrich_model_capability(provider_id: &str, mut model: Model) -> Model {
    let cap = super::capabilities::resolve(
        provider_id,
        &model.id,
        None,
        super::capabilities::GlobalThinkingPolicy::Auto,
    );
    model.reasoning = cap.reasoning;
    model.reasoning_required_in_tool_calls = cap.reasoning_required_in_tool_calls;
    model.supports_reasoning_effort = cap.supports_reasoning_effort;
    model
}

/// Fetch models from an OpenAI-compatible provider via `/models`.
async fn list_openai_compat_models(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<Model>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut request = PROVIDER_HTTP_CLIENT.get(&url);

    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Failed to connect to provider: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Provider returned {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse provider response: {e}"))?;

    let models = body
        .get("data")
        .and_then(|m| m.as_array())
        .ok_or_else(|| "Provider response missing 'data' field".to_string())?;

    // Heuristic: derive provider id from base_url host.  Ollama / OpenAI /
    // Anthropic etc. use distinct hosts.  Falls back to "" which makes
    // `enrich_model_capability` skip dict lookups (returns `Default`).
    let provider_id = derive_provider_id_from_base_url(base_url);
    Ok(models
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(|v| v.as_str())?;
            let canonical = resolve_model_alias(id);
            Some(enrich_model_capability(
                &provider_id,
                Model {
                    id: id.to_string(),
                    name: canonical,
                    context_window: None,
                    max_tokens: None,
                    modality: ModelModality::Text,
                    reasoning: false,
                    reasoning_required_in_tool_calls: false,
                    supports_reasoning_effort: false,
                },
            ))
        })
        .collect())
}

/// Best-effort host → known-provider id derivation. Used by
/// `list_openai_compat_models` to pick the right capability lookup
/// without requiring callers to thread the provider id through the
/// model fetch path.
fn derive_provider_id_from_base_url(base_url: &str) -> String {
    let url_lower = base_url.to_ascii_lowercase();
    for kp in super::known_providers::KNOWN_PROVIDERS {
        if !kp.default_base_url.is_empty()
            && url_lower.contains(&kp.default_base_url.to_ascii_lowercase())
        {
            return kp.id.to_string();
        }
    }
    // Soft host matching for users with custom prefixes (proxies / mirrors).
    for (host_substr, id) in [
        ("moonshot", "moonshot"),
        ("api.kimi.com/coding", "kimi-coding"),
        ("dashscope.aliyuncs.com/compatible", "dashscope"),
        ("coding.dashscope", "dashscope-coding"),
        ("deepseek", "deepseek"),
        ("ollama", "ollama"),
        ("11434", "ollama"),
        ("openai.com", "openai"),
        ("anthropic.com", "anthropic"),
        ("openrouter", "openrouter"),
        ("siliconflow", "siliconflow"),
        ("bigmodel.cn", "zhipu"),
        ("minimaxi", "minimax"),
    ] {
        if url_lower.contains(host_substr) {
            return id.to_string();
        }
    }
    String::new()
}

/// Configure a provider by saving its settings via `ConfigService`.
///
/// # Arguments
///
/// * `provider_config` - Provider configuration to save
pub async fn configure_provider(provider_config: &ProviderConfig) -> Result<(), String> {
    let service = ConfigService::new();
    service
        .save_config(&{
            let mut config = service
                .load_config()
                .await
                .map_err(|e| format!("Failed to load config: {e}"))?;
            config.active_provider = Some(provider_config.clone());
            config.upsert_configured_provider(provider_config.clone());
            config
        })
        .await
        .map_err(|e| format!("Failed to save provider: {e}"))
}

/// Configure a provider with multiple model selections.
/// The first model is set as the default (active_model).
///
/// # Arguments
///
/// * `provider_config` - Provider configuration to save
/// * `model_ids` - List of selected model IDs (first one becomes default)
pub async fn configure_provider_with_models(
    provider_config: &ProviderConfig,
    model_ids: &[String],
) -> Result<(), String> {
    use crate::modules::config::ModelSelection;
    use std::collections::HashSet;

    let service = ConfigService::new();
    service
        .save_config(&{
            let mut config = service
                .load_config()
                .await
                .map_err(|e| format!("Failed to load config: {e}"))?;

            // Incrementally merge models:
            // 1. Keep ALL existing models (including from the same provider with different auth)
            let existing = config.selected_models.clone();

            // 2. Build a set of existing keys for dedup
            let mut seen: HashSet<String> = existing
                .iter()
                .map(|m| format!("{}::{}", m.provider_id, m.model_id))
                .collect();

            // 3. Append only new model_ids that aren't already present
            let new_models: Vec<ModelSelection> = model_ids
                .iter()
                .filter(|id| {
                    let key = format!("{}::{}", provider_config.provider_id, id);
                    if seen.contains(&key) {
                        false
                    } else {
                        seen.insert(key);
                        true
                    }
                })
                .map(|id| ModelSelection {
                    provider_id: provider_config.provider_id.clone(),
                    model_id: id.clone(),
                    auth_variant: provider_config.auth_variant.clone(),
                })
                .collect();

            let mut merged = existing;
            merged.extend(new_models);
            config.selected_models = merged;

            config.active_provider = Some(provider_config.clone());

            // Persist the full provider config (base_url, api_key) so that
            // sync_to_triple_files can produce correct models.json entries even
            // when the user later switches to a different active provider.
            config.upsert_configured_provider(provider_config.clone());

            // First model becomes the default
            if let Some(first) = model_ids.first() {
                config.active_model = Some(ModelSelection {
                    provider_id: provider_config.provider_id.clone(),
                    model_id: first.clone(),
                    auth_variant: provider_config.auth_variant.clone(),
                });
            }
            config
        })
        .await
        .map_err(|e| format!("Failed to save provider config: {e}"))
}

/// Configure a provider and persist per-model runtime capability probes.
pub async fn configure_provider_with_model_capabilities(
    provider_config: &ProviderConfig,
    models: &[ModelCapabilitySelection],
) -> Result<(), String> {
    let model_ids = models
        .iter()
        .map(|model| model.id.clone())
        .collect::<Vec<_>>();
    configure_provider_with_models(provider_config, &model_ids).await?;

    let path = models_json_path();
    let mut models_json = read_json::<ModelsJson>(&path)
        .await
        .map_err(|e| format!("Failed to read models.json: {e}"))?
        .ok_or_else(|| "models.json not found after provider save".to_string())?;

    let provider_key = provider_key(provider_config);
    let Some(entry) = models_json.providers.get_mut(&provider_key) else {
        return Err(format!(
            "Provider '{}' not found in models.json after save",
            provider_key
        ));
    };

    for selected in models {
        if let Some(model) = entry
            .models
            .iter_mut()
            .find(|model| model.id == selected.id)
        {
            model.supports_thinking = Some(selected.supports_thinking);
        }
    }

    write_json(&models_json, &path)
        .await
        .map_err(|e| format!("Failed to write models.json capability probes: {e}"))
}

fn provider_key(provider_config: &ProviderConfig) -> String {
    match provider_config
        .auth_variant
        .as_deref()
        .filter(|v| !v.is_empty())
    {
        Some(variant) => format!("{}::{}", provider_config.provider_id, variant),
        None => provider_config.provider_id.clone(),
    }
}

/// Probe whether a model actually emits thinking/reasoning fields.
pub async fn probe_model_thinking(
    provider_config: &ProviderConfig,
    model_id: &str,
) -> Result<ThinkingProbeResult, String> {
    if model_id.trim().is_empty() {
        return Err("model_id is required".to_string());
    }
    if provider_config.provider_id == "anthropic" {
        return Ok(ThinkingProbeResult {
            model_id: model_id.to_string(),
            supports_thinking: false,
            chunks_read: 0,
            error: Some("anthropic-messages probe is not supported yet".to_string()),
        });
    }

    let base = provider_config
        .base_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| {
            if provider_config.provider_id == "ollama" {
                "http://localhost:11434/v1"
            } else {
                ""
            }
        });
    if base.is_empty() {
        return Err("base_url is required for thinking probe".to_string());
    }
    let endpoint = format!(
        "{}/chat/completions",
        openai_compat_base_url(&provider_config.provider_id, base)
    );

    let mut req = PROVIDER_HTTP_CLIENT.post(&endpoint);
    if let Some(api_key) = provider_config
        .api_key
        .as_deref()
        .filter(|key| !key.is_empty())
    {
        req = req.bearer_auth(api_key);
    }

    let response = req
        .json(&thinking_probe_body(&provider_config.provider_id, model_id))
        .send()
        .await
        .map_err(|e| format!("thinking probe HTTP error: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let raw = response.text().await.unwrap_or_default();
        return Ok(ThinkingProbeResult {
            model_id: model_id.to_string(),
            supports_thinking: false,
            chunks_read: 0,
            error: Some(format!(
                "http {}: {}",
                status.as_u16(),
                raw.chars().take(160).collect::<String>()
            )),
        });
    }

    let mut stream = response.bytes_stream();
    let mut line_buf = String::new();
    let mut chunks_read = 0u32;
    let mut supports_thinking = false;
    let thinking_fields = [
        "thinking",
        "reasoning_content",
        "reasoning",
        "thinking_content",
    ];

    'outer: while let Some(chunk) =
        tokio::time::timeout(std::time::Duration::from_secs(20), stream.next())
            .await
            .unwrap_or(None)
    {
        let bytes = match chunk {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(ThinkingProbeResult {
                    model_id: model_id.to_string(),
                    supports_thinking: false,
                    chunks_read,
                    error: Some(format!("stream chunk error: {error}")),
                });
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        for ch in text.chars() {
            if ch == '\n' {
                let line = line_buf.trim().to_string();
                line_buf.clear();
                let data = if let Some(data) = line.strip_prefix("data:") {
                    data.trim()
                } else {
                    continue;
                };
                if data == "[DONE]" {
                    break 'outer;
                }
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
                    chunks_read += 1;
                    if json_has_thinking_field(&value, &thinking_fields) {
                        supports_thinking = true;
                        break 'outer;
                    }
                    if chunks_read >= 60 {
                        break 'outer;
                    }
                }
            } else {
                line_buf.push(ch);
            }
        }
    }

    Ok(ThinkingProbeResult {
        model_id: model_id.to_string(),
        supports_thinking,
        chunks_read,
        error: None,
    })
}

fn openai_compat_base_url(provider_id: &str, base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if provider_id == "ollama" && !trimmed.ends_with("/v1") {
        format!("{trimmed}/v1")
    } else {
        trimmed.to_string()
    }
}

fn thinking_probe_body(provider_id: &str, model_id: &str) -> serde_json::Value {
    let mut body = json!({
        "model": model_id,
        "messages": [{ "role": "user", "content": "hi" }],
        "stream": true,
        "max_tokens": 64,
    });
    if provider_id == "ollama" {
        body["think"] = serde_json::Value::Bool(true);
    }
    if provider_id == "dashscope" || provider_id == "dashscope-coding" {
        body["enable_thinking"] = serde_json::Value::Bool(true);
        body["thinking_budget"] = json!(8192);
    }
    body
}

fn json_has_thinking_field(value: &serde_json::Value, fields: &[&str]) -> bool {
    if let Some(delta) = value
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
    {
        for field in fields {
            if delta
                .get(field)
                .and_then(|value| value.as_str())
                .is_some_and(|value| !value.trim().is_empty())
            {
                return true;
            }
        }
    }

    value
        .get("message")
        .and_then(|message| message.get("thinking"))
        .and_then(|thinking| thinking.as_str())
        .is_some_and(|thinking| !thinking.trim().is_empty())
}

/// Get previously configured models for a given provider.
pub async fn get_configured_models(provider_id: &str) -> Result<Vec<String>, String> {
    let service = ConfigService::new();
    let config = service
        .load_config()
        .await
        .map_err(|e| format!("Failed to load config: {e}"))?;
    Ok(config
        .selected_models
        .iter()
        .filter(|m| m.provider_id == provider_id)
        .map(|m| m.model_id.clone())
        .collect())
}

/// Get all configured models grouped by provider.
/// Used for the unified model pool display.
pub async fn get_all_configured_models() -> Result<Vec<(String, Vec<String>)>, String> {
    let service = ConfigService::new();
    let config = service
        .load_config()
        .await
        .map_err(|e| format!("Failed to load config: {e}"))?;
    let mut groups: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for m in &config.selected_models {
        groups
            .entry(m.provider_id.clone())
            .or_default()
            .push(m.model_id.clone());
    }
    Ok(groups.into_iter().collect())
}

/// List all provider IDs that have been configured.
/// Checks both `active_provider` and `selected_models` for completeness.
#[must_use]
pub async fn list_configured_providers() -> Vec<String> {
    let service = ConfigService::new();
    let Ok(config) = service.load_config().await else {
        return vec![];
    };
    let mut ids = std::collections::HashSet::new();
    if let Some(ref p) = config.active_provider {
        if p.is_complete() {
            ids.insert(p.provider_id.clone());
        }
    }
    for m in &config.selected_models {
        ids.insert(m.provider_id.clone());
    }
    ids.into_iter().collect()
}

/// Select a model by saving the selection via `ConfigService`.
///
/// # Arguments
///
/// * `provider_id` - Provider ID
/// * `model_id` - Model ID to select
pub async fn select_model(
    provider_id: &str,
    model_id: &str,
    auth_variant: Option<&str>,
) -> Result<(), String> {
    use crate::modules::config::model_resolver::build_active_model_selection;

    let service = ConfigService::new();
    service
        .save_config(&{
            let mut config = service
                .load_config()
                .await
                .map_err(|e| format!("Failed to load config: {e}"))?;
            config.active_model = Some(build_active_model_selection(
                provider_id,
                model_id,
                auth_variant,
            ));
            config
        })
        .await
        .map_err(|e| format!("Failed to save model selection: {e}"))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::provider::registry::find_provider;

    #[test]
    fn test_list_providers_returns_all() {
        let providers = list_providers();
        assert_eq!(providers.len(), 16);
    }

    #[test]
    fn test_list_anthropic_models_returns_models() {
        let models = list_anthropic_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.contains("sonnet")));
    }

    #[test]
    fn test_find_provider_ollama() {
        let provider = find_provider("ollama");
        assert!(provider.is_some());
        assert!(provider.unwrap().is_local);
    }

    #[test]
    fn test_find_provider_nonexistent() {
        let provider = find_provider("nonexistent-provider");
        assert!(provider.is_none());
    }

    #[tokio::test]
    async fn test_list_ollama_models_returns_something() {
        // Ollama may or may not be running — just ensure the function doesn't panic
        let _ = list_models("ollama", "http://localhost:11434", None).await;
    }

    #[tokio::test]
    async fn test_list_openai_compat_models_fails_with_bad_url() {
        let result = list_models("openai", "http://invalid-host-12345.com", Some("sk-test")).await;
        assert!(result.is_err());
    }
}
