//! ConfigService — unified configuration management.
//!
//! Implements the trait-based config service defined in ADR-014 Section 4.2.
//!
//! `ConfigService::save_config()` performs a **dual-write**:
//! 1. Layer 1: `~/.if2ai/config.json` (shortcut format)
//! 2. Layer 2: `~/.if2ai/providers.yaml` + `auth.json` + `models.json` (runtime format)
//!
//! Concurrency is protected by `tokio::sync::Mutex` — only one write
//! operation can run at a time within the same process.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::modules::config::store::{
    auth_json_path, channels_config_path, config_json_path, delete_file, models_json_path,
    providers_yaml_path, read_json, read_yaml, write_json, ConfigStoreError,
};
use crate::modules::config::triple_files::sync_to_triple_files;
use crate::modules::config::types::{
    AppConfig, AuthEntry, AuthJson, ChannelConfig, ChannelRouting, ModelSelection, ProviderConfig,
    ProviderYamlEntry, ProvidersYaml,
};
use crate::modules::onboarding::store::{delete_state, load_state, save_state};

/// Unified configuration service.
///
/// Manages all onboarding configuration with concurrency protection.
pub struct ConfigService {
    /// Mutex guard for write operations — prevents concurrent writes.
    write_lock: Arc<Mutex<()>>,
}

impl ConfigService {
    /// Create a new `ConfigService` instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Load the full application configuration.
    ///
    /// Reads from `~/.if2ai/config.json` (Layer 1).
    /// If the file does not exist, returns a fresh default config with
    /// the onboarding state loaded from `~/.if2ai/state.json`.
    ///
    /// Performs version migration: if `config.version < CONFIG_VERSION`,
    /// the config is upgraded in-place before returning.
    pub async fn load_config(&self) -> Result<AppConfig, ConfigStoreError> {
        let path = config_json_path();
        let config = read_json::<AppConfig>(&path).await?;

        match config {
            Some(mut config) => {
                // Version migration
                if config.version < crate::modules::config::types::CONFIG_VERSION {
                    config = migrate_config(config);
                    // Write upgraded version back
                    write_json(&config, &path).await?;
                }
                Ok(config)
            }
            None => {
                // No config.json yet — build from onboarding state
                let onboarding = load_state().await.map_err(|e| {
                    ConfigStoreError::Io(
                        std::io::Error::other(format!("{e}")),
                        "load onboarding state".to_string(),
                    )
                })?;
                let config = AppConfig {
                    onboarding,
                    ..AppConfig::new()
                };

                Ok(config)
            }
        }
    }

    /// Save the full application configuration.
    ///
    /// Performs the dual-write:
    /// 1. Layer 1: `config.json`
    /// 2. Layer 2: `providers.yaml` + `auth.json` + `models.json`
    ///
    /// All writes are protected by a mutex to prevent concurrent corruption.
    pub async fn save_config(&self, config: &AppConfig) -> Result<(), ConfigStoreError> {
        let _guard = self.write_lock.lock().await;

        // Step 1: Write Layer 1 (config.json)
        write_json(config, &config_json_path()).await?;

        // Step 2: Sync to Layer 2 (triple files)
        sync_to_triple_files(config).await?;

        // Also save onboarding state to state.json
        save_state(&config.onboarding).await.map_err(|e| {
            ConfigStoreError::Io(
                std::io::Error::other(format!("{e}")),
                "save onboarding state".to_string(),
            )
        })?;

        Ok(())
    }

    /// Save a provider configuration to Layer 2 files.
    ///
    /// Updates providers.yaml and auth.json with the given provider,
    /// merging with existing entries instead of replacing the entire file.
    pub async fn save_provider(&self, provider: &ProviderConfig) -> Result<(), ConfigStoreError> {
        let _guard = self.write_lock.lock().await;

        // Read existing providers.yaml, merge in new provider
        let mut existing = read_yaml::<ProvidersYaml>(&providers_yaml_path())
            .await?
            .unwrap_or_else(|| ProvidersYaml {
                providers: std::collections::HashMap::new(),
            });
        let api_type = crate::modules::config::triple_files::detect_api_type(
            &provider.provider_id,
            &provider.base_url,
        );
        existing.providers.insert(
            provider.provider_id.clone(),
            ProviderYamlEntry {
                api_key: provider.api_key.clone(),
                base_url: provider.base_url.clone(),
                api: Some(api_type),
            },
        );
        crate::modules::config::store::write_yaml(&existing, &providers_yaml_path()).await?;

        // Read existing auth.json, merge in new API key if present
        if provider.api_key.is_some() {
            let mut existing_auth = read_json::<AuthJson>(&auth_json_path())
                .await?
                .unwrap_or_else(|| AuthJson {
                    providers: std::collections::HashMap::new(),
                });
            existing_auth.providers.insert(
                provider.provider_id.clone(),
                AuthEntry {
                    api_key: provider.api_key.clone(),
                },
            );
            write_json(&existing_auth, &auth_json_path()).await?;
        }

        Ok(())
    }

