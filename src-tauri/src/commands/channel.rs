//! Channel Tauri commands — list, configure, test, and list configured channels.
//!
//! Provides 4 commands:
//! - `channel_list`: Returns all builtin channels
//! - `channel_configure`: Save channel configuration
//! - `channel_test`: Test channel connection (delegates to 5-platform test service)
//! - `channel_list_configured`: Returns configured channels (redacted, safe for frontend)

use crate::modules::channel::registry::builtin_channels;
use crate::modules::channel::test::test_channel_connection;
use crate::modules::channel::types::PlatformConfig;
use crate::modules::config::{ChannelConfig, ChannelConfigRedacted, ConfigService};
use crate::modules::provider::types::TestResult;

/// List all builtin channels.
#[tauri::command]
pub fn channel_list() -> Vec<crate::modules::channel::types::Channel> {
    builtin_channels()
}

/// Save channel configuration.
#[tauri::command]
pub async fn channel_configure(config: ChannelConfig) -> Result<(), String> {
    let svc = ConfigService::new();
    svc.save_channel(&config)
        .await
        .map_err(|e| format!("Failed to save channel: {e}"))
}

/// Test channel connection.
///
/// Dispatches to the platform-specific test service.
#[tauri::command]
pub async fn channel_test(config: ChannelConfig) -> Result<TestResult, String> {
    let platform_config = channel_config_to_platform(&config);
    let result = test_channel_connection(&config.channel_id, &platform_config).await;
    Ok(result)
}

/// List configured channels (redacted — safe for frontend).
#[tauri::command]
pub async fn channel_list_configured() -> Vec<ChannelConfigRedacted> {
    let svc = ConfigService::new();
    let config = svc.load_config().await.unwrap_or_default();
    config.channels.iter().map(|c| c.redact()).collect()
}

/// Convert a `ChannelConfig` to a `PlatformConfig` for testing.
///
/// Maps the generic fields to platform-specific fields based on channel_id.
fn channel_config_to_platform(config: &ChannelConfig) -> PlatformConfig {
    match config.channel_id.as_str() {
        "telegram" => PlatformConfig {
            bot_token: config.bot_token.clone(),
            ..Default::default()
        },
        "feishu" => PlatformConfig {
            app_id: config.bot_token.clone(),
            app_secret: config.app_secret.clone(),
            ..Default::default()
        },
        "qq" => PlatformConfig {
            app_id: config.bot_token.clone(),
            app_secret: config.app_secret.clone(),
            ..Default::default()
        },
        "whatsapp" => PlatformConfig {
            access_token: config.bot_token.clone(),
            phone_number_id: config.app_secret.clone(),
            ..Default::default()
        },
        "teams" => PlatformConfig {
            app_id_teams: config.bot_token.clone(),
            app_password: config.app_secret.clone(),
            ..Default::default()
        },
        _ => PlatformConfig {
            bot_token: config.bot_token.clone(),
            app_secret: config.app_secret.clone(),
            webhook_url: config.webhook_url.clone(),
            ..Default::default()
        },
    }
}
