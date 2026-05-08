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
///
/// After the cycle runs, emits a `RuntimeEventType::DaydreamCycle` envelope so
/// the frontend projection picks it up and updates `daydreamHistory`. Without
/// this emit the manual path would skip the projection (only the idle poll
/// loop in `main.rs` dispatches), and the status row would never refresh on
/// "Run now". The report is still returned via the command result so the UI
/// can surface a toast on success/failure.
#[tauri::command]
pub async fn daydream_run_cycle(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<DayDreamReport, String> {
    let report = state
        .daydream_coordinator
        .trigger_manual()
        .await
        .map_err(|e| format!("daydream cycle failed: {e}"))?;

    let family = if report.all_succeeded() {
        crate::modules::runtime::contracts::common::daydream_family::COMPLETED
    } else {
        crate::modules::runtime::contracts::common::daydream_family::FAILED
    };
    tracing::info!(
        target: "daydream",
        cycle_id = %report.cycle_id,
        trigger = %report.trigger,
        family = %family,
        steps = report.steps.len(),
        "[daydream] dispatching runtime_event envelope"
    );
    let dispatch_result = crate::modules::runtime::runtime_event::dispatch(
        Some(&app),
        crate::modules::runtime::contracts::common::RuntimeEventType::DaydreamCycle,
        family,
        crate::modules::runtime::contracts::common::CorrelationIds::default(),
        &report,
        None,
    );
    match dispatch_result {
        Ok(envelope) => tracing::info!(
            target: "daydream",
            event_type = ?envelope.event_type,
            payload_family = %envelope.payload_family.0,
            "[daydream] dispatch ok"
        ),
        Err(err) => tracing::warn!(
            target: "daydream",
            error = %err,
            "[daydream] dispatch failed"
        ),
    }

    Ok(report)
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
