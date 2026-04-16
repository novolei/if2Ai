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
    AppConfig, AuthEntry, AuthJson, ModelEntry, ModelsJson, ProviderYamlEntry, ProvidersYaml,
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
/// Includes all configured providers with their base_url and api type.
fn build_providers_yaml(config: &AppConfig) -> ProvidersYaml {
    let mut providers = HashMap::new();

    if let Some(ref provider) = config.active_provider {
        let api_type = detect_api_type(&provider.provider_id, &provider.base_url);
        providers.insert(
            provider.provider_id.clone(),
            ProviderYamlEntry {
                api_key: provider.api_key.clone(),
                base_url: provider.base_url.clone(),
                api: Some(api_type),
            },
        );
    }

    ProvidersYaml { providers }
}

/// Build auth.json from AppConfig.
///
/// Contains only API keys, keyed by provider_id.
fn build_auth_json(config: &AppConfig) -> AuthJson {
    let mut providers = HashMap::new();

    if let Some(ref provider) = config.active_provider {
        if provider.api_key.is_some() {
            providers.insert(
                provider.provider_id.clone(),
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
fn build_models_json(config: &AppConfig) -> ModelsJson {
    let mut providers = HashMap::new();

    if let (Some(provider), Some(model)) = (&config.active_provider, &config.active_model) {
        let api_type = detect_api_type(&provider.provider_id, &provider.base_url);
        providers.insert(
            provider.provider_id.clone(),
            crate::modules::config::types::ModelsProviderEntry {
                base_url: provider.base_url.clone(),
                api: Some(api_type),
                api_key: provider.api_key.clone(),
                models: vec![ModelEntry {
                    id: model.model_id.clone(),
                    name: model.model_id.clone(),
                    input: vec!["text".to_string()],
                    context_window: None,
                }],
            },
        );
    }

    ModelsJson { providers }
}

/// Detect the API type string for providers.yaml.
fn detect_api_type(provider_id: &str, base_url: &Option<String>) -> String {
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
            }),
            active_model: Some(ModelSelection {
                provider_id: "ollama".to_string(),
                model_id: "qwen3:4b".to_string(),
            }),
            channels: vec![],
            routing: None,
            onboarding,
            security_confirmed: false,
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
}
