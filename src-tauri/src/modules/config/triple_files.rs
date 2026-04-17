//! Triple-file synchronization (Layer 2).
//!
//! Converts an `AppConfig` (Layer 1, single JSON) into the three
//! runtime-consumable files:
//! - `~/.if2ai/providers.yaml` — provider definitions (base_url, api type)
//! - `~/.if2ai/auth.json` — API keys (overrides providers.yaml)
//! - `~/.if2ai/models.json` — model registry (pi-coding-agent compatible)
//!
//! These files are consumed by `ConfigLoader` at runtime.

use std::collections::HashMap;

use crate::modules::config::store::{
    auth_json_path, models_json_path, providers_yaml_path, write_json, write_yaml, ConfigStoreError,
};
use crate::modules::config::types::{
    AppConfig, AuthEntry, AuthJson, ModelEntry, ModelsJson, ModelsProviderEntry, ProviderConfig,
    ProviderYamlEntry, ProvidersYaml,
};

/// Synchronize `AppConfig` to the three Layer 2 files.
///
/// This is called internally by `ConfigService::save_config()`.
/// It writes all three files; if any write fails, the error is returned
/// (no rollback — the caller decides how to handle partial writes).
pub async fn sync_to_triple_files(config: &AppConfig) -> Result<(), ConfigStoreError> {
    let providers_yaml = build_providers_yaml(config);
    let auth_json = build_auth_json(config);
    let models_json = build_models_json(config);

    write_yaml(&providers_yaml, &providers_yaml_path()).await?;
    write_json(&auth_json, &auth_json_path()).await?;
    write_json(&models_json, &models_json_path()).await?;

    Ok(())
}

/// Build providers.yaml from AppConfig.
///
/// Writes ALL entries from `configured_providers` (the cumulative list), so that
/// switching the active provider never erases a previously configured provider's
/// connection info from `providers.yaml`.
///
/// Falls back to `active_provider` for backward compatibility with configs that
/// were written before `configured_providers` was introduced.
pub(crate) fn build_providers_yaml(config: &AppConfig) -> ProvidersYaml {
    let mut providers = HashMap::new();

    // Primary source: configured_providers (accumulates all ever-configured providers)
    for provider in &config.configured_providers {
        let key = provider_key(&provider.provider_id, provider.auth_variant.as_deref());
        let api_type = detect_api_type(&provider.provider_id, &provider.base_url);
        providers.insert(
            key,
            ProviderYamlEntry {
                api_key: provider.api_key.clone(),
                base_url: provider.base_url.clone(),
                api: Some(api_type),
            },
        );
    }

    // Fallback: ensure active_provider is always present (covers old configs without
    // configured_providers, and the case where configured_providers is empty).
    if let Some(ref provider) = config.active_provider {
        let key = provider_key(&provider.provider_id, provider.auth_variant.as_deref());
        providers.entry(key).or_insert_with(|| {
            let api_type = detect_api_type(&provider.provider_id, &provider.base_url);
            ProviderYamlEntry {
                api_key: provider.api_key.clone(),
                base_url: provider.base_url.clone(),
                api: Some(api_type),
            }
        });
    }

    ProvidersYaml { providers }
}

/// Build auth.json from AppConfig.
///
/// Contains API keys for all configured providers, keyed by `{provider_id}::{auth_variant}`
/// when auth_variant is present, or just `provider_id` otherwise.
pub(crate) fn build_auth_json(config: &AppConfig) -> AuthJson {
    let mut providers = HashMap::new();

    // Collect API keys from active_provider
    if let Some(ref provider) = config.active_provider {
        if provider.api_key.is_some() {
            let key = provider_key(&provider.provider_id, provider.auth_variant.as_deref());
            providers.insert(
                key,
                AuthEntry {
                    api_key: provider.api_key.clone(),
                },
            );
        }
    }

    AuthJson { providers }
}

