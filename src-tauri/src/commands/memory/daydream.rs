//! A.2 daydream Tauri commands — manual trigger + config get/set.

use std::path::PathBuf;
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::daydream::{DayDreamConfig, DayDreamReport};

/// Resolve the `~/.if2ai` directory inline. Mirrors the
/// `load_context_budget` pattern in `bootstrap/app.rs`.
fn if2ai_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".if2ai"))
        .unwrap_or_else(|| PathBuf::from(".if2ai"))
}

/// Trigger one daydream cycle on demand. Bypasses the idle gate.
#[tauri::command]
pub async fn daydream_run_cycle(state: State<'_, AppState>) -> Result<DayDreamReport, String> {
    state
        .daydream_coordinator
        .trigger_manual()
        .await
        .map_err(|e| format!("daydream cycle failed: {e}"))
}

/// Read the persisted config from `~/.if2ai/daydream.json`. Returns
/// defaults (`enabled: false`) when the file is missing.
#[tauri::command]
pub async fn daydream_get_config() -> Result<DayDreamConfig, String> {
    Ok(crate::bootstrap::daydream_config::load(&if2ai_dir()))
}

/// Persist the config to `~/.if2ai/daydream.json`.
///
/// The in-memory engine config does NOT update until the next launch —
/// this is intentional to keep the engine immutable per boot. Toggling
/// `enabled` to `false` takes effect at next startup.
#[tauri::command]
pub async fn daydream_set_config(config: DayDreamConfig) -> Result<(), String> {
    crate::bootstrap::daydream_config::save(&if2ai_dir(), &config)
        .map_err(|e| format!("write daydream.json failed: {e}"))
}
