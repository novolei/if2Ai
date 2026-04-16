//! Provider Tauri commands — list, configure, test, and select providers.
//!
//! Provides 6 commands:
//! - `provider_list`: Returns all builtin providers
//! - `provider_configure`: Save provider configuration
//! - `provider_test`: Test provider connection
//! - `provider_list_models`: List models from a provider (supports base_url + api_key)
//! - `model_select`: Select an active model
//! - `model_test`: Test a specific model

use crate::modules::config::{ConfigService, ProviderConfig};
use crate::modules::provider::registry::builtin_providers;
use crate::modules::provider::test::test_provider_connection;
use crate::modules::provider::types::{Model, TestResult};

/// List all builtin providers.
#[tauri::command]
pub fn provider_list() -> Vec<crate::modules::provider::types::Provider> {
    builtin_providers()
}

/// Save provider configuration.
#[tauri::command]
pub async fn provider_configure(config: ProviderConfig) -> Result<(), String> {
    let svc = ConfigService::new();
    svc.save_provider(&config)
        .await
        .map_err(|e| format!("Failed to save provider: {e}"))
}

/// Test provider connection.
#[tauri::command]
pub async fn provider_test(config: ProviderConfig) -> Result<TestResult, String> {
    let base_url = config
        .base_url
        .clone()
        .unwrap_or_else(|| default_base_url(&config.provider_id));
    let result =
        test_provider_connection(&config.provider_id, &base_url, config.api_key.as_deref()).await;
    Ok(result)
}

/// List models from a provider.
///
/// Supports `base_url` and `api_key` for custom/remote providers.
#[tauri::command]
pub async fn provider_list_models(
    provider_id: String,
    base_url: String,
    api_key: Option<String>,
) -> Result<Vec<Model>, String> {
    crate::modules::provider::service::list_models(&provider_id, &base_url, api_key.as_deref())
        .await
}

/// Select an active model.
#[tauri::command]
pub async fn model_select(provider_id: String, model_id: String) -> Result<(), String> {
    let svc = ConfigService::new();
    let selection = crate::modules::config::ModelSelection {
        provider_id,
        model_id,
    };
    svc.save_model(&selection)
        .await
        .map_err(|e| format!("Failed to save model selection: {e}"))
}

/// Test a specific model by sending a test prompt.
#[tauri::command]
pub async fn model_test(_provider_id: String, model_id: String) -> Result<TestResult, String> {
    let svc = ConfigService::new();
    let config = svc.load_config().await.map_err(|e| e.to_string())?;
    let provider = config
        .active_provider
        .ok_or_else(|| "No provider configured".to_string())?;
    crate::modules::provider::test::test_model(&provider, &model_id)
        .await
        .map_err(|e| format!("Model test failed: {e}"))
}

/// Get the default base URL for a builtin provider.
fn default_base_url(provider_id: &str) -> String {
    match provider_id {
        "ollama" => "http://localhost:11434".to_string(),
        _ => "".to_string(),
    }
}