/// Build models.json from AppConfig.
///
/// Compatible with pi-coding-agent ModelsConfigSchema.
/// Syncs ALL selected_models (not just active_provider) to ensure
/// every configured model is available at runtime.
///
/// Connection details (base_url, api_key) are resolved via the priority chain:
/// 1. `configured_providers` — cumulative store of all ever-configured providers
/// 2. `active_provider` — fallback for configs that predate `configured_providers`
pub(crate) fn build_models_json(config: &AppConfig) -> ModelsJson {
    let mut providers = HashMap::new();

    // Helper: look up a provider's connection details from the cumulative store first,
    // then fall back to active_provider for backward compatibility.
    let find_provider = |pid: &str, av: Option<&str>| -> Option<&ProviderConfig> {
        config.find_configured_provider(pid, av).or_else(|| {
            config
                .active_provider
                .as_ref()
                .filter(|p| p.provider_id == pid && p.auth_variant.as_deref() == av)
        })
    };

    // 1. Sync all selected_models
    for selection in &config.selected_models {
        let key = provider_key(&selection.provider_id, selection.auth_variant.as_deref());
        let provider = find_provider(&selection.provider_id, selection.auth_variant.as_deref());

        let entry = providers.entry(key).or_insert_with(|| {
            let api_type = detect_api_type(
                &selection.provider_id,
                &provider.and_then(|p| p.base_url.clone()),
            );
            ModelsProviderEntry {
                base_url: provider.and_then(|p| p.base_url.clone()),
                api: Some(api_type),
                api_key: provider.and_then(|p| p.api_key.clone()),
                models: vec![],
            }
        });

        entry.models.push(ModelEntry {
            id: selection.model_id.clone(),
            name: selection.model_id.clone(),
            input: vec!["text".to_string()],
            context_window: None,
        });
    }

    // 2. Also include active_provider/active_model if not already covered
    if let (Some(provider), Some(model)) = (&config.active_provider, &config.active_model) {
        let key = provider_key(&provider.provider_id, provider.auth_variant.as_deref());
        if let std::collections::hash_map::Entry::Vacant(e) = providers.entry(key) {
            let api_type = detect_api_type(&provider.provider_id, &provider.base_url);
            e.insert(ModelsProviderEntry {
                base_url: provider.base_url.clone(),
                api: Some(api_type),
                api_key: provider.api_key.clone(),
                models: vec![ModelEntry {
                    id: model.model_id.clone(),
                    name: model.model_id.clone(),
                    input: vec!["text".to_string()],
                    context_window: None,
                }],
            });
        }
    }

    ModelsJson { providers }
}

/// Generate a unique key for a provider, accounting for auth variants.
///
/// When auth_variant is present, returns `"{provider_id}::{auth_variant}"`
/// to distinguish same-provider different-auth configs.
/// Otherwise returns just `provider_id` for backward compatibility.
fn provider_key(provider_id: &str, auth_variant: Option<&str>) -> String {
    match auth_variant {
        Some(variant) => format!("{provider_id}::{variant}"),
        None => provider_id.to_string(),
    }
}

