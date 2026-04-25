//! Tauri commands for APP-UPDATER-001.

use crate::modules::updater::{
    check_manifest_url, configured_manifest_url, current_version, runtime_state,
    UpdaterCheckResult, UpdaterRuntimeState,
};

/// Return the idle updater state and configured manifest URL.
#[tauri::command]
pub fn app_updater_get_state() -> UpdaterRuntimeState {
    runtime_state(configured_manifest_url())
}

/// Check the configured release manifest for a newer build.
#[tauri::command]
pub async fn app_updater_check(manifest_url: Option<String>) -> Result<UpdaterCheckResult, String> {
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
            release_notes_url: None,
            diagnostic: Some("IF2AI_UPDATE_MANIFEST_URL is not configured".to_string()),
        });
    };

    Ok(check_manifest_url(current_version(), manifest_url).await)
}
