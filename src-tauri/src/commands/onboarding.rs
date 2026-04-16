//! Onboarding Tauri commands — state machine control.
//!
//! Provides 5 commands for the onboarding flow:
//! - `onboarding_get_state`: Returns current onboarding state
//! - `onboarding_next_step`: Advances to the next step
//! - `onboarding_prev_step`: Returns to the previous step
//! - `onboarding_complete`: Marks onboarding as complete
//! - `security_confirm`: Confirms all security items (Step 3)

use crate::modules::config::ConfigService;
use crate::modules::onboarding::flow::OnboardingFlow;
use crate::modules::onboarding::state::{AppOnboardingState, OnboardingState};
use crate::modules::onboarding::store::{load_state, save_state};

/// Get the current onboarding state.
///
/// Returns `AppOnboardingState` enum: `FirstLaunch`, `Onboarding { step }`, or `Ready`.
#[tauri::command]
pub async fn onboarding_get_state() -> AppOnboardingState {
    let state = load_state()
        .await
        .unwrap_or_else(|_| OnboardingState::new());
    OnboardingFlow::to_app_state(&state)
}

/// Advance to the next onboarding step.
///
/// Validates preconditions before advancing.
/// Returns the new state on success.
#[tauri::command]
pub async fn onboarding_next_step() -> Result<AppOnboardingState, String> {
    let current = load_state()
        .await
        .map_err(|e| format!("Failed to load state: {e}"))?;

    let next = OnboardingFlow::next_step(&current).map_err(|e| e.to_string())?;
    save_state(&next)
        .await
        .map_err(|e| format!("Failed to save state: {e}"))?;

    // Sync to AppConfig
    let config_svc = ConfigService::new();
    let mut config = config_svc.load_config().await.unwrap_or_default();
    config.onboarding = next.clone();
    let _ = config_svc.save_config(&config).await;

    Ok(OnboardingFlow::to_app_state(&next))
}

/// Return to the previous onboarding step.
///
/// Fails if already at the first step.
#[tauri::command]
pub async fn onboarding_prev_step() -> Result<AppOnboardingState, String> {
    let current = load_state()
        .await
        .map_err(|e| format!("Failed to load state: {e}"))?;

    let prev = OnboardingFlow::prev_step(&current).map_err(|e| e.to_string())?;
    save_state(&prev)
        .await
        .map_err(|e| format!("Failed to save state: {e}"))?;

    // Sync to AppConfig
    let config_svc = ConfigService::new();
    let mut config = config_svc.load_config().await.unwrap_or_default();
    config.onboarding = prev.clone();
    let _ = config_svc.save_config(&config).await;

    Ok(OnboardingFlow::to_app_state(&prev))
}

/// Mark onboarding as complete.
///
/// Transitions the app to `Ready` state, enabling the main interface.
#[tauri::command]
pub async fn onboarding_complete() -> Result<(), String> {
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
    let _ = config_svc.save_config(&config).await;

    Ok(())
}

/// Confirm all security items (Step 3).
///
/// Sets `security_confirmed = true` in the config and advances to the next step.
#[tauri::command]
pub async fn security_confirm() -> Result<(), String> {
    let current = load_state()
        .await
        .map_err(|e| format!("Failed to load state: {e}"))?;

    let next = OnboardingFlow::next_step(&current).map_err(|e| e.to_string())?;
    save_state(&next)
        .await
        .map_err(|e| format!("Failed to save state: {e}"))?;

    // Mark security_confirmed in AppConfig
    let config_svc = ConfigService::new();
    let mut config = config_svc.load_config().await.unwrap_or_default();
    config.security_confirmed = true;
    config.onboarding = next;
    let _ = config_svc.save_config(&config).await;

    Ok(())
}
