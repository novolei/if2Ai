//! Harness Control IPC commands
//!
//! Tauri commands that allow the frontend (and harness-cli) to control the
//! agent loop observability harness: start/stop recording, query telemetry,
//! and inspect recording status.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::AppState;
use crate::modules::harness::SessionTelemetry;

// ──────────────────────────────────────────────────────────────────────────────
// Response types
// ──────────────────────────────────────────────────────────────────────────────

/// Response payload for harness recording status queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessStatusResponse {
    /// Whether the harness is initialised (i.e., `AppState.harness` is `Some`).
    pub harness_enabled: bool,
    /// Session IDs currently being recorded.
    pub active_recordings: Vec<String>,
}

/// Response payload for telemetry queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessTelemetryResponse {
    /// `true` if telemetry data was found for the requested session.
    pub found: bool,
    /// Telemetry snapshot, or `None` when `found == false`.
    pub telemetry: Option<SessionTelemetry>,
}

// ──────────────────────────────────────────────────────────────────────────────
// IPC commands
// ──────────────────────────────────────────────────────────────────────────────

/// Return the current harness status.
///
/// Returns `harness_enabled: false` when the harness is not initialised in
/// `AppState`; no error is returned.
#[tauri::command]
pub async fn get_harness_status(
    state: State<'_, AppState>,
) -> Result<HarnessStatusResponse, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(HarnessStatusResponse {
            harness_enabled: false,
            active_recordings: vec![],
        });
    };

    let active_recordings = harness.recorder.active_sessions().await;

    Ok(HarnessStatusResponse {
        harness_enabled: true,
        active_recordings,
    })
}

/// Start recording agent events for `session_id` to a JSONL trace file.
///
/// Returns `Ok(())` if recording was started (or was already active).
/// Returns `Err` if the harness is not initialised or the trace file cannot be
/// created.
#[tauri::command]
pub async fn start_harness_recording(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let harness = state
        .harness
        .as_ref()
        .ok_or_else(|| "harness not initialised".to_string())?;

    harness
        .recorder
        .start(&session_id, &harness.event_bus)
        .await
        .map_err(|e| format!("failed to start recording for session {session_id}: {e}"))
}

/// Stop recording agent events for `session_id`.
///
/// The JSONL trace file is flushed and closed. Returns `Ok(())` regardless of
/// whether recording was active.
#[tauri::command]
pub async fn stop_harness_recording(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(());
    };

    harness.recorder.stop(&session_id).await;
    Ok(())
}

/// Return telemetry for a specific session.
///
/// `found` is `false` when the harness is not initialised or no events have
/// been recorded for the session yet.
#[tauri::command]
pub async fn get_session_telemetry(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<HarnessTelemetryResponse, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(HarnessTelemetryResponse {
            found: false,
            telemetry: None,
        });
    };

    let telemetry = harness.telemetry.snapshot(&session_id).await;
    Ok(HarnessTelemetryResponse {
        found: telemetry.is_some(),
        telemetry,
    })
}

/// Return telemetry snapshots for all sessions seen since app start.
///
/// Returns an empty list when the harness is not initialised.
#[tauri::command]
pub async fn get_all_session_telemetry(
    state: State<'_, AppState>,
) -> Result<Vec<SessionTelemetry>, String> {
    let Some(harness) = state.harness.as_ref() else {
        return Ok(vec![]);
    };

    Ok(harness.telemetry.all_snapshots().await)
}
