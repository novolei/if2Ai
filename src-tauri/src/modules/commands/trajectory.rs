//! Tauri commands for trajectory management
//!
//! Exposes trajectory export functionality to the frontend via Tauri IPC.
//!
//! # `#![allow(dead_code)]` justification
//! These commands will be invoked from the settings panel to export
//! trajectory data for external RL training pipelines.

#![allow(dead_code)]

use std::path::PathBuf;

use crate::modules::learning::trajectory::{TrajectoryError, TrajectoryManager};

/// Export all trajectories to a specified output file
///
/// Returns the number of trajectory lines exported.
#[tauri::command]
pub async fn export_trajectories(output_path: String) -> Result<u64, String> {
    let output = PathBuf::from(&output_path);
    let base_path = output
        .parent()
        .ok_or_else(|| format!("invalid output path: {output_path}"))?
        .to_path_buf();

    let manager = TrajectoryManager::new(base_path).map_err(|e| e.to_string())?;
    manager
        .export_all(&output)
        .await
        .map_err(|e| e.to_string())
}

/// Get the number of trajectory files on disk
#[tauri::command]
pub async fn get_trajectory_count() -> Result<u64, String> {
    let base_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("trajectories");

    // Ensure directory exists (manager creates it)
    let manager = TrajectoryManager::new(base_path).map_err(|e| e.to_string())?;
    manager
        .count_files()
        .await
        .map_err(|e| e.to_string())
}

/// Get the default trajectory storage path
#[tauri::command]
pub async fn get_trajectory_path() -> Result<String, String> {
    let path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("trajectories");

    path.to_str()
        .map(String::from)
        .ok_or_else(|| "path contains invalid UTF-8".to_string())
}
