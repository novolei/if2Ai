//! Provider Tauri commands — list, configure, test, and select providers.
//!
//! Provides commands for:
//! - Provider listing, configuration, and testing
//! - Model selection and listing
//! - Role-based model configuration (chat, utility, etc.)

use crate::modules::config::{ConfigService, ProviderConfig};
use crate::modules::provider::registry::builtin_providers;
use crate::modules::provider::service::{ModelCapabilitySelection, ThinkingProbeResult};
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
        auth_variant: None,
    };
    svc.save_model(&selection)
        .await
        .map_err(|e| format!("Failed to save model selection: {e}"))
}

/// Configure a provider with multiple model selections.
/// The first model is set as the default (active_model).
#[tauri::command]
pub async fn provider_configure_with_models(
    provider_config: ProviderConfig,
    model_ids: Vec<String>,
) -> Result<(), String> {
    crate::modules::provider::service::configure_provider_with_models(&provider_config, &model_ids)
        .await
}

#[tauri::command]
pub async fn provider_probe_model_thinking(
    provider_config: ProviderConfig,
    model_id: String,
) -> Result<ThinkingProbeResult, String> {
    crate::modules::provider::service::probe_model_thinking(&provider_config, &model_id).await
}

#[tauri::command]
pub async fn provider_configure_with_model_capabilities(
    provider_config: ProviderConfig,
    models: Vec<ModelCapabilitySelection>,
) -> Result<(), String> {
    crate::modules::provider::service::configure_provider_with_model_capabilities(
        &provider_config,
        &models,
    )
    .await
}

/// Get previously configured model IDs for a given provider.
#[tauri::command]
pub async fn provider_get_configured_models(provider_id: String) -> Result<Vec<String>, String> {
    crate::modules::provider::service::get_configured_models(&provider_id).await
}

/// Get the saved provider configuration from Layer 2 files.
/// Returns the stored base_url and api_key for the given provider_id.
#[tauri::command]
pub async fn provider_get_config(provider_id: String) -> Result<Option<ProviderConfig>, String> {
    let svc = ConfigService::new();
    svc.load_provider(&provider_id)
        .await
        .map_err(|e| format!("Failed to load provider config: {e}"))
}

/// List all provider IDs that have been configured.
#[tauri::command]
pub async fn provider_list_configured() -> Vec<String> {
    crate::modules::provider::service::list_configured_providers().await
}

/// Get all configured models grouped by provider.
/// Returns a list of [provider_id, [model_id, ...]] pairs.
#[tauri::command]
pub async fn provider_get_all_configured_models() -> Vec<(String, Vec<String>)> {
    crate::modules::provider::service::get_all_configured_models()
        .await
        .unwrap_or_default()
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

// ── Multi-model management commands (Slice 2) ──────────────────────────────

/// List all available models grouped by provider.
///
/// Combines configured models from `~/.if2ai/models.json` with
/// builtin provider names for display.
#[tauri::command]
pub async fn model_list_available(
) -> Vec<crate::modules::config::model_resolver::AvailableModelGroup> {
    crate::modules::config::model_resolver::ModelResolver::list_available_models().await
}

/// Get the current active model selection.
#[tauri::command]
pub async fn model_get_active() -> Result<Option<crate::modules::config::ModelSelection>, String> {
    crate::modules::config::model_resolver::ModelResolver::get_active_model().await
}

/// Set the active model (per-session selection).
#[tauri::command]
pub async fn model_set_active(provider_id: String, model_id: String) -> Result<(), String> {
    crate::modules::config::model_resolver::ModelResolver::set_active_model(&provider_id, &model_id)
        .await
}

/// Get role-based model assignments (chat, utility, summarizer, etc.).
#[tauri::command]
pub async fn model_get_role_config() -> Result<Vec<crate::modules::config::ModelRoleConfig>, String>
{
    crate::modules::config::model_resolver::ModelResolver::get_role_config().await
}

/// Set a role's model assignment.
///
/// # Arguments
///
/// * `role` - Role name: "chat", "utility", "utility_large", "summarizer", "compiler"
/// * `model_ref` - Model reference in "provider_id/model_id" format
#[tauri::command]
pub async fn model_set_role_config(role: String, model_ref: String) -> Result<(), String> {
    crate::modules::config::model_resolver::ModelResolver::set_role_config(&role, &model_ref)
        .await?;

    // The chat role is also the user-visible composer default. Keep the
    // shortcut active_model in lockstep so all windows can refresh from one
    // canonical command (`model_get_active`) after settings changes.
    if role == "chat" {
        let model = crate::modules::config::ModelRef::parse(&model_ref).ok_or_else(|| {
            format!("Invalid model reference '{model_ref}'. Expected 'provider_id/model_id'.")
        })?;
        crate::modules::config::model_resolver::ModelResolver::set_active_model(
            &model.provider_id,
            &model.model_id,
        )
        .await?;
    }

    Ok(())
}

/// Look up the provider-advertised context window (in tokens) for a
/// `(provider_id, model_id)` pair via the built-in `known_models`
/// registry.  Falls back to
/// [`crate::modules::application::provider_service::DEFAULT_CONTEXT_WINDOW`]
/// (128k) for models we don't know yet so the UI never has to handle
/// "unknown" specially.
#[tauri::command]
pub async fn model_get_context_window(
    provider_id: String,
    model_id: String,
) -> Result<u64, String> {
    Ok(
        crate::modules::provider::known_models::lookup(&provider_id, &model_id)
            .map(|m| m.context)
            .unwrap_or(crate::modules::application::provider_service::DEFAULT_CONTEXT_WINDOW),
    )
}
