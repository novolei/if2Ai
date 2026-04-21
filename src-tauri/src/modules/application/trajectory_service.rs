//! Trajectory service — records agent conversation as a JSONL trajectory
//! for future RL training. Uses the `AppState`-level `TrajectoryManager`
//! when available; falls back to a temporary manager.
//!
//! Extracted from `commands/agent.rs` in GFR-006c (pure structural move,
//! function bodies byte-identical).

use std::path::PathBuf;
use std::sync::Arc;

use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::runtime::session::Session as RuntimeSession;

/// Record the conversation as a trajectory for future RL training.
///
/// Uses the `AppState`-level `TrajectoryManager` when available to avoid
/// re-creating the manager (and re-scanning the directory) on every turn.
/// Falls back to constructing a one-off manager if the state-level one is
/// absent (e.g. during tests or early startup).
///
/// Errors are logged as warnings and never block the main flow.
/// Short sessions (<3 turns) are silently skipped per privacy defaults.
pub(crate) async fn record_trajectory_if_possible(
    session: &RuntimeSession,
    system_prompt: &[String],
    tm: Option<&Arc<TrajectoryManager>>,
) {
    let system_text = system_prompt.join("\n");

    // Prefer the shared AppState manager.
    if let Some(manager) = tm {
        match manager.record(session, &system_text, "if2ai-default").await {
            Ok(id) => tracing::info!("[record_trajectory] Recorded trajectory {id}"),
            Err(e) => tracing::debug!("[record_trajectory] Skipping trajectory record: {e}"),
        }
        return;
    }

    // Fallback: create a temporary manager.
    let trajectories_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("trajectories");

    let manager = match TrajectoryManager::new(trajectories_dir) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("[record_trajectory] Failed to create TrajectoryManager: {e}");
            return;
        }
    };

    match manager.record(session, &system_text, "if2ai-default").await {
        Ok(id) => tracing::info!("[record_trajectory] Recorded trajectory {id}"),
        Err(e) => tracing::debug!("[record_trajectory] Skipping trajectory record: {e}"),
    }
}
