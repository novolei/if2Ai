//! Activation Tauri commands — validate, start, test, and complete activation.
//!
//! Provides 4 commands per ADR-014 Section 18.9:
//! - `activation_validate`: Check that all preconditions are met
//! - `activation_start`: Start the agent for the first time
//! - `activation_test_message`: Send a test message to verify end-to-end
//! - `activation_complete`: Mark activation as complete

use serde::{Deserialize, Serialize};

use crate::modules::config::{ChannelRouting, ConfigService};
use crate::modules::onboarding::flow::OnboardingFlow;
use crate::modules::onboarding::store::{load_state, save_state};
use crate::modules::provider::types::TestResult;

/// Activation checklist — all items that must be verified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationChecklist {
    pub system_check: bool,
    pub security_confirmed: bool,
    pub provider_configured: bool,
    pub channels_configured: bool,
}

/// Result of starting the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationResult {
    pub success: bool,
    pub session_id: Option<String>,
    pub message: String,
    /// First response from the configured LLM — displayed during ceremony.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_response: Option<String>,
}

/// Validate that all activation preconditions are met.
#[tauri::command]
pub async fn activation_validate() -> Result<ActivationChecklist, String> {
    let svc = ConfigService::new();
    let config = svc.load_config().await.map_err(|e| e.to_string())?;

    Ok(ActivationChecklist {
        system_check: true, // System check was run if we're at Step 6
        security_confirmed: config.security_confirmed,
        provider_configured: config
            .active_provider
            .as_ref()
            .is_some_and(|p| p.is_complete()),
        channels_configured: !config.channels.is_empty(),
    })
}

/// Start the agent for the first time.
///
/// Sends a real greeting to the configured chat model, receives the response,
/// and returns it for the onboarding ceremony display.
#[tauri::command]
pub async fn activation_start() -> Result<ActivationResult, String> {
    // Verify preconditions
    let svc = ConfigService::new();
    let config = svc.load_config().await.map_err(|e| e.to_string())?;

    if config.active_provider.is_none_or(|p| !p.is_complete()) {
        return Err("No provider configured. Please complete Step 4 first.".to_string());
    }

    // Send a real greeting to the configured chat model with a 30s timeout.
    // Greeting failure is non-fatal — activation succeeds regardless.
    let ai_response = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        crate::modules::provider::test::send_greeting(),
    )
    .await
    .map_err(|_| {
        tracing::warn!("[activation] Greeting timed out after 30s");
    })
    .ok()
    .and_then(|r| r.ok())
    .flatten();

    if ai_response.is_none() {
        tracing::info!("[activation] No greeting response — proceeding without ceremony message");
    }

    Ok(ActivationResult {
        success: true,
        session_id: Some("activation".to_string()),
        message: "if2AI Agent 已启动".to_string(),
        ai_response,
    })
}

/// Send a test message to verify end-to-end connectivity.
#[tauri::command]
pub async fn activation_test_message() -> Result<TestResult, String> {
    // Send a simple test prompt to verify the provider is reachable.
    let svc = ConfigService::new();
    let config = svc.load_config().await.map_err(|e| e.to_string())?;

    let provider = config
        .active_provider
        .ok_or_else(|| "No provider configured".to_string())?;

    let base_url = provider.base_url.as_deref().unwrap_or_default();
    let result = crate::modules::provider::test::test_provider_connection(
        &provider.provider_id,
        base_url,
        provider.api_key.as_deref(),
    )
    .await;

    Ok(result)
}

/// Mark activation as complete and finish onboarding.
#[tauri::command]
pub async fn activation_complete() -> Result<(), String> {
    let current = load_state()
        .await
        .map_err(|e| format!("Failed to load state: {e}"))?;

    let completed = OnboardingFlow::complete(&current).map_err(|e| e.to_string())?;
    save_state(&completed)
        .await
        .map_err(|e| format!("Failed to save state: {e}"))?;

    // Sync to AppConfig
    let config_svc = ConfigService::new();
    let mut config = config_svc.load_config().await.unwrap_or_default();
    config.onboarding = completed;
    config.routing.get_or_insert_with(ChannelRouting::default);
    let _ = config_svc.save_config(&config).await;

    Ok(())
}
