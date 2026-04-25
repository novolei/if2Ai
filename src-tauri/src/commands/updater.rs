//! Tauri commands for APP-UPDATER-001.

use crate::modules::updater::{
    check_manifest_url, check_signed_update, configured_manifest_url, current_runtime_state,
    current_version, download_and_install_signed_update, download_and_open_update, set_preferences,
    UpdaterCheckResult, UpdaterDownloadResult, UpdaterPreferences, UpdaterRuntimeState,
};
use tauri::AppHandle;

/// Return the idle updater state and configured manifest URL.
#[tauri::command]
pub fn app_updater_get_state() -> UpdaterRuntimeState {
    current_runtime_state()
}

/// Check the signed Tauri updater endpoint for a newer build.
#[tauri::command]
pub async fn app_updater_check(app: AppHandle) -> Result<UpdaterCheckResult, String> {
    Ok(check_signed_update(app).await)
}

/// Check the legacy If2Ai release manifest for diagnostics and CI parity.
#[tauri::command]
pub async fn app_updater_check_manifest(
    manifest_url: Option<String>,
) -> Result<UpdaterCheckResult, String> {
    let manifest_url = manifest_url
        .or_else(configured_manifest_url)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let Some(manifest_url) = manifest_url else {
        return Ok(UpdaterCheckResult {
            status: crate::modules::updater::UpdaterCheckStatus::Failed,
            current_version: current_version().to_string(),
            latest_version: None,
            manifest_url: None,
            artifact_url: None,
            artifact_checksum_sha256: None,
            release_notes_url: None,
            diagnostic: Some("IF2AI_UPDATE_MANIFEST_URL is not configured".to_string()),
        });
    };

    Ok(check_manifest_url(current_version(), manifest_url).await)
}

/// Download, verify, and install a signed update using the Tauri updater.
#[tauri::command]
pub async fn app_updater_download_and_install(
    app: AppHandle,
) -> Result<UpdaterDownloadResult, String> {
    Ok(download_and_install_signed_update(app).await)
}

/// Download the latest update artifact, verify it, and open it.
#[tauri::command]
pub async fn app_updater_download_and_open(
    manifest_url: Option<String>,
) -> Result<UpdaterDownloadResult, String> {
    let manifest_url = manifest_url
        .or_else(configured_manifest_url)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "IF2AI_UPDATE_MANIFEST_URL is not configured".to_string())?;

    Ok(download_and_open_update(current_version(), manifest_url).await)
}

/// Persist updater preferences and return the updated runtime state.
#[tauri::command]
pub fn app_updater_set_preferences(
    app: AppHandle,
    preferences: UpdaterPreferences,
) -> UpdaterRuntimeState {
    set_preferences(&app, preferences)
}