/// Detect the API type string for providers.yaml.
pub(crate) fn detect_api_type(provider_id: &str, base_url: &Option<String>) -> String {
    match provider_id {
        "ollama" => "openai-completions".to_string(),
        "openai" | "openrouter" | "xai" | "google" | "zai" | "moonshot" | "qwen" | "minimax"
        | "qianfan" | "xiaomi" | "volcengine" | "byteplus" | "anthropic" => {
            "openai-completions".to_string()
        }
        // Check base URL for Anthropic protocol hint
        _ => {
            if base_url
                .as_ref()
                .is_some_and(|u| u.contains("anthropic.com"))
            {
                "anthropic-messages".to_string()
            } else {
                "openai-completions".to_string()
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::config::types::{ModelSelection, ProviderConfig};
    use crate::modules::onboarding::OnboardingState;

    fn test_config() -> AppConfig {
        let mut onboarding = OnboardingState::new();
        onboarding.current_step = 4;

        AppConfig {
            version: 1,
            active_provider: Some(ProviderConfig {
                provider_id: "ollama".to_string(),
                display_name: "Ollama (Local)".to_string(),
                api_key: None,
                base_url: Some("http://localhost:11434".to_string()),
                auth_variant: None,
            }),
            active_model: Some(ModelSelection {
                provider_id: "ollama".to_string(),
                model_id: "qwen3:4b".to_string(),
                auth_variant: None,
            }),
            selected_models: vec![],
            role_models: vec![],
            channels: vec![],
            routing: None,
            onboarding,
            security_confirmed: false,
            configured_providers: vec![],
        }
    }

    #[test]
    fn test_build_providers_yaml_has_ollama() {
        let config = test_config();
        let yaml = build_providers_yaml(&config);

        assert!(yaml.providers.contains_key("ollama"));
        let entry = yaml.providers.get("ollama").unwrap();
        assert_eq!(entry.base_url, Some("http://localhost:11434".to_string()));
        assert_eq!(entry.api, Some("openai-completions".to_string()));
    }

    #[test]
    fn test_build_auth_json_has_no_keys_for_ollama() {
        let config = test_config();
        let auth = build_auth_json(&config);

        // Ollama has no api_key, so auth.json should be empty
        assert!(auth.providers.is_empty());
    }

    #[test]
    fn test_build_auth_json_has_key_for_cloud_provider() {
        let mut config = test_config();
        if let Some(ref mut p) = config.active_provider {
            p.provider_id = "openai".to_string();
            p.api_key = Some("sk-test".to_string());
        }
        if let Some(ref mut m) = config.active_model {
            m.provider_id = "openai".to_string();
        }

        let auth = build_auth_json(&config);
        assert!(auth.providers.contains_key("openai"));
        assert_eq!(
            auth.providers.get("openai").unwrap().api_key,
            Some("sk-test".to_string())
        );
    }

    #[test]
    fn test_build_models_json_has_model() {
        let config = test_config();
        let models = build_models_json(&config);

        assert!(models.providers.contains_key("ollama"));
        let entry = models.providers.get("ollama").unwrap();
        assert_eq!(entry.models.len(), 1);
        assert_eq!(entry.models[0].id, "qwen3:4b");
    }

    #[test]
    fn test_detect_api_type_ollama() {
        assert_eq!(
            detect_api_type("ollama", &Some("http://localhost:11434".to_string())),
            "openai-completions"
        );
    }

    #[test]
    fn test_detect_api_type_anthropic_by_url() {
        assert_eq!(
            detect_api_type("unknown", &Some("https://api.anthropic.com/v1".to_string())),
            "anthropic-messages"
        );
    }

    #[test]
    fn test_detect_api_type_default() {
        assert_eq!(detect_api_type("unknown", &None), "openai-completions");
    }

    #[test]
    fn test_build_models_json_syncs_all_selected_models() {
        let mut onboarding = OnboardingState::new();
        onboarding.current_step = 4;

        let config = AppConfig {
            version: 1,
            active_provider: Some(ProviderConfig {
                provider_id: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                api_key: Some("sk-test".to_string()),
                base_url: Some("https://api.openai.com/v1".to_string()),
                auth_variant: None,
            }),
            active_model: Some(ModelSelection {
                provider_id: "openai".to_string(),
                model_id: "gpt-4".to_string(),
                auth_variant: None,
            }),
            selected_models: vec![
                ModelSelection {
                    provider_id: "openai".to_string(),
                    model_id: "gpt-4".to_string(),
                    auth_variant: None,
                },
                ModelSelection {
                    provider_id: "openai".to_string(),
                    model_id: "gpt-4o".to_string(),
                    auth_variant: None,
                },
                ModelSelection {
                    provider_id: "ollama".to_string(),
                    model_id: "qwen3:4b".to_string(),
                    auth_variant: None,
                },
            ],
            role_models: vec![],
            channels: vec![],
            routing: None,
            onboarding,
            security_confirmed: false,
            configured_providers: vec![],
        };

        let models = build_models_json(&config);
        // Should have 2 provider entries
        assert_eq!(models.providers.len(), 2);
        // OpenAI should have 2 models
        let openai = models.providers.get("openai").unwrap();
        assert_eq!(openai.models.len(), 2);
        // Ollama should have 1 model (without active_provider config, uses active_provider as fallback)
        let ollama = models.providers.get("ollama").unwrap();
        assert_eq!(ollama.models.len(), 1);
    }

    #[test]
    fn test_provider_key_with_auth_variant() {
        assert_eq!(provider_key("moonshot", None), "moonshot");
        assert_eq!(provider_key("moonshot", Some("cn")), "moonshot::cn");
        assert_eq!(provider_key("moonshot", Some("code")), "moonshot::code");
    }
}
