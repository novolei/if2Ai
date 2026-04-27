//! System check Tauri commands.
//!
//! Provides commands:
//! - `system_check_run`: Run full system detection (CPU/GPU/Memory + embedded model)
//! - `embedded_model_download`: Download the embedded model
//! - `embedded_model_progress`: Get download progress (0.0 to 1.0)
//! - `get_model_config`: Get the embedded model name configuration
//! - `set_model_config`: Set the embedded model name configuration

use serde::{Deserialize, Serialize};

use crate::modules::system_check::env::run_full_check;
use crate::modules::system_check::model_download::{
    get_download_progress, start_embedded_model_download, MODEL_NAME,
};
use crate::modules::system_check::types::SystemReport;

/// Run a full system check.
///
/// Detects CPU, GPU, Memory, and checks embedded model status.
#[tauri::command]
pub async fn system_check_run() -> Result<SystemReport, String> {
    let report = run_full_check().await;
    Ok(report)
}

/// Download the embedded model.
///
/// Progress can be tracked via `embedded_model_progress()`.
#[tauri::command]
pub fn embedded_model_download(app: tauri::AppHandle) -> Result<(), String> {
    start_embedded_model_download(Some(app)).map_err(|e| format!("Download failed: {e}"))
}

/// Get the embedded model download progress.
///
/// Returns a value from 0.0 to 1.0.
#[tauri::command]
pub fn embedded_model_progress() -> Result<f64, String> {
    Ok(get_download_progress())
}

// ── Model Configuration ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub embedded_model_name: String,
    /// Optional HuggingFace mirror URL (e.g. `https://hf-mirror.com`).
    /// When set, all model downloads go through this mirror instead of
    /// `huggingface.co`. Useful for users behind the Great Firewall.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hf_mirror_url: Option<String>,
}

fn model_config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home)
        .join(".if2ai")
        .join("model_config.json")
}

/// Get the current embedded model name configuration.
///
/// Returns the default `MODEL_NAME` if no custom config exists.
#[tauri::command]
pub fn get_model_config() -> ModelConfig {
    let path = model_config_path();
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<ModelConfig>(&raw) {
            return cfg;
        }
    }
    ModelConfig {
        embedded_model_name: MODEL_NAME.to_string(),
        hf_mirror_url: None,
    }
}

/// Save the embedded model name configuration.
#[tauri::command]
pub fn set_model_config(config: ModelConfig) -> Result<ModelConfig, String> {
    let path = model_config_path();
    let dir = path
        .parent()
        .ok_or_else(|| "Invalid config path".to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(config)
}

/// Get the configured HuggingFace mirror base URL, if any.
///
/// Returns `Some(url)` when the user has set a mirror (e.g.
/// `https://hf-mirror.com`), or `None` to fall back to
/// `https://huggingface.co`.
pub(crate) fn get_hf_mirror_url() -> Option<String> {
    let path = model_config_path();
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<ModelConfig>(&raw) {
            return cfg.hf_mirror_url;
        }
    }
    None
}