    /// Save the active model selection to Layer 2 models.json.
    pub async fn save_model(&self, model: &ModelSelection) -> Result<(), ConfigStoreError> {
        let _guard = self.write_lock.lock().await;
        let models_json = crate::modules::config::types::ModelsJson {
            providers: [(
                model.provider_id.clone(),
                crate::modules::config::types::ModelsProviderEntry {
                    base_url: None,
                    api: Some("openai-completions".to_string()),
                    api_key: None,
                    models: vec![crate::modules::config::types::ModelEntry {
                        id: model.model_id.clone(),
                        name: model.model_id.clone(),
                        input: vec!["text".to_string()],
                        context_window: None,
                    }],
                },
            )]
            .into_iter()
            .collect(),
        };
        write_json(&models_json, &models_json_path()).await?;
        Ok(())
    }

    /// Read a provider configuration from Layer 2 files.
    ///
    /// Reads providers.yaml for base_url + api type,
    /// and auth.json for the API key.
    /// Returns `None` if the provider is not found in providers.yaml.
    pub async fn load_provider(
        &self,
        provider_id: &str,
    ) -> Result<Option<ProviderConfig>, ConfigStoreError> {
        let yaml = read_yaml::<ProvidersYaml>(&providers_yaml_path()).await?;
        let Some(yaml) = yaml else { return Ok(None) };
        let Some(entry) = yaml.providers.get(provider_id) else {
            return Ok(None);
        };

        // Read API key from auth.json (overrides providers.yaml)
        let auth = read_json::<AuthJson>(&auth_json_path()).await?;
        let api_key = auth
            .as_ref()
            .and_then(|a| a.providers.get(provider_id))
            .and_then(|e| e.api_key.clone())
            .or_else(|| entry.api_key.clone());

        Ok(Some(ProviderConfig {
            provider_id: provider_id.to_string(),
            display_name: provider_id.to_string(),
            api_key,
            base_url: entry.base_url.clone(),
            auth_variant: None,
        }))
    }

    /// Save a channel configuration.
    ///
    /// Appends to the channels list in config.json and writes
    /// channels-config.json for runtime consumption.
    pub async fn save_channel(&self, channel: &ChannelConfig) -> Result<(), ConfigStoreError> {
        let _guard = self.write_lock.lock().await;

        // Update config.json channels list
        let mut config = self.load_config().await?;

        // Replace existing channel or append
        let existing = config
            .channels
            .iter()
            .position(|c| c.channel_id == channel.channel_id);
        if let Some(idx) = existing {
            config.channels[idx] = channel.clone();
        } else {
            config.channels.push(channel.clone());
        }

        // Write routing defaults if first channel
        if config.channels.len() == 1 && config.routing.is_none() {
            config.routing = Some(ChannelRouting::default());
        }

        write_json(&config, &config_json_path()).await?;

        // Write channels-config.json
        let channels_config = serde_json::json!({
            "enabled": true,
            "platforms": {
                &channel.channel_id: {
                    "enabled": true,
                    "bot_token": channel.bot_token,
                    "app_secret": channel.app_secret,
                    "webhook_url": channel.webhook_url,
                }
            },
            "routing": config.routing.as_ref().map(|r| {
                serde_json::json!({
                    "owner_user_ids": r.owner_user_ids,
                    "default_agent_id": r.default_agent_id,
                    "debounce_ms": r.debounce_ms,
                    "rate_limit_per_minute": r.rate_limit_per_minute,
                })
            }),
        });
        write_json(&channels_config, &channels_config_path()).await?;

        Ok(())
    }

    /// Validate the current configuration.
    ///
    /// Returns a list of validation issues (empty if config is complete).
    pub async fn validate_config(&self) -> Result<Vec<String>, ConfigStoreError> {
        let config = self.load_config().await?;
        Ok(config.validate())
    }

    /// Reset all onboarding configuration.
    ///
    /// Deletes:
    /// - `~/.if2ai/state.json`
    /// - `~/.if2ai/config.json`
    /// - `~/.if2ai/providers.yaml`
    /// - `~/.if2ai/auth.json`
    /// - `~/.if2ai/models.json`
    /// - `~/.if2ai/channels-config.json`
    ///
    /// Does NOT delete:
    /// - `~/.if2ai/memory_config.json`
    /// - `~/.if2ai/trajectories/`
    /// - `~/.if2ai/models/embedded-rs/`
    pub async fn reset_onboarding(&self) -> Result<(), ConfigStoreError> {
        let _guard = self.write_lock.lock().await;

        // Delete onboarding state
        delete_state().await.map_err(|e| {
            ConfigStoreError::Io(
                std::io::Error::other(format!("{e}")),
                "delete onboarding state".to_string(),
            )
        })?;

        // Delete config files
        for path_fn in [
            config_json_path,
            providers_yaml_path,
            auth_json_path,
            models_json_path,
            channels_config_path,
        ] {
            delete_file(&path_fn()).await?;
        }

        Ok(())
    }
}

