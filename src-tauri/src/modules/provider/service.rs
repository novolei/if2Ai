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
use crate::modules::config::{ConfigService, ProviderConfig};

use super::client::PROVIDER_HTTP_CLIENT;
use super::registry::builtin_providers;
use super::types::{Model, ModelModality, Provider};

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
            Some(Model {
                id: name.to_string(),
                name: canonical,
                context_window: None,
                max_tokens: None,
                modality: ModelModality::Text,
            })
        })
        .collect())
}

/// Return known Anthropic/Claude models from builtin registry.
///
/// Anthropic doesn't expose a public model listing endpoint,
/// so we use a hardcoded list of known models.
fn list_anthropic_models() -> Vec<Model> {
    vec![
        Model {
            id: "claude-opus-4-6".to_string(),
            name: "Claude Opus 4.6".to_string(),
            context_window: Some(200_000),
            max_tokens: Some(32_000),
            modality: ModelModality::Text,
        },
        Model {
            id: "claude-sonnet-4-6".to_string(),
            name: "Claude Sonnet 4.6".to_string(),
            context_window: Some(200_000),
            max_tokens: Some(64_000),
            modality: ModelModality::Text,
        },
        Model {
            id: "claude-sonnet-4-5-20250514".to_string(),
            name: "Claude Sonnet 4.5".to_string(),
            context_window: Some(200_000),
            max_tokens: Some(64_000),
            modality: ModelModality::Text,
        },
        Model {
            id: "claude-haiku-4-5-20251213".to_string(),
            name: "Claude Haiku 4.5".to_string(),
            context_window: Some(200_000),
            max_tokens: Some(8_000),
            modality: ModelModality::Text,
        },
    ]
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

    Ok(models
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(|v| v.as_str())?;
            let canonical = resolve_model_alias(id);
            Some(Model {
                id: id.to_string(),
                name: canonical,
                context_window: None,
                max_tokens: None,
                modality: ModelModality::Text,
            })
        })
        .collect())
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
pub async fn select_model(provider_id: &str, model_id: &str) -> Result<(), String> {
    use crate::modules::config::ModelSelection;

    let service = ConfigService::new();
    service
        .save_config(&{
            let mut config = service
                .load_config()
                .await
                .map_err(|e| format!("Failed to load config: {e}"))?;
            config.active_model = Some(ModelSelection {
                provider_id: provider_id.to_string(),
                model_id: model_id.to_string(),
                auth_variant: None,
            });
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
