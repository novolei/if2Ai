//! Configuration Tauri commands — load, save, validate, and reset onboarding config.
//!
//! Provides 4 commands:
//! - `config_load`: Load the full AppConfig
//! - `config_save`: Save the full AppConfig
//! - `config_validate`: Validate config and return list of issues
//! - `config_reset_onboarding`: Reset onboarding state and clear config

use crate::modules::config::{AppConfig, ConfigService};

/// Load the full application configuration.
#[tauri::command]
pub async fn config_load() -> Result<AppConfig, String> {
    let svc = ConfigService::new();
    svc.load_config()
        .await
        .map_err(|e| format!("Failed to load config: {e}"))
}

/// Save the full application configuration.
#[tauri::command]
pub async fn config_save(config: AppConfig) -> Result<(), String> {
    let svc = ConfigService::new();
    svc.save_config(&config)
        .await
        .map_err(|e| format!("Failed to save config: {e}"))
}

/// Validate the current configuration.
///
/// Returns a list of validation issues (empty if valid).
#[tauri::command]
pub async fn config_validate() -> Result<Vec<String>, String> {
    let svc = ConfigService::new();
    svc.validate_config()
        .await
        .map_err(|e| format!("Validation failed: {e}"))
}

/// Reset onboarding state and clear configuration.
///
/// Deletes state.json and resets config.json to defaults.
/// Does NOT affect memory_config.json or trajectories.
#[tauri::command]
pub async fn config_reset_onboarding() -> Result<(), String> {
    let svc = ConfigService::new();
    svc.reset_onboarding()
        .await
        .map_err(|e| format!("Failed to reset onboarding: {e}"))
}
