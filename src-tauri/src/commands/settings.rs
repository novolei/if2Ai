//! Memory settings Tauri commands.
//!
//! Provides configuration for the memory subsystem (token budget, trajectory export).

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;

/// Memory recall mode — selects between lexical-only and hybrid (vector +
/// FTS + episodic) retrieval pipelines.  Mirrors the Rust runtime
/// `MemoryRecallMode` enum so the frontend can drive the same feature flag.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemoryRecallModeSetting {
    Lexical,
    #[default]
    Hybrid,
}

/// Memory write policy enforce mode — `shadow` audits decisions without
/// blocking, `enforce` rejects denied writes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemoryPolicyEnforceModeSetting {
    #[default]
    Shadow,
    Enforce,
}

/// Memory configuration returned by the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Total token budget (default 4000)
    pub total_tokens: usize,
    /// System prompt slot percentage (0-100)
    pub system_pct: u8,
    /// Episodic memory slot percentage (0-100)
    pub episodic_pct: u8,
    /// Semantic memory slot percentage (0-100)
    pub semantic_pct: u8,
    /// Working memory slot percentage (0-100)
    pub working_pct: u8,
    /// Number of trajectories recorded
    pub trajectory_count: usize,
    /// Memory Control Plane V1 — master kill-switch for the new memory pipeline.
    pub control_plane_v1_enabled: bool,
    /// Recall mode (`lexical` keeps the legacy SQL search; `hybrid` enables
    /// vector + FTS + episodic fusion).
    pub recall_mode: MemoryRecallModeSetting,
    /// Policy enforcement mode (`shadow` audits only, `enforce` blocks
    /// denied writes).
    pub policy_enforce_mode: MemoryPolicyEnforceModeSetting,
    /// Promotion thresholds — surfaced so the Memory Settings UI can tune
    /// when the background scanner recommends `session→project` and
    /// `project→global` upgrades.  See
    /// [`crate::modules::memory::promotion::PromotionThresholds`].
    pub promotion: crate::modules::memory::promotion::PromotionThresholds,
}

/// Configuration to persist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfigInput {
    pub total_tokens: usize,
    pub system_pct: u8,
    pub episodic_pct: u8,
    pub semantic_pct: u8,
    pub working_pct: u8,
    /// Optional so older clients that don't ship feature-flag UI keep working.
    #[serde(default)]
    pub control_plane_v1_enabled: Option<bool>,
    #[serde(default)]
    pub recall_mode: Option<MemoryRecallModeSetting>,
    #[serde(default)]
    pub policy_enforce_mode: Option<MemoryPolicyEnforceModeSetting>,
    /// Optional so older clients without the promotion-tuning UI keep
    /// working; backend falls back to
    /// [`crate::modules::memory::promotion::PromotionThresholds::default`].
    #[serde(default)]
    pub promotion: Option<crate::modules::memory::promotion::PromotionThresholds>,
}

fn read_persisted_config() -> Option<MemoryConfigInput> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::Path::new(&home).join(".if2ai/memory_config.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_persisted_config(cfg: &MemoryConfigInput) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let dir = std::path::Path::new(&home).join(".if2ai");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("memory_config.json");
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn count_trajectories() -> usize {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::path::Path::new(&home).join(".if2ai/trajectories");
    match std::fs::read_dir(&path) {
        Ok(entries) => entries
            .filter(|e| {
                e.as_ref()
                    .map(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
                    .unwrap_or(false)
            })
            .count(),
        Err(_) => 0,
    }
}

/// Get the current memory configuration.
///
/// Resolves feature-flag values from the persisted config first, falling
/// back to safe defaults (`control_plane_v1_enabled = true`, recall =
/// `Hybrid`, policy = `Shadow`) when older configs are read.
#[tauri::command]
pub fn get_memory_config(state: State<'_, AppState>) -> MemoryConfig {
    let persisted = read_persisted_config();
    let budget = &state.context_budget;

    let config = persisted.unwrap_or(MemoryConfigInput {
        total_tokens: budget.total,
        system_pct: (budget.system_pct * 100.0) as u8,
        episodic_pct: (budget.episodic_pct * 100.0) as u8,
        semantic_pct: (budget.semantic_pct * 100.0) as u8,
        working_pct: (budget.working_pct * 100.0) as u8,
        control_plane_v1_enabled: None,
        recall_mode: None,
        policy_enforce_mode: None,
        promotion: None,
    });

    MemoryConfig {
        total_tokens: config.total_tokens,
        system_pct: config.system_pct,
        episodic_pct: config.episodic_pct,
        semantic_pct: config.semantic_pct,
        working_pct: config.working_pct,
        trajectory_count: count_trajectories(),
        control_plane_v1_enabled: config.control_plane_v1_enabled.unwrap_or(true),
        recall_mode: config.recall_mode.unwrap_or_default(),
        policy_enforce_mode: config.policy_enforce_mode.unwrap_or_default(),
        promotion: config.promotion.unwrap_or_default(),
    }
}

/// Save memory configuration.
#[tauri::command]
pub fn set_memory_config(
    _state: State<'_, AppState>,
    config: MemoryConfigInput,
) -> Result<MemoryConfig, String> {
    let total: u16 = config.system_pct as u16
        + config.episodic_pct as u16
        + config.semantic_pct as u16
        + config.working_pct as u16;
    if total != 100 {
        return Err(format!("Slot percentages must sum to 100%, got {}%", total));
    }

    // Surface validation errors instead of silently writing a malformed
    // promotion config — Memory Settings UI relies on the error string to
    // highlight the bad field.
    if let Some(promo) = &config.promotion {
        promo
            .validate()
            .map_err(|msg| format!("promotion thresholds invalid: {msg}"))?;
    }

    write_persisted_config(&config)?;

    // Note: ContextBudget is shared via Arc so we can't replace it in-place.
    // The runtime reads persisted config on next initialization, so the
    // disk write above is sufficient — no in-place update needed.

    Ok(MemoryConfig {
        total_tokens: config.total_tokens,
        system_pct: config.system_pct,
        episodic_pct: config.episodic_pct,
        semantic_pct: config.semantic_pct,
        working_pct: config.working_pct,
        trajectory_count: count_trajectories(),
        control_plane_v1_enabled: config.control_plane_v1_enabled.unwrap_or(true),
        recall_mode: config.recall_mode.unwrap_or_default(),
        policy_enforce_mode: config.policy_enforce_mode.unwrap_or_default(),
        promotion: config.promotion.unwrap_or_default(),
    })
}

/// Export trajectories to a user-selected directory.
#[tauri::command]
pub fn export_trajectories() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let traj_dir = std::path::Path::new(&home).join(".if2ai/trajectories");
    let export_dir = traj_dir.join("export");
    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;

    // Copy all .jsonl files to the export directory
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(&traj_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                if let Some(name) = path.file_name() {
                    let dest = export_dir.join(name);
                    std::fs::copy(&path, &dest).map_err(|e| e.to_string())?;
                    count += 1;
                }
            }
        }
    }

    Ok(format!(
        "已导出 {} 个轨迹文件到 {}",
        count,
        export_dir.display()
    ))
}