impl Default for ConfigService {
    fn default() -> Self {
        Self::new()
    }
}

// ── Migration ───────────────────────────────────────────────────────────────

/// Migrate an older config format to the current version.
fn migrate_config(mut config: AppConfig) -> AppConfig {
    // Version 0 → 1: initial migration (no fields changed yet)
    // Future migrations go here as version bumps occur
    if config.version == 0 {
        config.version = 1;
    }
    config
}

// ── Bridge helpers ──────────────────────────────────────────────────────────

/// Extract a `ProviderConfig` from bridge settings (if present).
#[cfg(test)]
fn extract_provider_from_bridge(
    bridge: crate::modules::config::bridge::ClawSettings,
) -> Option<ProviderConfig> {
    let env = bridge.env.as_ref()?;

    // Try to find API key in known env vars
    let api_key = env
        .get("ANTHROPIC_API_KEY")
        .or_else(|| env.get("OPENAI_API_KEY"))
        .or_else(|| env.get("ZAI_API_KEY"))
        .cloned();

    let base_url = env
        .get("ANTHROPIC_BASE_URL")
        .or_else(|| env.get("OPENAI_BASE_URL"))
        .cloned();

    let provider_id = if env.contains_key("ANTHROPIC_API_KEY") {
        "anthropic"
    } else if env.contains_key("OPENAI_API_KEY") {
        "openai"
    } else if env.contains_key("ZAI_API_KEY") {
        "zai"
    } else if api_key.is_some() {
        "custom"
    } else {
        return None;
    };

    Some(ProviderConfig {
        provider_id: provider_id.to_string(),
        display_name: provider_id.to_string(),
        api_key,
        base_url,
        auth_variant: None,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::onboarding::OnboardingState;

    #[tokio::test]
    async fn test_config_service_new() {
        let service = ConfigService::new();
        assert!(Arc::strong_count(&service.write_lock) == 1);
    }

    #[test]
    fn test_migrate_config_version_zero() {
        let onboarding = OnboardingState::new();
        let config = AppConfig {
            version: 0,
            active_provider: None,
            active_model: None,
            selected_models: vec![],
            role_models: vec![],
            channels: vec![],
            routing: None,
            onboarding,
            security_confirmed: false,
        };

        let migrated = migrate_config(config);
        assert_eq!(migrated.version, 1);
    }

    #[test]
    fn test_migrate_config_version_one_no_change() {
        let onboarding = OnboardingState::new();
        let config = AppConfig {
            version: 1,
            active_provider: None,
            active_model: None,
            selected_models: vec![],
            role_models: vec![],
            channels: vec![],
            routing: None,
            onboarding,
            security_confirmed: false,
        };

        let migrated = migrate_config(config);
        assert_eq!(migrated.version, 1);
    }

    #[test]
    fn test_extract_provider_from_bridge_anthropic() {
        use crate::modules::config::bridge::ClawSettings;
        use std::collections::HashMap;

        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-ant-123".to_string());
        env.insert(
            "ANTHROPIC_BASE_URL".to_string(),
            "https://api.anthropic.com".to_string(),
        );
        env.insert("model".to_string(), "claude-sonnet-4-6".to_string());

        let bridge = ClawSettings {
            env: Some(env),
            model: Some("claude-sonnet-4-6".to_string()),
            other: HashMap::new(),
        };

        let provider = extract_provider_from_bridge(bridge).unwrap();
        assert_eq!(provider.provider_id, "anthropic");
        assert_eq!(provider.api_key, Some("sk-ant-123".to_string()));
    }

    #[test]
    fn test_extract_provider_from_bridge_openai() {
        use crate::modules::config::bridge::ClawSettings;
        use std::collections::HashMap;

        let mut env = HashMap::new();
        env.insert("OPENAI_API_KEY".to_string(), "sk-123".to_string());

        let bridge = ClawSettings {
            env: Some(env),
            model: None,
            other: HashMap::new(),
        };

        let provider = extract_provider_from_bridge(bridge).unwrap();
        assert_eq!(provider.provider_id, "openai");
    }

    #[test]
    fn test_extract_provider_from_bridge_empty_returns_none() {
        use crate::modules::config::bridge::ClawSettings;
        use std::collections::HashMap;

        let bridge = ClawSettings {
            env: Some(HashMap::new()),
            model: None,
            other: HashMap::new(),
        };

        assert!(extract_provider_from_bridge(bridge).is_none());
    }
}
