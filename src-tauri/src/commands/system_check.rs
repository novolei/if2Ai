//! System check Tauri commands.
//!
//! Provides 3 commands:
//! - `system_check_run`: Run full system detection (CPU/GPU/Node.js + embedded model)
//! - `embedded_model_download`: Download the embedded model
//! - `embedded_model_progress`: Get download progress (0.0 to 1.0)

use crate::modules::system_check::env::run_full_check;
use crate::modules::system_check::model_download::{
    download_embedded_model, get_download_progress,
};
use crate::modules::system_check::types::SystemReport;

/// Run a full system check.
///
/// Detects CPU, GPU, Node.js, and checks embedded model status.
#[tauri::command]
pub async fn system_check_run() -> Result<SystemReport, String> {
    let report = run_full_check().await;
    Ok(report)
}

/// Download the embedded model.
///
/// Progress can be tracked via `embedded_model_progress()`.
#[tauri::command]
pub async fn embedded_model_download() -> Result<(), String> {
    download_embedded_model::<fn(u64, u64)>(None, None)
        .await
        .map_err(|e| format!("Download failed: {e}"))
}

/// Get the embedded model download progress.
///
/// Returns a value from 0.0 to 1.0.
#[tauri::command]
pub fn embedded_model_progress() -> Result<f64, String> {
    Ok(get_download_progress())
}
